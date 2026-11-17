//! The WebGPU back end of the graphics interface, on `wgpu`: natively (Direct3D 12, Vulkan, Metal) for
//! tests and the Windows libraries, and on the browser's WebGPU when built for WebAssembly.
//!
//! Made from the `wgpu` device it drives ([`WebGpu::new`]), so the features the device was made with
//! decide what it declares. Blocks are plain data; a draw's combination of blocks, target formats,
//! topology and layout keys names a cached pipeline, and binding sets become cached bind groups whose
//! uniform buffers take dynamic offsets (plan sections 4.4 and 4.5).

mod convert;
mod draw;
mod edge;
mod fill;
mod layout;
mod memory;
mod probe;

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use wcontainers_gpu::{
    Backend, BindingEntry, BindingLayoutKey, BlendDesc, BufferDesc, BufferUsage, Caps,
    CopyBufferTexture, DepthStencilDesc, DeviceStatus, Extent, Features, FillMode, Filter, Format,
    FormatCaps, FormatKind, FormatUse, GpuError, Helpers, Limits, LossReason, MAX_BINDING_SETS,
    MapIntent, Object, PassDesc, Program, ProvokingVertex, QueryKind, RasterDesc, ReadSource,
    ReadState, SamplerDesc, ShaderModule, ShaderStage, SlowPaths, StepMode, TextureDesc,
    TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, VertexLayoutDesc, WriteMode,
};

pub use draw::Counters;
pub use probe::Depth24;

/// Readback states, set by the map callback.
const MAP_PENDING: u8 = 0;
const MAP_READY: u8 = 1;
const MAP_FAILED: u8 = 2;

/// The WebGPU back end. One thread owns it.
pub struct WebGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<WebGpu>,
    /// Set by wgpu's device-lost and uncaptured-error callbacks, and by the back end on a rule it
    /// cannot carry out.
    lost: Arc<Mutex<Option<LossReason>>>,
    next_id: u64,
    /// The number of the command encoder being recorded; [`Backend::submit`] moves to the next.
    serial: u64,
    /// The highest serial whose work the GPU has finished.
    completed: Arc<AtomicU64>,
    encoder: Option<wgpu::CommandEncoder>,
    /// Submitted ahead of `encoder`: writes that take effect at once but that the queue cannot make
    /// (bytes not in whole words), in call order.
    lead: Option<wgpu::CommandEncoder>,
    pass: Option<draw::Pass>,
    /// Read backs recorded in the current encoder, mapped once it is submitted.
    to_map: Vec<Readback>,
    /// Destroyed objects the current encoder still uses, destroyed once it is submitted.
    destroy_after_submit: Vec<Doomed>,
    blocks: draw::BlockTables,
    caches: draw::Caches,
    pool: memory::BufferPool,
    arena: Option<memory::Arena>,
    edge: Option<edge::EdgeProgram>,
    fills: fill::Fills,
    depth24: Option<Depth24>,
    /// Colour clears reach the API as floats, so integers past 2^24 cannot be cleared exactly.
    float_clears: bool,
    /// Changed whenever a buffer a binding set names moves so that a bind group made with it may no
    /// longer be right; draws rebind nothing while it stays the same.
    bind_epoch: u64,
    /// Host windows the graphics hook registered, by the number it gives them.
    windows: HashMap<u32, wgpu::Surface<'static>>,
    counters: Counters,
    /// The texture usages each format allows on this device, by [`Format::index`].
    usages: [wgpu::TextureUsages; Format::COUNT],
}

#[derive(Debug)]
enum Doomed {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
}

macro_rules! handle {
    ($(#[$m:meta])* $name:ident, $obj:ty) => {
        $(#[$m])*
        #[derive(Clone, Debug)]
        pub struct $name(Rc<$obj>);

        /// Handles are equal when they name the same object.
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.0, &other.0)
            }
        }

        impl Eq for $name {}
    };
}

handle!(
    /// A buffer; its storage may be renamed by `Discard` writes.
    Buffer,
    BufferObj
);
handle!(Texture, TextureObj);
handle!(TextureView, ViewObj);
handle!(Sampler, SamplerObj);
handle!(Shader, ShaderObj);
handle!(ProgramHandle, ProgramObj);
handle!(Blend, draw::Block<BlendDesc>);
handle!(DepthStencil, draw::Block<DepthStencilDesc>);
handle!(Raster, draw::Block<RasterDesc>);
handle!(VertexLayout, draw::Block<VertexLayoutDesc>);
handle!(BindingSet, draw::SetObj);
handle!(QuerySet, QuerySetObj);
handle!(Readback, ReadbackObj);
handle!(Surface, SurfaceObj);

#[derive(Debug)]
struct BufferObj {
    id: u64,
    desc: BufferDesc,
    usage: wgpu::BufferUsages,
    /// The GPU buffer holding its bytes: its own, or the uniform arena's.
    raw: RefCell<wgpu::Buffer>,
    /// Where its bytes start in `raw`: zero unless it lives in the arena.
    base: Cell<u64>,
    /// A number for `raw` that no other GPU buffer had, changed when `raw` is.
    storage: Cell<u64>,
    /// Bumped whenever `raw` or `base` changes, so bindings naming the old place are made again.
    generation: Cell<u64>,
    /// It lives in the uniform arena: uniform-only and written often.
    in_arena: bool,
    /// A binding set names it, so a change of `raw` may change bind groups.
    in_sets: Cell<bool>,
    /// A bind group names it at a fixed offset, so a change of `base` changes that group.
    static_bound: Cell<bool>,
    /// The serial of the last encoder that named it.
    used_in: Cell<u64>,
    /// The serial of the last lead encoder that writes it: queue writes to it then go behind those.
    lead_in: Cell<u64>,
}

#[derive(Debug)]
struct TextureObj {
    desc: TextureDesc,
    raw: wgpu::Texture,
    used_in: Cell<u64>,
}

#[derive(Debug)]
struct ViewObj {
    id: u64,
    format: Format,
    /// None for a surface's texture.
    texture: Option<Texture>,
    /// Every aspect: what a pass draws into.
    raw: wgpu::TextureView,
    /// The aspect the view reads: what a binding names.
    read: wgpu::TextureView,
    width: u32,
    height: u32,
    samples: u32,
    three_d: bool,
}

#[derive(Debug)]
struct SamplerObj {
    id: u64,
    raw: wgpu::Sampler,
    /// Any of its filters is linear.
    filtering: bool,
}

#[derive(Debug)]
struct ShaderObj {
    module: naga::Module,
    info: naga::valid::ModuleInfo,
    entry_index: usize,
    entry: String,
    stage: naga::ShaderStage,
    raw: wgpu::ShaderModule,
    /// Per set, the texture bits of the textures it samples through a sampler.
    paired: [u128; MAX_BINDING_SETS],
}

#[derive(Debug)]
enum ProgramObj {
    Render {
        id: u64,
        vertex: Shader,
        fragment: Option<Shader>,
    },
    Compute {
        id: u64,
        shader: Shader,
    },
}

impl ProgramObj {
    fn id(&self) -> u64 {
        match self {
            ProgramObj::Render { id, .. } | ProgramObj::Compute { id, .. } => *id,
        }
    }

    /// The layout key of each bound set, kept to the textures the program samples through a
    /// sampler: the only bits its layout depends on.
    fn layout_keys(
        &self,
        bindings: &[Option<&BindingSet>; MAX_BINDING_SETS],
    ) -> [BindingLayoutKey; MAX_BINDING_SETS] {
        self.own_keys(bindings.map(|s| s.map_or_else(BindingLayoutKey::default, |s| s.0.key)))
    }

    /// The parts of the sets' layout keys this program's layout depends on: the textures it samples
    /// through a sampler.
    fn own_keys(
        &self,
        sets: [BindingLayoutKey; MAX_BINDING_SETS],
    ) -> [BindingLayoutKey; MAX_BINDING_SETS] {
        let mut keys = [BindingLayoutKey::default(); MAX_BINDING_SETS];
        for shader in self.stages() {
            for ((k, s), paired) in keys.iter_mut().zip(&sets).zip(shader.paired) {
                k.unfilterable_textures |= s.unfilterable_textures & paired;
            }
        }
        keys
    }

    fn stages(&self) -> impl Iterator<Item = &ShaderObj> {
        let (first, second) = match self {
            ProgramObj::Render {
                vertex, fragment, ..
            } => (vertex, fragment.as_ref()),
            ProgramObj::Compute { shader, .. } => (shader, None),
        };
        std::iter::once(&*first.0).chain(second.map(|s| &*s.0))
    }
}

#[derive(Debug)]
struct QuerySetObj {
    kind: QueryKind,
    count: u32,
    raw: wgpu::QuerySet,
}

#[derive(Debug)]
struct ReadbackObj {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    /// The serial of the encoder holding its copy.
    serial: u64,
    /// Bytes to skip at the start, bytes per packed row, bytes per copied row, and rows.
    skip: u64,
    row: u64,
    pitch: u64,
    rows: u64,
}

#[derive(Debug)]
struct SurfaceObj {
    raw: wgpu::Surface<'static>,
    /// The frame handed out by `current_texture`, shown by `present`.
    current: RefCell<Option<Frame>>,
    format: Cell<Format>,
    config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    /// Drawn into while the window shows nothing (occluded, or no texture in time); kept at the
    /// configured size.
    unshown: RefCell<Option<wgpu::Texture>>,
}

/// This frame's texture: the window's, or the unshown one when the window has none to give.
#[derive(Debug)]
enum Frame {
    Shown(wgpu::SurfaceTexture),
    Unshown,
}

/// The encoder a command is recorded in.
#[derive(Clone, Copy, Debug)]
enum Stream {
    /// The commands in the order they were recorded.
    Main,
    /// Ahead of them: writes taking effect at once.
    Lead,
}

/// Never made: the back end has no resource tables.
#[derive(Clone, Debug)]
pub enum NoTable {}

/// A point in the submitted work.
#[derive(Clone, Debug)]
pub struct Fence {
    done: Arc<AtomicBool>,
    index: wgpu::SubmissionIndex,
}

impl WebGpu {
    /// The features this back end uses where the adapter has them, and the adapter's highest limits:
    /// what to make its device with.
    pub fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
        let wanted = wgpu::Features::DEPTH_CLIP_CONTROL
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D
            | wgpu::Features::INDIRECT_FIRST_INSTANCE
            | wgpu::Features::SHADER_F16
            | wgpu::Features::FLOAT32_FILTERABLE
            | wgpu::Features::FLOAT32_BLENDABLE
            | wgpu::Features::DUAL_SOURCE_BLENDING
            | wgpu::Features::CLIP_DISTANCES
            | wgpu::Features::DEPTH32FLOAT_STENCIL8
            | wgpu::Features::RG11B10UFLOAT_RENDERABLE
            | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        wgpu::DeviceDescriptor {
            label: None,
            required_features: adapter.features() & wanted,
            required_limits: adapter.limits(),
            ..Default::default()
        }
    }

    /// Makes the back end on a device made from `adapter`, declaring what that device can do.
    pub fn new(adapter: &wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let lost = Arc::new(Mutex::new(None));
        let on_error = lost.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            eprintln!("webgpu: device error: {e}");
            let reason = match e {
                wgpu::Error::OutOfMemory { .. } => LossReason::OutOfMemory,
                _ => LossReason::Error,
            };
            on_error.lock().unwrap().get_or_insert(reason);
        }));
        let on_lost = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            eprintln!("webgpu: device lost: {message}");
            let reason = match reason {
                wgpu::DeviceLostReason::Destroyed => LossReason::Removed,
                _ => LossReason::Reset,
            };
            on_lost.lock().unwrap().get_or_insert(reason);
        });
        let (mut caps, usages) = caps(adapter, &device);
        // The browser cannot wait for the probe's answer yet (plan 4.8), so there the bit stays clear
        // and fronts bias in the program, which is right for either storage.
        #[cfg(not(target_arch = "wasm32"))]
        let depth24 = Some(probe::run(&device, &queue));
        #[cfg(target_arch = "wasm32")]
        let depth24 = None;
        if let Some(Depth24::Unorm(_)) = depth24 {
            caps.features |= Features::DEPTH24_UNORM;
        }
        // wgpu's Direct3D 12 clears colour targets through 32-bit floats.
        let float_clears = adapter.get_info().backend == wgpu::Backend::Dx12;
        WebGpu {
            device,
            queue,
            caps,
            slow: SlowPaths::default(),
            helpers: Helpers::default(),
            lost,
            next_id: 1,
            serial: 1,
            completed: Arc::new(AtomicU64::new(0)),
            encoder: None,
            lead: None,
            pass: None,
            to_map: Vec::new(),
            destroy_after_submit: Vec::new(),
            blocks: Default::default(),
            caches: Default::default(),
            pool: Default::default(),
            arena: None,
            edge: None,
            fills: Default::default(),
            float_clears,
            bind_epoch: 0,
            windows: HashMap::new(),
            counters: Counters::default(),
            usages,
            depth24,
        }
    }

    /// What the probe of how `depth24plus` is stored found, where it ran.
    pub fn depth24(&self) -> Option<Depth24> {
        self.depth24
    }

    /// Registers a host window for [`Backend::create_surface`] under the number the graphics hook
    /// uses for it.
    pub fn add_window(&mut self, window: u32, surface: wgpu::Surface<'static>) {
        self.windows.insert(window, surface);
    }

    /// What the caches have made so far.
    pub fn counters(&self) -> &Counters {
        &self.counters
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Records a rule the back end could not carry out: the device is lost with `Error`, as plan
    /// section 4.10 says of recording calls.
    fn fail(&self, why: &str) {
        eprintln!("webgpu: {why}");
        self.lost.lock().unwrap().get_or_insert(LossReason::Error);
    }

    fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.stream(Stream::Main)
    }

    fn stream(&mut self, stream: Stream) -> &mut wgpu::CommandEncoder {
        let e = match stream {
            Stream::Main => &mut self.encoder,
            Stream::Lead => &mut self.lead,
        };
        e.get_or_insert_with(|| {
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
        })
    }

    /// Whether the lead encoder being recorded writes this buffer.
    fn lead_pending(&self, b: &BufferObj) -> bool {
        b.lead_in.get() == self.serial && self.lead.is_some()
    }

    /// The view a pass draws into (every aspect) and the view a binding reads (the aspect `stencil`
    /// names, in that aspect's own format); one view where the two are the same.
    fn raw_views(
        &self,
        texture: &TextureObj,
        desc: &TextureViewDesc,
    ) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
        let format = convert::texture_format(desc.format)?;
        let aspect = convert::aspect(desc.stencil, desc.format);
        let view = |aspect, format| {
            texture.raw.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(format),
                dimension: Some(convert::view_dimension(desc.dimension)),
                usage: None,
                aspect,
                base_mip_level: desc.base_mip,
                mip_level_count: Some(desc.mip_count),
                base_array_layer: desc.base_layer,
                array_layer_count: Some(desc.layer_count),
            })
        };
        let whole = view(wgpu::TextureAspect::All, format);
        if aspect == wgpu::TextureAspect::All {
            return Some((whole.clone(), whole));
        }
        let read = view(aspect, format.aspect_specific_format(aspect)?);
        Some((whole, read))
    }
}

/// What the device can do, in the interface's words (plan section 4.11). Only what this back end
/// carries out is declared.
fn caps(
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
) -> (Caps, [wgpu::TextureUsages; Format::COUNT]) {
    let f = device.features();
    let l = device.limits();
    let mut features = Features::COMPUTE
        | Features::VERTEX_STORAGE_READ
        | Features::FRAGMENT_STORAGE_WRITE
        | Features::PROVOKING_VERTEX_FIRST
        | Features::INDEPENDENT_BLEND
        | Features::SAMPLE_SHADING
        | Features::SAMPLE_MASK
        | Features::MULTISAMPLE_READ
        | Features::INDIRECT
        | Features::BASE_VERTEX_INSTANCE
        | Features::TEXTURE_VIEWS
        | Features::SEPARATE_SAMPLERS
        | Features::CLIP_DEPTH_ZERO_ONE
        | Features::DEPTH_BIAS_CLAMP
        | Features::TEXTURE_LEVEL_QUERY;
    for (wgpu_feature, ours) in [
        (wgpu::Features::DEPTH_CLIP_CONTROL, Features::DEPTH_CLAMP),
        (wgpu::Features::CLIP_DISTANCES, Features::CLIP_DISTANCE),
        (
            wgpu::Features::DUAL_SOURCE_BLENDING,
            Features::DUAL_SOURCE_BLEND,
        ),
        (
            wgpu::Features::INDIRECT_FIRST_INSTANCE,
            Features::INDIRECT_FIRST_INSTANCE,
        ),
        (wgpu::Features::SHADER_F16, Features::SHADER_F16),
        (
            wgpu::Features::TEXTURE_COMPRESSION_BC,
            Features::BLOCK_COMPRESSION,
        ),
    ] {
        if f.contains(wgpu_feature) {
            features |= ours;
        }
    }
    // In the browser `prepare`'s pipeline compiles in the GPU process while the page goes on;
    // natively wgpu compiles it at once on the calling thread.
    if cfg!(target_arch = "wasm32") {
        features |= Features::BACKGROUND_PREPARE;
    }
    let adapter_specific = f.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
    let mut formats = FormatCaps {
        uses: [FormatUse::empty(); Format::COUNT],
        samples: [0; Format::COUNT],
    };
    let mut usages = [wgpu::TextureUsages::empty(); Format::COUNT];
    for (i, allowed) in usages.iter_mut().enumerate() {
        let format = format_at(i);
        let Some(raw) = convert::texture_format(format) else {
            continue;
        };
        if !raw.required_features().is_empty() && !f.contains(raw.required_features()) {
            continue;
        }
        let guaranteed = raw.guaranteed_format_features(f);
        let ff = if adapter_specific {
            adapter.get_texture_format_features(raw)
        } else {
            guaranteed
        };
        // Copies follow WebGPU's rules whatever the adapter has: `depth24plus` is never copied.
        let copies = wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST;
        let u = (ff.allowed_usages - copies) | (guaranteed.allowed_usages & copies);
        *allowed = u;
        let flags = ff.flags;
        let mut uses = FormatUse::empty();
        let depth = format.is_depth_stencil();
        let pairs = [
            (
                u.contains(wgpu::TextureUsages::TEXTURE_BINDING),
                FormatUse::SAMPLE,
            ),
            (
                !depth && flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE),
                FormatUse::FILTER,
            ),
            (
                !depth && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
                FormatUse::RENDER,
            ),
            // wgpu marks every filterable float format blendable, renderable or not.
            (
                !depth
                    && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                    && flags.contains(wgpu::TextureFormatFeatureFlags::BLENDABLE),
                FormatUse::BLEND,
            ),
            (
                flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE),
                FormatUse::RESOLVE,
            ),
            (
                flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_READ_ONLY),
                FormatUse::STORAGE_READ,
            ),
            (
                flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY),
                FormatUse::STORAGE_WRITE,
            ),
            (
                depth && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
                FormatUse::DEPTH_STENCIL,
            ),
            (
                u.contains(wgpu::TextureUsages::COPY_SRC),
                FormatUse::COPY_OUT,
            ),
            (
                matches!(
                    format,
                    Format::Bgra8Unorm | Format::Rgba8Unorm | Format::Rgba16Float
                ),
                FormatUse::PRESENT,
            ),
        ];
        for (has, bit) in pairs {
            if has {
                uses |= bit;
            }
        }
        formats.uses[i] = uses;
        let mut samples = 1u8;
        for (flag, bit) in [
            (wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2, 2),
            (wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4, 4),
            (wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X8, 8),
            (wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X16, 16),
        ] {
            if flags.contains(flag) {
                samples |= bit;
            }
        }
        formats.samples[i] = samples;
    }
    let limits = Limits {
        max_texture_2d: l.max_texture_dimension_2d,
        max_texture_cube: l.max_texture_dimension_2d,
        max_texture_3d: l.max_texture_dimension_3d,
        max_texture_layers: l.max_texture_array_layers,
        max_buffer_size: l.max_buffer_size,
        max_uniform_buffer_size: l.max_uniform_buffer_binding_size as u32,
        max_storage_buffer_size: l.max_storage_buffer_binding_size,
        uniform_buffers_per_stage: l.max_uniform_buffers_per_shader_stage,
        textures_per_stage: l.max_sampled_textures_per_shader_stage,
        samplers_per_stage: l.max_samplers_per_shader_stage,
        storage_buffers_per_stage: l.max_storage_buffers_per_shader_stage,
        storage_textures_per_stage: l.max_storage_textures_per_shader_stage,
        vertex_streams: l.max_vertex_buffers,
        vertex_attributes: l.max_vertex_attributes,
        max_vertex_stride: l.max_vertex_buffer_array_stride,
        color_targets: l.max_color_attachments,
        color_bytes_per_sample: l.max_color_attachment_bytes_per_sample,
        inter_stage_slots: l.max_inter_stage_shader_variables,
        min_uniform_offset_alignment: l.min_uniform_buffer_offset_alignment,
        min_storage_offset_alignment: l.min_storage_buffer_offset_alignment,
        buffer_copy_alignment: wgpu::COPY_BUFFER_ALIGNMENT as u32,
        immediate_bytes: 0,
        compute_workgroup_storage: l.max_compute_workgroup_storage_size,
        compute_invocations: l.max_compute_invocations_per_workgroup,
        compute_size: [
            l.max_compute_workgroup_size_x,
            l.max_compute_workgroup_size_y,
            l.max_compute_workgroup_size_z,
        ],
        max_dispatch: l.max_compute_workgroups_per_dimension,
        max_point_size: 1.0,
        max_line_width: 1.0,
        max_anisotropy: 16,
        capture_buffers: 0,
    };
    let info = adapter.get_info();
    let caps = Caps {
        adapter: wcontainers_gpu::AdapterInfo {
            name: info.name,
            vendor: info.vendor,
            device: info.device,
            software: info.device_type == wgpu::DeviceType::Cpu,
        },
        features,
        limits,
        formats,
    };
    (caps, usages)
}

/// The format at position `i` of [`Format`]'s list.
fn format_at(i: usize) -> Format {
    // Every format, in order, so the table indexes match `Format::index`.
    const ALL: [Format; Format::COUNT] = {
        use Format::*;
        [
            R8Unorm,
            R8Snorm,
            R8Uint,
            R8Sint,
            R16Unorm,
            R16Snorm,
            R16Uint,
            R16Sint,
            R16Float,
            Rg8Unorm,
            Rg8Snorm,
            Rg8Uint,
            Rg8Sint,
            R32Uint,
            R32Sint,
            R32Float,
            Rg16Unorm,
            Rg16Snorm,
            Rg16Uint,
            Rg16Sint,
            Rg16Float,
            Rgba8Unorm,
            Rgba8UnormSrgb,
            Rgba8Snorm,
            Rgba8Uint,
            Rgba8Sint,
            Bgra8Unorm,
            Bgra8UnormSrgb,
            Bgrx8Unorm,
            Bgrx8UnormSrgb,
            Rgb10a2Unorm,
            Rgb10a2Uint,
            Rg11b10Float,
            Rgb9e5Float,
            B5g6r5Unorm,
            Bgr5a1Unorm,
            Bgra4Unorm,
            Rgbg8Unorm,
            Grgb8Unorm,
            Rg32Uint,
            Rg32Sint,
            Rg32Float,
            Rgba16Unorm,
            Rgba16Snorm,
            Rgba16Uint,
            Rgba16Sint,
            Rgba16Float,
            Rgb32Uint,
            Rgb32Sint,
            Rgb32Float,
            Rgba32Uint,
            Rgba32Sint,
            Rgba32Float,
            A8Unorm,
            Stencil8,
            Depth16Unorm,
            Depth24Plus,
            Depth24PlusStencil8,
            Depth32Float,
            Depth32FloatStencil8,
            Bc1RgbaUnorm,
            Bc1RgbaUnormSrgb,
            Bc2RgbaUnorm,
            Bc2RgbaUnormSrgb,
            Bc3RgbaUnorm,
            Bc3RgbaUnormSrgb,
            Bc4RUnorm,
            Bc4RSnorm,
            Bc5RgUnorm,
            Bc5RgSnorm,
            Bc6hRgbUfloat,
            Bc6hRgbFloat,
            Bc7RgbaUnorm,
            Bc7RgbaUnormSrgb,
            Etc2Rgb8Unorm,
            Etc2Rgb8UnormSrgb,
            Etc2Rgba8Unorm,
            Etc2Rgba8UnormSrgb,
            Astc4x4Unorm,
            Astc4x4UnormSrgb,
        ]
    };
    ALL[i]
}

/// Every buffer can be copied either way, and written by the edge program of unaligned ranges.
fn buffer_usage(u: BufferUsage) -> wgpu::BufferUsages {
    let mut w =
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::STORAGE;
    for (ours, theirs) in [
        (BufferUsage::VERTEX, wgpu::BufferUsages::VERTEX),
        (BufferUsage::INDEX, wgpu::BufferUsages::INDEX),
        (BufferUsage::UNIFORM, wgpu::BufferUsages::UNIFORM),
        (BufferUsage::STORAGE, wgpu::BufferUsages::STORAGE),
        (BufferUsage::INDIRECT, wgpu::BufferUsages::INDIRECT),
    ] {
        if u.contains(ours) {
            w |= theirs;
        }
    }
    w
}

impl Backend for WebGpu {
    type Buffer = Buffer;
    type Texture = Texture;
    type TextureView = TextureView;
    type Sampler = Sampler;
    type Shader = Shader;
    type Program = ProgramHandle;
    type Blend = Blend;
    type DepthStencil = DepthStencil;
    type Raster = Raster;
    type VertexLayout = VertexLayout;
    type BindingSet = BindingSet;
    type QuerySet = QuerySet;
    type Readback = Readback;
    type Fence = Fence;
    type ResourceTable = NoTable;
    type Surface = Surface;

    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }

    fn status(&mut self) -> DeviceStatus {
        match *self.lost.lock().unwrap() {
            Some(reason) => DeviceStatus::Lost(reason),
            None => DeviceStatus::Ok,
        }
    }

    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Buffer, GpuError> {
        if desc.size > self.caps.limits.max_buffer_size {
            return Err(GpuError::Invalid("buffer larger than max_buffer_size"));
        }
        let in_arena = desc.usage == BufferUsage::UNIFORM && desc.intent == MapIntent::Often;
        let (usage, raw, base, storage) = if in_arena {
            let (raw, base, storage) = self.arena_slice(desc.size);
            (memory::arena_usage(), raw, base, storage)
        } else {
            let usage = buffer_usage(desc.usage);
            let raw = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: desc.size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage,
                mapped_at_creation: false,
            });
            (usage, raw, 0, self.id())
        };
        Ok(Buffer(Rc::new(BufferObj {
            id: self.id(),
            desc: *desc,
            usage,
            raw: RefCell::new(raw),
            base: Cell::new(base),
            storage: Cell::new(storage),
            generation: Cell::new(0),
            in_arena,
            in_sets: Cell::new(false),
            static_bound: Cell::new(false),
            used_in: Cell::new(0),
            lead_in: Cell::new(0),
        })))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Texture, GpuError> {
        let format = convert::texture_format(desc.format)
            .filter(|_| !self.caps.formats.get(desc.format).is_empty())
            .ok_or(GpuError::Invalid(
                "texture format this back end does not hold",
            ))?;
        let mut usage = wgpu::TextureUsages::empty();
        for (ours, theirs) in [
            (TextureUsage::SAMPLE, wgpu::TextureUsages::TEXTURE_BINDING),
            (TextureUsage::RENDER, wgpu::TextureUsages::RENDER_ATTACHMENT),
            (
                TextureUsage::PRESENT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            (TextureUsage::STORAGE, wgpu::TextureUsages::STORAGE_BINDING),
            (TextureUsage::COPY_SRC, wgpu::TextureUsages::COPY_SRC),
            (TextureUsage::COPY_DST, wgpu::TextureUsages::COPY_DST),
        ] {
            if desc.usage.contains(ours) {
                usage |= theirs;
            }
        }
        // Read backs and the staging route of ordered writes copy in and out of every texture whose
        // format allows it.
        let copies = wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST;
        let allowed = self.usages[desc.format.index()];
        usage |= copies & allowed;
        // Depth copies WebGPU has no copy for are draws: from a bound source into an attachment.
        if desc.usage.contains(TextureUsage::COPY_SRC) && fill::DepthSource::readable(desc.format) {
            usage |= allowed & wgpu::TextureUsages::TEXTURE_BINDING;
        }
        if desc.usage.contains(TextureUsage::COPY_DST) && desc.format.is_depth_stencil() {
            usage |= allowed & wgpu::TextureUsages::RENDER_ATTACHMENT;
        }
        let srgb_twin = (format.remove_srgb_suffix() != format)
            .then(|| format.remove_srgb_suffix())
            .or_else(|| (format.add_srgb_suffix() != format).then(|| format.add_srgb_suffix()));
        let raw = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: desc.width,
                height: desc.height,
                depth_or_array_layers: desc.depth_or_layers,
            },
            mip_level_count: desc.mip_levels,
            sample_count: desc.samples,
            dimension: match desc.dimension {
                TextureDimension::D2 => wgpu::TextureDimension::D2,
                TextureDimension::D3 => wgpu::TextureDimension::D3,
            },
            format,
            usage,
            view_formats: srgb_twin.as_slice(),
        });
        Ok(Texture(Rc::new(TextureObj {
            desc: *desc,
            raw,
            used_in: Cell::new(0),
        })))
    }

    fn create_view(
        &mut self,
        texture: &Texture,
        desc: &TextureViewDesc,
    ) -> Result<TextureView, GpuError> {
        let (raw, read) = self.raw_views(&texture.0, desc).ok_or(GpuError::Invalid(
            "view format or aspect this back end does not hold",
        ))?;
        let t = &texture.0.desc;
        let size = |n: u32| (n >> desc.base_mip).max(1);
        Ok(TextureView(Rc::new(ViewObj {
            id: self.id(),
            format: desc.format,
            texture: Some(texture.clone()),
            raw,
            read,
            width: size(t.width),
            height: size(t.height),
            samples: t.samples,
            three_d: t.dimension == TextureDimension::D3,
        })))
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Sampler, GpuError> {
        let mut address = [wgpu::AddressMode::ClampToEdge; 3];
        for (a, ours) in address.iter_mut().zip(desc.address) {
            *a = convert::address(ours).ok_or(GpuError::Absent(match ours {
                wcontainers_gpu::AddressMode::MirrorOnce => Features::MIRROR_ONCE,
                _ => Features::BORDER_COLOR,
            }))?;
        }
        let raw = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: address[0],
            address_mode_v: address[1],
            address_mode_w: address[2],
            mag_filter: convert::filter(desc.mag),
            min_filter: convert::filter(desc.min),
            mipmap_filter: convert::mip_filter(desc.mip),
            lod_min_clamp: f32::from_bits(desc.lod_range[0]).max(0.0),
            lod_max_clamp: f32::from_bits(desc.lod_range[1]).min(32.0),
            compare: desc.compare.map(convert::compare),
            // Anisotropy applies only to fully linear filtering, in WebGPU as in Direct3D.
            anisotropy_clamp: if [desc.min, desc.mag, desc.mip].contains(&Filter::Nearest) {
                1
            } else {
                desc.anisotropy.clamp(1, 16)
            },
            border_color: None,
        });
        Ok(Sampler(Rc::new(SamplerObj {
            id: self.id(),
            raw,
            filtering: [desc.min, desc.mag, desc.mip].contains(&Filter::Linear),
        })))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Shader, GpuError> {
        let stage = match module.stage {
            ShaderStage::Vertex => naga::ShaderStage::Vertex,
            ShaderStage::Fragment => naga::ShaderStage::Fragment,
            ShaderStage::Compute => naga::ShaderStage::Compute,
        };
        let entry_index = module
            .module
            .entry_points
            .iter()
            .position(|e| e.name == module.entry_point && e.stage == stage)
            .ok_or(GpuError::Invalid("the module has no such entry point"))?;
        // Checked against what WebGPU's core language and this device's features allow (plan 6.3),
        // so a program the browser would refuse is refused here too, natively as well.
        let info = wgpu_naga_bridge::create_validator(
            self.device.features(),
            wgpu::DownlevelFlags::compliant(),
            naga::valid::ValidationFlags::all(),
        )
        .validate(&module.module)
        .map_err(|e| {
            eprintln!("webgpu: program refused: {}", e.as_inner());
            GpuError::Invalid("a program WebGPU's language and this device's features do not allow")
        })?;
        let raw = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Naga(Cow::Owned(module.module.clone())),
            });
        Ok(Shader(Rc::new(ShaderObj {
            module: module.module.clone(),
            paired: layout::paired(&module.module, info.get_entry_point(entry_index)),
            info,
            entry_index,
            entry: module.entry_point.clone(),
            stage,
            raw,
        })))
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<ProgramHandle, GpuError> {
        let id = self.id();
        Ok(ProgramHandle(Rc::new(match program {
            Program::Render { vertex, fragment } => ProgramObj::Render {
                id,
                vertex: (*vertex).clone(),
                fragment: fragment.cloned(),
            },
            Program::Compute(shader) => ProgramObj::Compute {
                id,
                shader: (*shader).clone(),
            },
        })))
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Blend, GpuError> {
        if desc.logic_op.is_some() {
            return Err(GpuError::Absent(Features::LOGIC_OP));
        }
        let factors = desc
            .targets
            .iter()
            .filter_map(|t| t.blend)
            .flat_map(|(c, a)| [c.src, c.dst, a.src, a.dst]);
        let dual = self.caps.features.contains(Features::DUAL_SOURCE_BLEND);
        for f in factors {
            match convert::blend_factor(f) {
                None => return Err(GpuError::Absent(Features::BLEND_CONSTANT_ALPHA)),
                Some(
                    wgpu::BlendFactor::Src1
                    | wgpu::BlendFactor::OneMinusSrc1
                    | wgpu::BlendFactor::Src1Alpha
                    | wgpu::BlendFactor::OneMinusSrc1Alpha,
                ) if !dual => return Err(GpuError::Absent(Features::DUAL_SOURCE_BLEND)),
                Some(_) => {}
            }
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.blends.get(desc, id);
        if made {
            self.next_id = id;
        }
        Ok(Blend(block))
    }

    fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<DepthStencil, GpuError> {
        if desc.depth_bounds.is_some() {
            return Err(GpuError::Absent(Features::DEPTH_BOUNDS));
        }
        if desc.stencil_read_mask[0] != desc.stencil_read_mask[1]
            || desc.stencil_write_mask[0] != desc.stencil_write_mask[1]
        {
            return Err(GpuError::Absent(Features::STENCIL_PER_FACE));
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.depth_stencils.get(desc, id);
        if made {
            self.next_id = id;
        }
        Ok(DepthStencil(block))
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Raster, GpuError> {
        let absent = [
            (desc.fill != FillMode::Solid, Features::POLYGON_MODE),
            (
                desc.provoking == ProvokingVertex::Last,
                Features::PROVOKING_VERTEX_LAST,
            ),
            (desc.depth_minus_one, Features::CLIP_DEPTH_MINUS_ONE),
            (f32::from_bits(desc.line_width) != 1.0, Features::WIDE_LINES),
            (
                !desc.depth_clip && !self.caps.has(Features::DEPTH_CLAMP),
                Features::DEPTH_CLAMP,
            ),
            (
                f32::from_bits(desc.depth_bias).fract() != 0.0,
                Features::FRACTIONAL_DEPTH_BIAS,
            ),
        ];
        if let Some((_, feature)) = absent.iter().find(|(missing, _)| *missing) {
            return Err(GpuError::Absent(*feature));
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.rasters.get(desc, id);
        if made {
            self.next_id = id;
        }
        Ok(Raster(block))
    }

    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<VertexLayout, GpuError> {
        for s in desc.streams.iter().flatten() {
            if !matches!(s.step, StepMode::Vertex | StepMode::Instance(1)) {
                return Err(GpuError::Invalid(
                    "instance step rates other than one are fetched in the program",
                ));
            }
        }
        for a in desc.attributes.iter().flatten() {
            if convert::vertex_format(a.format).is_none() {
                self.fail(
                    "a vertex format WebGPU lacks, which this back end converts (VertexFormatConverted), is not built yet",
                );
                return Err(GpuError::DeviceLost);
            }
            if desc
                .streams
                .get(a.stream as usize)
                .copied()
                .flatten()
                .is_none()
            {
                return Err(GpuError::Invalid("attribute of a stream the layout lacks"));
            }
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.vertex_layouts.get(desc, id);
        if made {
            self.next_id = id;
        }
        Ok(VertexLayout(block))
    }

    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<BindingSet, GpuError> {
        self.binding_set(entries)
    }

    fn binding_layout_key(&self, set: &BindingSet) -> BindingLayoutKey {
        set.0.key
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<QuerySet, GpuError> {
        let ty = match kind {
            QueryKind::OcclusionAny => wgpu::QueryType::Occlusion,
            QueryKind::Occlusion => return Err(GpuError::Absent(Features::OCCLUSION_COUNT)),
            QueryKind::Timestamp => return Err(GpuError::Absent(Features::TIMESTAMPS)),
        };
        let raw = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty,
            count,
        });
        Ok(QuerySet(Rc::new(QuerySetObj { kind, count, raw })))
    }

    fn destroy(&mut self, object: Object<Self>) {
        let pending = |used_in: u64| used_in == self.serial && self.encoder.is_some();
        match object {
            Object::Buffer(b) if b.0.in_arena => self.release_slice(&b.0),
            Object::Buffer(b) => {
                let raw = b.0.raw.borrow().clone();
                if pending(b.0.used_in.get()) || self.lead_pending(&b.0) {
                    self.destroy_after_submit.push(Doomed::Buffer(raw));
                } else {
                    raw.destroy();
                }
            }
            Object::Texture(t) => {
                if pending(t.0.used_in.get()) {
                    self.destroy_after_submit
                        .push(Doomed::Texture(t.0.raw.clone()));
                } else {
                    t.0.raw.destroy();
                }
            }
            Object::QuerySet(_)
            | Object::TextureView(_)
            | Object::Sampler(_)
            | Object::Shader(_)
            | Object::Program(_)
            | Object::Blend(_)
            | Object::DepthStencil(_)
            | Object::Raster(_)
            | Object::VertexLayout(_)
            | Object::BindingSet(_)
            | Object::Surface(_) => {}
            Object::ResourceTable(t) => match t {},
        }
    }

    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8], mode: WriteMode) {
        self.write_buffer_impl(buffer, offset, data, mode);
    }

    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        self.write_texture_impl(region, extent, data, row_pitch, rows_per_layer, mode);
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        self.begin_pass_impl(pass);
    }

    fn end_pass(&mut self) {
        if self.pass.take().is_none() {
            self.fail("end_pass without a pass");
        }
    }

    fn prepare(&mut self, desc: &wcontainers_gpu::PrepareDesc<'_, Self>) {
        self.prepare_impl(desc);
    }

    fn draw(&mut self, call: &wcontainers_gpu::DrawCall<'_, Self>) {
        self.draw_impl(call);
    }

    fn begin_query(&mut self, index: u32) {
        match &mut self.pass {
            Some(p) if p.occlusion && !p.query_running => {
                p.rp.begin_occlusion_query(index);
                p.query_running = true;
            }
            _ => self.fail("begin_query outside a pass with an occlusion set, or nested"),
        }
    }

    fn end_query(&mut self) {
        match &mut self.pass {
            Some(p) if p.query_running => {
                p.rp.end_occlusion_query();
                p.query_running = false;
            }
            _ => self.fail("end_query without a running query"),
        }
    }

    fn copy_buffer(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        self.copy_buffer_impl(src, src_offset, dst, dst_offset, size);
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.copy_texture_impl(src, dst, extent);
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.copy_buffer_texture(src, dst, extent, true);
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        self.copy_buffer_texture(dst, src, extent, false);
    }

    fn clear_buffer(&mut self, buffer: &Buffer, offset: u64, size: u64, pattern: &[u8]) {
        self.clear_buffer_impl(buffer, offset, size, pattern);
    }

    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        self.resolve_impl(src, dst, format);
    }

    fn submit(&mut self) -> Fence {
        if self.pass.is_some() {
            self.fail("submit inside a pass");
            self.pass = None;
        }
        let commands = [self.lead.take(), self.encoder.take()]
            .into_iter()
            .flatten()
            .map(|e| e.finish());
        let index = self.queue.submit(commands);
        if let Some(a) = &mut self.arena {
            a.submitted();
        }
        self.caches.prune();
        for rb in self.to_map.drain(..) {
            let state = rb.0.state.clone();
            rb.0.buffer
                .map_async(wgpu::MapMode::Read, .., move |result| {
                    let s = if result.is_ok() {
                        MAP_READY
                    } else {
                        MAP_FAILED
                    };
                    state.store(s, Ordering::Release);
                });
        }
        for d in self.destroy_after_submit.drain(..) {
            match d {
                Doomed::Buffer(b) => b.destroy(),
                Doomed::Texture(t) => t.destroy(),
            }
        }
        let done = Arc::new(AtomicBool::new(false));
        let (flag, completed, serial) = (done.clone(), self.completed.clone(), self.serial);
        self.queue.on_submitted_work_done(move || {
            flag.store(true, Ordering::Release);
            completed.fetch_max(serial, Ordering::AcqRel);
        });
        self.serial += 1;
        Fence { done, index }
    }

    fn fence_done(&mut self, fence: &Fence) -> bool {
        if !fence.done.load(Ordering::Acquire) {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        fence.done.load(Ordering::Acquire)
    }

    fn wait_fence(&mut self, fence: &Fence) {
        self.block_until(
            || fence.done.load(Ordering::Acquire),
            Some(fence.index.clone()),
        );
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Readback, GpuError> {
        self.read_back_impl(src)
    }

    fn read_queries(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Readback, GpuError> {
        self.read_queries_impl(set, first, count)
    }

    fn read_state(&mut self, readback: &Readback) -> ReadState {
        if readback.0.state.load(Ordering::Acquire) == MAP_PENDING {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        match readback.0.state.load(Ordering::Acquire) {
            MAP_PENDING => ReadState::Pending,
            _ => ReadState::Ready,
        }
    }

    fn wait_read(&mut self, readback: &Readback) {
        if readback.0.serial == self.serial {
            self.submit();
        }
        let state = readback.0.state.clone();
        self.block_until(move || state.load(Ordering::Acquire) != MAP_PENDING, None);
    }

    fn finish_read(&mut self, readback: Readback, dst: &mut [u8]) {
        self.finish_read_impl(readback, dst);
    }

    fn create_surface(&mut self, window: u32) -> Result<Surface, GpuError> {
        let raw = self
            .windows
            .remove(&window)
            .ok_or(GpuError::Invalid("no window of that number was registered"))?;
        Ok(Surface(Rc::new(SurfaceObj {
            raw,
            current: RefCell::new(None),
            format: Cell::new(Format::Rgba8Unorm),
            config: RefCell::new(None),
            unshown: RefCell::new(None),
        })))
    }

    fn configure_surface(
        &mut self,
        surface: &Surface,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        if !self.caps.formats.get(format).contains(FormatUse::PRESENT) {
            return Err(GpuError::Invalid("surface format without PRESENT"));
        }
        let raw = convert::texture_format(format).ok_or(GpuError::Invalid("surface format"))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: raw,
            width,
            height,
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        surface.0.raw.configure(&self.device, &config);
        surface.0.format.set(format);
        *surface.0.config.borrow_mut() = Some(config);
        *surface.0.unshown.borrow_mut() = None;
        Ok(())
    }

    fn current_texture(&mut self, surface: &Surface) -> Result<TextureView, GpuError> {
        let config = surface.0.config.borrow().clone().ok_or(GpuError::Invalid(
            "current_texture before configure_surface",
        ))?;
        use wgpu::CurrentSurfaceTexture as C;
        let mut got = surface.0.raw.get_current_texture();
        if matches!(got, C::Outdated) {
            // The window changed under the configuration: configure it again as it was.
            surface.0.raw.configure(&self.device, &config);
            got = surface.0.raw.get_current_texture();
        }
        let (texture, width, height) = match got {
            C::Success(t) | C::Suboptimal(t) => {
                let view = t.texture.create_view(&Default::default());
                let size = (t.texture.width(), t.texture.height());
                *surface.0.current.borrow_mut() = Some(Frame::Shown(t));
                (view, size.0, size.1)
            }
            // The window shows nothing now, as a real swap chain's occluded window: the frame is
            // drawn into a texture of the same size and not shown.
            C::Timeout | C::Occluded | C::Outdated => {
                let device = &self.device;
                let view = surface
                    .0
                    .unshown
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        device.create_texture(&wgpu::TextureDescriptor {
                            label: None,
                            size: wgpu::Extent3d {
                                width: config.width,
                                height: config.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: config.format,
                            usage: config.usage,
                            view_formats: &[],
                        })
                    })
                    .create_view(&Default::default());
                *surface.0.current.borrow_mut() = Some(Frame::Unshown);
                (view, config.width, config.height)
            }
            C::Lost => {
                self.fail("the window's surface was lost; making it again is not built yet");
                return Err(GpuError::DeviceLost);
            }
            C::Validation => {
                self.fail("getting the window's texture failed validation");
                return Err(GpuError::DeviceLost);
            }
        };
        Ok(TextureView(Rc::new(ViewObj {
            id: self.id(),
            format: surface.0.format.get(),
            texture: None,
            raw: texture.clone(),
            read: texture,
            width,
            height,
            samples: 1,
            three_d: false,
        })))
    }

    fn present(&mut self, surface: &Surface) -> Result<(), GpuError> {
        self.submit();
        let frame = surface
            .0
            .current
            .borrow_mut()
            .take()
            .ok_or(GpuError::Invalid("present without current_texture"))?;
        if let Frame::Shown(texture) = frame {
            self.queue.present(texture);
        }
        Ok(())
    }

    fn resolve_queries(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
        dst: &Buffer,
        dst_offset: u64,
    ) -> Result<(), GpuError> {
        self.resolve_queries_impl(set, first, count, dst, dst_offset)
    }

    fn dispatch(&mut self, call: &wcontainers_gpu::DispatchCall<'_, Self>) -> Result<(), GpuError> {
        self.dispatch_impl(call)
    }
}

impl WebGpu {
    /// Pauses the device thread until `done` holds. Natively by waiting on the device; in the browser
    /// the device thread suspends through JSPI (plan section 4.8), which the call channel brings.
    fn block_until(&mut self, done: impl Fn() -> bool, index: Option<wgpu::SubmissionIndex>) {
        #[cfg(not(target_arch = "wasm32"))]
        while !done() && self.status() == DeviceStatus::Ok {
            let _ = self.device.poll(wgpu::PollType::Wait {
                submission_index: index.clone(),
                timeout: None,
            });
        }
        #[cfg(target_arch = "wasm32")]
        if !done() {
            let _ = index;
            panic!(
                "webgpu: waiting in the browser needs the JSPI suspension of plan section 4.8, not built yet; poll instead"
            );
        }
    }
}

/// Whether a format has stencil (true) or is a depth format.
fn has_stencil(format: Format) -> bool {
    matches!(
        format.kind(),
        FormatKind::DepthStencil | FormatKind::Stencil
    )
}

/// Bytes of one row of blocks and rows of blocks of `extent` in `format`.
fn row_bytes(format: Format, width: u32, height: u32) -> (u64, u64) {
    let (bw, bh, bytes) = format.block();
    (
        u64::from(width.div_ceil(bw)) * u64::from(bytes),
        u64::from(height.div_ceil(bh)),
    )
}
