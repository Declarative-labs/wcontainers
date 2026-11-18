//! Pipeline and stream output statistics queries (plan 4.12, other queries), made by the front from
//! the work it issues. Input assembler, vertex shader and clipper counts come from each draw's
//! arguments, as a GPU without vertex reuse or clipping splits counts them (indexed strips with a
//! cut value read their indices back; indirect arguments are read back); compute invocations from
//! each dispatch's groups; pixel shader invocations from the pixel counter, an occlusion counter
//! replaying the draws with the depth and stencil tests off and a program counting each invocation.
//! Draws through geometry shaders or stream output are refused while a pipeline statistics query
//! runs and no hull or domain shader is made, so those stages' counts are zero. Stream output
//! statistics are the stage helper's counts of primitives written and needed, read back.

use wcontainers_gpu::{DepthStencilDesc, Reflection, StageExtras, SystemValue};

use super::*;

/// The fields of `D3D12_QUERY_DATA_PIPELINE_STATISTICS`, in order.
const IA_VERTICES: usize = 0;
const IA_PRIMITIVES: usize = 1;
const VS_INVOCATIONS: usize = 2;
const C_INVOCATIONS: usize = 5;
const C_PRIMITIVES: usize = 6;
const PS_INVOCATIONS: usize = 7;
pub(super) const CS_INVOCATIONS: usize = 10;

/// The pixel counter's programs: one count a pixel, the same on multisampled targets (the first
/// covered sample counts), and one count a sample.
const PER_PIXEL: &str = "@fragment fn main() -> @location(0) vec4f { return vec4f(1.0); }";
const PER_PIXEL_MASKED: &str = "
struct Out { @location(0) count: vec4f, @builtin(sample_mask) mask: u32 }
@fragment fn main(@builtin(sample_mask) covered: u32) -> Out {
    return Out(vec4f(1.0), covered & (~covered + 1u));
}";
const PER_SAMPLE: &str = "
@fragment fn main(@builtin(sample_index) i: u32) -> @location(0) vec4f { return vec4f(1.0); }";

/// The pixel counter's program and depth-stencil block for one draw.
type PixelCounting<B> = (<B as Backend>::Program, <B as Backend>::DepthStencil);

/// A running pipeline statistics query: heap, index and the counts so far.
pub(super) struct Running {
    heap: QueryHeapId,
    index: u32,
    values: [u64; 11],
}

impl Running {
    pub(super) fn add(&mut self, field: usize, n: u64) {
        self.values[field] += n;
    }
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
        let code = match (r.targets.samples > 1, r.per_sample) {
            (false, _) => PER_PIXEL,
            (true, false) => PER_PIXEL_MASKED,
            (true, true) => PER_SAMPLE,
        };
        let vertex = r.vertex.clone();
        let fragment = ShaderModule::from_wgsl(code, "main", ShaderStage::Fragment)?;
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

    /// The depth-stencil block with both tests off, made once.
    fn tests_off(&mut self) -> Result<B::DepthStencil, Error> {
        if let Some(d) = &self.tests_off {
            return Ok(d.clone());
        }
        let d = self.backend.create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })?;
        self.tests_off = Some(d.clone());
        Ok(d)
    }
}

impl<B: Backend> Replay<B> {
    /// `BeginQuery` of pipeline statistics: the counts start at zero, the pixel counter from the
    /// targets as they are now.
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
        d.pixels.begin(&mut d.backend, &mut d.passes)?;
        self.statistics = Some(Running {
            heap,
            index,
            values: [0; 11],
        });
        Ok(())
    }

    /// `EndQuery` of pipeline statistics: the pixel counter's count added and the value written.
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
        let mut s = self.statistics.take().expect("a running query");
        s.values[PS_INVOCATIONS] += d.pixels.end(&mut d.backend, &mut d.passes)?;
        d.write_value(heap, index, &s.values)
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
        self.streams.push((heap, index, stream, [0; 2]));
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
        d.write_value(heap, index, &q.3)
    }

    /// Adds a direct draw of pipeline `pid` to the running statistics query, and gives the pixel
    /// counter's program and depth-stencil block where the pipeline has a pixel shader (a draw
    /// without one invokes none).
    pub(super) fn count_draw(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
        range: &DrawRange<'_, B>,
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
        let s = self.statistics.as_mut().expect("a running query");
        s.add(IA_VERTICES, vertices);
        s.add(IA_PRIMITIVES, prims);
        s.add(VS_INVOCATIONS, vertices);
        s.add(C_INVOCATIONS, prims);
        s.add(C_PRIMITIVES, prims);
        let pixel = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .is_some_and(|r| !r.pixel.is_empty());
        if !pixel || vertices == 0 {
            return Ok(None);
        }
        let program = d.pixel_counting(pid)?;
        let off = d.tests_off()?;
        self.track_targets(d, pid)?;
        Ok(Some((program, off)))
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
        let size: u64 = if view.format == 57 { 2 } else { 4 };
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
        let indices: Vec<&[u8]> = data.chunks(size as usize).collect();
        for run in indices.split(|i| i.iter().all(|&b| b == 0xff)) {
            let n = run.len() as u64;
            vertices += n;
            prims += n.saturating_sub(if line { 1 } else { 2 });
        }
        Ok((vertices, prims))
    }
}
