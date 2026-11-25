//! A pass as it is recorded: its attachments, its draws as Metal's render commands, and the functions and pipelines those
//! draws build.

use wcontainers_gpu::{
    AspectOps, DrawCall, DrawRange, EmptyPass, FORMAT_FACTS, Format, FormatKind, Gpu, IndexFormat,
    LoadOp, MAX_COLOR_TARGETS, PassDesc, ScissorRect, ShaderStage, StepMode, StoreOp, TargetLayout,
    TextureRegion, TextureViewDesc, Topology, ViewDimension,
};
use wcontainers_metal::{
    Blending, ColorWriteMask, DepthClipMode, FunctionSource, IndexType, LoadAction,
    MAX_COLOR_ATTACHMENTS, Metal, PrimitiveTopologyClass, PrimitiveType, RenderCommand,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipelineColorAttachmentDescriptor, RenderPipelineDescriptor, ResolveTarget, ResourceSlot,
    StoreAction, TriangleFillMode, VertexAttribute, VertexBufferLayout, VertexStepFunction,
    VisibilityResultMode,
};

use crate::objects::{QueryObj, SetResource, ShaderObj, Shared, SlotKind, ViewObj};
use crate::{
    FIRST_STREAM_SLOT, IMMEDIATES_SLOT, MetalGpu, ProgramObj, QUERY_BYTES, SIZES_SLOT, convert,
};

/// What keys a render pipeline: the program, vertex layout and blend blocks, the targets, and the primitive class.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineKey {
    program: u64,
    vertex_layout: u64,
    blend: u64,
    targets: TargetLayout,
    class: u8,
}

/// One color attachment of an open pass.
struct OpenColor<M: Metal> {
    view: Shared<ViewObj<M>>,
    depth_slice: u32,
    resolve: Option<Shared<ViewObj<M>>>,
    load: LoadOp<[u32; 4]>,
    store: StoreOp,
}

/// The depth and stencil attachment of an open pass.
struct OpenDepth<M: Metal> {
    view: Shared<ViewObj<M>>,
    depth: Option<AspectOps<u32>>,
    stencil: Option<AspectOps<u8>>,
}

/// A pass begun and not yet ended: its attachments and the commands of its draws so far.
pub(crate) struct OpenPass<M: Metal> {
    colors: Vec<Option<OpenColor<M>>>,
    depth: Option<OpenDepth<M>>,
    empty: Option<EmptyPass>,
    occlusion: Option<Shared<QueryObj<M>>>,
    commands: Vec<RenderCommand<M>>,
    targets: TargetLayout,
    width: u32,
    height: u32,
}

impl<M: Metal> OpenPass<M> {
    /// Opens a pass as the interface describes it.
    pub(crate) fn new(pass: &PassDesc<'_, MetalGpu<M>>) -> OpenPass<M> {
        let colors: Vec<Option<OpenColor<M>>> = pass
            .colors
            .iter()
            .map(|c| {
                c.as_ref().map(|c| OpenColor {
                    view: c.view.clone(),
                    depth_slice: c.depth_slice,
                    resolve: c.resolve.cloned(),
                    load: c.load,
                    store: c.store,
                })
            })
            .collect();
        let depth = pass.depth.as_ref().map(|d| OpenDepth {
            view: d.view.clone(),
            depth: d.depth,
            stencil: d.stencil,
        });
        let first = colors
            .iter()
            .flatten()
            .map(|c| &c.view)
            .chain(depth.as_ref().map(|d| &d.view))
            .next();
        let (width, height, samples) = match (first, pass.empty) {
            (Some(view), _) => (view.width, view.height, view.samples),
            (None, Some(empty)) => (empty.width, empty.height, empty.samples),
            (None, None) => (0, 0, 1),
        };
        let mut target_colors = [None; MAX_COLOR_TARGETS];
        for (into, color) in target_colors.iter_mut().zip(&colors) {
            *into = color.as_ref().map(|c| c.view.format);
        }
        OpenPass {
            targets: TargetLayout {
                colors: target_colors,
                depth_stencil: depth.as_ref().map(|d| d.view.format),
                samples,
            },
            colors,
            depth,
            empty: pass.empty,
            occlusion: pass.occlusion.cloned(),
            commands: Vec::new(),
            width,
            height,
        }
    }

    /// Returns the pass that goes on after this one is ended early: the same attachments, loading what this one stored.
    fn resumed(&self) -> OpenPass<M> {
        OpenPass {
            colors: self
                .colors
                .iter()
                .map(|c| {
                    c.as_ref().map(|c| OpenColor {
                        view: c.view.clone(),
                        depth_slice: c.depth_slice,
                        resolve: c.resolve.clone(),
                        load: LoadOp::Load,
                        store: c.store,
                    })
                })
                .collect(),
            depth: self.depth.as_ref().map(|d| OpenDepth {
                view: d.view.clone(),
                depth: d.depth.map(|a| AspectOps {
                    load: LoadOp::Load,
                    ..a
                }),
                stencil: d.stencil.map(|a| AspectOps {
                    load: LoadOp::Load,
                    ..a
                }),
            }),
            empty: self.empty,
            occlusion: self.occlusion.clone(),
            commands: Vec::new(),
            targets: self.targets,
            width: self.width,
            height: self.height,
        }
    }

    /// Returns Metal's descriptor of the pass.
    fn descriptor(&self) -> RenderPassDescriptor<'_, M> {
        let mut color_attachments = [const { None }; MAX_COLOR_ATTACHMENTS];
        for (into, color) in color_attachments.iter_mut().zip(&self.colors) {
            let Some(c) = color else {
                continue;
            };
            let (load_action, clear_color) = match c.load {
                LoadOp::Load => (LoadAction::Load, [0.0; 4]),
                LoadOp::DontCare => (LoadAction::DontCare, [0.0; 4]),
                LoadOp::Clear(bits) => (LoadAction::Clear, clear_color(c.view.format, bits)),
            };
            *into = Some(RenderPassColorAttachment {
                texture: &c.view.raw,
                level: 0,
                slice: 0,
                depth_plane: c.depth_slice,
                load_action,
                store_action: store_action(c.store),
                clear_color,
                resolve: c.resolve.as_ref().map(|r| ResolveTarget {
                    texture: &r.raw,
                    level: 0,
                    slice: 0,
                    depth_plane: 0,
                }),
            });
        }
        let depth_attachment = self.depth.as_ref().and_then(|d| {
            d.depth.map(|a| {
                let load = match a.load {
                    LoadOp::Clear(bits) => LoadOp::Clear(f64::from(f32::from_bits(bits))),
                    LoadOp::Load => LoadOp::Load,
                    LoadOp::DontCare => LoadOp::DontCare,
                };
                aspect(&d.view, load, a.store)
            })
        });
        let stencil_attachment = self.depth.as_ref().and_then(|d| {
            d.stencil.map(|a| {
                let load = match a.load {
                    LoadOp::Clear(v) => LoadOp::Clear(f64::from(v)),
                    LoadOp::Load => LoadOp::Load,
                    LoadOp::DontCare => LoadOp::DontCare,
                };
                aspect(&d.view, load, a.store)
            })
        });
        RenderPassDescriptor {
            color_attachments,
            depth_attachment,
            stencil_attachment,
            visibility_result_buffer: self.occlusion.as_ref().map(|q| &q.buffer),
            render_target_width: self.width,
            render_target_height: self.height,
            default_raster_sample_count: self.targets.samples.max(1),
        }
    }
}

/// Returns Metal's depth or stencil attachment of a view, loaded and stored as given.
fn aspect<M: Metal>(
    view: &Shared<ViewObj<M>>,
    load: LoadOp<f64>,
    store: StoreOp,
) -> RenderPassDepthStencilAttachment<'_, M> {
    let (load_action, clear_value) = match load {
        LoadOp::Load => (LoadAction::Load, 0.0),
        LoadOp::DontCare => (LoadAction::DontCare, 0.0),
        LoadOp::Clear(v) => (LoadAction::Clear, v),
    };
    RenderPassDepthStencilAttachment {
        texture: &view.raw,
        level: 0,
        slice: 0,
        load_action,
        store_action: store_action(store),
        clear_value,
    }
}

/// Returns a clear color's value as Metal takes it, by the kind of the format's values.
fn clear_color(format: Format, bits: [u32; 4]) -> [f64; 4] {
    match FORMAT_FACTS[format as usize].kind {
        FormatKind::Uint => bits.map(f64::from),
        FormatKind::Sint => bits.map(|b| f64::from(b as i32)),
        _ => bits.map(|b| f64::from(f32::from_bits(b))),
    }
}

/// Returns Metal's store action of an interface one.
fn store_action(store: StoreOp) -> StoreAction {
    match store {
        StoreOp::Store => StoreAction::Store,
        StoreOp::Discard => StoreAction::DontCare,
    }
}

/// Returns the primitive type of a topology, and its class's number in pipeline keys.
fn primitive(topology: Topology) -> Option<(PrimitiveType, PrimitiveTopologyClass)> {
    Some(match topology {
        Topology::PointList => (PrimitiveType::Point, PrimitiveTopologyClass::Point),
        Topology::LineList => (PrimitiveType::Line, PrimitiveTopologyClass::Line),
        Topology::LineStrip => (PrimitiveType::LineStrip, PrimitiveTopologyClass::Line),
        Topology::TriangleList => (PrimitiveType::Triangle, PrimitiveTopologyClass::Triangle),
        Topology::TriangleStrip => (
            PrimitiveType::TriangleStrip,
            PrimitiveTopologyClass::Triangle,
        ),
        Topology::TriangleFan | Topology::LineLoop => return None,
    })
}

/// Returns whether a format holds depth, and whether it holds stencil.
fn aspects(format: Format) -> (bool, bool) {
    match FORMAT_FACTS[format as usize].kind {
        FormatKind::Depth => (true, false),
        FormatKind::Stencil => (false, true),
        FormatKind::DepthStencil => (true, true),
        _ => (false, false),
    }
}

/// Returns Metal's vertex buffers of a vertex layout, each stream in its own slot.
fn vertex_buffers(
    layout: &wcontainers_gpu::VertexLayoutDesc,
) -> Result<Vec<VertexBufferLayout>, &'static str> {
    let mut out = Vec::new();
    for (stream, s) in layout.streams.iter().enumerate() {
        let Some(s) = s else {
            continue;
        };
        let mut attributes = Vec::new();
        let mut end = 0;
        for a in layout
            .attributes
            .iter()
            .flatten()
            .filter(|a| a.stream as usize == stream)
        {
            let format = convert::vertex_format(a.format)
                .ok_or("a vertex format naga's vertex reads lack")?;
            end = end.max(a.offset + convert::vertex_bytes(a.format));
            attributes.push(VertexAttribute {
                location: a.location,
                offset: a.offset,
                format,
            });
        }
        let (stride, step_function) = match (s.stride, s.step) {
            (0, _) => (end.next_multiple_of(4).max(4), VertexStepFunction::Constant),
            (stride, StepMode::Vertex) => (stride, VertexStepFunction::PerVertex),
            (stride, StepMode::Instance(_)) => (stride, VertexStepFunction::PerInstance),
        };
        out.push(VertexBufferLayout {
            buffer_index: FIRST_STREAM_SLOT + stream as u32,
            stride,
            step_function,
            attributes,
        });
    }
    Ok(out)
}

impl<M: Metal> MetalGpu<M> {
    /// Compiles a shader's entry point with the given vertex buffers.
    pub(crate) fn make_function(
        &mut self,
        shader: &ShaderObj,
        vertex_buffers: &[VertexBufferLayout],
        point_size: bool,
    ) -> Result<M::Function, &'static str> {
        let resources: Vec<ResourceSlot> = shader
            .slots
            .iter()
            .map(|(&(group, binding), &(_, index))| ResourceSlot {
                group,
                binding,
                index,
            })
            .collect();
        let stage = match shader.stage {
            ShaderStage::Vertex => naga::ShaderStage::Vertex,
            ShaderStage::Fragment => naga::ShaderStage::Fragment,
            ShaderStage::Compute => naga::ShaderStage::Compute,
        };
        self.metal
            .make_function(&FunctionSource {
                module: &shader.module,
                info: &shader.info,
                stage,
                entry_point: &shader.entry_point,
                resources: &resources,
                immediates_buffer_index: IMMEDIATES_SLOT,
                sizes_buffer_index: SIZES_SLOT,
                vertex_buffers,
                point_size,
            })
            .map_err(|error| {
                eprintln!("metal: {} refused: {error:?}", shader.entry_point);
                "a function Metal refuses"
            })
    }

    /// Returns the render pipeline of a draw in the open pass, made at its first use.
    fn pipeline(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: TargetLayout,
        class: PrimitiveTopologyClass,
    ) -> Result<M::RenderPipelineState, &'static str> {
        let ProgramObj::Render {
            id,
            vertex,
            fragment,
        } = &*call.program.0
        else {
            return Err("a draw of a compute program");
        };
        let key = PipelineKey {
            program: *id,
            vertex_layout: call.vertex_layout.id,
            blend: call.blend.id,
            targets,
            class: class as u8,
        };
        if let Some(pipeline) = self.render_pipelines.get(&key) {
            return Ok(pipeline.clone());
        }
        self.record_slow_path(wcontainers_gpu::SlowPath::PipelineMiss);
        let points = class == PrimitiveTopologyClass::Point;
        let vertex_key = (vertex.id, call.vertex_layout.id, points);
        let vertex_function = match self.vertex_functions.get(&vertex_key) {
            Some(function) => function.clone(),
            None => {
                let buffers = vertex_buffers(&call.vertex_layout.desc)?;
                let function = self.make_function(vertex, &buffers, points)?;
                self.vertex_functions.insert(vertex_key, function.clone());
                function
            }
        };
        let fragment_function = match fragment {
            None => None,
            Some(shader) => Some(match self.fragment_functions.get(&shader.id) {
                Some(function) => function.clone(),
                None => {
                    let function = self.make_function(shader, &[], false)?;
                    self.fragment_functions.insert(shader.id, function.clone());
                    function
                }
            }),
        };
        let blend = &call.blend.desc;
        let mut color_attachments = [None; MAX_COLOR_ATTACHMENTS];
        for (index, format) in targets.colors.iter().enumerate() {
            let Some(format) = format else {
                continue;
            };
            let pixel_format = convert::pixel_format(*format)
                .ok_or("a target format this implementation lacks")?;
            let target = if blend.same_for_all {
                blend.targets[0]
            } else {
                blend.targets[index]
            };
            let integer = matches!(
                FORMAT_FACTS[*format as usize].kind,
                FormatKind::Uint | FormatKind::Sint
            );
            color_attachments[index] = Some(RenderPipelineColorAttachmentDescriptor {
                pixel_format,
                blending: target
                    .blend
                    .filter(|_| !integer)
                    .map(|(color, alpha)| Blending {
                        rgb_blend_operation: convert::blend_op(color.op),
                        alpha_blend_operation: convert::blend_op(alpha.op),
                        source_rgb_blend_factor: convert::blend_factor(color.src),
                        destination_rgb_blend_factor: convert::blend_factor(color.dst),
                        source_alpha_blend_factor: convert::blend_factor(alpha.src),
                        destination_alpha_blend_factor: convert::blend_factor(alpha.dst),
                    }),
                write_mask: ColorWriteMask {
                    red: target.writes.contains(wcontainers_gpu::ColorWrites::R),
                    green: target.writes.contains(wcontainers_gpu::ColorWrites::G),
                    blue: target.writes.contains(wcontainers_gpu::ColorWrites::B),
                    alpha: target.writes.contains(wcontainers_gpu::ColorWrites::A),
                },
            });
        }
        let (depth_format, stencil_format) = match targets.depth_stencil {
            Some(format) => {
                let pixel_format = convert::pixel_format(format)
                    .ok_or("a depth format this implementation lacks")?;
                let (depth, stencil) = aspects(format);
                (
                    depth.then_some(pixel_format),
                    stencil.then_some(pixel_format),
                )
            }
            None => (None, None),
        };
        let pipeline = self
            .metal
            .make_render_pipeline_state(&RenderPipelineDescriptor {
                vertex_function: &vertex_function,
                fragment_function: fragment_function.as_ref(),
                color_attachments,
                depth_attachment_pixel_format: depth_format,
                stencil_attachment_pixel_format: stencil_format,
                raster_sample_count: targets.samples.max(1),
                alpha_to_coverage_enabled: blend.alpha_to_coverage,
                rasterization_enabled: true,
                input_primitive_topology: class,
            })
            .map_err(|error| {
                eprintln!("metal: pipeline refused: {error:?}");
                "a render pipeline Metal refuses"
            })?;
        self.render_pipelines.insert(key, pipeline.clone());
        Ok(pipeline)
    }

    /// Records one draw into the open pass.
    pub(crate) fn record_draw(&mut self, call: &DrawCall<'_, Self>) -> Result<(), &'static str> {
        let Some(pass) = &self.pass else {
            return Err("a draw outside a pass");
        };
        let (targets, width, height) = (pass.targets, pass.width, pass.height);
        let (primitive_type, class) =
            primitive(call.topology).ok_or("a fan or loop through draw")?;
        let raster = call.raster.desc;
        let scissor = match call.scissors.first().filter(|_| raster.scissor) {
            Some(r) => {
                let (x0, y0) = (r.x.min(width), r.y.min(height));
                let (x1, y1) = (
                    r.x.saturating_add(r.width).min(width),
                    r.y.saturating_add(r.height).min(height),
                );
                if x1 <= x0 || y1 <= y0 {
                    return Ok(());
                }
                ScissorRect {
                    x: x0,
                    y: y0,
                    width: x1 - x0,
                    height: y1 - y0,
                }
            }
            None => ScissorRect {
                x: 0,
                y: 0,
                width,
                height,
            },
        };
        let (instances, empty) = match call.range {
            DrawRange::Vertices {
                count, instances, ..
            }
            | DrawRange::Indexed {
                count, instances, ..
            } => (instances, count == 0),
            DrawRange::Indirect { .. } | DrawRange::IndexedIndirect { .. } => {
                return Err("an indirect draw, which this implementation does not declare");
            }
        };
        if instances == 0 || empty {
            return Ok(());
        }
        let pipeline = self.pipeline(call, targets, class)?;
        let depth_stencil = self
            .depth_stencil_states
            .get(&call.depth_stencil.id)
            .cloned()
            .ok_or("a depth and stencil block of no state")?;
        let ProgramObj::Render {
            vertex, fragment, ..
        } = &*call.program.0
        else {
            return Err("a draw of a compute program");
        };
        let (vertex, fragment) = (vertex.clone(), fragment.clone());
        let mut commands = vec![
            RenderCommand::SetRenderPipelineState(pipeline),
            RenderCommand::SetDepthStencilState(depth_stencil),
            RenderCommand::SetCullMode(convert::cull(raster.cull)),
            RenderCommand::SetFrontFacingWinding(convert::winding(raster.front)),
            RenderCommand::SetTriangleFillMode(match raster.fill {
                wcontainers_gpu::FillMode::Line => TriangleFillMode::Lines,
                _ => TriangleFillMode::Fill,
            }),
            RenderCommand::SetDepthClipMode(if raster.depth_clip {
                DepthClipMode::Clip
            } else {
                DepthClipMode::Clamp
            }),
            RenderCommand::SetDepthBias {
                depth_bias: f32::from_bits(raster.depth_bias),
                slope_scale: f32::from_bits(raster.depth_bias_slope),
                clamp: f32::from_bits(raster.depth_bias_clamp),
            },
            RenderCommand::SetViewports(
                call.viewports
                    .iter()
                    .map(|v| wcontainers_metal::Viewport {
                        origin_x: f64::from(f32::from_bits(v.x)),
                        origin_y: f64::from(f32::from_bits(v.y)),
                        width: f64::from(f32::from_bits(v.width)),
                        height: f64::from(f32::from_bits(v.height)),
                        znear: f64::from(f32::from_bits(v.min_depth)),
                        zfar: f64::from(f32::from_bits(v.max_depth)),
                    })
                    .collect(),
            ),
            RenderCommand::SetScissorRects(vec![wcontainers_metal::ScissorRect {
                x: scissor.x,
                y: scissor.y,
                width: scissor.width,
                height: scissor.height,
            }]),
            RenderCommand::SetBlendColor(call.blend_constant.map(f32::from_bits)),
            RenderCommand::SetStencilReferenceValues {
                front: u32::from(call.stencil_reference[0]),
                back: u32::from(call.stencil_reference[1]),
            },
        ];
        for (group, set) in call.bindings.iter().enumerate() {
            let Some(set) = set else {
                continue;
            };
            for (binding, resource) in &set.entries {
                for (shader, vertex_stage) in [(Some(&vertex), true), (fragment.as_ref(), false)] {
                    let Some(&(kind, index)) =
                        shader.and_then(|s| s.slots.get(&(group as u32, *binding)))
                    else {
                        continue;
                    };
                    commands.push(match (resource, kind, vertex_stage) {
                        (
                            SetResource::Buffer {
                                buffer,
                                offset,
                                size,
                            },
                            SlotKind::Buffer,
                            _,
                        ) => {
                            self.used(buffer);
                            let (buffer, offset, length) =
                                (buffer.raw.borrow().clone(), *offset, *size);
                            if vertex_stage {
                                RenderCommand::SetVertexBuffer {
                                    buffer,
                                    offset,
                                    length,
                                    index,
                                }
                            } else {
                                RenderCommand::SetFragmentBuffer {
                                    buffer,
                                    offset,
                                    length,
                                    index,
                                }
                            }
                        }
                        (SetResource::Texture(view), SlotKind::Texture, true) => {
                            RenderCommand::SetVertexTexture {
                                texture: view.raw.clone(),
                                index,
                            }
                        }
                        (SetResource::Texture(view), SlotKind::Texture, false) => {
                            RenderCommand::SetFragmentTexture {
                                texture: view.raw.clone(),
                                index,
                            }
                        }
                        (SetResource::Sampler(sampler), SlotKind::Sampler, true) => {
                            RenderCommand::SetVertexSamplerState {
                                sampler: sampler.raw.clone(),
                                index,
                            }
                        }
                        (SetResource::Sampler(sampler), SlotKind::Sampler, false) => {
                            RenderCommand::SetFragmentSamplerState {
                                sampler: sampler.raw.clone(),
                                index,
                            }
                        }
                        _ => return Err("a binding of another kind than the program reads"),
                    });
                }
            }
        }
        for (stream, binding) in call.vertex_buffers.iter().enumerate() {
            let Some(binding) = binding else {
                continue;
            };
            if call
                .vertex_layout
                .desc
                .streams
                .get(stream)
                .copied()
                .flatten()
                .is_none()
            {
                continue;
            }
            self.used(binding.buffer);
            commands.push(RenderCommand::SetVertexBuffer {
                buffer: binding.buffer.raw.borrow().clone(),
                offset: binding.offset.min(binding.buffer.size),
                length: binding.buffer.size.saturating_sub(binding.offset),
                index: FIRST_STREAM_SLOT + stream as u32,
            });
        }
        if !call.immediates.is_empty() {
            commands.push(RenderCommand::SetVertexBytes {
                bytes: call.immediates.to_vec(),
                index: IMMEDIATES_SLOT,
            });
            commands.push(RenderCommand::SetFragmentBytes {
                bytes: call.immediates.to_vec(),
                index: IMMEDIATES_SLOT,
            });
        }
        commands.push(match call.range {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => RenderCommand::DrawPrimitives {
                primitive_type,
                vertex_start: first,
                vertex_count: count,
                instance_count: instances,
                base_instance: first_instance,
            },
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => {
                let index = call
                    .index
                    .as_ref()
                    .ok_or("an indexed draw with no index buffer")?;
                let (index_type, size) = match index.format {
                    IndexFormat::U16 => (IndexType::UInt16, 2),
                    IndexFormat::U32 => (IndexType::UInt32, 4),
                    IndexFormat::U8 => {
                        return Err("8-bit indices, which this implementation does not declare");
                    }
                };
                self.used(index.buffer);
                RenderCommand::DrawIndexedPrimitives {
                    primitive_type,
                    index_count: count,
                    index_type,
                    index_buffer: index.buffer.raw.borrow().clone(),
                    index_buffer_offset: index.offset + u64::from(first) * size,
                    instance_count: instances,
                    base_vertex,
                    base_instance: first_instance,
                }
            }
            DrawRange::Indirect { .. } | DrawRange::IndexedIndirect { .. } => {
                unreachable!("refused above")
            }
        });
        if let Some(pass) = &mut self.pass {
            pass.commands.extend(commands);
        }
        Ok(())
    }

    /// Starts or stops counting into the open pass's occlusion set.
    pub(crate) fn record_query(&mut self, index: Option<u32>) {
        let Some(pass) = &mut self.pass else {
            self.fail("a query outside a pass");
            return;
        };
        let Some(set) = &pass.occlusion else {
            self.fail("a query in a pass with no occlusion set");
            return;
        };
        let command = match index {
            Some(index) if index < set.count => RenderCommand::SetVisibilityResultMode {
                mode: VisibilityResultMode::Boolean,
                offset: u64::from(index) * QUERY_BYTES,
            },
            Some(_) => {
                self.fail("a query past its set's end");
                return;
            }
            None => RenderCommand::SetVisibilityResultMode {
                mode: VisibilityResultMode::Disabled,
                offset: 0,
            },
        };
        pass.commands.push(command);
    }

    /// Ends the open pass, encoding it, and returns the pass that would go on after it.
    pub(crate) fn end_open_pass(&mut self) -> Option<OpenPass<M>> {
        let pass = self.pass.take()?;
        let resumed = pass.resumed();
        self.flush_blits();
        let command_buffer = self.command_buffer()?;
        if let Err(error) =
            self.metal
                .encode_render_commands(&command_buffer, &pass.descriptor(), &pass.commands)
        {
            self.fail(&format!("a pass refused: {error:?}"));
        }
        Some(resumed)
    }

    /// Records a resolve of one subresource into another, as a pass that only resolves.
    pub(crate) fn record_resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) -> Result<(), &'static str> {
        let view = |b: &mut Self, region: &TextureRegion<'_, Self>| {
            b.create_view(
                region.texture,
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: region.mip,
                    mip_count: 1,
                    base_layer: region.layer,
                    layer_count: 1,
                    stencil: false,
                },
            )
        };
        let from = view(self, src).map_err(|_| "a resolve source view")?;
        let into = view(self, dst).map_err(|_| "a resolve target view")?;
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(wcontainers_gpu::ColorAttachment {
            view: &from,
            depth_slice: 0,
            resolve: Some(&into),
            load: LoadOp::Load,
            store: StoreOp::Store,
        });
        self.begin_pass(&pass);
        self.end_pass();
        Ok(())
    }
}
