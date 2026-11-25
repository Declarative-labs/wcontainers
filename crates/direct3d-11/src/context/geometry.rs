//! Geometry shaders and stream output (plan section 7): the shaders crate's stage helper runs the
//! vertex stage, every input fetched in the program, and the geometry stage as compute work before
//! the draw (`GeometryAsCompute`), and writes the last stage's output into the bound stream output
//! targets (`StreamOutAsCompute`). A pass-through vertex program draws what a geometry stage made
//! with the pixel program; without one the draw is drawn as usual after its stream output.

use super::tessellation::{DomainDraw, PIXEL_HELPERS_BEHIND_DOMAIN, THROUGH_BEHIND_HULL};
use super::*;
use crate::desc::{ViewDesc, bind};
use crate::device::COUNTER_BYTES;
use wcontainers_gpu::{BufferUsage, VariantFlags};
use wcontainers_shaders::ConvertError;
use wcontainers_shaders::Converted;
use wcontainers_shaders::stage::{
    DomainSets, FinalDraw, GeometryReplay, InputTopology, ReplayDraw, STREAM_BUFFERS,
    STREAM_FILLED_WORDS, STREAMS, StageDraw, StageError, StageIndex, Stages, StreamTargets,
};

/// The stage helper of one vertex shader, input layout, hull and domain shader where bound,
/// geometry shader object and pixel shader.
pub(crate) struct StageRun<B: Gpu> {
    /// Out while a draw runs it.
    stages: Option<Stages<B>>,
    /// On a back end without compute programs, the geometry stage's replay in place of `stages`.
    replay: Option<GeometryReplay<B>>,
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
    /// The vertex stage's lines are drawn as the helper's rectangles, which the pass-through draws
    /// with their coverage (`AntialiasedLinesAsQuads`).
    lines: bool,
}

/// The bytes of the stage helper's filled sizes and primitive counts.
const FILLED_BYTES: u64 = (STREAM_FILLED_WORDS * COUNTER_BYTES) as u64;

/// Cull distances or alpha-antialiased lines through the stage helper where the back end has no
/// compute programs: the replay takes a geometry shader or stream output only.
const STAGE_ROUTE_WITHOUT_COMPUTE: &str =
    "cull distances or alpha-antialiased lines through the stage helper without compute programs";

/// A vertex shader variant the replay cannot capture: what follows the geometry stage (the
/// viewport, clip distances) or inputs fetched in the program.
const VARIANT_BEHIND_REPLAY: &str = "a vertex shader variant behind a replayed geometry shader";
/// Storage textures read through buffers of their texels behind a geometry stage.
const THROUGH_BEHIND_GEOMETRY: &str =
    "storage textures read through buffers behind a geometry shader";
const GEOMETRY_WHILE_COUNTED: &str = "a draw through a geometry shader or stream output while a \
     counted occlusion or statistics query runs";
const VARIANT_BEHIND_GEOMETRY: &str = "a vertex shader variant behind a geometry shader";
const INDIRECT_BEHIND_GEOMETRY: &str =
    "an indirect draw through a geometry shader or stream output";
const HELPERS_IN_GEOMETRY: &str = "a geometry shader reading helper constants";
const LAYERS_OF_UNEQUAL_TARGETS: &str =
    "a geometry shader choosing a layer of target views of different layer counts";
const LAYER_OF_REINTERPRETING_TARGET: &str =
    "a geometry shader choosing a layer of a target view reinterpreting its texture";

/// One API draw through a replayed geometry stage.
pub(super) struct Replayed {
    topology: InputTopology,
    start: u32,
    count: u32,
    base_vertex: i32,
    indexed: bool,
    first_instance: u32,
    instances: u32,
}

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
            eprintln!("{WARNING_PREFIX}: the stage helper's programs: {e}");
            Error::NotImplemented("a geometry stage the converter got wrong")
        }
        StageError::TopologyMismatch => {
            Error::InvalidArg("a topology that does not give the geometry shader's input")
        }
        StageError::Declaration => Error::InvalidArg("a stream output declaration"),
    }
}

impl<B: Gpu> Device<B> {
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
        self.backend.record_slow_path(SlowPath::ReadBackWait);
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
    /// hull and domain shaders before the geometry shader object, when bound, `vk` the vertex
    /// shader's variant and `lines` the pixel shader's key where the vertex stage's lines are drawn
    /// alpha-antialiased.
    fn make_staged(
        &mut self,
        gs: &Geometry<B>,
        vs: &Shader<B>,
        vk: StageVariant,
        tess: Option<(&Shader<B>, &Shader<B>)>,
        lines: Option<&VariantKey>,
    ) -> Result<StageRun<B>> {
        let vertex = self.pulled_vertex(vs, vk)?;
        // The geometry stage writes its outputs as the pixel shader interpolates them.
        let interpolation = interpolation(self.ctx.ps.as_deref());
        // A stage naming layers passes the layer named to the pixel program, past both
        // programs' values (`LAYER_FROM_STAGE`), as the draw's pixel variant reads it.
        let layer_named = gs
            .stage
            .as_ref()
            .is_some_and(|(s, _)| crate::shader::names_layer(&s.reflection.outputs));
        let (geometry, passthrough, passthrough_id) = match &gs.stage {
            None => (None, None, 0),
            Some((shader, plain)) if interpolation.is_empty() && !layer_named => {
                (Some(plain.clone()), None, shader.gpu.id)
            }
            Some((shader, _)) => {
                let mut key = VariantKey {
                    output_interpolation: interpolation,
                    ..crate::shader::geometry_key(&shader.typed, shader.layered, shader.clip, false)
                };
                if layer_named {
                    key.flags |= VariantFlags::LAYER_FROM_STAGE;
                    key.linked.location = crate::shader::linked_location(
                        shader
                            .reflection
                            .outputs
                            .iter()
                            .chain(self.ctx.ps.iter().flat_map(|p| &p.reflection.inputs)),
                    );
                }
                let c = crate::shader::convert_geometry(&shader.bytecode, &key)?;
                let gpu = self.backend.create_shader(&c.modules[1]);
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                let kept = Kept::new(gpu, id, Object::Shader, &self.graveyard);
                (Some(c), Some(kept), id)
            }
        };
        let culled = vs.culled && geometry.is_none() && tess.is_none();
        let lines = lines.filter(|_| geometry.is_none() && tess.is_none());
        let (stages, domain) = match (tess, lines) {
            (None, Some(pixel)) => (
                Stages::antialiased_lines(&mut self.backend, &vertex, pixel),
                None,
            ),
            (None, None) if culled => (
                Stages::culled(&mut self.backend, &vertex, gs.stream.as_ref()),
                None,
            ),
            (None, None) => (
                Stages::new(
                    &mut self.backend,
                    &vertex,
                    geometry.as_ref(),
                    gs.stream.as_ref(),
                ),
                None,
            ),
            (Some((hs, ds)), _) => {
                let (hull, domain) = self.tessellation_programs(hs, ds, geometry.is_some())?;
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
        // The cull or line route's pass-through, made for this conversion's outputs.
        let (passthrough, passthrough_id) = match stages.passthrough() {
            Some(module) if culled || lines.is_some() => {
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
            Some(c) if stages.selects() => {
                Rc::new(Helpers::of_passthrough(&c.modules[1], layer_named))
            }
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
            replay: None,
            vertex_helpers: Rc::new(Helpers::of(&vertex)),
            layout,
            passthrough,
            passthrough_id,
            run_helpers,
            domain,
            culled,
            lines: lines.is_some(),
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
            return not_implemented(GEOMETRY_WHILE_COUNTED);
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
        let own_route = (vs.culled || pk.line_coverage) && gs.stage.is_none();
        let passed = (StageVariant { pulled: 0, ..vk }) == StageVariant::default()
            || (own_route && drawn_variant == StageVariant::default());
        let replayed = !self.backend.caps().features.contains(Features::COMPUTE);
        if !passed && !replayed {
            return not_implemented(VARIANT_BEHIND_GEOMETRY);
        }
        let Some(direct) = range.direct() else {
            return not_implemented(INDIRECT_BEHIND_GEOMETRY);
        };
        let Direct {
            start,
            count,
            base,
            indexed,
        } = direct;
        if self.ctx.topology == topology::UNDEFINED || (indexed && self.ctx.index.is_none()) {
            return Ok(false);
        }
        if gs.stage.as_ref().is_some_and(|(s, _)| s.helpers.size > 0) {
            return not_implemented(HELPERS_IN_GEOMETRY);
        }
        if let Some((s, _)) = &gs.stage {
            self.program_paths(s);
        }
        if replayed {
            let draw = Replayed {
                topology,
                start,
                count,
                base_vertex: base,
                indexed,
                first_instance,
                instances,
            };
            return self.draw_replayed(gs, vs, vk, ps, pk, raster, draw);
        }
        let tess = match topology {
            InputTopology::PatchList(_) => self.ctx.hs.clone().zip(self.ctx.ds.clone()),
            _ => None,
        };
        // The pixel shader's key on the line route, whose coverage location its programs write.
        let lines = match ps {
            Some(p) if pk.line_coverage => Some(self.variant_key(p, pk, SourceStage::Pixel)),
            _ => None,
        };
        let key = (
            vs.gpu.id,
            self.ctx.layout.as_ref().map_or(0, |l| l.id),
            tess.as_ref().map_or([0; 2], |(h, d)| [h.gpu.id, d.gpu.id]),
            gs.id,
            ps.map_or(0, |p| p.gpu.id),
            lines
                .as_ref()
                .map(wcontainers_shaders::stage::line_coverage_location),
        );
        let mut staged = match self.staged.remove(&key) {
            Some(s) => s,
            None => self.make_staged(
                gs,
                vs,
                vk,
                tess.as_ref().map(|(h, d)| (&**h, &**d)),
                lines.as_ref(),
            )?,
        };
        let r = self.run_staged(
            gs,
            &mut staged,
            ps,
            pk,
            raster,
            topology,
            direct,
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
        Direct {
            start,
            count,
            base,
            indexed,
        }: Direct,
        first_instance: u32,
        instances: u32,
    ) -> Result<bool> {
        let geometry = gs.stage.is_some();
        let tessellated = matches!(topology, InputTopology::PatchList(_));
        let selects = staged.stages.as_ref().is_some_and(Stages::selects);
        let bases = [if indexed { base as u32 } else { start }, first_instance];
        // The program of the pass-through (or the domain stage's) and pixel stages, and the pixel
        // stage's helpers.
        let culled = staged.culled;
        let lines = staged.lines;
        let program = match (&gs.stage, &staged.domain) {
            _ if !gs.drawn => None,
            (None, None) if culled || lines => {
                let vertex = staged
                    .passthrough
                    .as_ref()
                    .expect("the cull or line route's pass-through")
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
                    return not_implemented(PIXEL_HELPERS_BEHIND_DOMAIN);
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
                return not_implemented(THROUGH_BEHIND_HULL);
            }
        }
        if let Some((_, ph)) = &program {
            self.update_set(PIXEL, &[], &ph.relocated)?;
            if !self.ctx.through[PIXEL].is_empty() {
                return not_implemented(THROUGH_BEHIND_GEOMETRY);
            }
            // Runs per layer or viewport write theirs at each run.
            if !selects && (ph.size > 0 || !ph.merged.is_empty()) {
                self.write_helpers(&[Rc::default(), ph.clone()], bases, None)?;
            }
        }
        let vertex_set = self.helper_set(&vertex_block, StageSet::COMPUTE)?;
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

        let (filled, counters) = self.stream_begin(gs)?;
        self.end_pass();
        let twin_writes = if tessellated {
            self.twins_in(&[VERTEX, HULL, DOMAIN, GEOMETRY, PIXEL], PIXEL)
        } else {
            self.twins_in(&[VERTEX, GEOMETRY, PIXEL], PIXEL)
        };

        let ctx = &self.ctx;
        let target_handles: [_; STREAM_BUFFERS] = std::array::from_fn(|i| {
            ctx.so_targets[i]
                .as_ref()
                .map(|b| (b.gpu.borrow(), b.desc.byte_width))
        });
        let index_handle = ctx
            .index
            .as_ref()
            .filter(|_| indexed)
            .map(|(b, f, offset)| (b.gpu.borrow(), whole_words(b.size.get()), *offset, *f));
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
                base_vertex: base,
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
                        size: whole_words(u64::from(*size)),
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
        } else if lines {
            // The rectangles' widths are in pixels of the first viewport, which the draw is in.
            let size = ctx
                .viewports
                .first()
                .map_or([0.0; 2], |v| [v.width, v.height]);
            stages.run_antialiased_lines(&mut self.backend, &draw, size)
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
        self.stream_end(filled.as_ref(), &counters)?;
        if !geometry && !tessellated && !culled && !lines {
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
        let pixel = (&program, &ph, pixel_helpers.as_ref());
        self.draw_made(staged, pixel, raster, &out, bases, selects)?;
        if twin_writes {
            self.twins_out(PIXEL);
        }
        Ok(false)
    }

    /// The draw of what a geometry, cull or line stage made, through `staged`'s layout, with the
    /// pixel stage's program, helpers and helper set; once per layer and viewport where the stage
    /// `selects` them.
    fn draw_made(
        &mut self,
        staged: &StageRun<B>,
        (program, ph, pixel_helpers): (&B::Program, &Rc<Helpers>, Option<&B::BindingSet>),
        raster: Option<&B::Raster>,
        out: &FinalDraw<B>,
        bases: [u32; 2],
        selects: bool,
    ) -> Result<()> {
        let layout = staged
            .layout
            .as_ref()
            .expect("a geometry, cull or line stage's layout")
            .handle
            .clone();
        if selects {
            let run = staged.run_helpers.clone();
            self.draw_runs(&layout, program, [&run, ph], raster, out, bases)
        } else {
            self.draw_output(&layout, program, pixel_helpers, raster, out, 0)
        }
    }

    /// Stream output's start: each bound target's filled size gathered into one block the stage
    /// helper advances, and each target's counter. A running query counts the primitives needed
    /// even with no target bound. Ends the pass.
    #[allow(clippy::type_complexity)]
    fn stream_begin(
        &mut self,
        gs: &Geometry<B>,
    ) -> Result<(Option<B::Buffer>, [Option<B::Buffer>; STREAM_BUFFERS])> {
        let mut counters: [Option<B::Buffer>; STREAM_BUFFERS] = Default::default();
        let streamed = gs.stream.is_some()
            && (self.ctx.so_targets.iter().any(Option::is_some) || self.queries.streams_running());
        if !streamed {
            return Ok((None, counters));
        }
        let f = self.stream_filled()?;
        self.end_pass();
        self.backend
            .write_buffer(&f, 0, &[0; FILLED_BYTES as usize], WriteMode::Ordered);
        for (i, t) in self.ctx.so_targets.clone().iter().enumerate() {
            if let Some(t) = t {
                let c = self.filled_counter(t)?;
                let at = (COUNTER_BYTES * i) as u64;
                self.backend
                    .copy_buffer(&c, 0, &f, at, COUNTER_BYTES as u64);
                counters[i] = Some(c);
            }
        }
        Ok((Some(f), counters))
    }

    /// Stream output's end: each target's counter advanced from the block, and the running queries
    /// counted.
    fn stream_end(
        &mut self,
        filled: Option<&B::Buffer>,
        counters: &[Option<B::Buffer>; STREAM_BUFFERS],
    ) -> Result<()> {
        let Some(f) = filled else {
            return Ok(());
        };
        // Copies of the targets read since (windows, texture copies) are copied again.
        for t in self.ctx.so_targets.iter().flatten() {
            t.writes.set(t.writes.get() + 1);
        }
        for (i, c) in counters.iter().enumerate() {
            if let Some(c) = c {
                let at = (COUNTER_BYTES * i) as u64;
                self.backend.copy_buffer(f, at, c, 0, COUNTER_BYTES as u64);
            }
        }
        self.stream_counted(f)
    }

    /// The stage run of a geometry shader replayed (`GeometryAsReplay`): the vertex shader as
    /// drawn, and the geometry shader's replay form converted for the pixel shader's interpolation.
    fn make_replayed(
        &mut self,
        gs: &Geometry<B>,
        vs: &Shader<B>,
        vk: StageVariant,
    ) -> Result<StageRun<B>> {
        let key = self.variant_key(vs, vk, SourceStage::Vertex);
        let vertex = crate::shader::convert_variant(&vs.bytecode, SourceStage::Vertex, &key)?;
        let (replay, layer_named) = match (&gs.stage, &gs.stream) {
            (Some((shader, _)), stream) => {
                let layer_named = crate::shader::names_layer(&shader.reflection.outputs);
                let mut key = VariantKey {
                    output_interpolation: interpolation(self.ctx.ps.as_deref()),
                    ..crate::shader::geometry_key(&shader.typed, shader.layered, shader.clip, true)
                };
                if layer_named {
                    key.flags |= VariantFlags::LAYER_FROM_STAGE;
                    key.linked.location = crate::shader::linked_location(
                        shader
                            .reflection
                            .outputs
                            .iter()
                            .chain(self.ctx.ps.iter().flat_map(|p| &p.reflection.inputs)),
                    );
                }
                let geometry = crate::shader::convert_geometry(&shader.bytecode, &key)?;
                let replay =
                    GeometryReplay::new(&mut self.backend, &vertex, &geometry, stream.as_ref());
                (replay, layer_named)
            }
            // A vertex shader alone, captured for its stream output and drawn from the capture.
            (None, Some(stream)) => (
                GeometryReplay::streamed(&mut self.backend, &vertex, stream),
                false,
            ),
            (None, None) => return not_implemented(STAGE_ROUTE_WITHOUT_COMPUTE),
        };
        let replay = replay.map_err(|e| self.staged_error(e))?;
        let gpu = self.backend.create_shader(replay.passthrough());
        let gpu = self.gpu(gpu)?;
        let passthrough_id = self.id();
        let passthrough = Kept::new(gpu, passthrough_id, Object::Shader, &self.graveyard);
        let run_helpers = if replay.selects() {
            Rc::new(Helpers::of_passthrough(replay.passthrough(), layer_named))
        } else {
            Rc::default()
        };
        let gpu = self.backend.create_vertex_layout(&replay.vertex_layout());
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        let layout = Kept::new(gpu, id, Object::VertexLayout, &self.graveyard);
        Ok(StageRun {
            stages: None,
            replay: Some(replay),
            vertex_helpers: Rc::new(Helpers::of(&vertex)),
            layout: Some(layout),
            passthrough: Some(passthrough),
            passthrough_id,
            run_helpers,
            domain: None,
            culled: false,
            lines: false,
        })
    }

    /// A draw through a geometry shader on a back end without compute programs: the shaders
    /// crate's replay captures the vertex stage as drawn, then the geometry shader's replay form,
    /// outside a pass, and the pass-through draws what it captured with the pixel program.
    #[allow(clippy::too_many_arguments)]
    fn draw_replayed(
        &mut self,
        gs: &Rc<Geometry<B>>,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
        raster: Option<&B::Raster>,
        draw: Replayed,
    ) -> Result<bool> {
        if gs.stage.is_none() && gs.stream.is_none() {
            return not_implemented(STAGE_ROUTE_WITHOUT_COMPUTE);
        }
        // The vertex stage is captured as drawn, so a variant applying what follows the geometry
        // stage (the viewport, clip distances) or fetching its inputs has no place there.
        if (StageVariant {
            texture_info: false,
            ..vk
        }) != StageVariant::default()
        {
            return not_implemented(VARIANT_BEHIND_REPLAY);
        }
        if self.ctx.topology == topology::UNDEFINED || (draw.indexed && self.ctx.index.is_none()) {
            return Ok(false);
        }
        let key = (
            vs.gpu.id,
            self.ctx.layout.as_ref().map_or(0, |l| l.id),
            [0; 2],
            gs.id,
            ps.map_or(0, |p| p.gpu.id),
            None,
        );
        let mut staged = match self.staged.remove(&key) {
            Some(s) => s,
            None => self.make_replayed(gs, vs, vk)?,
        };
        let r = self.run_replayed(gs, &mut staged, vs, ps, pk, raster, draw);
        self.staged.insert(key, staged);
        r.map(|()| false)
    }

    #[allow(clippy::too_many_arguments)]
    fn run_replayed(
        &mut self,
        gs: &Geometry<B>,
        staged: &mut StageRun<B>,
        vs: &Rc<Shader<B>>,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
        raster: Option<&B::Raster>,
        draw: Replayed,
    ) -> Result<()> {
        let Replayed {
            start,
            count,
            base_vertex,
            ..
        } = draw;
        let first = if draw.indexed {
            base_vertex as u32
        } else {
            start
        };
        let bases = [first, draw.first_instance];
        let selects = staged.replay.as_ref().is_some_and(GeometryReplay::selects);
        let program = match &staged.passthrough {
            Some(k) if gs.drawn => {
                let vertex = k.handle.clone();
                Some(self.stage_program(&vertex, staged.passthrough_id, ps, pk)?)
            }
            _ => None,
        };

        // The vertex stage's slots and helper block, as for its own draw; the geometry stage's
        // slots, read by its replay form's vertex program.
        self.ctx.vertex_compute = false;
        self.ctx.sets_dirty[VERTEX] = true;
        self.grow_constants()?;
        self.refresh_windows()?;
        self.update_set(VERTEX, &[], &[])?;
        let vertex_helpers = staged.vertex_helpers.clone();
        let vertex_block = self.helper_block(&vertex_helpers, VERTEX, bases, None)?;
        self.update_set(GEOMETRY, &[], &[])?;
        if let Some((_, ph)) = &program {
            self.update_set(PIXEL, &[], &ph.relocated)?;
            if !self.ctx.through[PIXEL].is_empty() {
                return not_implemented(THROUGH_BEHIND_GEOMETRY);
            }
            if !selects && (ph.size > 0 || !ph.merged.is_empty()) {
                self.write_helpers(&[Rc::default(), ph.clone()], bases, None)?;
            }
        }
        let vertex_set = self.helper_set(&vertex_block, StageSet::VERTEX)?;
        let pixel_helpers = match &program {
            Some((_, ph)) if !selects && (ph.size > 0 || !ph.merged.is_empty()) => {
                self.helper_blocks.as_ref().map(|h| h.set.handle.clone())
            }
            _ => None,
        };
        self.helper_blocks = None;
        self.compute_helpers = None;
        let vertex_layout = self.vertex_layout(vs)?;
        let empty_streams = self.ctx.layout.as_ref().is_some_and(|l| {
            l.elements
                .iter()
                .any(|e| self.ctx.vertex_buffers[e.slot as usize].is_none())
        });
        let zero_stream = if empty_streams {
            Some(self.zero_buffer()?)
        } else {
            None
        };
        let (filled, counters) = self.stream_begin(gs)?;
        self.end_pass();
        let twin_writes = self.twins_in(&[VERTEX, GEOMETRY, PIXEL], PIXEL);

        let ctx = &self.ctx;
        let target_handles: [_; STREAM_BUFFERS] = std::array::from_fn(|i| {
            ctx.so_targets[i]
                .as_ref()
                .map(|b| (b.gpu.borrow(), b.desc.byte_width))
        });
        let vertex_handles: [_; VERTEX_SLOTS] = std::array::from_fn(|slot| {
            ctx.vertex_buffers[slot]
                .as_ref()
                .map(|(b, _, offset)| (b.gpu.borrow(), *offset))
        });
        let read = |slot: usize| {
            ctx.layout
                .as_ref()
                .is_some_and(|l| l.elements.iter().any(|e| e.slot as usize == slot))
        };
        let vertex_buffers: [Option<VertexBinding<'_, B>>; VERTEX_SLOTS] =
            std::array::from_fn(|slot| match (&vertex_handles[slot], &zero_stream) {
                (Some((b, offset)), _) => Some(VertexBinding {
                    buffer: &b.handle,
                    offset: u64::from(*offset),
                }),
                (None, Some(zero)) if read(slot) => Some(VertexBinding {
                    buffer: zero,
                    offset: 0,
                }),
                _ => None,
            });
        let used = vertex_buffers
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        let index_handle = ctx
            .index
            .as_ref()
            .filter(|_| draw.indexed)
            .map(|(b, f, offset)| (b.gpu.borrow(), *offset, *f));
        let mut vertex_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        vertex_sets[0] = ctx.sets[VERTEX].as_ref().map(|k| &k.handle);
        vertex_sets[HELPER_SET as usize] = vertex_set.as_ref();
        let mut geometry_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] =
            [None; MAX_BINDING_SETS];
        geometry_sets[0] = ctx.sets[GEOMETRY].as_ref().map(|k| &k.handle);
        let range = if draw.indexed {
            DrawRange::Indexed {
                first: start,
                count,
                base_vertex,
                first_instance: draw.first_instance,
                instances: draw.instances,
            }
        } else {
            DrawRange::Vertices {
                first: start,
                count,
                first_instance: draw.first_instance,
                instances: draw.instances,
            }
        };
        let replay = ReplayDraw {
            vertex_sets,
            vertex_layout: &vertex_layout,
            vertex_buffers: &vertex_buffers[..used],
            index: index_handle
                .as_ref()
                .map(|(b, offset, format)| IndexBinding {
                    buffer: &b.handle,
                    offset: u64::from(*offset),
                    format: *format,
                }),
            immediates: &[],
            geometry_sets,
            topology: draw.topology,
            range,
            stream_output: filled.as_ref().map(|f| StreamTargets {
                buffers: std::array::from_fn(|i| {
                    target_handles[i].as_ref().map(|(b, size)| BufferRange {
                        buffer: &b.handle,
                        offset: 0,
                        size: u64::from(*size),
                    })
                }),
                filled: BufferRange {
                    buffer: f,
                    offset: 0,
                    size: FILLED_BYTES,
                },
            }),
        };
        let mut route = staged.replay.take().expect("the replay");
        let ran = route.run(&mut self.backend, replay);
        staged.replay = Some(route);
        drop(target_handles);
        drop(vertex_handles);
        drop(index_handle);
        let out = match ran {
            Ok(out) => out,
            // Primitives the geometry shader does not take, which Direct3D does not draw either.
            Err(StageError::TopologyMismatch) => None,
            Err(e) => return Err(self.staged_error(e)),
        };
        self.stream_end(filled.as_ref(), &counters)?;
        if let (Some(out), Some((program, ph))) = (out, program) {
            let pixel = (&program, &ph, pixel_helpers.as_ref());
            self.draw_made(staged, pixel, raster, &out, bases, selects)?;
        }
        if twin_writes {
            self.twins_out(PIXEL);
        }
        Ok(())
    }

    /// A helper block bound for `stages` at the vertex stage's helper binding, where it has one.
    /// The stage helper and these blocks write the data ring, so the draws' and dispatches' blocks
    /// as last written are forgotten by the caller.
    pub(super) fn helper_set(
        &mut self,
        block: &[u8],
        stages: StageSet,
    ) -> Result<Option<B::BindingSet>> {
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
            stages,
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
        let (fragment, ph) = self.pixel_stage(ps, pk)?;
        let program = self.backend.create_program(&Program::Render {
            vertex,
            fragment: fragment.as_ref(),
        });
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

    /// The primitives a draw's stream output wrote and needed per stream, added to the running
    /// stream output queries (read back now, `ReadBackWait`).
    fn stream_counted(&mut self, filled: &B::Buffer) -> Result<()> {
        if !self.queries.streams_running() {
            return Ok(());
        }
        let mut bytes = [0u8; 2 * STREAMS * COUNTER_BYTES];
        let read = self.backend.read_back_now(
            &ReadSource::Buffer {
                buffer: filled,
                offset: (STREAM_BUFFERS * COUNTER_BYTES) as u64,
                size: bytes.len() as u64,
            },
            &mut bytes,
        );
        self.gpu(read)?;
        self.backend.record_slow_path(SlowPath::ReadBackWait);
        let mut counts = crate::query::Streamed::default();
        for (count, word) in counts
            .as_flattened_mut()
            .iter_mut()
            .zip(bytes.as_chunks::<COUNTER_BYTES>().0)
        {
            *count = u32::from_le_bytes(*word).into();
        }
        self.queries.add_streamed(&counts);
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
            self.backend.record_slow_path(SlowPath::LayerSplit);
        }
        if viewports > 1 {
            self.backend.record_slow_path(SlowPath::MultiViewportSplit);
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
                self.ctx.run_layers = layers;
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
        self.ctx.run_layers = 0;
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
            return not_implemented(LAYERS_OF_UNEQUAL_TARGETS);
        }
        let mut views = self.ctx.targets.iter().flatten().chain(&self.ctx.depth);
        if layers > 1 && views.any(|v| v.copied()) {
            return not_implemented(LAYER_OF_REINTERPRETING_TARGET);
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
    pub(super) fn layer_view(
        &mut self,
        v: &Rc<View<B>>,
        layer: u32,
        kind: u32,
    ) -> Result<Rc<View<B>>> {
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
