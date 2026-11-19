//! Geometry shaders and stream output (plan section 7): the shaders crate's stage helper runs the
//! vertex stage, every input fetched in the program, and the geometry stage as compute work before
//! the draw (`GeometryAsCompute`), and writes the last stage's output into the bound stream output
//! targets (`StreamOutAsCompute`). A pass-through vertex program draws what a geometry stage made
//! with the pixel program; without one the draw is drawn as usual after its stream output.

use super::tessellation::DomainDraw;
use super::*;
use crate::desc::{ViewDesc, bind};
use wcontainers_gpu::BufferUsage;
use wcontainers_shaders::ConvertError;
use wcontainers_shaders::Converted;
use wcontainers_shaders::stage::{
    DomainSets, FinalDraw, InputTopology, StageDraw, StageError, StageIndex, Stages, StreamTargets,
};

/// The stage helper of one vertex shader, input layout, hull and domain shader where bound,
/// geometry shader object and pixel shader.
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
    /// The pass-through's helper constants, where the geometry stage chooses a layer or viewport.
    run_helpers: Rc<Helpers>,
    /// After a hull and domain stage, the draw of the tessellated points where no geometry stage
    /// follows.
    domain: Option<DomainDraw<B>>,
    /// The vertex stage's cull distances are applied by the helper, whose output the pass-through
    /// draws (`CullDistanceAsGeometry`).
    culled: bool,
}

/// The bytes of the stage helper's filled sizes and primitive counts.
const FILLED_BYTES: u64 = 32;
/// The bytes of one filled size or primitive count.
const COUNTER_BYTES: usize = size_of::<u32>();

/// Direct3D's primitive topology as the stage helper takes it; `None` for patch lists.
pub(super) fn input_topology(t: u32) -> Option<InputTopology> {
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

    /// The counter of the bytes stream output has filled `b` to, made at its first use.
    pub(super) fn filled_counter(&mut self, b: &Buffer<B>) -> Result<B::Buffer> {
        if let Some(f) = &*b.filled.borrow() {
            return Ok(f.handle.clone());
        }
        let f = self.backend.create_buffer(&wcontainers_gpu::BufferDesc {
            size: COUNTER_BYTES as u64,
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
            .write_buffer(&f, 0, &[0; COUNTER_BYTES], WriteMode::Ordered);
        let id = self.id();
        *b.filled.borrow_mut() = Some(Kept::new(f.clone(), id, Object::Buffer, &self.graveyard));
        Ok(f)
    }

    /// The geometry stage of a draw without a geometry shader object: none, stream 0 drawn.
    pub(super) fn no_geometry(&mut self) -> Rc<Geometry<B>> {
        if let Some(g) = &self.no_geometry {
            return g.clone();
        }
        let g = Rc::new(Geometry {
            id: self.id(),
            stage: None,
            stream: None,
            drawn: true,
        });
        self.no_geometry = Some(g.clone());
        g
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
        let mut bytes = [0u8; COUNTER_BYTES];
        let read = self.backend.read_back_now(
            &ReadSource::Buffer {
                buffer: &counter,
                offset: 0,
                size: COUNTER_BYTES as u64,
            },
            &mut bytes,
        );
        self.gpu(read)?;
        self.backend.slow_paths().hit(SlowPath::ReadBackWait);
        Ok(Some(
            u32::from_le_bytes(bytes).saturating_sub(offset) / stride,
        ))
    }

    /// The vertex shader converted as `vk` asks with every input fetched in the program, for the
    /// stage helper.
    pub(super) fn pulled_vertex(&self, vs: &Shader<B>, vk: StageVariant) -> Result<Converted> {
        let pulled = self.ctx.layout.as_ref().map_or(Vec::new(), |l| {
            l.all.iter().map(|e| (e.location, e.format)).collect()
        });
        let key = VariantKey {
            pulled,
            ..self.variant_key(vs, StageVariant { pulled: 0, ..vk }, SourceStage::Vertex)
        };
        crate::shader::convert_variant(&vs.bytecode, SourceStage::Vertex, &key)
    }

    /// The stage helper of the bound shaders, made at their first draw together; `tess` are the
    /// hull and domain shaders before the geometry shader object, when bound, and `vk` the vertex
    /// shader's variant.
    fn make_staged(
        &mut self,
        gs: &Geometry<B>,
        vs: &Shader<B>,
        vk: StageVariant,
        tess: Option<(&Shader<B>, &Shader<B>)>,
    ) -> Result<StageRun<B>> {
        let vertex = self.pulled_vertex(vs, vk)?;
        // The geometry stage writes its outputs as the pixel shader interpolates them.
        let interpolation = interpolation(self.ctx.ps.as_deref());
        let (geometry, passthrough, passthrough_id) = match &gs.stage {
            None => (None, None, 0),
            Some((shader, plain)) if interpolation.is_empty() => {
                (Some(plain.clone()), None, shader.gpu.id)
            }
            Some((shader, _)) => {
                let key = VariantKey {
                    output_interpolation: interpolation,
                    ..crate::shader::base_key(SourceStage::Geometry, &shader.typed, shader.layered)
                };
                let c = crate::shader::convert_geometry(&shader.bytecode, &key)?;
                let gpu = self.backend.create_shader(&c.modules[1]);
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                let kept = Kept::new(gpu, id, Object::Shader, &self.graveyard);
                (Some(c), Some(kept), id)
            }
        };
        let culled = vs.culled && geometry.is_none() && tess.is_none();
        let (stages, domain) = match tess {
            None if culled => (
                Stages::culled(&mut self.backend, &vertex, gs.stream.as_ref()),
                None,
            ),
            None => (
                Stages::new(
                    &mut self.backend,
                    &vertex,
                    geometry.as_ref(),
                    gs.stream.as_ref(),
                ),
                None,
            ),
            Some((hs, ds)) => {
                if ds.clip.is_some() && geometry.is_some() {
                    return not_implemented(
                        "clip distances from a domain shader before a geometry shader, without the \
                         back end's",
                    );
                }
                let (hull, domain) = self.tessellation_programs(hs, ds)?;
                let drawn = match geometry {
                    None if gs.drawn => Some(self.domain_draw(&domain)?),
                    _ => None,
                };
                let stages = Stages::tessellated_with(
                    &mut self.backend,
                    &vertex,
                    &hull,
                    &domain,
                    geometry.as_ref(),
                    gs.stream.as_ref(),
                );
                (stages, drawn)
            }
        };
        let stages = stages.map_err(|e| self.staged_error(e))?;
        // The cull route's pass-through, made for this conversion's outputs.
        let (passthrough, passthrough_id) = match stages.passthrough() {
            Some(module) if culled => {
                let gpu = self.backend.create_shader(module);
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                (
                    Some(Kept::new(gpu, id, Object::Shader, &self.graveyard)),
                    id,
                )
            }
            _ => (passthrough, passthrough_id),
        };
        let run_helpers = match &geometry {
            Some(c) if stages.selects() => Rc::new(Helpers::of_passthrough(&c.modules[1])),
            _ => Rc::default(),
        };
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
            run_helpers,
            domain,
            culled,
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
        topology: InputTopology,
    ) -> Result<bool> {
        if !self.queries.counted.is_empty() || !self.queries.statistics.is_empty() {
            return not_implemented(
                "a draw through a geometry shader or stream output while a counted occlusion or \
                 statistics query runs",
            );
        }
        // Behind a culled vertex shader the pixel program applies its clip distances and clamped
        // depth, which the pass-through passes on.
        let drawn_variant = StageVariant {
            pulled: 0,
            linked: 0,
            clip_distances: 0,
            depth_clamp: false,
            texture_info: false,
            ..vk
        };
        let passed = (StageVariant { pulled: 0, ..vk }) == StageVariant::default()
            || (vs.culled && gs.stage.is_none() && drawn_variant == StageVariant::default());
        if !passed {
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
        if gs.stage.as_ref().is_some_and(|(s, _)| s.helpers.size > 0) {
            return not_implemented("a geometry shader reading helper constants");
        }
        if let Some((s, _)) = &gs.stage {
            self.program_paths(s);
        }
        let tess = match topology {
            InputTopology::PatchList(_) => self.ctx.hs.clone().zip(self.ctx.ds.clone()),
            _ => None,
        };
        let key = (
            vs.gpu.id,
            self.ctx.layout.as_ref().map_or(0, |l| l.id),
            tess.as_ref().map_or([0; 2], |(h, d)| [h.gpu.id, d.gpu.id]),
            gs.id,
            ps.map_or(0, |p| p.gpu.id),
        );
        let mut staged = match self.staged.remove(&key) {
            Some(s) => s,
            None => self.make_staged(gs, vs, vk, tess.as_ref().map(|(h, d)| (&**h, &**d)))?,
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
        // The next plain draw makes the vertex and domain sets for its own program again.
        self.ctx.vertex_compute = false;
        self.ctx.sets_dirty[VERTEX] = true;
        if self.ctx.domain_compute {
            self.ctx.domain_compute = false;
            self.ctx.sets_dirty[DOMAIN] = true;
        }
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
        let tessellated = matches!(topology, InputTopology::PatchList(_));
        let selects = staged.stages.as_ref().is_some_and(Stages::selects);
        let bases = [if indexed { base } else { start }, first_instance];
        // The program of the pass-through (or the domain stage's) and pixel stages, and the pixel
        // stage's helpers.
        let culled = staged.culled;
        let program = match (&gs.stage, &staged.domain) {
            _ if !gs.drawn => None,
            (None, None) if culled => {
                let vertex = staged
                    .passthrough
                    .as_ref()
                    .expect("the cull route's pass-through")
                    .handle
                    .clone();
                Some(self.stage_program(&vertex, staged.passthrough_id, ps, pk)?)
            }
            (Some((s, _)), _) => {
                let vertex = match &staged.passthrough {
                    Some(k) => k.handle.clone(),
                    None => s.gpu.handle.clone(),
                };
                Some(self.stage_program(&vertex, staged.passthrough_id, ps, pk)?)
            }
            (None, Some(d)) => {
                let p = self.stage_program(&d.shader.handle.clone(), d.id, ps, pk)?;
                if p.1.size > 0 || !p.1.merged.is_empty() {
                    return not_implemented(
                        "a pixel shader reading helper constants behind a domain shader",
                    );
                }
                Some(p)
            }
            (None, None) => None,
        };

        // The vertex stage's slots and fetched streams, and its helper block, for compute.
        self.ctx.vertex_compute = true;
        self.ctx.sets_dirty[VERTEX] = true;
        self.ctx.pulled_set = u64::MAX;
        self.grow_constants()?;
        self.refresh_windows()?;
        let pulled = staged.vertex_helpers.pulled.clone();
        self.update_set(VERTEX, &pulled, &[])?;
        let vertex_helpers = staged.vertex_helpers.clone();
        let vertex_block = self.helper_block(&vertex_helpers, VERTEX, bases, None)?;
        if geometry {
            self.update_set(GEOMETRY, &[], &[])?;
        }
        if tessellated {
            self.update_set(HULL, &[], &[])?;
            self.ctx.domain_compute = true;
            self.ctx.sets_dirty[DOMAIN] = true;
            self.update_set(DOMAIN, &[], &[])?;
            if [HULL, DOMAIN, GEOMETRY]
                .iter()
                .any(|&s| !self.ctx.through[s].is_empty())
            {
                return not_implemented(
                    "storage textures read through buffers behind a hull shader",
                );
            }
        }
        if let Some((_, ph)) = &program {
            self.update_set(PIXEL, &[], &ph.relocated)?;
            if !self.ctx.through[PIXEL].is_empty() {
                return not_implemented(
                    "storage textures read through buffers behind a geometry shader",
                );
            }
            // Runs per layer or viewport write theirs at each run.
            if !selects && (ph.size > 0 || !ph.merged.is_empty()) {
                self.write_helpers(&[Rc::default(), ph.clone()], bases, None)?;
            }
        }
        let vertex_set = self.compute_helper_set(&vertex_block)?;
        let pixel_helpers = if !selects
            && program
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
        let twin_writes = if tessellated {
            self.twins_in(&[VERTEX, HULL, DOMAIN, GEOMETRY, PIXEL], PIXEL)
        } else {
            self.twins_in(&[VERTEX, GEOMETRY, PIXEL], PIXEL)
        };
        let mut counters: [Option<B::Buffer>; 4] = Default::default();
        if let Some(f) = &filled {
            self.backend
                .write_buffer(f, 0, &[0; FILLED_BYTES as usize], WriteMode::Ordered);
            for (i, t) in self.ctx.so_targets.clone().iter().enumerate() {
                if let Some(t) = t {
                    let c = self.filled_counter(t)?;
                    let at = (COUNTER_BYTES * i) as u64;
                    self.backend.copy_buffer(&c, 0, f, at, COUNTER_BYTES as u64);
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
        // After a hull stage the stage helper's second sets are the hull stage's, and the domain
        // and geometry stages' follow.
        let mut hull_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        hull_sets[0] = ctx.sets[HULL].as_ref().map(|k| &k.handle);
        let mut domain_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        domain_sets[0] = ctx.sets[DOMAIN].as_ref().map(|k| &k.handle);
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
        let draw = StageDraw {
            vertex_sets,
            geometry_sets: if tessellated {
                hull_sets
            } else {
                geometry_sets
            },
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
        };
        let ran = if tessellated {
            let later = DomainSets {
                domain_sets,
                geometry_sets,
            };
            stages.run_after_domain(&mut self.backend, &draw, &later)
        } else {
            stages.run(&mut self.backend, &draw)
        };
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
                    let at = (COUNTER_BYTES * i) as u64;
                    self.backend.copy_buffer(f, at, c, 0, COUNTER_BYTES as u64);
                }
            }
            self.stream_counted(f)?;
        }
        if !geometry && !tessellated && !culled {
            return Ok(gs.drawn);
        }
        let (Some(out), Some((program, ph))) = (out, program) else {
            return Ok(false);
        };
        if let (false, Some(d)) = (geometry, &staged.domain) {
            // The domain stage's set again for its draw.
            self.ctx.domain_compute = false;
            self.ctx.sets_dirty[DOMAIN] = true;
            self.update_set(DOMAIN, &[], &[])?;
            self.draw_tessellated_output(d, &program, raster, &out)?;
            if twin_writes {
                self.twins_out(PIXEL);
            }
            return Ok(false);
        }
        let layout = staged
            .layout
            .as_ref()
            .expect("a geometry or cull stage's layout")
            .handle
            .clone();
        if selects {
            let run = staged.run_helpers.clone();
            self.draw_runs(&layout, &program, [&run, &ph], raster, &out, bases)?;
        } else {
            self.draw_output(&layout, &program, pixel_helpers.as_ref(), raster, &out, 0)?;
        }
        if twin_writes {
            self.twins_out(PIXEL);
        }
        Ok(false)
    }

    /// The vertex stage's helper block bound for the stage helper's compute programs, where it has
    /// one. The stage helper and these blocks write the data ring, so the draws' and dispatches'
    /// blocks as last written are forgotten by the caller.
    pub(super) fn compute_helper_set(&mut self, block: &[u8]) -> Result<Option<B::BindingSet>> {
        if block.is_empty() {
            return Ok(None);
        }
        let limits = self.backend.caps().limits;
        let mut offsets = [0u64];
        let ring = self.backend.write_buffer_ring(
            RingKind::Data,
            &[block],
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
                size: block.len() as u64,
            }),
        }]);
        Ok(Some(self.gpu(set)?))
    }

    /// The program drawing what the stage helper made: the vertex program `vertex` (numbered
    /// `vertex_id`) and the pixel shader's variant `pk`, and the pixel stage's helpers.
    pub(super) fn stage_program(
        &mut self,
        vertex: &B::Shader,
        vertex_id: u64,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<(B::Program, Rc<Helpers>)> {
        let key = (
            vertex_id,
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
        let (written, needed) = bytes.split_at(COUNTER_BYTES);
        let written = u32::from_le_bytes(written.try_into().unwrap());
        let needed = u32::from_le_bytes(needed.try_into().unwrap());
        self.queries.add_streamed(written.into(), needed.into());
        Ok(())
    }

    /// The draw of a geometry stage's output that chooses a layer or viewport: once per layer of
    /// the bound views, into views of that layer alone (`LayerSplit`), and per bound viewport
    /// (`MultiViewportSplit`), each run keeping the primitives that name it, with its layer and
    /// viewport in the helpers `[pass-through, pixel]`. A primitive naming a layer or viewport
    /// past the bound ones is in no run.
    fn draw_runs(
        &mut self,
        layout: &B::VertexLayout,
        program: &B::Program,
        helpers: [&Rc<Helpers>; 2],
        raster: Option<&B::Raster>,
        out: &FinalDraw<B>,
        bases: [u32; 2],
    ) -> Result<()> {
        let layers = self.target_layers()?;
        let viewports = self.ctx.viewports.len() as u32;
        if layers > 1 {
            self.backend.slow_paths().hit(SlowPath::LayerSplit);
        }
        if viewports > 1 {
            self.backend.slow_paths().hit(SlowPath::MultiViewportSplit);
        }
        let bound = (self.ctx.targets.clone(), self.ctx.depth.clone());
        let mut r = Ok(());
        for layer in 0..layers {
            if layers > 1 {
                r = self.bind_layer(&bound, layer);
                if r.is_err() {
                    break;
                }
            }
            for viewport in 0..viewports {
                self.ctx.run = [layer, viewport];
                r = self
                    .write_helpers(&[helpers[0].clone(), helpers[1].clone()], bases, None)
                    .and_then(|()| {
                        let set = self.helper_blocks.as_ref().map(|h| h.set.handle.clone());
                        self.draw_output(layout, program, set.as_ref(), raster, out, viewport)
                    });
                if r.is_err() {
                    break;
                }
            }
            if r.is_err() {
                break;
            }
        }
        if layers > 1 {
            self.end_pass();
            (self.ctx.targets, self.ctx.depth) = bound;
        }
        self.ctx.run = [0; 2];
        self.helper_blocks = None;
        r
    }

    /// The layers of the bound target and depth views, which are alike in a layered draw.
    fn target_layers(&self) -> Result<u32> {
        let mut views = self.ctx.targets.iter().flatten().chain(&self.ctx.depth);
        let Some(first) = views.next() else {
            return Ok(1);
        };
        let layers = first.desc.layer_count;
        let mut views = self.ctx.targets.iter().flatten().chain(&self.ctx.depth);
        if views.any(|v| v.desc.layer_count != layers) {
            return not_implemented(
                "a geometry shader choosing a layer of target views of different layer counts",
            );
        }
        let mut views = self.ctx.targets.iter().flatten().chain(&self.ctx.depth);
        if layers > 1 && views.any(|v| v.copied()) {
            return not_implemented(
                "a geometry shader choosing a layer of a target view reinterpreting its texture",
            );
        }
        Ok(layers)
    }

    /// Binds layer `layer` of each of the `bound` targets and depth view, after ending the pass.
    #[allow(clippy::type_complexity)]
    fn bind_layer(
        &mut self,
        (targets, depth): &(Vec<Option<Rc<View<B>>>>, Option<Rc<View<B>>>),
        layer: u32,
    ) -> Result<()> {
        self.end_pass();
        for (i, t) in targets.iter().enumerate() {
            self.ctx.targets[i] = match t {
                Some(v) => Some(self.layer_view(v, layer, bind::RENDER_TARGET)?),
                None => None,
            };
        }
        self.ctx.depth = match depth {
            Some(v) => Some(self.layer_view(v, layer, bind::DEPTH_STENCIL)?),
            None => None,
        };
        Ok(())
    }

    /// Layer `layer` of the drawn view `v` (bound as `kind`) as a view of that layer alone.
    fn layer_view(&mut self, v: &Rc<View<B>>, layer: u32, kind: u32) -> Result<Rc<View<B>>> {
        if v.layers.borrow().is_empty() {
            let mut made = Vec::with_capacity(v.desc.layer_count as usize);
            for l in 0..v.desc.layer_count {
                let desc = ViewDesc {
                    first_layer: v.desc.first_layer + l,
                    layer_count: 1,
                    ..v.desc
                };
                made.push(self.create_view(&v.texture, Some(&desc), kind)?);
            }
            *v.layers.borrow_mut() = made;
        }
        Ok(v.layers.borrow()[layer as usize].clone())
    }

    /// The draw of a geometry or domain stage's output through `layout`, in a pass opened for it,
    /// with `helpers` in the helper set, through viewport and scissor rectangle `viewport`.
    pub(super) fn draw_output(
        &mut self,
        layout: &B::VertexLayout,
        program: &B::Program,
        helpers: Option<&B::BindingSet>,
        raster: Option<&B::Raster>,
        out: &FinalDraw<B>,
        viewport: u32,
    ) -> Result<()> {
        let desc = &self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc;
        let lines = matches!(out.topology, Topology::LineList | Topology::LineStrip);
        // As the plain draw's lines (`ContextState::draw`).
        if lines && desc.antialiased_lines && !desc.multisample {
            return not_implemented(super::ALPHA_ANTIALIASED_LINES);
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
        if self.viewport_in_shader().is_some() {
            return not_implemented(super::VIEWPORT_BEHIND_GEOMETRY);
        }
        self.open_pass();
        if !self.open_stretch() {
            self.end_pass();
            self.open_pass();
            self.open_stretch();
        }
        let ctx = &self.ctx;
        let v = ctx.viewports[viewport as usize];
        let rect = ctx.scissors.get(viewport as usize);
        let viewport = GpuViewport {
            x: v.x.to_bits(),
            y: v.y.to_bits(),
            width: v.width.to_bits(),
            height: v.height.to_bits(),
            min_depth: v.min_depth.to_bits(),
            max_depth: v.max_depth.to_bits(),
        };
        let blend = ctx.blend.as_ref().unwrap_or(&self.defaults.blend);
        let depth_stencil = &self.drawn_depth_stencil();
        let bound_raster = ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
        let no_rect = [ScissorRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }];
        let scissors: &[ScissorRect] = match rect {
            _ if !bound_raster.desc.scissor => &[],
            Some(rect) => std::slice::from_ref(rect),
            None => &no_rect,
        };
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        bindings[1] = ctx.sets[PIXEL].as_ref().map(|k| &k.handle);
        bindings[HELPER_SET as usize] = helpers;
        let stream = [Some(VertexBinding {
            buffer: &out.buffer,
            offset: 0,
        })];
        let mut vertex_buffers: &[Option<VertexBinding<'_, B>>] = &stream;
        // The tessellated points are drawn by the domain program from its own sets, as an indexed
        // indirect draw the tessellator wrote.
        let mut index = out.index_offset.map(|offset| IndexBinding {
            buffer: &out.buffer,
            offset,
            format: IndexFormat::U32,
        });
        let range = match (&out.tessellated, out.index_offset) {
            (Some(t), _) => {
                bindings[0] = ctx.sets[DOMAIN].as_ref().map(|k| &k.handle);
                vertex_buffers = &[];
                index = Some(IndexBinding {
                    buffer: &out.buffer,
                    offset: 0,
                    format: IndexFormat::U32,
                });
                DrawRange::IndexedIndirect {
                    args: &t.arguments,
                    offset: 0,
                }
            }
            (None, Some(_)) => DrawRange::Indexed {
                first: 0,
                count: out.count,
                base_vertex: 0,
                first_instance: 0,
                instances: 1,
            },
            (None, None) => DrawRange::Vertices {
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
            vertex_layout: layout,
            bindings,
            vertex_buffers,
            index,
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
