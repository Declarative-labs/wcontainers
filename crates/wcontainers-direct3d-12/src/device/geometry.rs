//! Geometry shaders and stream output (plan section 7): the shaders crate's stage helper runs the
//! vertex stage, and the geometry stage where there is one, as compute work before the draw
//! (`GeometryAsCompute`), every vertex input fetched in the program, and writes the last stage's
//! output into the bound stream output buffers (`StreamOutAsCompute`). A pass-through vertex
//! program draws what a geometry stage made with the pipeline's pixel program; without one the
//! draw is drawn as usual after the stream output. Hull and domain shaders take the same route
//! (`TessellationAsCompute`): the vertex and hull stages and the tessellator run as compute work,
//! and the domain program draws the tessellator's points with the pixel program.

use wcontainers_gpu::{
    BufferRange, DOMAIN_PATCHES_BINDING, DOMAIN_POINTS_BINDING, SourceStage, StageExtras,
    VertexFormat, WriteMode,
};
use wcontainers_shaders::stage::{
    DomainSets, FinalDraw, StageDraw, StageError, StageIndex, Stages, StreamOutput, StreamTargets,
};

use super::*;
use pull::Bases;
use wcontainers_directx_graphics_infrastructure::{d3d, dxgi};

/// What a pipeline with a geometry shader, stream output or hull and domain shaders adds.
pub(super) struct Geometry<B: Backend> {
    /// The stages' compute programs and buffers; out while a draw runs them.
    pub stages: Option<Stages<B>>,
    /// The vertex stage's helper block and its pulled streams' bindings (binding, location).
    pub helpers: Helpers,
    pub streams: Vec<(u32, u32)>,
    /// The vertex layout of the draw after a geometry or domain stage; `None` without one.
    pub layout: Option<B::VertexLayout>,
    /// The pipeline streams its output out.
    pub streamed: bool,
    /// Stream 0 is drawn (`RasterizedStream`).
    pub rasterized: bool,
    /// The stage run between the vertex stage and the draw (geometry or hull): its
    /// `D3D12_SHADER_VISIBILITY` and helper block.
    pub middle: (u32, Helpers),
    /// After a hull stage, the domain program's helper block.
    pub domain: Option<Helpers>,
    /// Where a geometry shader or stream output follows the domain stage, which then runs as
    /// compute work too: the geometry shader's helper block when there is one.
    pub after_domain: Option<Option<Helpers>>,
    /// The last stage writes a layer (`SV_RenderTargetArrayIndex`) and a viewport
    /// (`SV_ViewportArrayIndex`): the draw after it is drawn once per layer of the bound views
    /// (`LayerSplit`) and per viewport (`MultiViewportSplit`).
    pub selects: Selects,
    /// The stage helper draws the vertex stage's lines alpha-antialiased
    /// (`AntialiasedLinesAsQuads`), for the first viewport's size.
    pub antialiased_lines: bool,
}

/// Whether a geometry stage writes a layer and a viewport, and the bytes of the pass-through
/// program's helper block naming the run (`stage::PASSTHROUGH_HELPERS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Selects {
    pub layer: bool,
    pub viewport: bool,
    pub block: u32,
}

impl Selects {
    fn of(r: &wcontainers_gpu::Reflection, passthrough: Option<&ShaderModule>) -> Self {
        let writes = |v| r.outputs.iter().any(|o| o.system == Some(v));
        Selects {
            layer: writes(wcontainers_gpu::SystemValue::RenderTargetArrayIndex),
            viewport: writes(wcontainers_gpu::SystemValue::ViewportArrayIndex),
            block: passthrough.map_or(0, run_block_size),
        }
    }
}

/// The size of the uniform block at the vertex stage's helper binding of `module`, as its
/// layout pads it; zero without one.
fn run_block_size(module: &ShaderModule) -> u32 {
    let m = &module.module;
    let binding = naga::ResourceBinding {
        group: HELPER_SET,
        binding: wcontainers_gpu::helper_binding(ShaderStage::Vertex),
    };
    m.global_variables
        .iter()
        .find(|(_, v)| v.space == naga::AddressSpace::Uniform && v.binding == Some(binding))
        .map_or(0, |(_, v)| m.types[v.ty].inner.size(m.to_ctx()))
}

/// A pipeline's stages as made: what draws need, the converted stages whose bindings the
/// pipeline binds, and the primitives the draw after them draws.
pub(super) struct Made<B: Backend> {
    pub geometry: Geometry<B>,
    pub vertex: Converted,
    /// The geometry or hull stage.
    pub shader: Option<Converted>,
    pub domain: Option<Converted>,
    /// A geometry shader after the domain stage.
    pub late: Option<Converted>,
    pub output: Option<Topology>,
    /// What the last stage passes the pixel program: clip distances the back end lacks
    /// (`ClipDistanceInShader`) and the layer a geometry shader names; default where nothing.
    pub linked: Link,
}

/// `D3D12_SHADER_VISIBILITY_HULL` and `_DOMAIN`.
pub(super) const HULL: u32 = 2;
pub(super) const DOMAIN: u32 = 3;

/// Whether converted vertex program `c` writes cull distances.
pub(super) fn writes_cull_distances(c: &Converted) -> bool {
    c.reflection
        .outputs
        .iter()
        .any(|v| v.system == Some(wcontainers_gpu::SystemValue::CullDistance))
}

impl<B: Backend> Device<B> {
    /// The last stage before the rasteriser, a geometry or domain shader `code`, converted with
    /// `key`, and what it passes the pixel program: where it writes clip distances the back end
    /// lacks, its pass-through passes them and the pixel program applies them
    /// (`ClipDistanceInShader`); where a geometry shader names a layer, its pass-through passes it,
    /// the pixel program reads it there, and a primitive naming a layer past the views is drawn
    /// into their first layer, as Direct3D draws it (`LAYER_FROM_STAGE`).
    fn last_stage_converted(
        &mut self,
        code: &[u8],
        stage: SourceStage,
        key: VariantKey,
        pixel_inputs: Option<&[StageValue]>,
    ) -> Result<(Converted, Link), Error> {
        let converted = self.convert_staged(code, stage, key.clone())?;
        let outputs = &converted.reflection.outputs;
        let clip_distances = match self.backend.caps().has(Features::CLIP_DISTANCE) {
            true => 0,
            false => super::clip_distances(outputs),
        };
        let mut flags = VariantFlags::empty();
        if clip_distances != 0 {
            pixel_inputs.ok_or(Error::NotImplemented(super::CLIP_WITHOUT_PIXEL_SHADER))?;
            flags |= VariantFlags::CLIP_DISTANCE;
        }
        if stage == SourceStage::Geometry
            && outputs
                .iter()
                .any(|o| o.system == Some(SystemValue::RenderTargetArrayIndex))
        {
            flags |= VariantFlags::LAYER_FROM_STAGE;
        }
        if flags.is_empty() {
            return Ok((converted, Link::default()));
        }
        let link = Link {
            flags,
            values: LinkedValues {
                location: super::linked_location(
                    outputs.iter().chain(pixel_inputs.unwrap_or_default()),
                ),
                clip_distances,
            },
        };
        let mut key = key;
        link.apply(&mut key);
        let converted = self.convert_staged(code, stage, key)?;
        Ok((converted, link))
    }

    /// The stages of a pipeline with a geometry shader `gs` (or none, empty), stream output `so`,
    /// or hull and domain shaders `hs` and `ds` (or none): `vs` converted with every input of
    /// `inputs` fetched in the program, the last stage before the pixel stage writing its
    /// outputs as that interpolates them, and the stage helper made from them; with `lines`, the
    /// pixel program's key, `vs` alone drawing its lines alpha-antialiased.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn geometry_stage(
        &mut self,
        vs: &[u8],
        hs: &[u8],
        ds: &[u8],
        gs: &[u8],
        inputs: &[(InputElement, u32)],
        interpolation: &[(u32, Interpolation)],
        so: Option<&(StreamOutput, u32)>,
        pixel_inputs: Option<&[StageValue]>,
        lines: Option<&VariantKey>,
        root: &RootSignatureDesc,
    ) -> Result<Made<B>, Error> {
        if !self.backend.caps().has(Features::COMPUTE) {
            return Err(Error::NotImplemented(
                "geometry shaders, stream output and tessellation on this back end",
            ));
        }
        let tessellated = !hs.is_empty();
        let rasterized = match so.map(|s| s.1) {
            None | Some(0) => true,
            Some(d3d::SO_NO_RASTERIZED_STREAM) => false,
            Some(_) => return Err(Error::NotImplemented("drawing a stream other than 0")),
        };
        let pulled: Vec<(u32, VertexFormat)> = inputs
            .iter()
            .map(|(e, location)| Ok((*location, vertex_format(e.format)?.0)))
            .collect::<Result<_, Error>>()?;
        let direct = gs.is_empty() && !tessellated;
        let vertex = self.convert(
            vs,
            ShaderStage::Vertex,
            VariantKey {
                pulled,
                draw_base: DrawBaseSource::Helpers,
                // A vertex stage the rasteriser follows writes its outputs as the pixel stage
                // interpolates them.
                output_interpolation: match direct {
                    true => interpolation.to_vec(),
                    false => Vec::new(),
                },
                ..formats::typed_key(vs, self.texel_route(), root, bind::VERTEX)
            },
        )?;
        let interpolated = VariantKey {
            output_interpolation: interpolation.to_vec(),
            ..VariantKey::default()
        };
        let mut linked = Link::default();
        let (shader, domain, late) = match (gs.is_empty(), tessellated) {
            (false, false) => {
                let (gs, l) = self.last_stage_converted(
                    gs,
                    SourceStage::Geometry,
                    interpolated,
                    pixel_inputs,
                )?;
                linked = l;
                (Some(gs), None, None)
            }
            (_, true) => {
                let hull = self.convert_staged(
                    hs,
                    SourceStage::Hull,
                    formats::typed_key(hs, self.texel_route(), root, HULL),
                )?;
                // A domain stage the rasteriser follows writes its outputs as the pixel stage
                // interpolates them.
                let domain_key = match gs.is_empty() {
                    true => VariantKey {
                        output_interpolation: interpolation.to_vec(),
                        ..formats::typed_key(ds, self.texel_route(), root, DOMAIN)
                    },
                    false => formats::typed_key(ds, self.texel_route(), root, DOMAIN),
                };
                let domain = match gs.is_empty() {
                    true => {
                        let (domain, l) = self.last_stage_converted(
                            ds,
                            SourceStage::Domain,
                            domain_key,
                            pixel_inputs,
                        )?;
                        linked = l;
                        domain
                    }
                    false => self.convert_staged(ds, SourceStage::Domain, domain_key)?,
                };
                let late = match gs.is_empty() {
                    true => None,
                    false => {
                        let (gs, l) = self.last_stage_converted(
                            gs,
                            SourceStage::Geometry,
                            interpolated,
                            pixel_inputs,
                        )?;
                        linked = l;
                        Some(gs)
                    }
                };
                (Some(hull), Some(domain), late)
            }
            (true, false) => (None, None, None),
        };
        let last = late.as_ref().or(shader.as_ref());
        let output = match last.map(|s| &s.reflection.extras) {
            // The line route draws rectangles of two triangles.
            None if lines.is_some() => Some(Topology::TriangleList),
            None => None,
            Some(StageExtras::Geometry {
                output_vertices, ..
            }) => Some(match output_vertices {
                1 => Topology::PointList,
                2 => Topology::LineStrip,
                _ => Topology::TriangleStrip,
            }),
            Some(StageExtras::Hull {
                output_primitive, ..
            }) => Some(match output_primitive {
                0 => Topology::PointList,
                1 => Topology::LineList,
                _ => Topology::TriangleList,
            }),
            Some(_) => return Err(Error::Invalid("a shader stage without its declarations")),
        };
        let lines = lines.filter(|_| direct);
        let stages = match (&shader, &domain, lines) {
            (_, _, Some(pixel)) => Stages::antialiased_lines(&mut self.backend, &vertex, pixel),
            (Some(hull), Some(domain), None) => Stages::tessellated_with(
                &mut self.backend,
                &vertex,
                hull,
                domain,
                late.as_ref(),
                so.map(|s| &s.0),
            ),
            _ if direct && writes_cull_distances(&vertex) => {
                Stages::culled(&mut self.backend, &vertex, so.map(|s| &s.0))
            }
            _ => Stages::new(
                &mut self.backend,
                &vertex,
                shader.as_ref(),
                so.map(|s| &s.0),
            ),
        }
        .map_err(|e| {
            self.messages.push(format!(
                "the stage helper refused a pipeline's stages: {e:?}"
            ));
            stage_error(e)
        })?;
        let selects = last
            .map(|s| Selects::of(&s.reflection, stages.passthrough()))
            .unwrap_or_default();
        if stages.selects() != (selects.layer || selects.viewport) {
            stages.destroy(&mut self.backend);
            return Err(Error::Invalid(
                "the stage helper and the geometry shader disagree on its layer and viewport",
            ));
        }
        let layout = match (stages.vertex_layout(), tessellated) {
            (Some(l), _) => Some(self.layout(l)?),
            // The domain program reads its points from the helper's buffers.
            (None, true) => Some(self.layout(VertexLayoutDesc {
                streams: [None; MAX_VERTEX_STREAMS],
                attributes: [None; MAX_VERTEX_STREAMS],
            })?),
            (None, false) => None,
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
                middle: (
                    match tessellated {
                        true => HULL,
                        false => bind::GEOMETRY,
                    },
                    shader.as_ref().map(Helpers::of).unwrap_or_default(),
                ),
                domain: domain.as_ref().map(Helpers::of),
                after_domain: (tessellated && (late.is_some() || so.is_some()))
                    .then(|| late.as_ref().map(Helpers::of)),
                selects,
                antialiased_lines: lines.is_some(),
            },
            vertex,
            shader,
            domain,
            late,
            output,
            linked,
        })
    }
}

/// A stage helper refusal as the front reports it.
fn stage_error(e: StageError) -> Error {
    match e {
        StageError::Gpu(e) => e.into(),
        StageError::Unsupported(what) => Error::NotImplemented(what),
        StageError::Convert(ConvertError::Unsupported(_) | ConvertError::Reader(_)) => {
            Error::NotImplemented("a shader feature the stage helper lacks")
        }
        StageError::Convert(_) => {
            Error::NotImplemented("a geometry, hull or domain shader the converter got wrong")
        }
        StageError::TopologyMismatch => {
            Error::Invalid("a topology that does not give the geometry or hull shader's input")
        }
        StageError::Declaration => Error::Invalid("a stream output or tessellation declaration"),
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
    /// A draw of a pipeline with a geometry, hull or domain shader or stream output: the stage
    /// helper's dispatches outside the pass, then the draw of its output in it. Answers
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
                "a predicated draw through a geometry, hull or domain shader or stream output",
            ));
        }
        if self.counted.is_some() || self.statistics.is_some() {
            return Err(Error::NotImplemented(
                "a draw through a geometry, hull or domain shader or stream output while a counted \
                 occlusion or statistics query runs",
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
                    "an indirect draw through a geometry, hull or domain shader or stream output",
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
        let (helpers, has_geometry, streamed, rasterized, antialiased_lines) = (
            g.helpers.clone(),
            g.layout.is_some(),
            g.streamed,
            g.rasterized,
            g.antialiased_lines,
        );
        let (middle, middle_helpers) = g.middle.clone();
        let domain = g.domain.clone();
        let after_domain = g.after_domain.clone();
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
        let mut vertex_sets = d
            .binding_sets(
                &self.graphics,
                pid,
                &[bind::VERTEX],
                &extra,
                vertex_stages,
                None,
            )?
            .0;
        d.vertex_formats(pid)?;
        if helpers.size != 0 {
            vertex_sets[HELPER_SET as usize] = Some(d.helper_set(
                &helpers,
                &Bases::Direct(bases),
                &fetch.inputs,
                StageSet::COMPUTE,
                None,
                Vec::new(),
            )?);
        }
        let mut geometry_sets = d
            .binding_sets(&self.graphics, pid, &[middle], &[], StageSet::COMPUTE, None)?
            .0;
        if middle_helpers.size != 0 {
            let block = d.stage_block(&middle_helpers, middle)?;
            let compute = wcontainers_gpu::helper_binding(ShaderStage::Compute);
            geometry_sets[HELPER_SET as usize] = Some(d.helper_blocks(
                &[(compute, StageSet::COMPUTE, block)],
                &[],
                &[],
                Vec::new(),
            )?);
        }
        // The sets of the domain stage run as compute and of a geometry stage after it.
        let compute = wcontainers_gpu::helper_binding(ShaderStage::Compute);
        let later_sets = match &after_domain {
            Some(late) => {
                let mut domain_sets = d
                    .binding_sets(&self.graphics, pid, &[DOMAIN], &[], StageSet::COMPUTE, None)?
                    .0;
                if let Some(helpers) = domain.as_ref().filter(|h| h.size != 0) {
                    // The compute form keeps the drawn program's helper binding.
                    let block = d.stage_block(helpers, DOMAIN)?;
                    let vertex = wcontainers_gpu::helper_binding(ShaderStage::Vertex);
                    domain_sets[HELPER_SET as usize] = Some(d.helper_blocks(
                        &[(vertex, StageSet::COMPUTE, block)],
                        &[],
                        &[],
                        Vec::new(),
                    )?);
                }
                let mut late_sets = Default::default();
                if let Some(helpers) = late {
                    late_sets = d
                        .binding_sets(
                            &self.graphics,
                            pid,
                            &[bind::GEOMETRY],
                            &[],
                            StageSet::COMPUTE,
                            None,
                        )?
                        .0;
                    if helpers.size != 0 {
                        let block = d.stage_block(helpers, bind::GEOMETRY)?;
                        late_sets[HELPER_SET as usize] = Some(d.helper_blocks(
                            &[(compute, StageSet::COMPUTE, block)],
                            &[],
                            &[],
                            Vec::new(),
                        )?);
                    }
                }
                Some((domain_sets, late_sets))
            }
            None => None,
        };
        // The domain program draws unless a geometry stage follows it.
        let drawn_domain = domain.filter(|_| !matches!(after_domain, Some(Some(_))));
        // The draw's sets but its helper set, and the helper blocks that set holds: after a
        // hull stage the domain stage's too, and the stage helper's patches and points beside
        // them once it ran.
        let pixel_sets = match has_geometry && rasterized {
            true => {
                let only: &[u32] = match drawn_domain {
                    Some(_) => &[DOMAIN, bind::PIXEL],
                    None => &[bind::PIXEL],
                };
                // The program after the stage helper is made with the pixel program's first
                // typed view formats only.
                let (sets, _) = d.binding_sets(
                    &self.graphics,
                    pid,
                    only,
                    &[],
                    StageSet::FRAGMENT,
                    Some(super::formats::VertexKind::Staged),
                )?;
                if !d.through.is_empty() {
                    return Err(Error::NotImplemented(
                        "storage textures read through buffers behind a geometry or domain shader",
                    ));
                }
                let mut blocks = Vec::new();
                if let Some(helpers) = drawn_domain.as_ref().filter(|h| h.size != 0) {
                    let block = d.stage_block(helpers, DOMAIN)?;
                    let vertex = wcontainers_gpu::helper_binding(ShaderStage::Vertex);
                    blocks.push((vertex, StageSet::VERTEX, block));
                }
                let helpers = d.pipelines[pid.0 as usize].helpers.clone();
                if helpers.size != 0 {
                    let block = d.stage_block(&helpers, bind::PIXEL)?;
                    let fragment = wcontainers_gpu::helper_binding(ShaderStage::Fragment);
                    blocks.push((fragment, StageSet::FRAGMENT, block));
                }
                Some((sets, blocks))
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
                    dxgi::R16_UINT => IndexFormat::U16,
                    dxgi::R32_UINT => IndexFormat::U32,
                    _ => return Err(Error::Invalid("index format")),
                };
                let size = d.resources[id.0 as usize]
                    .desc
                    .width
                    .next_multiple_of(STORAGE_WORD);
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
        let draw = StageDraw {
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
        };
        let ran = match &later_sets {
            Some((domain_sets, late_sets)) => stages.run_after_domain(
                &mut d.backend,
                &draw,
                &DomainSets {
                    domain_sets: domain_sets.each_ref().map(Option::as_ref),
                    geometry_sets: late_sets.each_ref().map(Option::as_ref),
                },
            ),
            None if antialiased_lines => match self.viewports.first() {
                Some(v) => stages.run_antialiased_lines(
                    &mut d.backend,
                    &draw,
                    [f32::from_bits(v.width), f32::from_bits(v.height)],
                ),
                // With no viewport nothing is rasterised.
                None => Ok(None),
            },
            None => stages.run(&mut d.backend, &draw),
        };
        let tessellated = match d.render_mut(pid).geometry.as_mut() {
            Some(g) => {
                g.stages = Some(stages);
                g.domain.is_some()
            }
            None => false,
        };
        let out = match ran {
            Ok(out) => out,
            // Real Direct3D 12 draws patches of another control point count than the hull
            // shader takes; the stage helper does not.
            Err(StageError::TopologyMismatch) if tessellated => {
                return Err(Error::NotImplemented(
                    "patches of another control point count than the hull shader takes",
                ));
            }
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
        let (Some(out), Some((mut pixel_sets, blocks))) = (out, pixel_sets) else {
            return Ok(false);
        };
        let reads: Vec<_> = out
            .tessellated
            .iter()
            .flat_map(|t| {
                [
                    (DOMAIN_PATCHES_BINDING, &t.patches),
                    (DOMAIN_POINTS_BINDING, &t.points),
                ]
            })
            .map(|(binding, r)| {
                (
                    binding,
                    StageSet::VERTEX,
                    BufferRange {
                        buffer: &r.buffer,
                        offset: r.offset,
                        size: r.size,
                    },
                )
            })
            .collect();
        let selects = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .and_then(|r| r.geometry.as_ref())
            .map_or(Selects::default(), |g| g.selects);
        if selects.layer || selects.viewport {
            self.draw_runs(d, pid, &out, bound_depth, pixel_sets, &reads, selects)?;
            return Ok(false);
        }
        if !blocks.is_empty() || !reads.is_empty() {
            pixel_sets[HELPER_SET as usize] =
                Some(d.helper_blocks(&blocks, &reads, &[], Vec::new())?);
        }
        if !self.pass_open {
            self.open_pass(d, pid)?;
        }
        self.draw_output(d, pid, &out, bound_depth, &pixel_sets, 0);
        Ok(false)
    }

    /// The draw after a geometry stage writing a layer or a viewport, once per layer of the
    /// bound target views (`LayerSplit`) and per viewport (`MultiViewportSplit`): each run draws
    /// into that layer's views through that viewport, its pass-through program keeping the
    /// primitives that name the run (`stage::PASSTHROUGH_HELPERS`) and its pixel program reading
    /// the layer and viewport. A primitive naming a layer past the views' is drawn in the first
    /// layer's run, as Direct3D draws it; one naming a viewport past those bound is not drawn.
    #[allow(clippy::too_many_arguments)]
    fn draw_runs(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
        out: &FinalDraw<B>,
        bound_depth: Option<Format>,
        mut sets: [Option<B::BindingSet>; wcontainers_gpu::MAX_BINDING_SETS],
        reads: &[(u32, StageSet, BufferRange<&B::Buffer>)],
        selects: Selects,
    ) -> Result<(), Error> {
        // Each run is a pass of its own, which a query would have to span.
        if self.query.is_some() || self.counted.is_some() || self.statistics.is_some() {
            return Err(Error::NotImplemented(
                "a draw split per layer or viewport while a query runs",
            ));
        }
        let layers = match selects.layer {
            true => self.target_layers(d)?,
            false => 1,
        };
        let viewports = match selects.viewport {
            true => self.viewports.len().max(1) as u32,
            false => 1,
        };
        if selects.layer {
            d.backend.slow_paths().hit(SlowPath::LayerSplit);
        }
        if selects.viewport {
            d.backend.slow_paths().hit(SlowPath::MultiViewportSplit);
        }
        // Clears waiting for the views are of every layer; each run's pass loads its own.
        self.settle(d)?;
        let pixel = d.pipelines[pid.0 as usize].helpers.clone();
        for layer in 0..layers {
            for viewport in 0..viewports {
                let run = [layer, viewport];
                let mut blocks = vec![(
                    wcontainers_gpu::helper_binding(ShaderStage::Vertex),
                    StageSet::VERTEX,
                    passthrough_block(run, layers, selects),
                )];
                if pixel.size != 0 {
                    blocks.push((
                        wcontainers_gpu::helper_binding(ShaderStage::Fragment),
                        StageSet::FRAGMENT,
                        d.run_block(&pixel, bind::PIXEL, run)?,
                    ));
                }
                sets[HELPER_SET as usize] =
                    Some(d.helper_blocks(&blocks, reads, &[], Vec::new())?);
                self.end_pass(d)?;
                self.pass_layer = layer;
                let opened = self.open_pass(d, pid);
                self.pass_layer = 0;
                opened?;
                self.draw_output(d, pid, out, bound_depth, &sets, viewport as usize);
            }
        }
        // The next draw opens a pass on the views' first layer again.
        self.end_pass(d)
    }

    /// The layers of the bound target views, which a layered draw's runs cover.
    fn target_layers(&self, d: &Device<B>) -> Result<u32, Error> {
        let views = self.colors.iter().filter_map(|c| match c {
            Descriptor::RenderTarget(v) => Some(v),
            _ => None,
        });
        let depth = match &self.depth {
            Some(Descriptor::DepthStencil { view, .. }) => Some(view),
            _ => None,
        };
        let mut layers = None;
        for v in views.chain(depth) {
            let three_d = matches!(
                d.resources.get(v.resource.0 as usize).map(|r| &r.storage),
                Some(Storage::Texture { desc, .. }) if desc.dimension == TextureDimension::D3
            );
            if three_d {
                return Err(Error::NotImplemented(
                    "a draw choosing the slice of a 3D texture's target view",
                ));
            }
            if layers.is_some_and(|l| l != v.layers) {
                return Err(Error::NotImplemented(
                    "a draw choosing a layer of target views of different layer counts",
                ));
            }
            layers = Some(v.layers);
        }
        Ok(layers.unwrap_or(1).max(1))
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
                q.3.primitives_written += u64::from(written);
                q.3.primitives_storage_needed += u64::from(needed);
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
        sets: &[Option<B::BindingSet>; wcontainers_gpu::MAX_BINDING_SETS],
        viewport: usize,
    ) {
        let p = &d.pipelines[pid.0 as usize];
        let r = p.render.as_ref().expect("a graphics pipeline");
        let g = r.geometry.as_ref().expect("a geometry stage");
        let vertex_buffers = [Some(VertexBinding {
            buffer: &out.buffer,
            offset: 0,
        })];
        // After a tessellation stage the domain program reads no stream.
        let vertex_buffers = match out.tessellated {
            Some(_) => &[][..],
            None => &vertex_buffers[..],
        };
        let range = match (&out.tessellated, out.index_offset) {
            (Some(t), _) => DrawRange::IndexedIndirect {
                args: &t.arguments,
                offset: 0,
            },
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
            vertex_buffers,
            index: out.index_offset.map(|offset| IndexBinding {
                buffer: &out.buffer,
                offset,
                format: IndexFormat::U32,
            }),
            topology: out.topology,
            restart: out.index_offset.is_some() && out.tessellated.is_none(),
            // A run of a draw split per viewport draws through that viewport and its scissor.
            viewports: self
                .viewports
                .get(viewport..viewport + 1)
                .unwrap_or(&self.viewports[..self.viewports.len().min(1)]),
            scissors: self
                .scissors
                .get(viewport..viewport + 1)
                .unwrap_or(&self.scissors[..self.scissors.len().min(1)]),
            blend_constant: self.blend_constant,
            stencil_reference: [self.stencil_reference; 2],
            immediates: &[],
            range,
        });
    }
}

/// The pass-through program's helper block for the run `run` (layer, viewport) of a draw split
/// per layer or viewport into views of `layers` layers; a pass-through choosing layers also reads
/// their count (`LAYER_FROM_STAGE`).
fn passthrough_block(run: [u32; 2], layers: u32, selects: Selects) -> Vec<u8> {
    use wcontainers_shaders::stage::{PASSTHROUGH_HELPERS, PASSTHROUGH_LAYER_COUNT};
    let word = size_of::<u32>();
    let mut block = vec![0u8; selects.block as usize];
    let count = selects.layer.then_some(&PASSTHROUGH_LAYER_COUNT);
    for slot in PASSTHROUGH_HELPERS.iter().chain(count) {
        let value = match slot.value {
            HelperValue::Layer => run[0],
            HelperValue::Viewport => run[1],
            HelperValue::LayerCount => layers,
            other => unreachable!("the pass-through reads its run alone, not {other:?}"),
        };
        let at = slot.offset as usize;
        block[at..at + word].copy_from_slice(&value.to_le_bytes());
    }
    block
}
