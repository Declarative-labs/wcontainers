//! Swap chains: the program's own back buffers, the copy into the host's surface at each present,
//! and frame pacing.
//!
//! The back buffers are ordinary textures the program can read, copy and keep across frames. At
//! `Present` the current one is copied into the surface's texture by one draw (converting format and
//! sRGB on the way) and the surface is shown. Flip models then move on to the next buffer; blit models
//! keep drawing into buffer 0, and a sequential one's contents move one buffer down. Present blocks
//! once more frames are queued on the GPU than the swap chain's maximum frame latency.

use std::collections::VecDeque;

use wcontainers_gpu::{
    Gpu, BindingEntry, BindingResource, BlendDesc, ColorAttachment, ColorWrites, CullMode,
    DepthStencilDesc, DeviceStatus, DrawCall, DrawRange, Extent, FORMAT_FACTS, FillMode, Format,
    FrontFace, GpuError, LoadOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, Object, Origin, PassDesc,
    Program, ProvokingVertex, RasterDesc, RenderTargetBlend, ScissorRect, ShaderModule,
    ShaderStage, SlowPath, StageSet, StencilFace, StoreOp, TextureDesc, TextureDimension,
    TextureRegion, TextureUsage, TextureViewDesc, Topology, VertexLayoutDesc, Viewport,
};

use crate::format::{BACK_BUFFER_FORMATS, back_buffer_format, dxgi, present_format};

/// The most buffers a swap chain may have (`DXGI_MAX_SWAP_CHAIN_BUFFERS`).
pub const MAX_BUFFERS: u32 = 16;
/// The frames queued before `Present` blocks, unless the program sets another
/// (`IDXGIDevice1::SetMaximumFrameLatency`, `IDXGISwapChain2::SetMaximumFrameLatency`).
pub const DEFAULT_LATENCY: u32 = 3;

/// The presentation model (`DXGI_SWAP_EFFECT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// Blit model, back buffer contents undefined after a present.
    Discard,
    /// Blit model, back buffer contents kept.
    Sequential,
    /// Flip model, each buffer's contents kept for its next turn.
    FlipSequential,
    /// Flip model, contents undefined after a present.
    FlipDiscard,
}

impl Model {
    pub fn from_dxgi(value: u32) -> Option<Model> {
        match value {
            0 => Some(Model::Discard),
            1 => Some(Model::Sequential),
            3 => Some(Model::FlipSequential),
            4 => Some(Model::FlipDiscard),
            _ => None,
        }
    }

    pub fn is_flip(self) -> bool {
        matches!(self, Model::FlipSequential | Model::FlipDiscard)
    }
}

/// What a swap chain is made with, after the shell has filled sizes of zero from the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapChainDesc {
    pub width: u32,
    pub height: u32,
    /// A `DXGI_FORMAT` value.
    pub format: u32,
    pub buffers: u32,
    pub model: Model,
    /// What the program may do with the back buffers besides drawing into them and presenting
    /// (sampling and storage, from `DXGI_USAGE`).
    pub usage: TextureUsage,
}

impl SwapChainDesc {
    /// The creation check: `Err` holds why DXGI refuses the description (`DXGI_ERROR_INVALID_CALL`).
    pub fn check(&self) -> Result<Format, &'static str> {
        if !BACK_BUFFER_FORMATS.contains(&self.format) {
            return Err("the format is not a back buffer format");
        }
        let format = back_buffer_format(self.format).ok_or("unknown format")?;
        if self.width == 0 || self.height == 0 {
            return Err("a back buffer has no size");
        }
        if self.model.is_flip() {
            if !(2..=MAX_BUFFERS).contains(&self.buffers) {
                return Err("flip models take 2 to 16 buffers");
            }
            if format != FORMAT_FACTS[format as usize].linear {
                return Err(
                    "flip models take no sRGB formats; sRGB views of the buffers draw sRGB",
                );
            }
            if self.format == dxgi::R10G10B10_XR_BIAS_A2_UNORM {
                return Err("flip models take no extended-range format");
            }
        } else if !(1..=MAX_BUFFERS).contains(&self.buffers) {
            return Err("blit models take 1 to 16 buffers");
        }
        Ok(format)
    }
}

/// Why a swap chain call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapChainError {
    /// DXGI refuses the call (`DXGI_ERROR_INVALID_CALL`), for this reason.
    InvalidCall(&'static str),
    Gpu(GpuError),
}

impl From<GpuError> for SwapChainError {
    fn from(e: GpuError) -> Self {
        SwapChainError::Gpu(e)
    }
}

/// The program copying a back buffer into the surface: a full-screen triangle loading the texel
/// under each pixel. `encode` writes sRGB-encoded values, for back buffers whose loads decode
/// (sRGB formats) or hold linear light (float formats); `extended` decodes the extended-range
/// format's codes, `(c - 384) / 510` (the functional specification's XR_BIAS mapping, -0.7529 to
/// 1.2529), and shows the part an SDR display shows, 0 to 1.
const PRESENT_WGSL: &str = r#"
@group(0) @binding(32) var source: texture_2d<f32>;

// The extended-range format's codes per channel, and the code and count of codes of 0 to 1.
const XR_CODES: f32 = 1023.0;
const XR_ZERO: f32 = 384.0;
const XR_ONE: f32 = 510.0;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

@fragment
fn copy(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(p.xy), 0);
}

@fragment
fn encode(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(source, vec2<i32>(p.xy), 0);
    let l = max(c.rgb, vec3<f32>(0.0));
    let e = select(1.055 * pow(l, vec3<f32>(1.0 / 2.4)) - 0.055, l * 12.92, l <= vec3<f32>(0.0031308));
    return vec4<f32>(e, c.a);
}

@fragment
fn extended(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(source, vec2<i32>(p.xy), 0);
    let v = (round(c.rgb * XR_CODES) - XR_ZERO) / XR_ONE;
    return vec4<f32>(saturate(v), c.a);
}
"#;

/// How the present copy writes a back buffer's texels into the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    /// As loaded.
    Copied,
    /// sRGB-encoded: sRGB formats, whose loads decode, and 16-bit float buffers, which hold linear
    /// light.
    Encoded,
    /// The extended-range format's codes decoded.
    Extended,
}

impl Shown {
    fn of(value: u32, format: Format) -> Shown {
        if value == dxgi::R10G10B10_XR_BIAS_A2_UNORM {
            Shown::Extended
        } else if format != FORMAT_FACTS[format as usize].linear || format == Format::Rgba16Float {
            Shown::Encoded
        } else {
            Shown::Copied
        }
    }

    /// The present program's fragment entry point of each.
    const COPIED: &str = "copy";
    const ENCODED: &str = "encode";
    const EXTENDED: &str = "extended";

    fn entry_point(self) -> &'static str {
        match self {
            Shown::Copied => Self::COPIED,
            Shown::Encoded => Self::ENCODED,
            Shown::Extended => Self::EXTENDED,
        }
    }
}

fn copy_program<B: Gpu>(backend: &mut B, shown: Shown) -> Result<B::Program, GpuError> {
    let fragment = shown.entry_point();
    let vs = backend.create_shader(&ShaderModule::from_wgsl(
        PRESENT_WGSL,
        "vs",
        ShaderStage::Vertex,
    )?)?;
    let fs = backend.create_shader(&ShaderModule::from_wgsl(
        PRESENT_WGSL,
        fragment,
        ShaderStage::Fragment,
    )?)?;
    let program = backend.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&fs),
    });
    backend.destroy(Object::Shader(vs));
    backend.destroy(Object::Shader(fs));
    program
}

/// The blocks and program of the present copy.
struct Copy<B: Gpu> {
    program: B::Program,
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
    vertex_layout: B::VertexLayout,
}

/// One back buffer: its texture, the view the present copy reads, and that view's binding set.
struct Buffer<B: Gpu> {
    texture: B::Texture,
    view: B::TextureView,
    set: B::BindingSet,
}

/// A DXGI swap chain on back end `B`.
pub struct SwapChain<B: Gpu> {
    desc: SwapChainDesc,
    format: Format,
    surface: B::Surface,
    surface_format: Format,
    vsync: bool,
    buffers: Vec<Buffer<B>>,
    /// Where a sequential blit swap chain of several buffers keeps the first's contents while the
    /// others move down.
    spare: Option<B::Texture>,
    current: u32,
    presents: u64,
    latency: u32,
    in_flight: VecDeque<B::Fence>,
    /// Presented frames whose GPU work has finished, counted when found finished.
    retired: u64,
    copy: Copy<B>,
    shown: Shown,
}

impl<B: Gpu> SwapChain<B> {
    /// Makes a swap chain on the host window `window`, as `IDXGIFactory2::CreateSwapChainForHwnd` and
    /// the others do.
    pub fn new(backend: &mut B, window: u32, desc: &SwapChainDesc) -> Result<Self, SwapChainError> {
        let format = desc.check().map_err(SwapChainError::InvalidCall)?;
        let surface = backend.create_surface(window)?;
        let surface_format = present_format(backend.caps(), format);
        backend.configure_surface(&surface, surface_format, desc.width, desc.height, true)?;
        let shown = Shown::of(desc.format, format);
        let copy = Copy {
            program: copy_program(backend, shown)?,
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
                stencil: None::<(StencilFace, StencilFace)>,
                stencil_read_mask: [!0; 2],
                stencil_write_mask: [!0; 2],
                depth_bounds: None,
            })?,
            raster: backend.create_raster(&RasterDesc {
                fill: FillMode::Solid,
                cull: CullMode::None,
                front: FrontFace::Clockwise,
                depth_bias: 0,
                depth_bias_slope: 0,
                depth_bias_clamp: 0,
                depth_clip: true,
                scissor: false,
                line_width: 1.0f32.to_bits(),
                provoking: ProvokingVertex::First,
                depth_minus_one: false,
                sample_mask: !0,
            })?,
            vertex_layout: backend.create_vertex_layout(&VertexLayoutDesc {
                streams: [None; MAX_VERTEX_STREAMS],
                attributes: [None; MAX_VERTEX_STREAMS],
            })?,
        };
        let mut chain = SwapChain {
            desc: *desc,
            format,
            surface,
            surface_format,
            vsync: true,
            buffers: Vec::new(),
            spare: None,
            current: 0,
            presents: 0,
            latency: DEFAULT_LATENCY,
            in_flight: VecDeque::new(),
            retired: 0,
            copy,
            shown,
        };
        chain.make_buffers(backend)?;
        Ok(chain)
    }

    fn make_buffers(&mut self, backend: &mut B) -> Result<(), GpuError> {
        // The discard blit model gives the program buffer 0 only; the others DXGI keeps for itself,
        // and one copy per frame needs none of them.
        let count = match self.desc.model {
            Model::Discard => 1,
            _ => self.desc.buffers,
        };
        if self.desc.model == Model::Sequential && count > 1 {
            self.spare = Some(backend.create_texture(&self.buffer_desc())?);
        }
        for _ in 0..count {
            let texture = backend.create_texture(&self.buffer_desc())?;
            let view = backend.create_view(
                &texture,
                &TextureViewDesc {
                    format: self.format,
                    dimension: wcontainers_gpu::ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )?;
            let set = backend.create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])?;
            self.buffers.push(Buffer { texture, view, set });
        }
        Ok(())
    }

    fn drop_buffers(&mut self, backend: &mut B) {
        if let Some(t) = self.spare.take() {
            backend.destroy(Object::Texture(t));
        }
        for b in self.buffers.drain(..) {
            backend.destroy(Object::BindingSet(b.set));
            backend.destroy(Object::TextureView(b.view));
            backend.destroy(Object::Texture(b.texture));
        }
    }

    pub fn desc(&self) -> &SwapChainDesc {
        &self.desc
    }

    /// What each back buffer texture is: 2D, one level, one sample, views of other formats of its
    /// group allowed so sRGB views work.
    pub fn buffer_desc(&self) -> TextureDesc {
        TextureDesc {
            dimension: TextureDimension::D2,
            format: self.format,
            width: self.desc.width,
            height: self.desc.height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: self.desc.usage
                | TextureUsage::RENDER
                | TextureUsage::SAMPLE
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: true,
        }
    }

    /// The back buffer `index` (`GetBuffer`): any but of the discard blit model, which gives buffer 0
    /// only. Of the sequential blit model, buffers past the first are for reading (DXGI refuses to
    /// draw into them).
    pub fn buffer(&self, index: u32) -> Option<&B::Texture> {
        self.buffers.get(index as usize).map(|b| &b.texture)
    }

    /// The buffer to draw this frame into (`GetCurrentBackBufferIndex`).
    pub fn current_index(&self) -> u32 {
        self.current
    }

    /// How many presents have been made (`GetLastPresentCount`).
    pub fn present_count(&self) -> u64 {
        self.presents
    }

    /// The format the host's surface is configured with.
    pub fn surface_format(&self) -> Format {
        self.surface_format
    }

    pub fn max_latency(&self) -> u32 {
        self.latency
    }

    /// Sets the frames queued before `Present` blocks, 1 to 16.
    pub fn set_max_latency(&mut self, frames: u32) -> Result<(), SwapChainError> {
        if !(1..=16).contains(&frames) {
            return Err(SwapChainError::InvalidCall("frame latency is 1 to 16"));
        }
        self.latency = frames;
        Ok(())
    }

    /// Frames presented whose GPU work has not finished yet.
    pub fn frames_queued(&mut self, backend: &mut B) -> u32 {
        while let Some(f) = self.in_flight.front() {
            if !backend.fence_done(f) {
                break;
            }
            self.in_flight.pop_front();
            self.retired += 1;
        }
        self.in_flight.len() as u32
    }

    /// Presented frames found finished so far (by `frames_queued` and the waits of `present`): what
    /// DXGI's frame latency waitable object is released for.
    pub fn frames_retired(&mut self, backend: &mut B) -> u64 {
        self.frames_queued(backend);
        self.retired
    }

    /// Whether presented frames are still on the GPU, without asking it.
    pub fn frames_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }

    /// Whether a present now would block for frame pacing (`DXGI_PRESENT_DO_NOT_WAIT` then answers
    /// `DXGI_ERROR_WAS_STILL_DRAWING`).
    pub fn would_wait(&mut self, backend: &mut B) -> bool {
        self.frames_queued(backend) >= self.latency
    }

    /// Shows the current back buffer. A sync interval of zero shows it at once; 1 to 4 wait for the
    /// display (the host shows each frame for at least one refresh). Work recorded before the call is
    /// submitted with the copy.
    pub fn present(&mut self, backend: &mut B, sync_interval: u32) -> Result<(), SwapChainError> {
        if sync_interval > 4 {
            return Err(SwapChainError::InvalidCall("sync interval is 0 to 4"));
        }
        if let DeviceStatus::Lost(_) = backend.status() {
            return Err(GpuError::DeviceLost.into());
        }
        let vsync = sync_interval > 0;
        if vsync != self.vsync {
            backend.configure_surface(
                &self.surface,
                self.surface_format,
                self.desc.width,
                self.desc.height,
                vsync,
            )?;
            self.vsync = vsync;
        }
        let target = backend.current_texture(&self.surface)?;
        let buffer = &self.buffers[self.current as usize];
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
        let (w, h) = (self.desc.width, self.desc.height);
        backend.draw(&DrawCall {
            program: &self.copy.program,
            blend: &self.copy.blend,
            depth_stencil: &self.copy.depth_stencil,
            raster: &self.copy.raster,
            vertex_layout: &self.copy.vertex_layout,
            bindings: [Some(&buffer.set), None, None, None],
            vertex_buffers: &[],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &[Viewport {
                x: 0f32.to_bits(),
                y: 0f32.to_bits(),
                width: (w as f32).to_bits(),
                height: (h as f32).to_bits(),
                min_depth: 0f32.to_bits(),
                max_depth: 1f32.to_bits(),
            }],
            scissors: &[ScissorRect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            }],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count: 3,
                first_instance: 0,
                instances: 1,
            },
        });
        backend.end_pass();
        if let Some(spare) = &self.spare {
            self.move_down(backend, spare);
        }
        let fence = backend.submit();
        let shown = backend.present(&self.surface);
        backend.destroy(Object::TextureView(target));
        shown?;
        self.presents += 1;
        if self.desc.model.is_flip() {
            self.current = (self.current + 1) % self.desc.buffers;
        }
        self.in_flight.push_back(fence);
        while self.frames_queued(backend) > self.latency {
            let oldest = self.in_flight.pop_front().expect("frames are queued");
            backend.wait_fence(&oldest);
            self.retired += 1;
        }
        Ok(())
    }

    /// The sequential blit model's move at present: each buffer takes the next one's contents, and
    /// the last takes the first's, by way of `spare`.
    fn move_down(&self, backend: &mut B, spare: &B::Texture) {
        backend.slow_paths().hit(SlowPath::SequentialBuffersCopied);
        let extent = Extent {
            width: self.desc.width,
            height: self.desc.height,
            depth_or_layers: 1,
        };
        let region = |texture| TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let last = self.buffers.len() - 1;
        backend.copy_texture(&region(&self.buffers[0].texture), &region(spare), extent);
        for i in 0..last {
            backend.copy_texture(
                &region(&self.buffers[i + 1].texture),
                &region(&self.buffers[i].texture),
                extent,
            );
        }
        backend.copy_texture(&region(spare), &region(&self.buffers[last].texture), extent);
    }

    /// `ResizeBuffers`: new buffers of the given count, size and format (zero keeps each), made
    /// after the front has checked the program holds no reference to the old ones.
    pub fn resize(
        &mut self,
        backend: &mut B,
        buffers: u32,
        width: u32,
        height: u32,
        format: u32,
    ) -> Result<(), SwapChainError> {
        let mut desc = self.desc;
        if buffers != 0 {
            desc.buffers = buffers;
        }
        if width != 0 {
            desc.width = width;
        }
        if height != 0 {
            desc.height = height;
        }
        if format != 0 {
            desc.format = format;
        }
        let new_format = desc.check().map_err(SwapChainError::InvalidCall)?;
        self.set_shown(backend, Shown::of(desc.format, new_format))?;
        self.drop_buffers(backend);
        self.desc = desc;
        self.format = new_format;
        self.surface_format = present_format(backend.caps(), new_format);
        backend.configure_surface(
            &self.surface,
            self.surface_format,
            desc.width,
            desc.height,
            self.vsync,
        )?;
        self.current = 0;
        self.make_buffers(backend)?;
        Ok(())
    }

    fn set_shown(&mut self, backend: &mut B, shown: Shown) -> Result<(), GpuError> {
        if shown != self.shown {
            let program = copy_program(backend, shown)?;
            let old = std::mem::replace(&mut self.copy.program, program);
            backend.destroy(Object::Program(old));
            self.shown = shown;
        }
        Ok(())
    }

    /// Releases everything the swap chain made.
    pub fn destroy(mut self, backend: &mut B) {
        self.drop_buffers(backend);
        backend.destroy(Object::Program(self.copy.program));
        backend.destroy(Object::Blend(self.copy.blend));
        backend.destroy(Object::DepthStencil(self.copy.depth_stencil));
        backend.destroy(Object::Raster(self.copy.raster));
        backend.destroy(Object::VertexLayout(self.copy.vertex_layout));
        backend.destroy(Object::Surface(self.surface));
    }
}
