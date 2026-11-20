//! Showing a frame: the window's surface on the back end, and the copy of the framebuffer's
//! picture into it.

use std::ffi::{c_uint, c_void};

use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendDesc, ColorAttachment, ColorWrites, CullMode,
    DepthStencilDesc, DrawCall, DrawRange, FillMode, Format, FormatUse, FrontFace, LoadOp,
    MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, Object, PassDesc, Program, ProvokingVertex, RasterDesc,
    RenderTargetBlend, ScissorRect, ShaderModule, ShaderStage, SlotKind, StageSet, StoreOp,
    TextureViewDesc, Topology, VertexLayoutDesc, Viewport,
};

use crate::context::Context;
use crate::device::HostWindows;
use crate::gallium::*;
use crate::resource::Resource;
use crate::screen::{DRIVER_NAME, Screen};

/// A host window the driver presents to, by the device context OpenGL names it with.
pub struct Window<B: Backend> {
    surface: B::Surface,
    /// Width, height and format the surface is configured for.
    configured: Option<(u32, u32, Format)>,
}

/// The copy of a picture into a window's texture: a full-screen triangle loading each pixel.
pub struct Copy<B: Backend> {
    program: B::Program,
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
    vertex_layout: B::VertexLayout,
}

/// The vertices of one triangle covering the target.
const COPY_VERTICES: u32 = 3;

impl<B: Backend> Copy<B> {
    fn new(backend: &mut B) -> Result<Copy<B>, wcontainers_gpu::GpuError> {
        let picture = SlotKind::Texture.first_binding();
        let source = format!(
            "@group(0) @binding({picture}) var picture: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {{
    return textureLoad(picture, vec2<i32>(at.xy), 0);
}}"
        );
        let vertex = backend.create_shader(&ShaderModule::from_wgsl(
            &source,
            "vs",
            ShaderStage::Vertex,
        )?)?;
        let fragment = backend.create_shader(&ShaderModule::from_wgsl(
            &source,
            "fs",
            ShaderStage::Fragment,
        )?)?;
        let program = backend.create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })?;
        backend.destroy(Object::Shader(vertex));
        backend.destroy(Object::Shader(fragment));
        Ok(Copy {
            program,
            blend: backend.create_blend(&BlendDesc {
                targets: [RenderTargetBlend {
                    blend: None,
                    writes: ColorWrites::all(),
                }; MAX_COLOR_TARGETS],
                same_for_all: true,
                alpha_to_coverage: false,
                logic_op: None,
            })?,
            depth_stencil: backend.create_depth_stencil(&DepthStencilDesc {
                depth_compare: None,
                depth_write: false,
                stencil: None,
                stencil_read_mask: [u8::MAX; 2],
                stencil_write_mask: [u8::MAX; 2],
                depth_bounds: None,
            })?,
            raster: backend.create_raster(&RasterDesc {
                fill: FillMode::Solid,
                cull: CullMode::None,
                front: FrontFace::CounterClockwise,
                depth_bias: 0,
                depth_bias_slope: 0,
                depth_bias_clamp: 0,
                depth_clip: true,
                scissor: false,
                line_width: 1.0f32.to_bits(),
                provoking: ProvokingVertex::First,
                depth_minus_one: false,
                sample_mask: u32::MAX,
            })?,
            vertex_layout: backend.create_vertex_layout(&VertexLayoutDesc {
                streams: [None; MAX_VERTEX_STREAMS],
                attributes: [None; MAX_VERTEX_STREAMS],
            })?,
        })
    }

    pub fn release(self, screen: &Screen<B>) {
        screen.retire(Object::Program(self.program));
        screen.retire(Object::Blend(self.blend));
        screen.retire(Object::DepthStencil(self.depth_stencil));
        screen.retire(Object::Raster(self.raster));
        screen.retire(Object::VertexLayout(self.vertex_layout));
    }
}

/// The format a window shows a picture of `format` in.
fn window_format<B: Backend>(backend: &B, format: Format) -> Option<Format> {
    [format, Format::Bgra8Unorm, Format::Rgba8Unorm]
        .into_iter()
        .find(|f| backend.caps().formats.get(*f).contains(FormatUse::PRESENT))
}

/// Shows one level and layer of `resource` in the window `device_context` names.
fn show<B: HostWindows>(
    context: &mut Context<B>,
    resource: &Resource<B>,
    level: c_uint,
    layer: c_uint,
    device_context: *mut c_void,
) -> Result<(), String> {
    let screen = context.screen();
    let mut backend = screen.backend();
    let backend = &mut *backend;
    context
        .tracker
        .close(backend)
        .map_err(|e| format!("{e:?}"))?;
    let (texture, desc) = resource.texture().ok_or("a buffer shown as a frame")?;
    let format = resource
        .format()
        .ok_or("a frame of a format the device lacks")?;
    let shown = window_format(backend, format).ok_or("no format the window shows")?;
    let (width, height) = ((desc.width >> level).max(1), (desc.height >> level).max(1));
    let mut windows = screen.windows.lock().map_err(|_| "the windows' lock")?;
    let key = device_context as usize;
    if !windows.contains_key(&key) {
        let id = windows.len() as u32;
        if !backend.add_window_of(id, device_context) {
            return Err("a window the back end cannot present to".into());
        }
        let surface = backend.create_surface(id).map_err(|e| format!("{e:?}"))?;
        windows.insert(
            key,
            Window {
                surface,
                configured: None,
            },
        );
    }
    let window = windows.get_mut(&key).ok_or("the window")?;
    if window.configured != Some((width, height, shown)) {
        backend
            .configure_surface(&window.surface, shown, width, height, false)
            .map_err(|e| format!("{e:?}"))?;
        window.configured = Some((width, height, shown));
    }
    if context.present.is_none() {
        context.present = Some(Copy::new(backend).map_err(|e| format!("{e:?}"))?);
    }
    let copy = context.present.as_ref().ok_or("the copy")?;
    let target = backend
        .current_texture(&window.surface)
        .map_err(|e| format!("{e:?}"))?;
    let picture = backend
        .create_view(
            texture,
            &TextureViewDesc {
                format,
                dimension: wcontainers_gpu::ViewDimension::D2,
                base_mip: level,
                mip_count: 1,
                base_layer: layer,
                layer_count: 1,
                stencil: false,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
    let set = backend
        .create_binding_set(&[BindingEntry {
            binding: SlotKind::Texture.first_binding(),
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&picture),
        }])
        .map_err(|e| format!("{e:?}"))?;
    let mut colors = [const { None }; MAX_COLOR_TARGETS];
    colors[0] = Some(ColorAttachment {
        view: &target,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::DontCare,
        store: StoreOp::Store,
    });
    backend.begin_pass(&PassDesc {
        colors,
        depth: None,
        empty: None,
        occlusion: None,
    });
    backend.draw(&DrawCall {
        program: &copy.program,
        blend: &copy.blend,
        depth_stencil: &copy.depth_stencil,
        raster: &copy.raster,
        vertex_layout: &copy.vertex_layout,
        bindings: [Some(&set), None, None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x: 0f32.to_bits(),
            y: 0f32.to_bits(),
            width: (width as f32).to_bits(),
            height: (height as f32).to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }],
        scissors: &[ScissorRect {
            x: 0,
            y: 0,
            width,
            height,
        }],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: COPY_VERTICES,
            first_instance: 0,
            instances: 1,
        },
    });
    backend.end_pass();
    backend.destroy(Object::BindingSet(set));
    backend.destroy(Object::TextureView(picture));
    let shown = backend
        .present(&window.surface)
        .map_err(|e| format!("{e:?}"));
    backend.destroy(Object::TextureView(target));
    shown
}

/// `pipe_screen::flush_frontbuffer`, which the WGL target calls to show a frame in the window of
/// the device context it passes.
#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
pub unsafe extern "C" fn flush_frontbuffer<B: HostWindows>(
    _: *mut pipe_screen,
    pipe: *mut pipe_context,
    resource: *mut pipe_resource,
    level: c_uint,
    layer: c_uint,
    device_context: *mut c_void,
    _: c_uint,
    _: *mut pipe_box,
) {
    if pipe.is_null() {
        eprintln!(
            "{}: a frame shown without a context is not shown",
            DRIVER_NAME.to_string_lossy()
        );
        return;
    }
    // SAFETY: the WGL target passes this driver's context and a texture of it.
    let (context, resource) = unsafe { (Context::<B>::of(pipe), Resource::<B>::of(resource)) };
    if let Err(why) = show(context, resource, level, layer, device_context) {
        eprintln!(
            "{}: a frame not shown: {why}",
            DRIVER_NAME.to_string_lossy()
        );
    }
}
