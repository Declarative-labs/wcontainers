//! Pipeline and stream output statistics queries (plan 4.12, other queries), made by the front from
//! the work it issues. Input assembler, vertex shader and clipper counts come from each draw's
//! arguments, as a GPU without vertex reuse or clipping splits counts them (indexed strips with a
//! cut value read their indices back; indirect arguments are read back); compute invocations from
//! each dispatch's groups; pixel shader invocations by drawing each draw again in its pass with
//! the depth and stencil tests off, no colour written and a program adding one a pixel (or a
//! sample, for pixel shaders run once a sample) to a counter buffer on the GPU
//! (`PixelInvocationsCounted`), which `EndQuery` copies into the value.
//! Draws through geometry, hull or domain shaders or stream output are refused while a pipeline
//! statistics query runs, so those stages' counts are zero. Stream output
//! statistics are the stage helper's counts of primitives written and needed, read back.

use wcontainers_gpu::{
    BindingEntry, BindingResource, BlendDesc, BufferRange, COUNTER_BINDING, ColorWrites,
    DepthStencilDesc, HELPER_SET, Reflection, RenderTargetBlend, StageExtras, SystemValue,
};

use super::*;
use wcontainers_directx_graphics_infrastructure::d3d;
use wcontainers_shaders::ENTRY_POINT;

/// The bytes of the pixel counter: one u64.
const COUNTER_BYTES: u64 = size_of::<u64>() as u64;

/// The helper set's entry binding the pixel counter `counter` for the counting draws.
pub(super) fn counter_entry<B: Backend>(counter: &B::Buffer) -> BindingEntry<'_, B> {
    BindingEntry {
        binding: COUNTER_BINDING,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::StorageBuffer {
            range: BufferRange {
                buffer: counter,
                offset: 0,
                size: COUNTER_BYTES,
            },
            writable: true,
        },
    }
}

/// The counting program: one invocation adds one to the counter, a u64 as two words.
fn counting_program(per_sample: bool) -> String {
    let input = if per_sample {
        "@builtin(sample_index) i: u32"
    } else {
        ""
    };
    format!(
        "@group({HELPER_SET}) @binding({COUNTER_BINDING})
var<storage, read_write> count: array<atomic<u32>, 2>;
@fragment fn {ENTRY_POINT}({input}) {{
    if atomicAdd(&count[0], 1u) == {low_max}u {{ atomicAdd(&count[1], 1u); }}
}}",
        low_max = u32::MAX
    )
}

/// The counting draw's program, depth-stencil block (both tests off) and blend block (no
/// colour written).
pub(super) type PixelCounting<B> = (
    <B as Backend>::Program,
    <B as Backend>::DepthStencil,
    <B as Backend>::Blend,
);

/// What every counting draw shares: the depth-stencil block with both tests off, the blend
/// block writing no colour, and the counter buffer.
pub(super) type Counting<B> = (
    <B as Backend>::DepthStencil,
    <B as Backend>::Blend,
    <B as Backend>::Buffer,
);

/// A running pipeline statistics query: heap, index and the counts so far.
pub(super) struct Running {
    heap: QueryHeapId,
    index: u32,
    pub values: d3d::PipelineStatistics,
}

/// Whether a pixel shader runs once a sample.
pub(super) fn per_sample(r: &Reflection) -> bool {
    matches!(
        r.extras,
        StageExtras::Pixel {
            per_sample: true,
            ..
        }
    ) || r.inputs.iter().any(|i| {
        i.system == Some(SystemValue::SampleIndex)
            || matches!(
                i.interpolation,
                Interpolation::PerspectiveSample | Interpolation::LinearSample
            )
    })
}

/// The primitives `n` vertices of topology `t` make (incomplete primitives are dropped).
fn primitives(t: Topology, n: u64) -> u64 {
    match t {
        Topology::PointList => n,
        Topology::LineList => n / 2,
        Topology::LineStrip => n.saturating_sub(1),
        Topology::TriangleList => n / 3,
        Topology::TriangleStrip => n.saturating_sub(2),
        // Not Direct3D topologies.
        Topology::TriangleFan | Topology::LineLoop => 0,
    }
}

impl<B: Backend> Device<B> {
    /// The program the pixel counter draws pipeline `pid`'s draws with, made once.
    fn pixel_counting(&mut self, pid: PipelineId) -> Result<B::Program, Error> {
        let r = self.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        if let Some(p) = &r.pixel_counting {
            return Ok(p.clone());
        }
        let code = counting_program(r.targets.samples > 1 && r.per_sample);
        let vertex = r.vertex.clone();
        let fragment = ShaderModule::from_wgsl(&code, ENTRY_POINT, ShaderStage::Fragment)?;
        let vertex = self.backend.create_shader(&vertex)?;
        let fragment = match self.backend.create_shader(&fragment) {
            Ok(f) => f,
            Err(e) => {
                self.backend.destroy(Object::Shader(vertex));
                return Err(e.into());
            }
        };
        let program = self.backend.create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(vertex));
        self.backend.destroy(Object::Shader(fragment));
        let program = program?;
        self.render_mut(pid).pixel_counting = Some(program.clone());
        Ok(program)
    }

    /// The counting draw's depth-stencil block (both tests off), blend block (no colour
    /// written) and counter buffer, made once.
    fn counting_blocks(&mut self) -> Result<Counting<B>, Error> {
        if let Some(c) = &self.counting {
            return Ok(c.clone());
        }
        let off = self.backend.create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })?;
        let none = self.backend.create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::empty(),
            }; MAX_COLOR_TARGETS],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })?;
        let counter = self.backend.create_buffer(&BufferDesc {
            size: COUNTER_BYTES,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })?;
        self.counting = Some((off, none, counter));
        Ok(self.counting.clone().expect("made"))
    }

    /// The counter buffer the counting draws add to.
    pub(super) fn pixel_counter(&mut self) -> Result<B::Buffer, Error> {
        Ok(self.counting_blocks()?.2)
    }
}

impl<B: Backend> Replay<B> {
    /// `BeginQuery` of pipeline statistics: the counts and the counter start at zero.
    pub(super) fn begin_statistics(
        &mut self,
        d: &mut Device<B>,
        heap: QueryHeapId,
        index: u32,
    ) -> Result<(), Error> {
        if d.heap_kind(heap, index)? != query::HEAP_PIPELINE_STATISTICS {
            return Err(Error::Invalid(
                "a statistics query in a heap of another kind",
            ));
        }
        if self.statistics.is_some() {
            return Err(Error::Invalid(
                "a pipeline statistics query begun inside another",
            ));
        }
        self.end_pass(d)?;
        let counter = d.pixel_counter()?;
        d.backend.clear_buffer(&counter, 0, COUNTER_BYTES, &[0]);
        self.statistics = Some(Running {
            heap,
            index,
            values: d3d::PipelineStatistics::default(),
        });
        Ok(())
    }

    /// `EndQuery` of pipeline statistics: the value written, with the pixel shader invocations
    /// copied from the counter.
    pub(super) fn end_statistics(
        &mut self,
        d: &mut Device<B>,
        heap: QueryHeapId,
        index: u32,
    ) -> Result<(), Error> {
        match &self.statistics {
            Some(s) if (s.heap, s.index) == (heap, index) => {}
            _ => return Err(Error::Invalid("EndQuery of a query not begun")),
        }
        self.end_pass(d)?;
        let s = self.statistics.take().expect("a running query");
        d.write_value(heap, index, bytemuck::bytes_of(&s.values))?;
        let counter = d.pixel_counter()?;
        let field = std::mem::offset_of!(d3d::PipelineStatistics, ps_invocations);
        d.copy_into_value(heap, index, field, &counter)
    }

    /// `BeginQuery` of stream output statistics.
    pub(super) fn begin_stream(
        &mut self,
        d: &mut Device<B>,
        heap: QueryHeapId,
        index: u32,
        stream: u32,
    ) -> Result<(), Error> {
        if d.heap_kind(heap, index)? != query::HEAP_SO_STATISTICS {
            return Err(Error::Invalid(
                "a statistics query in a heap of another kind",
            ));
        }
        if self.streams.iter().any(|q| (q.0, q.1) == (heap, index)) {
            return Err(Error::Invalid("a query begun twice"));
        }
        self.streams
            .push((heap, index, stream, d3d::StreamOutputStatistics::default()));
        Ok(())
    }

    /// `EndQuery` of stream output statistics: the primitives stream output draws wrote and
    /// needed while it ran (only stream 0 is ever written).
    pub(super) fn end_stream(
        &mut self,
        d: &mut Device<B>,
        heap: QueryHeapId,
        index: u32,
    ) -> Result<(), Error> {
        let at = self
            .streams
            .iter()
            .position(|q| (q.0, q.1) == (heap, index))
            .ok_or(Error::Invalid("EndQuery of a query not begun"))?;
        let q = self.streams.swap_remove(at);
        d.write_value(heap, index, bytemuck::bytes_of(&q.3))
    }

    /// Adds a direct draw of pipeline `pid` to the running statistics query, and gives what its
    /// counting draw is drawn with where the pipeline has a pixel shader (a draw without one
    /// invokes none). `pulled` says the draw fetches streams in its vertex program, which the
    /// counting program is not made with.
    pub(super) fn count_draw(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
        range: &DrawRange<'_, B>,
        pulled: bool,
    ) -> Result<Option<PixelCounting<B>>, Error> {
        let restart = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .is_some_and(|r| r.strip_cut != 0);
        let strip = matches!(self.topology, Topology::LineStrip | Topology::TriangleStrip);
        let (vertices, prims, instances) = match *range {
            DrawRange::Vertices {
                count, instances, ..
            } => (
                u64::from(count),
                primitives(self.topology, u64::from(count)),
                instances,
            ),
            DrawRange::Indexed {
                first,
                count,
                instances,
                ..
            } if restart && strip => {
                let (v, p) = self.indexed_strip(d, first, count)?;
                (v, p, instances)
            }
            DrawRange::Indexed {
                count, instances, ..
            } => (
                u64::from(count),
                primitives(self.topology, u64::from(count)),
                instances,
            ),
            _ => return Err(Error::Invalid("an indirect draw a statistics query counts")),
        };
        let (vertices, prims) = (
            vertices * u64::from(instances),
            prims * u64::from(instances),
        );
        let s = &mut self.statistics.as_mut().expect("a running query").values;
        s.ia_vertices += vertices;
        s.ia_primitives += prims;
        s.vs_invocations += vertices;
        s.c_invocations += prims;
        s.c_primitives += prims;
        let pixel = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .is_some_and(|r| !r.pixel.is_empty());
        if !pixel || vertices == 0 {
            return Ok(None);
        }
        if pulled {
            return Err(Error::NotImplemented(
                "counting pixel shader invocations of a draw fetching its streams in the program",
            ));
        }
        // The counting draw would be counted by a binary occlusion query running in the pass.
        if self.query.is_some() {
            return Err(Error::NotImplemented(
                "counting pixel shader invocations inside a binary occlusion query",
            ));
        }
        let program = d.pixel_counting(pid)?;
        let (off, none, _) = d.counting_blocks()?;
        d.backend
            .slow_paths()
            .hit(SlowPath::PixelInvocationsCounted);
        Ok(Some((program, off, none)))
    }

    /// The vertices and primitives of an indexed strip of `count` indices from `first`, read back:
    /// each cut index (all ones) ends one strip and starts the next, and is no vertex.
    fn indexed_strip(
        &mut self,
        d: &mut Device<B>,
        first: u32,
        count: u32,
    ) -> Result<(u64, u64), Error> {
        let Some(view) = self.index else {
            return Err(Error::Invalid("an indexed draw with no index buffer"));
        };
        let format = super::index_format(view.format).ok_or(Error::Invalid("index format"))?;
        let size = u64::from(format.size());
        let (id, offset) = d
            .addresses
            .find(view.address)
            .ok_or(Error::Invalid("an index buffer view of no buffer"))?;
        let start = u64::from(first) * size;
        let bytes = (u64::from(count) * size).min(u64::from(view.size).saturating_sub(start));
        if bytes == 0 {
            return Ok((0, 0));
        }
        self.settle(d)?;
        d.backend.slow_paths().hit(SlowPath::ReadBackWait);
        let data = d.read_now(id, offset + start, bytes)?;
        let line = self.topology == Topology::LineStrip;
        let (mut vertices, mut prims) = (0, 0);
        let indices: Vec<u32> = data
            .chunks(size as usize)
            .map(|i| i.iter().rev().fold(0, |v, &b| v << u8::BITS | u32::from(b)))
            .collect();
        for run in indices.split(|&i| i == format.restart_value()) {
            let n = run.len() as u64;
            vertices += n;
            prims += n.saturating_sub(if line { 1 } else { 2 });
        }
        Ok((vertices, prims))
    }
}
