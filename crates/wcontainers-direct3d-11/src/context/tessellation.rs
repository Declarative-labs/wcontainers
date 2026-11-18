//! Hull and domain shaders (plan section 7): the shaders crate's stage helper runs the vertex stage
//! (every input fetched in the program), the hull stage and the tessellator as compute work before
//! the draw (`TessellationAsCompute`); the domain program then draws the tessellated points with
//! the pixel program, reading the patches and points from its helper set.

use super::*;
use wcontainers_gpu::{DOMAIN_PATCHES_BINDING, DOMAIN_POINTS_BINDING};
use wcontainers_shaders::stage::{InputTopology, StageDraw, StageError, StageIndex, Stages};

/// The stage helper of one vertex shader, input layout, hull, domain and pixel shader.
pub(crate) struct TessRun<B: Backend> {
    /// Out while a draw runs it.
    stages: Option<Stages<B>>,
    /// Where the vertex stage's compute form reads its helper constants and fetched streams.
    vertex_helpers: Rc<Helpers>,
    /// The domain program converted for the pixel shader's interpolation, and the id programs of
    /// it are cached by.
    domain: Kept<B, B::Shader>,
    domain_id: u64,
    /// The layout of a draw without vertex streams.
    no_streams: Kept<B, B::VertexLayout>,
}

/// The control points of Direct3D's patch list topologies (33 to 64), none for the others.
fn patch_points(t: u32) -> Option<u8> {
    (topology::PATCHLIST_1..topology::PATCHLIST_1 + 32)
        .contains(&t)
        .then(|| (t - topology::PATCHLIST_1 + 1) as u8)
}

/// A buffer range the domain program reads.
fn read_only<B: Backend>(binding: u32, range: &BufferRange<B::Buffer>) -> BindingEntry<'_, B> {
    BindingEntry {
        binding,
        stages: StageSet::VERTEX,
        resource: BindingResource::StorageBuffer {
            range: BufferRange {
                buffer: &range.buffer,
                offset: range.offset,
                size: range.size,
            },
            writable: false,
        },
    }
}

impl<B: Backend> Device<B> {
    /// The stage helper of the bound shaders, made at their first draw together.
    fn make_tessellated(
        &mut self,
        vs: &Shader<B>,
        hs: &Shader<B>,
        ds: &Shader<B>,
    ) -> Result<TessRun<B>> {
        let pulled = self.ctx.layout.as_ref().map_or(Vec::new(), |l| {
            l.all.iter().map(|e| (e.location, e.format)).collect()
        });
        let key = VariantKey {
            pulled,
            ..self.variant_key(vs, StageVariant::default(), SourceStage::Vertex)
        };
        let vertex = crate::shader::convert_variant(&vs.bytecode, SourceStage::Vertex, &key)?;
        let hull = crate::shader::convert_variant(
            &hs.bytecode,
            SourceStage::Hull,
            &crate::shader::base_key(SourceStage::Hull, &hs.typed),
        )?;
        // The domain stage writes its outputs as the pixel shader interpolates them.
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
        let key = VariantKey {
            output_interpolation: interpolation,
            ..crate::shader::base_key(SourceStage::Domain, &ds.typed)
        };
        let domain = crate::shader::convert_variant(&ds.bytecode, SourceStage::Domain, &key)?;
        let stages = Stages::tessellated(&mut self.backend, &vertex, &hull, &domain)
            .map_err(|e| self.tessellation_error(e))?;
        let gpu = self.backend.create_shader(&domain.modules[0]);
        let gpu = self.gpu(gpu)?;
        let domain_id = self.id();
        let no_streams = self.backend.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; 32],
            attributes: [None; 32],
        });
        let no_streams = self.gpu(no_streams)?;
        let id = self.id();
        Ok(TessRun {
            stages: Some(stages),
            vertex_helpers: Rc::new(Helpers::of(&vertex)),
            domain: Kept::new(gpu, domain_id, Object::Shader, &self.graveyard),
            domain_id,
            no_streams: Kept::new(no_streams, id, Object::VertexLayout, &self.graveyard),
        })
    }

    fn tessellation_error(&mut self, e: StageError) -> Error {
        match e {
            StageError::Gpu(e) => self.gpu::<()>(Err(e)).unwrap_err(),
            StageError::Unsupported(what) => Error::NotImplemented(what),
            StageError::Convert(wcontainers_shaders::ConvertError::Unsupported(what)) => {
                Error::NotImplemented(what)
            }
            StageError::Convert(e) => {
                eprintln!("wcontainers direct3d 11: the tessellation stages' programs: {e}");
                Error::NotImplemented("a tessellation stage the converter got wrong")
            }
            StageError::TopologyMismatch | StageError::Declaration => {
                Error::InvalidArg("a draw the tessellation stages do not take")
            }
        }
    }

    /// A draw with a hull or domain shader bound: the stage helper's dispatches outside a pass,
    /// then the domain program's draw with the pixel program `ps` as `pk`, through `raster` where
    /// the draw needs a twin of the bound block.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_tessellated(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
        raster: Option<&B::Raster>,
        range: Range,
        first_instance: u32,
        instances: u32,
    ) -> Result<()> {
        // Direct3D draws nothing without both stages, or from a topology other than patches.
        let (Some(hs), Some(ds)) = (self.ctx.hs.clone(), self.ctx.ds.clone()) else {
            return Ok(());
        };
        let Some(points) = patch_points(self.ctx.topology) else {
            return Ok(());
        };
        if self.ctx.gs.is_some() {
            return not_implemented("a geometry shader or stream output after tessellation");
        }
        if !self.queries.counted.is_empty() || !self.queries.statistics.is_empty() {
            return not_implemented(
                "a draw through hull and domain shaders while a counted occlusion or statistics \
                 query runs",
            );
        }
        if (StageVariant { pulled: 0, ..vk }) != StageVariant::default() {
            return not_implemented("a vertex shader variant behind a hull shader");
        }
        if hs.helpers.size > 0 || ds.helpers.size > 0 {
            return not_implemented("a hull or domain shader reading helper constants");
        }
        let (start, count, base, indexed) = match range {
            Range::Vertices { count, start } => (start, count, 0, false),
            Range::Indexed { count, start, base } => (start, count, base, true),
            Range::Indirect { .. } => {
                return not_implemented("an indirect draw through hull and domain shaders");
            }
        };
        if indexed && self.ctx.index.is_none() {
            return Ok(());
        }
        let key = (
            vs.gpu.id,
            self.ctx.layout.as_ref().map_or(0, |l| l.id),
            hs.gpu.id,
            ds.gpu.id,
            ps.map_or(0, |p| p.gpu.id),
        );
        let mut run = match self.tessellated.remove(&key) {
            Some(r) => r,
            None => self.make_tessellated(vs, &hs, &ds)?,
        };
        let r = self.run_tessellated(
            &mut run,
            ps,
            pk,
            raster,
            InputTopology::PatchList(points),
            [start, count, base as u32],
            indexed,
            first_instance,
            instances,
        );
        self.tessellated.insert(key, run);
        // The next plain draw makes the vertex set for its own program again.
        self.ctx.vertex_compute = false;
        self.ctx.sets_dirty[VERTEX] = true;
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn run_tessellated(
        &mut self,
        run: &mut TessRun<B>,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
        raster: Option<&B::Raster>,
        topology: InputTopology,
        [start, count, base]: [u32; 3],
        indexed: bool,
        first_instance: u32,
        instances: u32,
    ) -> Result<()> {
        let bases = [if indexed { base } else { start }, first_instance];
        let (program, ph) =
            self.stage_program(&run.domain.handle.clone(), run.domain_id, ps, pk)?;
        if ph.size > 0 || !ph.merged.is_empty() {
            return not_implemented(
                "a pixel shader reading helper constants behind a domain shader",
            );
        }

        // The vertex stage's slots and fetched streams, and its helper block, for compute; the
        // hull stage's slots for compute; the domain and pixel stages' for the draw.
        self.ctx.vertex_compute = true;
        self.ctx.sets_dirty[VERTEX] = true;
        self.ctx.pulled_set = u64::MAX;
        self.grow_constants()?;
        self.refresh_windows()?;
        let pulled = run.vertex_helpers.pulled.clone();
        self.update_set(VERTEX, &pulled)?;
        let vertex_helpers = run.vertex_helpers.clone();
        let vertex_block = self.helper_block(&vertex_helpers, VERTEX, bases, None)?;
        self.update_set(HULL, &[])?;
        self.update_set(DOMAIN, &[])?;
        self.update_set(PIXEL, &[])?;
        if [HULL, DOMAIN, PIXEL]
            .iter()
            .any(|&s| !self.ctx.through[s].is_empty())
        {
            return not_implemented("storage textures read through buffers behind a hull shader");
        }
        let vertex_set = self.compute_helper_set(&vertex_block)?;
        self.helper_blocks = None;
        self.compute_helpers = None;
        self.end_pass();
        let twin_writes = self.twins_in(&[VERTEX, HULL, DOMAIN, PIXEL], PIXEL);

        let ctx = &self.ctx;
        let index_handle = ctx
            .index
            .as_ref()
            .filter(|_| indexed)
            .map(|(b, f, offset)| (b.gpu.borrow(), b.size.get() & !3, *offset, *f));
        let mut vertex_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        vertex_sets[0] = ctx.sets[VERTEX].as_ref().map(|k| &k.handle);
        vertex_sets[HELPER_SET as usize] = vertex_set.as_ref();
        let mut hull_sets: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        hull_sets[0] = ctx.sets[HULL].as_ref().map(|k| &k.handle);
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
        let mut stages = run.stages.take().expect("the stage helper");
        let ran = stages.run(
            &mut self.backend,
            &StageDraw {
                vertex_sets,
                geometry_sets: hull_sets,
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
                stream_output: None,
            },
        );
        run.stages = Some(stages);
        drop(index_handle);
        let out = match ran {
            Ok(Some(out)) => out,
            // No patch to draw, or patches of another size than the hull shader takes, which
            // Direct3D does not draw either.
            Ok(None) | Err(StageError::TopologyMismatch) => return Ok(()),
            Err(e) => return Err(self.tessellation_error(e)),
        };
        let Some(t) = &out.tessellated else {
            return not_implemented("a tessellation stage that made no tessellated draw");
        };
        let set = self.backend.create_binding_set(&[
            read_only::<B>(DOMAIN_PATCHES_BINDING, &t.patches),
            read_only::<B>(DOMAIN_POINTS_BINDING, &t.points),
        ]);
        let set = self.gpu(set)?;
        let layout = run.no_streams.handle.clone();
        self.draw_output(&layout, &program, Some(&set), raster, &out)?;
        if twin_writes {
            self.twins_out(PIXEL);
        }
        Ok(())
    }
}
