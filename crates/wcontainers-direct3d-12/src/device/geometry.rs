//! Geometry shaders and stream output (plan section 7): the shaders crate's stage helper runs the
//! vertex stage, and the geometry stage where there is one, as compute work before the draw
//! (`GeometryAsCompute`), every vertex input fetched in the program, and writes the last stage's
//! output into the bound stream output buffers (`StreamOutAsCompute`). A pass-through vertex
//! program draws what a geometry stage made with the pipeline's pixel program; without one the
//! draw is drawn as usual after the stream output.

use wcontainers_gpu::{BufferRange, StageExtras, VertexFormat, WriteMode};
use wcontainers_shaders::stage::{
    FinalDraw, StageDraw, StageError, StageIndex, Stages, StreamOutput, StreamTargets,
};

use super::*;
use pull::Bases;

/// What a pipeline with a geometry shader or stream output adds.
pub(super) struct Geometry<B: Backend> {
    /// The stages' compute programs and buffers; out while a draw runs them.
    pub stages: Option<Stages<B>>,
    /// The vertex stage's helper block and its pulled streams' bindings (binding, location).
    pub helpers: Helpers,
    pub streams: Vec<(u32, u32)>,
    /// The vertex layout of the draw after a geometry stage; `None` without one.
    pub layout: Option<B::VertexLayout>,
    /// The pipeline streams its output out.
    pub streamed: bool,
    /// Stream 0 is drawn (`RasterizedStream`).
    pub rasterized: bool,
}

/// A pipeline's stages as made: what draws need, the converted stages whose bindings the
/// pipeline binds, and the primitives a geometry stage draws.
pub(super) struct Made<B: Backend> {
    pub geometry: Geometry<B>,
    pub vertex: Converted,
    pub shader: Option<Converted>,
    pub output: Option<Topology>,
}

/// `D3D12_SO_NO_RASTERIZED_STREAM`.
pub const NO_RASTERIZED_STREAM: u32 = u32::MAX;

impl<B: Backend> Device<B> {
    /// The stages of a pipeline with a geometry shader `gs` (or none, empty) or stream output
    /// `so`: `vs` converted with every input of `inputs` fetched in the program, `gs` writing
    /// its outputs as the pixel stage interpolates them, and the stage helper made from both.
    pub(super) fn geometry_stage(
        &mut self,
        vs: &[u8],
        gs: &[u8],
        inputs: &[(InputElement, u32)],
        interpolation: &[(u32, Interpolation)],
        so: Option<&(StreamOutput, u32)>,
    ) -> Result<Made<B>, Error> {
        if !self.backend.caps().has(Features::COMPUTE) {
            return Err(Error::NotImplemented(
                "geometry shaders and stream output on this back end",
            ));
        }
        let rasterized = match so.map(|s| s.1) {
            None | Some(0) => true,
            Some(NO_RASTERIZED_STREAM) => false,
            Some(_) => return Err(Error::NotImplemented("drawing a stream other than 0")),
        };
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
        let shader = match gs.is_empty() {
            true => None,
            false => Some(self.convert_geometry(
                gs,
                VariantKey {
                    output_interpolation: interpolation.to_vec(),
                    ..VariantKey::default()
                },
            )?),
        };
        let output = match shader.as_ref().map(|s| &s.reflection.extras) {
            None => None,
            Some(StageExtras::Geometry {
                output_vertices, ..
            }) => Some(match output_vertices {
                1 => Topology::PointList,
                2 => Topology::LineStrip,
                _ => Topology::TriangleStrip,
            }),
            Some(_) => return Err(Error::Invalid("a geometry shader without its declarations")),
        };
        let stages = Stages::new(
            &mut self.backend,
            &vertex,
            shader.as_ref(),
            so.map(|s| &s.0),
        )
        .map_err(stage_error)?;
        let layout = match stages.vertex_layout() {
            Some(l) => Some(self.layout(l)?),
            None => None,
        };
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
                streamed: so.is_some(),
                rasterized,
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

/// The bytes of the stage helper's filled sizes and primitive counts.
const FILLED_BYTES: u64 = 32;

/// A draw's stream output targets: each bound buffer (from its offset, with its size), the
/// helper's filled sizes, and where each buffer's filled size lies (resource and offset).
struct Targets<B: Backend> {
    buffers: [Option<(B::Buffer, u64, u64)>; 4],
    filled: B::Buffer,
    sizes: [Option<(ResourceId, u64)>; 4],
}

impl<B: Backend> Replay<B> {
    /// A draw of a pipeline with a geometry shader or stream output: the stage helper's
    /// dispatches outside the pass, then the draw of a geometry stage's output in it. Answers
    /// whether the draw is still to be drawn as usual (stream output without a geometry stage,
    /// stream 0 drawn).
    pub(super) fn draw_geometry(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
        range: DrawRange<'_, B>,
        record: Option<&(B::Buffer, u64)>,
    ) -> Result<bool, Error> {
        if record.is_some() {
            return Err(Error::NotImplemented(
                "a predicated draw through a geometry shader or stream output",
            ));
        }
        if self.counted.is_some() || self.statistics.is_some() {
            return Err(Error::NotImplemented(
                "a draw through a geometry shader or stream output while a counted occlusion or \
                 statistics query runs",
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
                    "an indirect draw through a geometry shader or stream output",
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
        let (helpers, has_geometry, streamed, rasterized) = (
            g.helpers.clone(),
            g.layout.is_some(),
            g.streamed,
            g.rasterized,
        );
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
        let vertex_stages = match has_geometry {
            true => StageSet::COMPUTE,
            // The draw after the stream output binds the same sets.
            false => StageSet::COMPUTE | StageSet::VERTEX,
        };
        let mut vertex_sets = d.binding_sets(
            &self.graphics,
            pid,
            Some(bind::VERTEX),
            &extra,
            vertex_stages,
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
        let pixel_sets = match has_geometry && rasterized {
            true => {
                let sets = d.binding_sets(
                    &self.graphics,
                    pid,
                    Some(bind::PIXEL),
                    &[],
                    StageSet::FRAGMENT,
                )?;
                // The pass-through program is made with the pixel program's first storage
                // formats only.
                d.bound_program(pid, true)?;
                Some(sets)
            }
            false => None,
        };
        let targets = match streamed {
            true => self.stream_targets(d)?,
            false => None,
        };
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
        let bound_depth = match pixel_sets {
            Some(_) => self.check_targets(d, pid)?,
            None => None,
        };
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
                stream_output: targets.as_ref().map(|t| StreamTargets {
                    buffers: t.buffers.each_ref().map(|b| {
                        b.as_ref().map(|(buffer, offset, size)| BufferRange {
                            buffer,
                            offset: *offset,
                            size: *size,
                        })
                    }),
                    filled: BufferRange {
                        buffer: &t.filled,
                        offset: 0,
                        size: FILLED_BYTES,
                    },
                }),
            },
        );
        if let Some(g) = d.render_mut(pid).geometry.as_mut() {
            g.stages = Some(stages);
        }
        let out = match ran {
            Ok(out) => out,
            // A draw of primitives the geometry shader does not take, which Direct3D does not
            // draw either.
            Err(StageError::TopologyMismatch) => None,
            Err(e) => return Err(stage_error(e)),
        };
        if let Some(t) = targets {
            self.stream_written(d, t)?;
        }
        if !has_geometry {
            return Ok(rasterized);
        }
        // A draw of no primitive, or with stream 0 not drawn.
        let (Some(out), Some(pixel_sets)) = (out, pixel_sets) else {
            return Ok(false);
        };
        if !self.pass_open {
            self.open_pass(d, pid)?;
        }
        self.draw_output(d, pid, &out, bound_depth, pixel_sets);
        Ok(false)
    }

    /// The stream output targets of a draw: the bound buffers and a scratch buffer holding their
    /// filled sizes, copied from where the program keeps them, and the primitive counts.
    fn stream_targets(&mut self, d: &mut Device<B>) -> Result<Option<Targets<B>>, Error> {
        if self.stream_views.iter().all(|v| v.address == 0) {
            return Ok(None);
        }
        let filled = d.scratch(
            FILLED_BYTES,
            BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        )?;
        self.scratch.push(filled.clone());
        d.backend
            .write_buffer(&filled, 0, &[0; FILLED_BYTES as usize], WriteMode::Ordered);
        let align = u64::from(d.backend.caps().limits.min_storage_offset_alignment);
        let mut buffers: [Option<(B::Buffer, u64, u64)>; 4] = Default::default();
        let mut sizes = [None; 4];
        for (slot, v) in self.stream_views.iter().enumerate() {
            if v.address == 0 {
                continue;
            }
            let (id, offset) = d
                .addresses
                .find(v.address)
                .ok_or(Error::Invalid("a stream output buffer view of no buffer"))?;
            if !offset.is_multiple_of(align) {
                return Err(Error::NotImplemented(
                    "a stream output buffer at an offset the back end cannot bind",
                ));
            }
            let (fid, foff) = d
                .addresses
                .find(v.filled)
                .ok_or(Error::Invalid("a stream output filled size in no buffer"))?;
            d.upload(id)?;
            d.upload(fid)?;
            d.note(id, true);
            d.note(fid, true);
            let size = v
                .size
                .min(d.resources[id.0 as usize].desc.width.saturating_sub(offset));
            let fbuf = d.buffer(fid)?.0.clone();
            d.backend
                .copy_buffer(&fbuf, foff, &filled, 4 * slot as u64, 4);
            buffers[slot] = Some((d.buffer(id)?.0.clone(), offset, size));
            sizes[slot] = Some((fid, foff));
        }
        Ok(Some(Targets {
            buffers,
            filled,
            sizes,
        }))
    }

    /// After a draw's stream output: the filled sizes copied back to where the program keeps
    /// them, and the primitives written and needed added to the running stream 0 queries
    /// (read back, `ReadBackWait`).
    fn stream_written(&mut self, d: &mut Device<B>, t: Targets<B>) -> Result<(), Error> {
        for (slot, place) in t.sizes.iter().enumerate() {
            if let Some((fid, foff)) = *place {
                let fbuf = d.buffer(fid)?.0.clone();
                d.backend
                    .copy_buffer(&t.filled, 4 * slot as u64, &fbuf, foff, 4);
                if d.resources[fid.0 as usize].heap.reads_back() {
                    d.read_into(fid, foff, 4)?;
                }
            }
        }
        if self.streams.iter().any(|q| q.2 == 0) {
            let scratch = d.scratch(8, BufferUsage::COPY_DST | BufferUsage::READ_BACK)?;
            d.backend.copy_buffer(&t.filled, 16, &scratch, 0, 8);
            let mut bytes = [0u8; 8];
            let read = d.backend.read_back_now(
                &ReadSource::Buffer {
                    buffer: &scratch,
                    offset: 0,
                    size: 8,
                },
                &mut bytes,
            );
            d.backend.destroy(Object::Buffer(scratch));
            read?;
            let written = u32::from_le_bytes(bytes[..4].try_into().unwrap());
            let needed = u32::from_le_bytes(bytes[4..].try_into().unwrap());
            for q in self.streams.iter_mut().filter(|q| q.2 == 0) {
                q.3[0] += u64::from(written);
                q.3[1] += u64::from(needed);
            }
        }
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
            vertex_layout: g.layout.as_ref().expect("a geometry stage's layout"),
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
