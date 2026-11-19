//! Hull and domain shaders (plan section 7): the shaders crate's stage helper runs the vertex stage
//! (every input fetched in the program), the hull stage and the tessellator as compute work before
//! the draw (`TessellationAsCompute`); the domain program then draws the tessellated points with
//! the pixel program, reading the patches and points from its helper set.

use super::*;
use wcontainers_gpu::{DOMAIN_PATCHES_BINDING, DOMAIN_POINTS_BINDING};
use wcontainers_shaders::Converted;
use wcontainers_shaders::stage::{
    FinalDraw, InputTopology, StageDraw, StageError, StageIndex, Stages,
};

/// The stage helper of one vertex shader, input layout, hull, domain and pixel shader.
pub(crate) struct TessRun<B: Backend> {
    /// Out while a draw runs it.
    stages: Option<Stages<B>>,
    /// Where the vertex stage's compute form reads its helper constants and fetched streams.
    vertex_helpers: Rc<Helpers>,
    domain: DomainDraw<B>,
}

/// The draw of tessellated points: the domain program converted for the pixel shader's
/// interpolation, the id programs of it are cached by, and the layout of a draw without vertex
/// streams.
pub(crate) struct DomainDraw<B: Backend> {
    pub(super) shader: Kept<B, B::Shader>,
    pub(super) id: u64,
    no_streams: Kept<B, B::VertexLayout>,
}

/// The control points of Direct3D's patch list topologies (33 to 64), none for the others.
pub(super) fn patch_points(t: u32) -> Option<u8> {
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
        let vertex = self.pulled_vertex(vs, StageVariant::default())?;
        let (hull, domain) = self.tessellation_programs(hs, ds, false)?;
        let stages = Stages::tessellated(&mut self.backend, &vertex, &hull, &domain)
            .map_err(|e| self.tessellation_error(e))?;
        Ok(TessRun {
            stages: Some(stages),
            vertex_helpers: Rc::new(Helpers::of(&vertex)),
            domain: self.domain_draw(&domain)?,
        })
    }

    /// The hull shader's and domain shader's conversions for the stage helper, the domain stage
    /// writing its outputs as the pixel shader interpolates them and passing on its clip
    /// distances where the back end lacks them.
    pub(super) fn tessellation_programs(
        &mut self,
        hs: &Shader<B>,
        ds: &Shader<B>,
        before_geometry: bool,
    ) -> Result<(Converted, Converted)> {
        let hull = crate::shader::convert_variant(
            &hs.bytecode,
            SourceStage::Hull,
            &crate::shader::base_key(SourceStage::Hull, &hs.typed, hs.layered),
        )?;
        // A domain program writing clip distances the back end lacks passes them on for the pixel
        // program, as the vertex program does without tessellation; before a geometry stage they
        // are outputs that stage reads.
        let (linked, clip_distances) = ds.clip.filter(|_| !before_geometry).unwrap_or_default();
        let key = VariantKey {
            output_interpolation: interpolation(self.ctx.ps.as_deref()),
            ..StageVariant {
                linked,
                clip_distances,
                ..StageVariant::default()
            }
            .key(SourceStage::Domain, &ds.typed, ds.layered, &[])
        };
        let domain = crate::shader::convert_variant(&ds.bytecode, SourceStage::Domain, &key)?;
        Ok((hull, domain))
    }

    /// The draw of the points `domain` places.
    pub(super) fn domain_draw(&mut self, domain: &Converted) -> Result<DomainDraw<B>> {
        let gpu = self.backend.create_shader(&domain.modules[0]);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        let no_streams = self.backend.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        });
        let no_streams = self.gpu(no_streams)?;
        let layout_id = self.id();
        Ok(DomainDraw {
            shader: Kept::new(gpu, id, Object::Shader, &self.graveyard),
            id,
            no_streams: Kept::new(no_streams, layout_id, Object::VertexLayout, &self.graveyard),
        })
    }

    /// The draw of the tessellated points `out` through `domain` with `program`, the patches and
    /// points bound in its helper set.
    pub(super) fn draw_tessellated_output(
        &mut self,
        domain: &DomainDraw<B>,
        program: &B::Program,
        raster: Option<&B::Raster>,
        out: &FinalDraw<B>,
    ) -> Result<()> {
        let Some(t) = &out.tessellated else {
            return not_implemented("a tessellation stage that made no tessellated draw");
        };
        let set = self.backend.create_binding_set(&[
            read_only::<B>(DOMAIN_PATCHES_BINDING, &t.patches),
            read_only::<B>(DOMAIN_POINTS_BINDING, &t.points),
        ]);
        let set = self.gpu(set)?;
        let layout = domain.no_streams.handle.clone();
        self.draw_output(&layout, program, Some(&set), raster, out, 0)
    }

    fn tessellation_error(&mut self, e: StageError) -> Error {
        match e {
            StageError::Gpu(e) => self.gpu::<()>(Err(e)).unwrap_err(),
            StageError::Unsupported(what) => Error::NotImplemented(what),
            StageError::Convert(wcontainers_shaders::ConvertError::Unsupported(what)) => {
                Error::NotImplemented(what)
            }
            StageError::Convert(e) => {
                eprintln!("{WARNING_PREFIX}: the tessellation stages' programs: {e}");
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
        self.program_paths(&hs);
        self.program_paths(&ds);
        if let Some(gs) = self.ctx.gs.clone() {
            let topology = InputTopology::PatchList(points);
            return self
                .draw_staged(
                    &gs,
                    vs,
                    vk,
                    ps,
                    pk,
                    raster,
                    range,
                    first_instance,
                    instances,
                    topology,
                )
                .map(|_| ());
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
            self.stage_program(&run.domain.shader.handle.clone(), run.domain.id, ps, pk)?;
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
        self.update_set(VERTEX, &pulled, &[])?;
        let vertex_helpers = run.vertex_helpers.clone();
        let vertex_block = self.helper_block(&vertex_helpers, VERTEX, bases, None)?;
        self.update_set(HULL, &[], &[])?;
        self.update_set(DOMAIN, &[], &[])?;
        self.update_set(PIXEL, &[], &ph.relocated)?;
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
        self.draw_tessellated_output(&run.domain, &program, raster, &out)?;
        if twin_writes {
            self.twins_out(PIXEL);
        }
        Ok(())
    }
}
