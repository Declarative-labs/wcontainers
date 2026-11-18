//! Geometry shaders (plan section 7): the shaders crate's stage helper runs the vertex and
//! geometry stages as compute work before the draw (`GeometryAsCompute`), every vertex input
//! fetched in the program, and a pass-through vertex program draws what they made with the
//! pipeline's pixel program.

use wcontainers_gpu::{StageExtras, VertexFormat};
use wcontainers_shaders::stage::{FinalDraw, StageDraw, StageError, StageIndex, Stages};

use super::*;
use pull::Bases;

/// What a pipeline with a geometry shader adds.
pub(super) struct Geometry<B: Backend> {
    /// The two stages' compute programs and buffers; out while a draw runs them.
    pub stages: Option<Stages<B>>,
    /// The vertex stage's helper block and its pulled streams' bindings (binding, location).
    pub helpers: Helpers,
    pub streams: Vec<(u32, u32)>,
    /// The vertex layout of the draw after the stages.
    pub layout: B::VertexLayout,
}

/// A pipeline's geometry stage as made: what draws need, the two converted stages whose
/// bindings the pipeline binds, and the primitives drawn after it.
pub(super) struct Made<B: Backend> {
    pub geometry: Geometry<B>,
    pub vertex: Converted,
    pub shader: Converted,
    pub output: Topology,
}

impl<B: Backend> Device<B> {
    /// The geometry stage of a pipeline: `vs` converted with every input of `inputs` fetched in
    /// the program, `gs` writing its outputs as the pixel stage interpolates them, and the stage
    /// helper made from both.
    pub(super) fn geometry_stage(
        &mut self,
        vs: &[u8],
        gs: &[u8],
        inputs: &[(InputElement, u32)],
        interpolation: &[(u32, Interpolation)],
    ) -> Result<Made<B>, Error> {
        if !self.backend.caps().has(Features::COMPUTE) {
            return Err(Error::NotImplemented("geometry shaders on this back end"));
        }
        let pulled: Vec<(u32, VertexFormat)> = inputs
            .iter()
            .map(|(e, location)| Ok((*location, vertex_format(e.format)?.0)))
            .collect::<Result<_, Error>>()?;
        let vertex = self.convert(
            vs,
            ShaderStage::Vertex,
            VariantKey {
                pulled,
                draw_base: DrawBaseSource::Helpers,
                ..VariantKey::default()
            },
        )?;
        let shader = self.convert_geometry(
            gs,
            VariantKey {
                output_interpolation: interpolation.to_vec(),
                ..VariantKey::default()
            },
        )?;
        let StageExtras::Geometry {
            output_vertices, ..
        } = shader.reflection.extras
        else {
            return Err(Error::Invalid("a geometry shader without its declarations"));
        };
        let output = match output_vertices {
            1 => Topology::PointList,
            2 => Topology::LineStrip,
            _ => Topology::TriangleStrip,
        };
        let stages =
            Stages::new(&mut self.backend, &vertex, Some(&shader), None).map_err(stage_error)?;
        let layout = self.layout(stages.vertex_layout().expect("a geometry stage"))?;
        let streams = vertex
            .reflection
            .bindings
            .iter()
            .filter_map(|b| match b.added {
                Some(wcontainers_gpu::AddedBinding::PulledStream(location)) => {
                    Some((b.binding, location))
                }
                _ => None,
            })
            .collect();
        Ok(Made {
            geometry: Geometry {
                stages: Some(stages),
                helpers: Helpers::of(&vertex),
                streams,
                layout,
            },
            vertex,
            shader,
            output,
        })
    }
}

/// A stage helper refusal as the front reports it.
fn stage_error(e: StageError) -> Error {
    match e {
        StageError::Gpu(e) => e.into(),
        StageError::Unsupported(what) => Error::NotImplemented(what),
        StageError::Convert(_) => {
            Error::NotImplemented("a geometry shader the converter got wrong")
        }
        StageError::TopologyMismatch => {
            Error::Invalid("a topology that does not give the geometry shader's input")
        }
        StageError::Declaration => Error::Invalid("a stream output declaration"),
    }
}

impl<B: Backend> Replay<B> {
    /// A draw of a pipeline with a geometry shader: the stage helper's dispatches outside the
    /// pass, then the draw of their output in it.
    pub(super) fn draw_geometry(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
        range: DrawRange<'_, B>,
        record: Option<&(B::Buffer, u64)>,
    ) -> Result<(), Error> {
        if record.is_some() {
            return Err(Error::NotImplemented(
                "a predicated draw through a geometry shader",
            ));
        }
        if self.counted.is_some() || self.statistics.is_some() {
            return Err(Error::NotImplemented(
                "a draw through a geometry shader while a counted occlusion or statistics query \
                 runs",
            ));
        }
        let (first_instance, bases) = match range {
            DrawRange::Vertices {
                first,
                first_instance,
                ..
            } => (first_instance, [first, first_instance]),
            DrawRange::Indexed {
                base_vertex,
                first_instance,
                ..
            } => (first_instance, [base_vertex as u32, first_instance]),
            _ => {
                return Err(Error::NotImplemented(
                    "an indirect draw through a geometry shader",
                ));
            }
        };
        let topology = crate::state::input_topology(self.api_topology)?;
        // Every vertex input is fetched in the program.
        let inputs = std::mem::take(&mut d.render_mut(pid).inputs);
        let mut pulled = [None; MAX_VERTEX_STREAMS];
        for (e, _) in &inputs {
            pulled[e.slot as usize] = Some(SlowPath::GeometryAsCompute);
        }
        let fetch = d.fetch(&inputs, &self.vertex_buffers, &pulled, Some(first_instance));
        d.render_mut(pid).inputs = inputs;
        let fetch = fetch?;
        let r = d.render_mut(pid);
        let g = r.geometry.as_ref().expect("a geometry stage");
        let helpers = g.helpers.clone();
        let mut extra = Vec::with_capacity(g.streams.len());
        for &(binding, location) in &g.streams {
            let slot = r
                .inputs
                .iter()
                .find(|(_, l)| *l == location)
                .ok_or(Error::Invalid("a pulled input the pipeline lacks"))?
                .0
                .slot as usize;
            let (buffer, offset, size) = fetch.ranges[slot]
                .as_ref()
                .ok_or(Error::Invalid("a pulled stream the draw does not bind"))?;
            extra.push((binding, buffer, *offset, *size));
        }
        // The stages run outside a pass.
        self.end_pass(d)?;
        self.settle_unbound(d)?;
        let mut vertex_sets = d.binding_sets(
            &self.graphics,
            pid,
            Some(bind::VERTEX),
            &extra,
            StageSet::COMPUTE,
        )?;
        if helpers.size != 0 {
            vertex_sets[HELPER_SET as usize] = Some(d.helper_set(
                &helpers,
                &Bases::Direct(bases),
                &fetch.inputs,
                StageSet::COMPUTE,
            )?);
        }
        let geometry_sets = d.binding_sets(
            &self.graphics,
            pid,
            Some(bind::GEOMETRY),
            &[],
            StageSet::COMPUTE,
        )?;
        let pixel_sets = d.binding_sets(
            &self.graphics,
            pid,
            Some(bind::PIXEL),
            &[],
            StageSet::FRAGMENT,
        )?;
        if d.aliased != 0 {
            self.note_draw(d, matches!(range, DrawRange::Indexed { .. }));
            d.alias_settle()?;
        }
        if let Some((heap, _, QueryState::Ended)) = self.query
            && self.pass_heap != Some(heap)
        {
            return Err(Error::NotImplemented("an occlusion query across passes"));
        }
        let index = match (&range, &self.index) {
            (DrawRange::Indexed { .. }, Some(v)) => {
                let (id, offset) = d
                    .addresses
                    .find(v.address)
                    .ok_or(Error::Invalid("an index buffer view of no buffer"))?;
                let format = match v.format {
                    57 => IndexFormat::U16,
                    42 => IndexFormat::U32,
                    _ => return Err(Error::Invalid("index format")),
                };
                let size = d.resources[id.0 as usize].desc.width.next_multiple_of(4);
                Some((d.buffer(id)?.0.clone(), size, offset, format))
            }
            (DrawRange::Indexed { .. }, None) => {
                return Err(Error::Invalid("an indexed draw with no index buffer"));
            }
            _ => None,
        };
        let bound_depth = self.check_targets(d, pid)?;
        let mut stages = d
            .render_mut(pid)
            .geometry
            .as_mut()
            .and_then(|g| g.stages.take())
            .ok_or(Error::Invalid("a geometry stage in use"))?;
        let ran = stages.run(
            &mut d.backend,
            &StageDraw {
                vertex_sets: vertex_sets.each_ref().map(Option::as_ref),
                geometry_sets: geometry_sets.each_ref().map(Option::as_ref),
                index: index
                    .as_ref()
                    .map(|(buffer, size, offset, format)| StageIndex {
                        buffer,
                        size: *size,
                        offset: *offset,
                        format: *format,
                    }),
                topology,
                range: same_range(&range),
                stream_output: None,
            },
        );
        if let Some(g) = d.render_mut(pid).geometry.as_mut() {
            g.stages = Some(stages);
        }
        let out = match ran {
            Ok(Some(out)) => out,
            // A draw of no primitive, or of primitives the geometry shader does not take, which
            // Direct3D does not draw either.
            Ok(None) | Err(StageError::TopologyMismatch) => return Ok(()),
            Err(e) => return Err(stage_error(e)),
        };
        if !self.pass_open {
            self.open_pass(d, pid)?;
        }
        self.draw_output(d, pid, &out, bound_depth, pixel_sets);
        Ok(())
    }

    /// The draw of the geometry stage's output, in the open pass.
    fn draw_output(
        &self,
        d: &mut Device<B>,
        pid: PipelineId,
        out: &FinalDraw<B>,
        bound_depth: Option<Format>,
        sets: [Option<B::BindingSet>; wcontainers_gpu::MAX_BINDING_SETS],
    ) {
        let p = &d.pipelines[pid.0 as usize];
        let r = p.render.as_ref().expect("a graphics pipeline");
        let g = r.geometry.as_ref().expect("a geometry stage");
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
        d.backend.draw(&DrawCall {
            program: &p.program,
            blend: &r.blend,
            depth_stencil: match (&r.depth_stencil_bound, bound_depth) {
                (Some(d), Some(_)) => d,
                _ => &r.depth_stencil,
            },
            raster: &r.raster,
            vertex_layout: &g.layout,
            bindings: sets.each_ref().map(Option::as_ref),
            vertex_buffers: &vertex_buffers,
            index: out.index_offset.map(|offset| IndexBinding {
                buffer: &out.buffer,
                offset,
                format: IndexFormat::U32,
            }),
            topology: out.topology,
            restart: out.index_offset.is_some(),
            // The helper refuses geometry shaders choosing a viewport.
            viewports: &self.viewports[..self.viewports.len().min(1)],
            scissors: &self.scissors[..self.scissors.len().min(1)],
            blend_constant: self.blend_constant,
            stencil_reference: [self.stencil_reference; 2],
            immediates: &[],
            range,
        });
    }
}
