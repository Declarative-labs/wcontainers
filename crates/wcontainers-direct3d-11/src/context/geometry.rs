//! Geometry shaders and stream output (plan section 7): the shaders crate's stage helper runs the
//! vertex stage, every input fetched in the program, and the geometry stage as compute work before
//! the draw (`GeometryAsCompute`), and writes the last stage's output into the bound stream output
//! targets (`StreamOutAsCompute`). A pass-through vertex program draws what a geometry stage made
//! with the pixel program; without one the draw is drawn as usual after its stream output.

use super::*;
use wcontainers_gpu::BufferUsage;
use wcontainers_shaders::ConvertError;
use wcontainers_shaders::stage::{
    FinalDraw, InputTopology, StageDraw, StageError, StageIndex, Stages, StreamTargets,
};

/// The stage helper of one vertex shader, input layout, geometry shader object and pixel shader.
pub(crate) struct StageRun<B: Backend> {
    /// Out while a draw runs it.
    stages: Option<Stages<B>>,
    /// Where the vertex stage's compute form reads its helper constants and fetched streams.
    vertex_helpers: Rc<Helpers>,
    /// The vertex layout of the draw after a geometry stage.
    layout: Option<Kept<B, B::VertexLayout>>,
    /// The geometry stage's pass-through program converted for the pixel shader's interpolation
    /// (none where the plain one serves), and the id programs of it are cached by.
    passthrough: Option<Kept<B, B::Shader>>,
    passthrough_id: u64,
}

/// The bytes of the stage helper's filled sizes and primitive counts.
const FILLED_BYTES: u64 = 32;

/// Direct3D's primitive topology as the stage helper takes it; `None` for patch lists.
fn input_topology(t: u32) -> Option<InputTopology> {
    use InputTopology as T;
    Some(match t {
        topology::POINTLIST => T::PointList,
        topology::LINELIST => T::LineList,
        topology::LINESTRIP => T::LineStrip,
        topology::TRIANGLELIST => T::TriangleList,
        topology::TRIANGLESTRIP => T::TriangleStrip,
        topology::LINELIST_ADJ => T::LineListAdjacency,
        topology::LINESTRIP_ADJ => T::LineStripAdjacency,
        topology::TRIANGLELIST_ADJ => T::TriangleListAdjacency,
        topology::TRIANGLESTRIP_ADJ => T::TriangleStripAdjacency,
        _ => return None,
    })
}

/// A stage helper refusal in the front's terms (the back end's errors are the device's).
fn stage_error(e: StageError) -> Error {
    match e {
        StageError::Gpu(_) => unreachable!("taken by the device"),
        StageError::Unsupported(what) => Error::NotImplemented(what),
        StageError::Convert(ConvertError::Unsupported(what)) => Error::NotImplemented(what),
        StageError::Convert(e) => {
            eprintln!("wcontainers direct3d 11: the stage helper's programs: {e}");
            Error::NotImplemented("a geometry stage the converter got wrong")
        }
        StageError::TopologyMismatch => {
            Error::InvalidArg("a topology that does not give the geometry shader's input")
        }
        StageError::Declaration => Error::InvalidArg("a stream output declaration"),
    }
}

impl<B: Backend> Device<B> {
    fn staged_error(&mut self, e: StageError) -> Error {
        match e {
            StageError::Gpu(e) => self.gpu::<()>(Err(e)).unwrap_err(),
            e => stage_error(e),
        }
    }

    /// The 4-byte counter of the bytes stream output has filled `b` to, made at its first use.
    pub(super) fn filled_counter(&mut self, b: &Buffer<B>) -> Result<B::Buffer> {
        if let Some(f) = &*b.filled.borrow() {
            return Ok(f.handle.clone());
        }
        let f = self.backend.create_buffer(&wcontainers_gpu::BufferDesc {
            size: 4,
            usage: BufferUsage::STORAGE
                | BufferUsage::COPY_SRC
                | BufferUsage::COPY_DST
                | BufferUsage::READ_BACK,
            intent: wcontainers_gpu::MapIntent::Rarely,
        });
        let f = self.gpu(f)?;
        // A buffer never written by stream output is filled to nothing.
        self.end_pass();
        self.backend
            .write_buffer(&f, 0, &[0; 4], WriteMode::Ordered);
        let id = self.id();
        *b.filled.borrow_mut() = Some(Kept::new(f.clone(), id, Object::Buffer, &self.graveyard));
        Ok(f)
    }

    /// `DrawAuto`: as many vertices as stream output filled the buffer bound at vertex slot 0
    /// with, past its offset (read back now, `ReadBackWait`).
    pub fn draw_auto(&mut self) {
        let r = self.auto_count().and_then(|n| match n {
            Some(count) => self.try_draw(Range::Vertices { count, start: 0 }, 0, 1, None),
            None => Ok(()),
        });
        if let Err(e) = r {
            self.lose(e);
        }
    }

    fn auto_count(&mut self) -> Result<Option<u32>> {
        if self.removed.is_some() || self.predicated_out() {
            return Ok(None);
        }
        let Some((b, stride, offset)) = self.ctx.vertex_buffers[0].clone() else {
            return Ok(None);
        };
        let counter = b.filled.borrow().as_ref().map(|k| k.handle.clone());
        let (Some(counter), true) = (counter, stride != 0) else {
            return Ok(None);
        };
        self.end_pass();
        let mut bytes = [0u8; 4];
        let read = self.backend.read_back_now(
            &ReadSource::Buffer {
                buffer: &counter,
                offset: 0,
                size: 4,
            },
            &mut bytes,
        );
        self.gpu(read)?;
        self.backend.slow_paths().hit(SlowPath::ReadBackWait);
        Ok(Some(
            u32::from_le_bytes(bytes).saturating_sub(offset) / stride,
        ))
    }

    /// The stage helper of the bound shaders, made at their first draw together.
    fn make_staged(&mut self, gs: &Geometry<B>, vs: &Shader<B>) -> Result<StageRun<B>> {
        let pulled = self.ctx.layout.as_ref().map_or(Vec::new(), |l| {
            l.all.iter().map(|e| (e.location, e.format)).collect()
        });
        let key = VariantKey {
            pulled,
            ..self.variant_key(vs, StageVariant::default(), SourceStage::Vertex)
        };
        let vertex = crate::shader::convert_variant(&vs.bytecode, SourceStage::Vertex, &key)?;
        // The geometry stage writes its outputs as the pixel shader interpolates them.
        let mut interpolation: Vec<(u32, Interpolation)> = self
            .ctx
            .ps
            .iter()
            .flat_map(|p| &p.reflection.inputs)
            .filter(|i| i.system.is_none() && i.interpolation != Interpolation::Perspective)
            .map(|i| (i.location, i.interpolation))
            .collect();
        interpolation.sort_by_key(|i| i.0);
        interpolation.dedup_by_key(|i| i.0);
        let (geometry, passthrough, passthrough_id) = match &gs.stage {
            None => (None, None, 0),
            Some((shader, plain)) if interpolation.is_empty() => {
                (Some(plain.clone()), None, shader.gpu.id)
            }
            Some((shader, _)) => {
                let key = VariantKey {
                    output_interpolation: interpolation,
                    ..crate::shader::base_key(SourceStage::Geometry, &shader.typed)
                };
                let c = crate::shader::convert_geometry(&shader.bytecode, &key)?;
                let gpu = self.backend.create_shader(&c.modules[1]);
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                let kept = Kept::new(gpu, id, Object::Shader, &self.graveyard);
                (Some(c), Some(kept), id)
            }
        };
        let stages = Stages::new(
            &mut self.backend,
            &vertex,
            geometry.as_ref(),
            gs.stream.as_ref(),
        )
        .map_err(|e| self.staged_error(e))?;
        let layout = match stages.vertex_layout() {
            Some(desc) => {
                let gpu = self.backend.create_vertex_layout(&desc);
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                Some(Kept::new(gpu, id, Object::VertexLayout, &self.graveyard))
            }
            None => None,
        };
        Ok(StageRun {
            stages: Some(stages),
            vertex_helpers: Rc::new(Helpers::of(&vertex)),
            layout,
            passthrough,
            passthrough_id,
        })
    }

    /// A draw with a geometry shader object bound: the stage helper's dispatches outside a pass,
    /// then the draw of a geometry stage's output with the pixel program `ps` as `pk`, through
    /// `raster` where the draw needs a twin of the bound block. Answers whether the draw is still
    /// to be drawn as usual (stream output without a geometry stage, stream 0 drawn).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_staged(
        &mut self,
        gs: &Rc<Geometry<B>>,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
        raster: Option<&B::Raster>,
        range: Range,
        first_instance: u32,
        instances: u32,
    ) -> Result<bool> {
        if !self.queries.counted.is_empty() || !self.queries.statistics.is_empty() {
            return not_implemented(
                "a draw through a geometry shader or stream output while a counted occlusion or \
                 statistics query runs",
            );
        }
        if (StageVariant { pulled: 0, ..vk }) != StageVariant::default() {
            return not_implemented("a vertex shader variant behind a geometry shader");
        }
        let (start, count, base, indexed) = match range {
            Range::Vertices { count, start } => (start, count, 0, false),
            Range::Indexed { count, start, base } => (start, count, base, true),
            Range::Indirect { .. } => {
                return not_implemented(
                    "an indirect draw through a geometry shader or stream output",
                );
            }
        };
        if self.ctx.topology == topology::UNDEFINED || (indexed && self.ctx.index.is_none()) {
            return Ok(false);
        }
        let topology = input_topology(self.ctx.topology)
            .ok_or(Error::NotImplemented("hull and domain shaders"))?;
        if gs.stage.as_ref().is_some_and(|(s, _)| s.helpers.size > 0) {
            return not_implemented("a geometry shader reading helper constants");
        }
        let key = (
            vs.gpu.id,
            self.ctx.layout.as_ref().map_or(0, |l| l.id),
            gs.id,
            ps.map_or(0, |p| p.gpu.id),
        );
        let mut staged = match self.staged.remove(&key) {
            Some(s) => s,
            None => self.make_staged(gs, vs)?,
        };
        let r = self.run_staged(
            gs,
            &mut staged,
            ps,
            pk,
            raster,
            topology,
            [start, count, base as u32],
            indexed,
            first_instance,
            instances,
        );
        self.staged.insert(key, staged);
        // The next plain draw makes the vertex set for its own program again.
        self.ctx.vertex_compute = false;
        self.ctx.sets_dirty[VERTEX] = true;
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn run_staged(
        &mut self,
        gs: &Geometry<B>,
        staged: &mut StageRun<B>,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
        raster: Option<&B::Raster>,
        topology: InputTopology,
        [start, count, base]: [u32; 3],
        indexed: bool,
        first_instance: u32,
        instances: u32,
    ) -> Result<bool> {
        let geometry = gs.stage.is_some();
        let drawn = gs.drawn && geometry;
        let bases = [if indexed { base } else { start }, first_instance];
        // The program of the pass-through and pixel stages, and the pixel stage's helpers.
        let program = if drawn {
            Some(self.staged_program(gs, staged, ps, pk)?)
        } else {
            None
        };

        // The vertex stage's slots and fetched streams, and its helper block, for compute.
        self.ctx.vertex_compute = true;
        self.ctx.sets_dirty[VERTEX] = true;
        self.ctx.pulled_set = u64::MAX;
        self.grow_constants()?;
        self.refresh_windows()?;
        let pulled = staged.vertex_helpers.pulled.clone();
        self.update_set(VERTEX, &pulled)?;
        let vertex_helpers = staged.vertex_helpers.clone();
        let vertex_block = self.helper_block(&vertex_helpers, VERTEX, bases, None)?;
        if geometry {
            self.update_set(GEOMETRY, &[])?;
        }
        if let Some((_, ph)) = &program {
            self.update_set(PIXEL, &[])?;
            if ph.size > 0 || !ph.merged.is_empty() {
                self.write_helpers(&[Rc::default(), ph.clone()], bases, None)?;
            }
        }
        // The stage helper and these blocks write the data ring, so the draws' and dispatches'
        // blocks as last written are forgotten.
        let limits = self.backend.caps().limits;
        let vertex_set = if vertex_block.is_empty() {
            None
        } else {
            let mut offsets = [0u64];
            let ring = self.backend.write_buffer_ring(
                RingKind::Data,
                &[&vertex_block],
                limits.min_uniform_offset_alignment,
                &mut offsets,
            );
            let ring = self.gpu(ring)?;
            let set = self.backend.create_binding_set(&[BindingEntry {
                binding: helper_binding(ShaderStage::Vertex),
                stages: StageSet::COMPUTE,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ring,
                    offset: offsets[0],
                    size: vertex_block.len() as u64,
                }),
            }]);
            Some(self.gpu(set)?)
        };
        let pixel_helpers = if program
            .as_ref()
            .is_some_and(|p| p.1.size > 0 || !p.1.merged.is_empty())
        {
            self.helper_blocks.as_ref().map(|h| h.set.handle.clone())
        } else {
            None
        };
        self.helper_blocks = None;
        self.compute_helpers = None;

        // Stream output: each target's filled size gathered into one block the helper advances.
        let streamed = gs.stream.is_some() && self.ctx.so_targets.iter().any(Option::is_some);
        let filled = if streamed {
            Some(self.stream_filled()?)
        } else {
            None
        };
        self.end_pass();
        let mut counters: [Option<B::Buffer>; 4] = Default::default();
        if let Some(f) = &filled {
            self.backend
                .write_buffer(f, 0, &[0; FILLED_BYTES as usize], WriteMode::Ordered);
            for (i, t) in self.ctx.so_targets.clone().iter().enumerate() {
                if let Some(t) = t {
                    let c = self.filled_counter(t)?;
                    self.backend.copy_buffer(&c, 0, f, 4 * i as u64, 4);
                    counters[i] = Some(c);
                }
            }
        }

        let ctx = &self.ctx;
        let target_handles: [_; 4] = std::array::from_fn(|i| {
            ctx.so_targets[i]
                .as_ref()
                .map(|b| (b.gpu.borrow(), b.desc.byte_width))
        });
        let index_handle = ctx
            .index
            .as_ref()
            .filter(|_| indexed)
            .map(|(b, f, offset)| (b.gpu.borrow(), b.size.get() & !3, *offset, *f));
        let mut vertex_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        vertex_sets[0] = ctx.sets[VERTEX].as_ref().map(|k| &k.handle);
        vertex_sets[HELPER_SET as usize] = vertex_set.as_ref();
        let mut geometry_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] =
            [None; MAX_BINDING_SETS];
        geometry_sets[0] = ctx.sets[GEOMETRY].as_ref().map(|k| &k.handle);
        let gpu_range = if indexed {
            DrawRange::Indexed {
                first: start,
                count,
                base_vertex: base as i32,
                first_instance,
                instances,
            }
        } else {
            DrawRange::Vertices {
                first: start,
                count,
                first_instance,
                instances,
            }
        };
        let mut stages = staged.stages.take().expect("the stage helper");
        let ran = stages.run(
            &mut self.backend,
            &StageDraw {
                vertex_sets,
                geometry_sets,
                index: index_handle
                    .as_ref()
                    .map(|(b, size, offset, format)| StageIndex {
                        buffer: &b.handle,
                        size: *size,
                        offset: u64::from(*offset),
                        format: *format,
                    }),
                topology,
                range: gpu_range,
                stream_output: filled.as_ref().map(|f| StreamTargets {
                    buffers: std::array::from_fn(|i| {
                        target_handles[i].as_ref().map(|(b, size)| BufferRange {
                            buffer: &b.handle,
                            offset: 0,
                            size: u64::from(*size) & !3,
                        })
                    }),
                    filled: BufferRange {
                        buffer: f,
                        offset: 0,
                        size: FILLED_BYTES,
                    },
                }),
            },
        );
        staged.stages = Some(stages);
        drop(target_handles);
        drop(index_handle);
        let out = match ran {
            Ok(out) => out,
            // Primitives the geometry shader does not take, which Direct3D does not draw either.
            Err(StageError::TopologyMismatch) => None,
            Err(e) => return Err(self.staged_error(e)),
        };
        if let Some(f) = &filled {
            for (i, c) in counters.iter().enumerate() {
                if let Some(c) = c {
                    self.backend.copy_buffer(f, 4 * i as u64, c, 0, 4);
                }
            }
            self.stream_counted(f)?;
        }
        if !geometry {
            return Ok(gs.drawn);
        }
        let (Some(out), Some((program, _))) = (out, program) else {
            return Ok(false);
        };
        self.draw_output(staged, &program, pixel_helpers.as_ref(), raster, &out)?;
        Ok(false)
    }

    /// The program drawing a geometry stage's output: its pass-through and the pixel shader's
    /// variant `pk`, and the pixel stage's helpers.
    fn staged_program(
        &mut self,
        gs: &Geometry<B>,
        staged: &StageRun<B>,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<(B::Program, Rc<Helpers>)> {
        let key = (
            staged.passthrough_id,
            StageVariant::default(),
            ps.map_or(0, |p| p.gpu.id),
            pk,
        );
        if let Some(p) = self.programs.get(&key) {
            return Ok((p.program.handle.clone(), p.helpers[1].clone()));
        }
        if let Some(p) = ps
            && pk != StageVariant::default()
            && !self.variants.contains_key(&(p.gpu.id, pk))
        {
            let key = self.variant_key(p, pk, SourceStage::Pixel);
            let v = self.convert_kept(&p.bytecode, SourceStage::Pixel, &key)?;
            self.variants.insert((p.gpu.id, pk), v);
        }
        let vertex = match &staged.passthrough {
            Some(k) => &k.handle,
            None => &gs.stage.as_ref().expect("a geometry stage").0.gpu.handle,
        };
        let (fragment, ph) = match ps {
            None => (None, Rc::default()),
            Some(p) => self
                .variants
                .get(&(p.gpu.id, pk))
                .map_or((Some(&p.gpu.handle), p.helpers.clone()), |v| {
                    (Some(&v.shader.handle), v.helpers.clone())
                }),
        };
        let program = self
            .backend
            .create_program(&Program::Render { vertex, fragment });
        let program = self.gpu(program)?;
        let id = self.id();
        let helpers = [Rc::default(), ph.clone()];
        self.programs.insert(
            key,
            Linked {
                program: Kept::new(program.clone(), id, Object::Program, &self.graveyard),
                helpers,
            },
        );
        Ok((program, ph))
    }

    /// The block the stage helper keeps the targets' filled sizes and primitive counts in.
    fn stream_filled(&mut self) -> Result<B::Buffer> {
        if let Some(f) = &self.stream_filled {
            return Ok(f.handle.clone());
        }
        let f = self.backend.create_buffer(&wcontainers_gpu::BufferDesc {
            size: FILLED_BYTES,
            usage: BufferUsage::STORAGE
                | BufferUsage::COPY_SRC
                | BufferUsage::COPY_DST
                | BufferUsage::READ_BACK,
            intent: wcontainers_gpu::MapIntent::Rarely,
        });
        let f = self.gpu(f)?;
        let id = self.id();
        self.stream_filled = Some(Kept::new(f.clone(), id, Object::Buffer, &self.graveyard));
        Ok(f)
    }

    /// The primitives a draw's stream output wrote and needed, added to the running stream
    /// output queries of stream 0 (read back now, `ReadBackWait`).
    fn stream_counted(&mut self, filled: &B::Buffer) -> Result<()> {
        if !self.queries.streams_running() {
            return Ok(());
        }
        let mut bytes = [0u8; 8];
        let read = self.backend.read_back_now(
            &ReadSource::Buffer {
                buffer: filled,
                offset: 16,
                size: 8,
            },
            &mut bytes,
        );
        self.gpu(read)?;
        self.backend.slow_paths().hit(SlowPath::ReadBackWait);
        let written = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        let needed = u32::from_le_bytes(bytes[4..].try_into().unwrap());
        self.queries.add_streamed(written.into(), needed.into());
        Ok(())
    }

    /// The draw of a geometry stage's output, in a pass opened for it.
    fn draw_output(
        &mut self,
        staged: &StageRun<B>,
        program: &B::Program,
        pixel_helpers: Option<&B::BindingSet>,
        raster: Option<&B::Raster>,
        out: &FinalDraw<B>,
    ) -> Result<()> {
        let desc = &self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc;
        let lines = matches!(out.topology, Topology::LineList | Topology::LineStrip);
        if lines && (desc.multisample || desc.antialiased_lines) {
            return not_implemented("lines drawn by the quadrilateral or alpha-antialiased rule");
        }
        if (desc.depth_bias != 0 || desc.slope_scaled_depth_bias != 0.0)
            && !matches!(
                out.topology,
                Topology::TriangleList | Topology::TriangleStrip
            )
        {
            return not_implemented("depth bias on lines and points");
        }
        if self.ctx.viewports.is_empty() || out.count == 0 {
            return Ok(());
        }
        self.open_pass();
        if !self.open_stretch() {
            self.end_pass();
            self.open_pass();
            self.open_stretch();
        }
        let ctx = &self.ctx;
        let v = ctx.viewports[0];
        let viewport = GpuViewport {
            x: v.x.to_bits(),
            y: v.y.to_bits(),
            width: v.width.to_bits(),
            height: v.height.to_bits(),
            min_depth: v.min_depth.to_bits(),
            max_depth: v.max_depth.to_bits(),
        };
        let blend = ctx.blend.as_ref().unwrap_or(&self.defaults.blend);
        let depth_stencil = ctx
            .depth_stencil
            .as_ref()
            .unwrap_or(&self.defaults.depth_stencil);
        let bound_raster = ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
        let no_rect = [ScissorRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }];
        let scissors: &[ScissorRect] = match ctx.scissors.first() {
            _ if !bound_raster.desc.scissor => &[],
            Some(rect) => std::slice::from_ref(rect),
            None => &no_rect,
        };
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        bindings[1] = ctx.sets[PIXEL].as_ref().map(|k| &k.handle);
        bindings[HELPER_SET as usize] = pixel_helpers;
        let vertex_buffers = [Some(VertexBinding {
            buffer: &out.buffer,
            offset: 0,
        })];
        let range = match out.index_offset {
            Some(_) => DrawRange::Indexed {
                first: 0,
                count: out.count,
                base_vertex: 0,
                first_instance: 0,
                instances: 1,
            },
            None => DrawRange::Vertices {
                first: 0,
                count: out.count,
                first_instance: 0,
                instances: 1,
            },
        };
        self.backend.draw(&DrawCall {
            program,
            blend: &blend.gpu.handle,
            depth_stencil: &depth_stencil.gpu.handle,
            raster: raster.unwrap_or(&bound_raster.gpu.handle),
            vertex_layout: &staged
                .layout
                .as_ref()
                .expect("a geometry stage's layout")
                .handle,
            bindings,
            vertex_buffers: &vertex_buffers,
            index: out.index_offset.map(|offset| IndexBinding {
                buffer: &out.buffer,
                offset,
                format: IndexFormat::U32,
            }),
            topology: out.topology,
            restart: out.index_offset.is_some(),
            viewports: std::slice::from_ref(&viewport),
            scissors,
            blend_constant: ctx.blend_factor.map(f32::to_bits),
            stencil_reference: [ctx.stencil_ref as u8; 2],
            immediates: &[],
            range,
        });
        Ok(())
    }
}
