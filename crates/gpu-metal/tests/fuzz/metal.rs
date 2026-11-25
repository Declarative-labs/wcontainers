//! The batches that drive every method of the Metal trait on `DirectMetal` with random arguments.

use raw_window_handle::{HandleError, HasWindowHandle, WindowHandle};
use wcontainers_metal::{
    BlendFactor, BlendOperation, Blending, BlitCommand, BlitOption, ColorWriteMask,
    CommandBufferStatus, CompareFunction, ComputeCommand, CullMode, DepthClipMode,
    DepthStencilDescriptor, FunctionSource, IndexType, LayerDescriptor, LoadAction,
    MAX_COLOR_ATTACHMENTS, Metal, Origin, PixelFormat, PrimitiveTopologyClass, PrimitiveType,
    RenderCommand, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, RenderPipelineColorAttachmentDescriptor, RenderPipelineDescriptor,
    ResolveTarget, ResourceSlot, SamplerAddressMode, SamplerBorderColor, SamplerDescriptor,
    SamplerMinMagFilter, SamplerMipFilter, ScissorRect, Size, StandaloneMetal, StencilDescriptor,
    StencilOperation, StoreAction, TextureDescriptor, TextureType, TextureUsage,
    TextureViewDescriptor, TriangleFillMode, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexStepFunction, Viewport, VisibilityResultMode, Winding,
};
use wcontainers_metal_direct::DirectMetal;
use wcontainers_test_window::StandaloneTestWindow;
use wcontainers_test_window_appkit::AppkitTestWindow;

use crate::driver::Counts;
use crate::random::Random;
use crate::shadow::{Shadow, Tracked};
use crate::{LARGEST_MADE, POOL, TEXTURE_BUDGET};

type M = DirectMetal;
type Buffer = <M as Metal>::Buffer;
type Texture = <M as Metal>::Texture;

/// A buffer the batch made, with what it must still hold.
#[derive(Clone)]
struct Held {
    buffer: Buffer,
    shadow: Tracked,
}

/// A range of a buffer that a call may change, once Metal takes the call.
type Allowed = (Tracked, u64, u64);

/// Returns the texels of one block of a format on a side, and its bytes, as a copy into a buffer writes them.
///
/// A depth and stencil format copies one aspect; its widest is taken. https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
fn block(format: PixelFormat) -> (u32, u64) {
    use PixelFormat as P;
    match format {
        P::A8Unorm
        | P::R8Unorm
        | P::R8UnormSrgb
        | P::R8Snorm
        | P::R8Uint
        | P::R8Sint
        | P::Stencil8 => (1, 1),
        P::R16Unorm
        | P::R16Snorm
        | P::R16Uint
        | P::R16Sint
        | P::R16Float
        | P::Rg8Unorm
        | P::Rg8Snorm
        | P::Rg8Uint
        | P::Rg8Sint
        | P::B5g6r5Unorm
        | P::A1bgr5Unorm
        | P::Abgr4Unorm
        | P::Bgr5a1Unorm
        | P::Depth16Unorm => (1, 2),
        P::Rg32Uint
        | P::Rg32Sint
        | P::Rg32Float
        | P::Rgba16Unorm
        | P::Rgba16Snorm
        | P::Rgba16Uint
        | P::Rgba16Sint
        | P::Rgba16Float
        | P::Depth32FloatStencil8
        | P::X32Stencil8 => (1, 8),
        P::Rgba32Uint | P::Rgba32Sint | P::Rgba32Float => (1, 16),
        P::Bc1Rgba
        | P::Bc1RgbaSrgb
        | P::Bc4RUnorm
        | P::Bc4RSnorm
        | P::Etc2Rgb8
        | P::Etc2Rgb8Srgb => (4, 8),
        P::Bc2Rgba
        | P::Bc2RgbaSrgb
        | P::Bc3Rgba
        | P::Bc3RgbaSrgb
        | P::Bc5RgUnorm
        | P::Bc5RgSnorm
        | P::Bc6hRgbFloat
        | P::Bc6hRgbUfloat
        | P::Bc7RgbaUnorm
        | P::Bc7RgbaUnormSrgb
        | P::EacRgba8
        | P::EacRgba8Srgb
        | P::Astc4x4Ldr
        | P::Astc4x4Srgb => (4, 16),
        _ => (1, 4),
    }
}

/// Returns the rows of blocks a copy between a buffer and a texture region touches in the buffer: one range a row.
fn copy_rows(
    format: PixelFormat,
    offset: u64,
    bytes_per_row: u64,
    bytes_per_image: u64,
    size: Size,
) -> Vec<(u64, u64)> {
    let (side, bytes) = block(format);
    let row = u64::from(size.width.div_ceil(side)).saturating_mul(bytes);
    let rows = size.height.div_ceil(side);
    // A copy of many rows is refused by its range check long before; past 4096 rows only the whole span is noted.
    if u64::from(rows).saturating_mul(u64::from(size.depth)) > 4096 {
        return vec![(offset, u64::MAX)];
    }
    let mut ranges = Vec::new();
    for image in 0..size.depth {
        for at in 0..rows {
            let start = offset
                .saturating_add(u64::from(image).saturating_mul(bytes_per_image))
                .saturating_add(u64::from(at).saturating_mul(bytes_per_row));
            ranges.push((start, row));
        }
    }
    ranges
}

/// The functions the batches compile: every kind of resource bound, read and written at indices the data chooses.
const PROGRAMS: &str = "
struct Color { value: vec4<f32> }
struct Tail { head: u32, rest: array<u32> }
@group(0) @binding(0) var<uniform> color: Color;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> sink: array<u32>;
@group(0) @binding(3) var<storage, read_write> tail: Tail;
@group(0) @binding(4) var samp: sampler;
@group(0) @binding(5) var cmp: sampler_comparison;
@group(0) @binding(6) var tex_f: texture_2d<f32>;
@group(0) @binding(7) var tex_u: texture_2d<u32>;
@group(0) @binding(8) var tex_i: texture_2d<i32>;
@group(0) @binding(9) var tex_a: texture_2d_array<f32>;
@group(0) @binding(10) var tex_c: texture_cube<f32>;
@group(0) @binding(11) var tex_3: texture_3d<f32>;
@group(0) @binding(12) var tex_d: texture_depth_2d;
@group(0) @binding(13) var tex_m: texture_multisampled_2d<f32>;
var<workgroup> shared_words: array<u32, 64>;

@vertex fn vs_plain(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@vertex fn vs_pulled(@builtin(vertex_index) i: u32, @builtin(instance_index) n: u32) -> @builtin(position) vec4<f32> {
    let w = source[i * 7u + n];
    return vec4<f32>(f32(w & 3u) - 1.0, f32((w >> 2u) & 3u) - 1.0, 0.5, 1.0);
}
@vertex fn vs_inputs(@location(0) p: vec4<f32>, @location(1) q: vec4<u32>, @location(2) r: vec2<i32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p.xy + vec2<f32>(f32(q.x & 1u), f32(r.y & 1)), 0.5, 1.0);
}
@fragment fn fs_color() -> @location(0) vec4<f32> {
    return color.value;
}
@fragment fn fs_uint() -> @location(0) vec4<u32> {
    return vec4<u32>(7u);
}
@fragment fn fs_sint() -> @location(0) vec4<i32> {
    return vec4<i32>(-7);
}
struct Two { @location(0) a: vec4<f32>, @location(1) b: vec4<f32> }
@fragment fn fs_two() -> Two {
    return Two(color.value, vec4<f32>(0.25));
}
@fragment fn fs_write(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let i = source[u32(at.x)] ^ u32(at.y);
    sink[i] = i;
    tail.rest[i * 3u] = tail.head;
    return vec4<f32>(1.0);
}
@fragment fn fs_textures(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let c = vec2<i32>(at.xy) + vec2<i32>(i32(source[0]), i32(source[1]));
    var v = textureSample(tex_f, samp, at.xy) + vec4<f32>(textureLoad(tex_u, c, i32(source[2])));
    v += vec4<f32>(textureLoad(tex_i, c, 0)) + textureSample(tex_a, samp, at.xy, i32(source[3]));
    v += textureSample(tex_c, samp, at.xyz) + textureLoad(tex_3, vec3<i32>(c, i32(source[4])), 0);
    v += vec4<f32>(textureSampleCompare(tex_d, cmp, at.xy, 0.5)) + textureLoad(tex_m, c, i32(source[5]));
    return v;
}
@fragment fn fs_sample_f(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex_f, samp, at.xy / 4.0);
}
@fragment fn fs_load_u(@builtin(position) at: vec4<f32>) -> @location(0) vec4<u32> {
    return textureLoad(tex_u, vec2<i32>(at.xy) * i32(source[0]), i32(source[1]));
}
@compute @workgroup_size(64) fn cs_scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = source[id.x];
    sink[i] = id.x;
    tail.rest[i ^ 0x55u] = tail.head + i;
}
@compute @workgroup_size(8, 8, 1) fn cs_shared(@builtin(local_invocation_index) l: u32, @builtin(global_invocation_id) id: vec3<u32>) {
    shared_words[l] = source[id.x * 8u + id.y];
    workgroupBarrier();
    sink[shared_words[63u - l]] = shared_words[l ^ 1u];
}
@group(0) @binding(14) var st: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(15) var tex_1: texture_1d<f32>;
var<workgroup> big_words: array<u32, 16384>;
var<immediate> imm: vec4<f32>;
@fragment fn fs_store(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    textureStore(st, vec2<i32>(at.xy), vec4<f32>(1.0));
    return vec4<f32>(1.0);
}
@fragment fn fs_line(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(tex_1, i32(at.x), 0);
}
@fragment fn fs_imm() -> @location(0) vec4<f32> {
    return imm;
}
@compute @workgroup_size(1024, 2) fn cs_wide(@builtin(global_invocation_id) id: vec3<u32>) {
    sink[id.x] = id.y;
}
@compute @workgroup_size(64) fn cs_big(@builtin(local_invocation_index) l: u32) {
    big_words[l * 256u] = l;
    workgroupBarrier();
    sink[l] = big_words[(l ^ 1u) * 256u];
}
@compute @workgroup_size(1) fn cs_textures(@builtin(global_invocation_id) id: vec3<u32>) {
    let c = vec2<i32>(i32(source[0]), i32(source[1]));
    sink[id.x] = textureLoad(tex_u, c, i32(source[2])).x + u32(textureLoad(tex_i, c, 0).y);
    sink[id.x + 1u] = u32(textureLoad(tex_3, vec3<i32>(c, 2), 0).x * 255.0);
}
";

/// The entry points of the programs, with their stage.
const ENTRIES: &[(&str, naga::ShaderStage)] = &[
    ("vs_plain", naga::ShaderStage::Vertex),
    ("vs_pulled", naga::ShaderStage::Vertex),
    ("vs_inputs", naga::ShaderStage::Vertex),
    ("fs_color", naga::ShaderStage::Fragment),
    ("fs_uint", naga::ShaderStage::Fragment),
    ("fs_sint", naga::ShaderStage::Fragment),
    ("fs_two", naga::ShaderStage::Fragment),
    ("fs_write", naga::ShaderStage::Fragment),
    ("fs_textures", naga::ShaderStage::Fragment),
    ("fs_sample_f", naga::ShaderStage::Fragment),
    ("fs_load_u", naga::ShaderStage::Fragment),
    ("cs_scatter", naga::ShaderStage::Compute),
    ("cs_shared", naga::ShaderStage::Compute),
    ("cs_textures", naga::ShaderStage::Compute),
    ("fs_store", naga::ShaderStage::Fragment),
    ("fs_line", naga::ShaderStage::Fragment),
    ("fs_imm", naga::ShaderStage::Fragment),
    ("cs_wide", naga::ShaderStage::Compute),
    ("cs_big", naga::ShaderStage::Compute),
];

/// The slot of each binding of the programs: buffers 0 to 3, samplers 0 and 1, textures 0 to 9.
const SLOTS: &[(u32, u32)] = &[
    (0, 0),
    (1, 1),
    (2, 2),
    (3, 3),
    (4, 0),
    (5, 1),
    (6, 0),
    (7, 1),
    (8, 2),
    (9, 3),
    (10, 4),
    (11, 5),
    (12, 6),
    (13, 7),
    (14, 8),
    (15, 9),
];

/// The buffer slots of the vertex inputs, the immediates and the lengths, the fuzzer's own among Metal's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const VERTEX_SLOTS: [u32; 2] = [20, 21];
const IMMEDIATES_AT: u32 = 27;
const LENGTHS_AT: u32 = 28;

/// Every pixel format.
const FORMATS: &[PixelFormat] = &[
    PixelFormat::A8Unorm,
    PixelFormat::R8Unorm,
    PixelFormat::R8UnormSrgb,
    PixelFormat::R8Snorm,
    PixelFormat::R8Uint,
    PixelFormat::R8Sint,
    PixelFormat::R16Unorm,
    PixelFormat::R16Snorm,
    PixelFormat::R16Uint,
    PixelFormat::R16Sint,
    PixelFormat::R16Float,
    PixelFormat::Rg8Unorm,
    PixelFormat::Rg8Snorm,
    PixelFormat::Rg8Uint,
    PixelFormat::Rg8Sint,
    PixelFormat::B5g6r5Unorm,
    PixelFormat::A1bgr5Unorm,
    PixelFormat::Abgr4Unorm,
    PixelFormat::Bgr5a1Unorm,
    PixelFormat::R32Uint,
    PixelFormat::R32Sint,
    PixelFormat::R32Float,
    PixelFormat::Rg16Unorm,
    PixelFormat::Rg16Snorm,
    PixelFormat::Rg16Uint,
    PixelFormat::Rg16Sint,
    PixelFormat::Rg16Float,
    PixelFormat::Rgba8Unorm,
    PixelFormat::Rgba8UnormSrgb,
    PixelFormat::Rgba8Snorm,
    PixelFormat::Rgba8Uint,
    PixelFormat::Rgba8Sint,
    PixelFormat::Bgra8Unorm,
    PixelFormat::Bgra8UnormSrgb,
    PixelFormat::Rgb10a2Unorm,
    PixelFormat::Rgb10a2Uint,
    PixelFormat::Rg11b10Float,
    PixelFormat::Rgb9e5Float,
    PixelFormat::Rg32Uint,
    PixelFormat::Rg32Sint,
    PixelFormat::Rg32Float,
    PixelFormat::Rgba16Unorm,
    PixelFormat::Rgba16Snorm,
    PixelFormat::Rgba16Uint,
    PixelFormat::Rgba16Sint,
    PixelFormat::Rgba16Float,
    PixelFormat::Rgba32Uint,
    PixelFormat::Rgba32Sint,
    PixelFormat::Rgba32Float,
    PixelFormat::Bc1Rgba,
    PixelFormat::Bc1RgbaSrgb,
    PixelFormat::Bc2Rgba,
    PixelFormat::Bc2RgbaSrgb,
    PixelFormat::Bc3Rgba,
    PixelFormat::Bc3RgbaSrgb,
    PixelFormat::Bc4RUnorm,
    PixelFormat::Bc4RSnorm,
    PixelFormat::Bc5RgUnorm,
    PixelFormat::Bc5RgSnorm,
    PixelFormat::Bc6hRgbFloat,
    PixelFormat::Bc6hRgbUfloat,
    PixelFormat::Bc7RgbaUnorm,
    PixelFormat::Bc7RgbaUnormSrgb,
    PixelFormat::Etc2Rgb8,
    PixelFormat::Etc2Rgb8Srgb,
    PixelFormat::EacRgba8,
    PixelFormat::EacRgba8Srgb,
    PixelFormat::Astc4x4Ldr,
    PixelFormat::Astc4x4Srgb,
    PixelFormat::Depth16Unorm,
    PixelFormat::Depth32Float,
    PixelFormat::Stencil8,
    PixelFormat::Depth32FloatStencil8,
    PixelFormat::X32Stencil8,
];

/// The formats a batch draws into most, so that draws pass the checks and reach the GPU.
const DRAWN_FORMATS: &[PixelFormat] = &[
    PixelFormat::Rgba8Unorm,
    PixelFormat::Rgba8Uint,
    PixelFormat::Rgba8Sint,
    PixelFormat::R32Float,
    PixelFormat::Rgba16Float,
    PixelFormat::Bgra8Unorm,
];

const TEXTURE_TYPES: &[TextureType] = &[
    TextureType::Type2D,
    TextureType::Type2DArray,
    TextureType::Type2DMultisample,
    TextureType::TypeCube,
    TextureType::TypeCubeArray,
    TextureType::Type3D,
];

const VERTEX_FORMATS: &[VertexFormat] = &[
    VertexFormat::UChar2,
    VertexFormat::UChar4,
    VertexFormat::Char4Normalized,
    VertexFormat::UShort2,
    VertexFormat::Short4Normalized,
    VertexFormat::Half2,
    VertexFormat::Half4,
    VertexFormat::Float,
    VertexFormat::Float2,
    VertexFormat::Float3,
    VertexFormat::Float4,
    VertexFormat::UInt,
    VertexFormat::UInt4,
    VertexFormat::Int2,
    VertexFormat::UChar,
    VertexFormat::UInt1010102Normalized,
    VertexFormat::UChar4NormalizedBgra,
];

const COMPARES: &[CompareFunction] = &[
    CompareFunction::Never,
    CompareFunction::Less,
    CompareFunction::Equal,
    CompareFunction::LessEqual,
    CompareFunction::Greater,
    CompareFunction::NotEqual,
    CompareFunction::GreaterEqual,
    CompareFunction::Always,
];

const STENCIL_OPERATIONS: &[StencilOperation] = &[
    StencilOperation::Keep,
    StencilOperation::Zero,
    StencilOperation::Replace,
    StencilOperation::IncrementClamp,
    StencilOperation::DecrementClamp,
    StencilOperation::Invert,
    StencilOperation::IncrementWrap,
    StencilOperation::DecrementWrap,
];

const ADDRESS_MODES: &[SamplerAddressMode] = &[
    SamplerAddressMode::ClampToEdge,
    SamplerAddressMode::MirrorClampToEdge,
    SamplerAddressMode::Repeat,
    SamplerAddressMode::MirrorRepeat,
    SamplerAddressMode::ClampToZero,
    SamplerAddressMode::ClampToBorderColor,
];

const BLEND_FACTORS: &[BlendFactor] = &[
    BlendFactor::Zero,
    BlendFactor::One,
    BlendFactor::SourceColor,
    BlendFactor::OneMinusSourceAlpha,
    BlendFactor::DestinationColor,
    BlendFactor::SourceAlphaSaturated,
    BlendFactor::BlendColor,
    BlendFactor::BlendAlpha,
    BlendFactor::OneMinusBlendAlpha,
    BlendFactor::Source1Color,
    BlendFactor::OneMinusSource1Alpha,
];

const BLEND_OPERATIONS: &[BlendOperation] = &[
    BlendOperation::Add,
    BlendOperation::Subtract,
    BlendOperation::ReverseSubtract,
    BlendOperation::Min,
    BlendOperation::Max,
];

const PRIMITIVES: &[PrimitiveType] = &[
    PrimitiveType::Point,
    PrimitiveType::Line,
    PrimitiveType::LineStrip,
    PrimitiveType::Triangle,
    PrimitiveType::TriangleStrip,
];

/// A window with no handle, which a layer is refused of.
struct NoWindow;

impl HasWindowHandle for NoWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

/// A render pipeline the batch made, with the attachments it was made for.
#[derive(Clone)]
struct Pipeline {
    state: <M as Metal>::RenderPipelineState,
    colors: [Option<PixelFormat>; MAX_COLOR_ATTACHMENTS],
    depth: Option<PixelFormat>,
    stencil: Option<PixelFormat>,
    samples: u32,
    topology: PrimitiveTopologyClass,
}

/// Everything a batch has made, and what it found.
struct World {
    metal: M,
    module: naga::Module,
    info: naga::valid::ModuleInfo,
    buffers: Vec<Held>,
    /// Every texture and view the batch keeps, with what it is.
    textures: Vec<(Texture, Made)>,
    samplers: Vec<<M as Metal>::SamplerState>,
    depth_stencils: Vec<<M as Metal>::DepthStencilState>,
    functions: Vec<(<M as Metal>::Function, naga::ShaderStage, usize)>,
    pipelines: Vec<Pipeline>,
    /// Compute pipelines, with the threads of their threadgroup on each axis.
    computes: Vec<(<M as Metal>::ComputePipelineState, [u32; 3])>,
    command_buffers: Vec<<M as Metal>::CommandBuffer>,
    /// The last command buffer committed: every one before it completes before it.
    last_committed: Option<<M as Metal>::CommandBuffer>,
    layers: Vec<<M as Metal>::Layer>,
    drawables: Vec<<M as Metal>::Drawable>,
    counts: Counts,
    /// Every entry point compiled unmangled, in the order of `ENTRIES`.
    plain_functions: Vec<Option<<M as Metal>::Function>>,
    /// Render pipelines of unmangled functions, whose draws the batch makes valid.
    plain_pipelines: Vec<PlainPipeline>,
    /// Depth and stencil states, with whether each tests depth and stencil.
    ds_states: Vec<(<M as Metal>::DepthStencilState, bool, bool)>,
    /// The globals each entry point uses, and as what each is bound.
    used: Vec<Vec<(naga::Handle<naga::GlobalVariable>, Bound)>>,
}

/// Keeps at most `POOL` objects, dropping a random old one to make room.
fn keep<T>(random: &mut Random, pool: &mut Vec<T>, item: T) {
    if pool.len() >= POOL {
        let at = random.index(pool.len());
        pool[at] = item;
    } else {
        pool.push(item);
    }
}

/// Returns a random object of a pool, if it has one.
fn any<T: Clone>(random: &mut Random, pool: &[T]) -> Option<T> {
    (!pool.is_empty()).then(|| pool[random.index(pool.len())].clone())
}

/// Returns the bytes a texture of `descriptor` holds, at most, past which the batch does not make it.
fn texture_bytes(d: &TextureDescriptor) -> u64 {
    [
        d.width,
        d.height,
        d.depth,
        d.array_length,
        d.sample_count,
        16,
        if d.texture_type == TextureType::TypeCubeArray || d.texture_type == TextureType::TypeCube {
            6
        } else {
            1
        },
    ]
    .iter()
    .try_fold(1u64, |total, n| total.checked_mul(u64::from(*n)))
    .unwrap_or(u64::MAX)
}

/// Runs one batch of `calls` calls of the Metal trait.
pub fn run(random: &mut Random, calls: u64, window: bool) -> Counts {
    let module = naga::front::wgsl::parse_str(PROGRAMS).expect("the fuzzer's programs parse");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("the fuzzer's programs validate");
    let used = used_globals(&module, &info);
    let mut world = World {
        metal: M::new(()),
        module,
        info,
        plain_functions: Vec::new(),
        plain_pipelines: Vec::new(),
        ds_states: Vec::new(),
        used,
        buffers: Vec::new(),
        textures: Vec::new(),
        samplers: Vec::new(),
        depth_stencils: Vec::new(),
        functions: Vec::new(),
        pipelines: Vec::new(),
        computes: Vec::new(),
        command_buffers: Vec::new(),
        last_committed: None,
        layers: Vec::new(),
        drawables: Vec::new(),
        counts: Counts::default(),
    };
    // A window of four pixels, which takes no focus and closes when the process ends.
    let test_window = window.then(|| AppkitTestWindow::new(()));
    // A drawable from the start, so that the calls that must refuse a drawable's texture meet one.
    if let Some(window) = &test_window
        && let Ok(layer) = world.metal.make_layer(window)
    {
        let d = LayerDescriptor {
            pixel_format: PixelFormat::Bgra8Unorm,
            width: 4,
            height: 4,
            display_sync_enabled: false,
        };
        if world.metal.configure_layer(&layer, &d).is_ok()
            && let Ok(drawable) = world.metal.next_drawable(&layer)
        {
            let texture = world.metal.drawable_texture(&drawable);
            world.textures.push((texture, Made::drawable()));
            world.drawables.push(drawable);
        }
        world.layers.push(layer);
    }
    for (index, _) in ENTRIES.iter().enumerate() {
        world.make_function(random, index, false);
        let plain = world
            .functions
            .iter()
            .rev()
            .find(|(_, _, at)| *at == index)
            .map(|(function, _, _)| function.clone());
        if plain.is_none() && std::env::var("FUZZ_SHOW_PLAIN").is_ok() {
            eprintln!("PLAIN {} did not compile", ENTRIES[index].0);
        }
        world.plain_functions.push(plain);
    }
    for (depth, stencil) in [(false, false), (true, false), (false, true), (true, true)] {
        let face = StencilDescriptor {
            stencil_compare_function: CompareFunction::Always,
            stencil_failure_operation: StencilOperation::Keep,
            depth_failure_operation: StencilOperation::Keep,
            depth_stencil_pass_operation: StencilOperation::Replace,
            read_mask: 255,
            write_mask: 255,
        };
        let made = world.metal.make_depth_stencil_state(&DepthStencilDescriptor {
            depth_compare_function: if depth { CompareFunction::Less } else { CompareFunction::Always },
            depth_write_enabled: depth,
            front_face_stencil: stencil.then_some(face),
            back_face_stencil: stencil.then_some(face),
        });
        if let Ok(state) = made {
            world.ds_states.push((state, depth, stencil));
        }
    }
    for _ in 0..8 {
        world.make_buffer(random);
        world.make_texture(random, true);
        world.make_sampler(random);
    }
    for _ in 0..calls {
        // A call taken that had to be refused may hold work the GPU must not run, so the batch ends there.
        if world.counts.accepted > 0 {
            break;
        }
        world.step(random, test_window.as_ref());
    }
    world.finish()
}

impl World {
    /// Makes one random call.
    fn step(&mut self, random: &mut Random, window: Option<&AppkitTestWindow>) {
        // Most calls are valid but for one field, so that the checks deep in each call see them.
        if random.chance(55) {
            self.precise(random);
            return;
        }
        match random.below(100) {
            0..=7 => self.make_buffer(random),
            8..=11 => self.write_or_read(random),
            12..=17 => {
                let plausible = random.chance(70);
                self.make_texture(random, plausible)
            }
            18..=21 => self.make_view(random),
            22..=24 => self.make_sampler(random),
            25..=26 => self.make_depth_stencil(random),
            27..=28 => {
                let index = random.index(ENTRIES.len());
                let mangled = random.chance(60);
                self.make_function(random, index, mangled)
            }
            29..=34 => self.make_pipeline(random),
            35..=36 => self.make_compute(random),
            37..=40 => self.make_command_buffer(random),
            41..=58 => self.encode_render(random),
            59..=70 => self.encode_blits(random),
            71..=80 => self.encode_compute(random),
            81..=85 => self.commit(random),
            86..=87 => self.wait(random),
            88 => {
                if let Some(c) = any(random, &self.command_buffers) {
                    let _ = self.metal.status(&c);
                }
                self.counts.call(false);
            }
            89 => {
                let _ = self.metal.name();
                self.counts.call(false);
            }
            90..=93 => self.layer_call(random, window),
            94..=95 => self.drop_some(random),
            _ => self.other_device(random),
        }
    }

    /// Makes a buffer of a random length, filled with random bytes as far as 64 KiB.
    fn make_buffer(&mut self, random: &mut Random) {
        let most = self.metal.max_buffer_length();
        let length = if random.chance(5) {
            // The device's own limit and just past it.
            random.pick(&[most, most.saturating_add(1), most.saturating_add(4096)])
        } else {
            random.u64(&[4, 16, 64, 256, 1024, 4096, 65536])
        };
        // A length past the device's limit reaches `make_buffer` as drawn, which must refuse it; one within it but past
        // `LARGEST_MADE` would make a large buffer on the owner's Mac, so it is cut after that check.
        let refuse = length > most;
        let length = if !refuse && length > LARGEST_MADE {
            LARGEST_MADE
        } else {
            length
        };
        let made = self.metal.make_buffer(length);
        self.counts.call(made.is_err());
        let Ok(buffer) = made else {
            return;
        };
        if refuse {
            eprintln!("ACCEPTED a buffer of {length} bytes, past the most, {most}");
            self.counts.accepted = self.counts.accepted.saturating_add(1);
            return;
        }
        let held = Held {
            buffer,
            shadow: Shadow::new(length),
        };
        // Random words, so that the functions' indices come from the data.
        let fill: Vec<u8> = (0..length.min(65536))
            .map(|_| random.next() as u8)
            .collect();
        if self.metal.write_contents(&held.buffer, 0, &fill).is_ok() {
            held.shadow.borrow_mut().wrote(0, &fill);
        }
        self.keep_buffer(random, held);
    }

    /// Adds a buffer to the pool, checking the one it replaces before that one is dropped.
    fn keep_buffer(&mut self, random: &mut Random, held: Held) {
        if self.buffers.len() >= POOL {
            let at = random.index(self.buffers.len());
            let old = std::mem::replace(&mut self.buffers[at], held);
            self.check(&old);
        } else {
            self.buffers.push(held);
        }
    }

    /// Reads a buffer back, after the work that uses it, and counts its bytes changed outside every allowed range.
    ///
    /// A buffer that uncommitted work uses is not read: that work may still change it.
    fn check(&mut self, held: &Held) {
        let mut read = vec![0; held.shadow.borrow().length() as usize];
        if self.metal.read_contents(&held.buffer, 0, &mut read).is_ok() {
            let changed = held.shadow.borrow().changed(&read);
            self.counts.canary_bytes = self.counts.canary_bytes.saturating_add(changed);
        }
    }

    /// Writes or reads a random range of a random buffer.
    fn write_or_read(&mut self, random: &mut Random) {
        let Some(held) = any(random, &self.buffers) else {
            return;
        };
        let offset = random.u64(&[0, 4, 16, 60, 64]);
        let mut bytes = random.bytes(&[0, 1, 3, 4, 16, 64, 4096]);
        let refused = if random.chance(50) {
            let written = self.metal.write_contents(&held.buffer, offset, &bytes);
            if written.is_ok() {
                held.shadow.borrow_mut().wrote(offset, &bytes);
            }
            written.is_err()
        } else {
            self.metal
                .read_contents(&held.buffer, offset, &mut bytes)
                .is_err()
        };
        self.counts.call(refused);
    }

    /// Makes a texture of random sizes, plausible ones most of the time.
    fn make_texture(&mut self, random: &mut Random, plausible: bool) {
        let texture_type = random.pick(TEXTURE_TYPES);
        let pixel_format = if plausible && random.chance(70) {
            random.pick(DRAWN_FORMATS)
        } else {
            random.pick(FORMATS)
        };
        let side = random.u32(&[1, 2, 4, 8, 16]);
        let mut d = TextureDescriptor {
            texture_type,
            pixel_format,
            width: side,
            height: if plausible {
                side
            } else {
                random.u32(&[1, 4, 8])
            },
            depth: if texture_type == TextureType::Type3D || !plausible {
                random.u32(&[1, 2, 4])
            } else {
                1
            },
            mipmap_level_count: random.u32(&[1, 2, 3]),
            sample_count: if texture_type == TextureType::Type2DMultisample {
                random.pick(&[4, 2, 8, 3, 0])
            } else {
                random.u32(&[1])
            },
            array_length: match texture_type {
                TextureType::Type2DArray | TextureType::TypeCubeArray => random.u32(&[1, 2, 3]),
                _ if plausible => 1,
                _ => random.u32(&[1]),
            },
            usage: TextureUsage {
                shader_read: random.chance(85),
                shader_write: random.chance(20),
                render_target: random.chance(80),
                pixel_format_view: random.chance(40),
            },
        };
        // A texture past a limit reaches the call as drawn, and must be refused; one the checks take but past the budget
        // keeps its widest side and loses the rest.
        let refuse = texture_refused(&d);
        if !refuse && texture_bytes(&d) > TEXTURE_BUDGET {
            d.height = d.height.min(1);
            d.depth = d.depth.min(1);
            d.array_length = d.array_length.min(1);
            if texture_bytes(&d) > TEXTURE_BUDGET {
                d.width = d.width.min(4096);
            }
        }
        let made = self.metal.make_texture(&d);
        self.counts.call(made.is_err());
        self.must(made.is_err(), refuse, "a texture");
        if let Ok(texture) = made
            && !refuse
        {
            keep(random, &mut self.textures, (texture, Made::of(&d)));
        }
    }

    /// Makes a view of a random texture.
    fn make_view(&mut self, random: &mut Random) {
        let Some((texture, parent)) = any(random, &self.textures) else {
            return;
        };
        let d = TextureViewDescriptor {
            pixel_format: random.pick(FORMATS),
            texture_type: random.pick(TEXTURE_TYPES),
            first_level: random.u32(&[0, 1]),
            level_count: random.u32(&[1, 2]),
            first_slice: random.u32(&[0, 1, 6]),
            slice_count: random.u32(&[1, 6, 12]),
        };
        let refuse = view_refused(&parent, &d);
        let view = self.metal.make_texture_view(&texture, &d);
        self.counts.call(view.is_err());
        self.must(view.is_err(), refuse, "a texture view");
        if let Ok(view) = view {
            keep(random, &mut self.textures, (view, parent.view(&d)));
        }
    }

    /// Makes a sampler of random modes.
    fn make_sampler(&mut self, random: &mut Random) {
        let filters = [SamplerMinMagFilter::Nearest, SamplerMinMagFilter::Linear];
        let d = SamplerDescriptor {
            min_filter: random.pick(&filters),
            mag_filter: random.pick(&filters),
            mip_filter: random.pick(&[
                SamplerMipFilter::NotMipmapped,
                SamplerMipFilter::Nearest,
                SamplerMipFilter::Linear,
            ]),
            s_address_mode: random.pick(ADDRESS_MODES),
            t_address_mode: random.pick(ADDRESS_MODES),
            r_address_mode: random.pick(ADDRESS_MODES),
            border_color: random.pick(&[
                SamplerBorderColor::TransparentBlack,
                SamplerBorderColor::OpaqueBlack,
                SamplerBorderColor::OpaqueWhite,
            ]),
            lod_min_clamp: random.f32(&[0.0, 1.0]),
            lod_max_clamp: random.f32(&[0.0, 1000.0, 16.0]),
            max_anisotropy: random.u32(&[1, 16]),
            compare_function: random.chance(30).then(|| random.pick(COMPARES)),
        };
        let made = self.metal.make_sampler_state(&d);
        self.counts.call(made.is_err());
        if let Ok(sampler) = made {
            keep(random, &mut self.samplers, sampler);
        }
    }

    /// Returns a random stencil test.
    fn stencil(random: &mut Random) -> StencilDescriptor {
        StencilDescriptor {
            stencil_compare_function: random.pick(COMPARES),
            stencil_failure_operation: random.pick(STENCIL_OPERATIONS),
            depth_failure_operation: random.pick(STENCIL_OPERATIONS),
            depth_stencil_pass_operation: random.pick(STENCIL_OPERATIONS),
            read_mask: random.u32(&[255]),
            write_mask: random.u32(&[255]),
        }
    }

    /// Makes a depth and stencil state of random tests.
    fn make_depth_stencil(&mut self, random: &mut Random) {
        let d = DepthStencilDescriptor {
            depth_compare_function: random.pick(COMPARES),
            depth_write_enabled: random.chance(50),
            front_face_stencil: random.chance(40).then(|| Self::stencil(random)),
            back_face_stencil: random.chance(40).then(|| Self::stencil(random)),
        };
        let made = self.metal.make_depth_stencil_state(&d);
        self.counts.call(made.is_err());
        if let Ok(state) = made {
            keep(random, &mut self.depth_stencils, state);
        }
    }

    /// Compiles one entry point, its slots and vertex buffers mangled or not.
    fn make_function(&mut self, random: &mut Random, index: usize, mangled: bool) {
        let (entry_point, stage) = ENTRIES[index];
        let mut slots: Vec<ResourceSlot> = SLOTS
            .iter()
            .map(|&(binding, index)| ResourceSlot {
                group: 0,
                binding,
                index,
            })
            .collect();
        let mut vertex_buffers = vec![
            VertexBufferLayout {
                buffer_index: VERTEX_SLOTS[0],
                stride: 16,
                step_function: VertexStepFunction::PerVertex,
                attributes: vec![VertexAttribute {
                    location: 0,
                    offset: 0,
                    format: if entry_point == "vs_plain" {
                        VertexFormat::Float2
                    } else {
                        VertexFormat::Float4
                    },
                }],
            },
            VertexBufferLayout {
                buffer_index: VERTEX_SLOTS[1],
                stride: 24,
                step_function: VertexStepFunction::PerInstance,
                attributes: vec![
                    VertexAttribute {
                        location: 1,
                        offset: 0,
                        format: VertexFormat::UInt4,
                    },
                    VertexAttribute {
                        location: 2,
                        offset: 16,
                        format: VertexFormat::Int2,
                    },
                ],
            },
        ];
        let (mut immediates, mut sizes, mut point_size) = (IMMEDIATES_AT, LENGTHS_AT, false);
        if mangled {
            for slot in &mut slots {
                if random.chance(15) {
                    slot.index = random.u32(&[0, 1, 30, 31, 127, 128]);
                }
                if random.chance(5) {
                    slot.binding = random.u32(&[0, 99]);
                }
                if random.chance(5) {
                    slot.group = random.u32(&[1]);
                }
            }
            if random.chance(30) {
                immediates = random.u32(&[29, 30, 31]);
            }
            if random.chance(30) {
                sizes = random.u32(&[29, 30, 31]);
            }
            point_size = random.chance(30);
            for layout in &mut vertex_buffers {
                if random.chance(30) {
                    layout.buffer_index = random.u32(&[0, 1, 20, 21, 29, 30, 31]);
                }
                if random.chance(30) {
                    layout.stride = random.u32(&[0, 1, 4, 16, 2048]);
                }
                if random.chance(20) {
                    layout.step_function = random.pick(&[
                        VertexStepFunction::Constant,
                        VertexStepFunction::PerVertex,
                        VertexStepFunction::PerInstance,
                    ]);
                }
                for attribute in &mut layout.attributes {
                    if random.chance(30) {
                        attribute.offset = random.u32(&[0, 4, 12, 16, 2044]);
                    }
                    if random.chance(30) {
                        attribute.format = random.pick(VERTEX_FORMATS);
                    }
                    if random.chance(10) {
                        attribute.location = random.u32(&[0, 1, 2, 31]);
                    }
                }
            }
            if random.chance(10) {
                vertex_buffers.clear();
            }
        }
        let made = self.metal.make_function(&FunctionSource {
            module: &self.module,
            info: &self.info,
            stage,
            entry_point,
            resources: &slots,
            immediates_buffer_index: immediates,
            sizes_buffer_index: sizes,
            vertex_buffers: &vertex_buffers,
            point_size,
        });
        self.counts.call(made.is_err());
        if let Err(error) = &made
            && !mangled
            && std::env::var("FUZZ_SHOW_PLAIN").is_ok()
        {
            eprintln!("PLAIN {entry_point}: {error:?}");
        }
        if let Ok(function) = made {
            keep(random, &mut self.functions, (function, stage, index));
        }
    }

    /// Returns a random function of a stage, if the batch has one.
    fn function_of(
        &self,
        random: &mut Random,
        stage: naga::ShaderStage,
    ) -> Option<(<M as Metal>::Function, usize)> {
        let of: Vec<_> = self
            .functions
            .iter()
            .filter(|(_, s, _)| *s == stage)
            .collect();
        (!of.is_empty()).then(|| {
            let (function, _, index) = of[random.index(of.len())];
            (function.clone(), *index)
        })
    }

    /// Makes a render pipeline of random functions and attachments.
    fn make_pipeline(&mut self, random: &mut Random) {
        let stages = [
            naga::ShaderStage::Vertex,
            naga::ShaderStage::Fragment,
            naga::ShaderStage::Compute,
        ];
        let vertex_stage = if random.chance(90) {
            stages[0]
        } else {
            random.pick(&stages)
        };
        let Some((vertex, _)) = self.function_of(random, vertex_stage) else {
            return;
        };
        let fragment = if random.chance(85) {
            let stage = if random.chance(90) {
                stages[1]
            } else {
                random.pick(&stages)
            };
            self.function_of(random, stage)
        } else {
            None
        };
        let mut colors = [None; MAX_COLOR_ATTACHMENTS];
        let count = if random.chance(80) {
            1
        } else {
            random.index(MAX_COLOR_ATTACHMENTS)
        };
        for color in colors.iter_mut().take(count) {
            let blend_operation = random.pick(BLEND_OPERATIONS);
            *color = Some(RenderPipelineColorAttachmentDescriptor {
                pixel_format: if random.chance(80) {
                    random.pick(DRAWN_FORMATS)
                } else {
                    random.pick(FORMATS)
                },
                blending: random.chance(30).then(|| Blending {
                    rgb_blend_operation: blend_operation,
                    alpha_blend_operation: random.pick(BLEND_OPERATIONS),
                    source_rgb_blend_factor: random.pick(BLEND_FACTORS),
                    destination_rgb_blend_factor: random.pick(BLEND_FACTORS),
                    source_alpha_blend_factor: random.pick(BLEND_FACTORS),
                    destination_alpha_blend_factor: random.pick(BLEND_FACTORS),
                }),
                write_mask: ColorWriteMask {
                    red: random.chance(90),
                    green: random.chance(90),
                    blue: random.chance(90),
                    alpha: random.chance(90),
                },
            });
        }
        let depth_stencil = [
            None,
            Some(PixelFormat::Depth32Float),
            Some(PixelFormat::Depth16Unorm),
            Some(PixelFormat::Depth32FloatStencil8),
            Some(PixelFormat::Stencil8),
        ];
        let depth = if random.chance(70) {
            None
        } else {
            random.pick(&depth_stencil)
        };
        let stencil = match random.below(4) {
            0 => depth,
            1 => random.pick(&depth_stencil),
            _ => None,
        };
        let samples = if random.chance(85) {
            1
        } else {
            random.u32(&[2, 4, 8])
        };
        let topology = random.pick(&[
            PrimitiveTopologyClass::Triangle,
            PrimitiveTopologyClass::Triangle,
            PrimitiveTopologyClass::Line,
            PrimitiveTopologyClass::Point,
        ]);
        let made = self
            .metal
            .make_render_pipeline_state(&RenderPipelineDescriptor {
                vertex_function: &vertex,
                fragment_function: fragment.as_ref().map(|(f, _)| f),
                color_attachments: colors,
                depth_attachment_pixel_format: depth,
                stencil_attachment_pixel_format: stencil,
                raster_sample_count: samples,
                alpha_to_coverage_enabled: random.chance(10),
                rasterization_enabled: random.chance(90),
                input_primitive_topology: topology,
            });
        self.counts.call(made.is_err());
        if let Ok(state) = made {
            let pipeline = Pipeline {
                state,
                colors: colors.map(|c| c.map(|c| c.pixel_format)),
                depth,
                stencil,
                samples,
                topology,
            };
            keep(random, &mut self.pipelines, pipeline);
        }
    }

    /// Makes a compute pipeline of a random function.
    fn make_compute(&mut self, random: &mut Random) {
        let stage = if random.chance(90) {
            naga::ShaderStage::Compute
        } else {
            naga::ShaderStage::Fragment
        };
        let Some((function, index)) = self.function_of(random, stage) else {
            return;
        };
        let made = self.metal.make_compute_pipeline_state(&function);
        self.counts.call(made.is_err());
        if let Ok(state) = made {
            let threads = match ENTRIES[index].0 {
                "cs_scatter" => [64, 1, 1],
                "cs_shared" => [8, 8, 1],
                _ => [1, 1, 1],
            };
            keep(random, &mut self.computes, (state, threads));
        }
    }

    /// Makes a command buffer.
    fn make_command_buffer(&mut self, random: &mut Random) {
        let made = self.metal.make_command_buffer();
        self.counts.call(made.is_err());
        if let Ok(command_buffer) = made {
            keep(random, &mut self.command_buffers, command_buffer);
        }
    }

    /// Returns a command buffer of the pool, or a new one.
    fn command_buffer(&mut self, random: &mut Random) -> Option<<M as Metal>::CommandBuffer> {
        if self.command_buffers.is_empty() || random.chance(30) {
            self.make_command_buffer(random);
        }
        any(random, &self.command_buffers)
    }

    /// Makes a texture to draw into of `format`, at the pass's size and sample count.
    fn attachment_texture(
        &mut self,
        random: &mut Random,
        format: PixelFormat,
        samples: u32,
    ) -> Option<Texture> {
        if !self.textures.is_empty() && random.chance(25) {
            return any(random, &self.textures).map(|(texture, _)| texture);
        }
        let side = random.pick(&[4, 4, 8, 1]);
        self.metal
            .make_texture(&TextureDescriptor {
                texture_type: if samples > 1 {
                    TextureType::Type2DMultisample
                } else {
                    TextureType::Type2D
                },
                pixel_format: format,
                width: side,
                height: side,
                depth: 1,
                mipmap_level_count: 1,
                sample_count: samples,
                array_length: 1,
                usage: TextureUsage {
                    shader_read: true,
                    shader_write: false,
                    render_target: true,
                    pixel_format_view: false,
                },
            })
            .ok()
    }

    /// Returns a random buffer binding: a range of a random buffer, valid most of the time.
    fn buffer_range(&mut self, random: &mut Random) -> Option<(Held, u64, u64)> {
        let buffer = any(random, &self.buffers)?;
        let offset = if random.chance(70) {
            random.pick(&[0, 16, 256])
        } else {
            random.u64(&[0, 4, 8])
        };
        let length = if random.chance(60) {
            random.pick(&[16, 64, 256, 4096])
        } else {
            random.u64(&[0, 4])
        };
        Some((buffer, offset, length))
    }

    /// Returns random commands that bind and draw, the ranges they may change, and whether one must be refused.
    fn render_commands(
        &mut self,
        random: &mut Random,
        pipeline: Option<&Pipeline>,
    ) -> (Vec<RenderCommand<M>>, Vec<Allowed>, bool) {
        let mut commands = Vec::new();
        let mut allowed = Vec::new();
        let mut refuse = false;
        if let Some(pipeline) = pipeline {
            commands.push(RenderCommand::SetRenderPipelineState(
                pipeline.state.clone(),
            ));
        }
        for _ in 0..random.below(14) {
            let command = match random.below(24) {
                0 => any(random, &self.pipelines)
                    .map(|p| RenderCommand::SetRenderPipelineState(p.state)),
                1 => any(random, &self.depth_stencils).map(RenderCommand::SetDepthStencilState),
                // No vertex function writes, so a vertex buffer is a canary in all its bytes.
                2..=4 => self.buffer_range(random).map(|(held, offset, length)| {
                    RenderCommand::SetVertexBuffer {
                        buffer: held.buffer,
                        offset,
                        length,
                        index: random.u32(&[0, 1, 2, 3, 20, 21]),
                    }
                }),
                5..=7 => self.buffer_range(random).map(|(held, offset, length)| {
                    allowed.push((held.shadow, offset, length));
                    RenderCommand::SetFragmentBuffer {
                        buffer: held.buffer,
                        offset,
                        length,
                        index: random.u32(&[0, 1, 2, 3]),
                    }
                }),
                8 => Some(RenderCommand::SetVertexBytes {
                    bytes: random.bytes(&[0, 4, 16, 64, 4096, 4097]),
                    index: random.u32(&[0, 1, 2, 3, 20, 21]),
                }),
                9 => Some(RenderCommand::SetFragmentBytes {
                    bytes: random.bytes(&[0, 4, 16, 64, 4096, 4097]),
                    index: random.u32(&[0, 1, 2, 3]),
                }),
                10 => any(random, &self.textures).map(|(texture, _)| {
                    RenderCommand::SetVertexTexture {
                        texture,
                        index: random.u32(&[0, 1, 2, 3, 4, 5, 6, 7]),
                    }
                }),
                11 | 12 => any(random, &self.textures).map(|(texture, _)| {
                    RenderCommand::SetFragmentTexture {
                        texture,
                        index: random.u32(&[0, 1, 2, 3, 4, 5, 6, 7]),
                    }
                }),
                13 => any(random, &self.samplers).map(|sampler| {
                    RenderCommand::SetVertexSamplerState {
                        sampler,
                        index: random.u32(&[0, 1]),
                    }
                }),
                14 => any(random, &self.samplers).map(|sampler| {
                    RenderCommand::SetFragmentSamplerState {
                        sampler,
                        index: random.u32(&[0, 1]),
                    }
                }),
                15 => {
                    let count = random.pick(&[1, 1, 0, 16, 17]);
                    Some(RenderCommand::SetViewports(
                        (0..count)
                            .map(|_| Viewport {
                                origin_x: random.f64(&[0.0]),
                                origin_y: random.f64(&[0.0]),
                                width: random.f64(&[4.0, 8.0]),
                                height: random.f64(&[4.0, 8.0]),
                                znear: random.f64(&[0.0]),
                                zfar: random.f64(&[1.0]),
                            })
                            .collect(),
                    ))
                }
                16 => {
                    let count = random.pick(&[1, 1, 0, 16, 17]);
                    Some(RenderCommand::SetScissorRects(
                        (0..count)
                            .map(|_| ScissorRect {
                                x: random.u32(&[0, 1]),
                                y: random.u32(&[0, 1]),
                                width: random.u32(&[1, 4]),
                                height: random.u32(&[1, 4]),
                            })
                            .collect(),
                    ))
                }
                17 => Some(match random.below(4) {
                    0 => RenderCommand::SetCullMode(random.pick(&[
                        CullMode::None,
                        CullMode::Front,
                        CullMode::Back,
                    ])),
                    1 => RenderCommand::SetFrontFacingWinding(
                        random.pick(&[Winding::Clockwise, Winding::CounterClockwise]),
                    ),
                    2 => RenderCommand::SetTriangleFillMode(
                        random.pick(&[TriangleFillMode::Fill, TriangleFillMode::Lines]),
                    ),
                    _ => RenderCommand::SetDepthClipMode(
                        random.pick(&[DepthClipMode::Clip, DepthClipMode::Clamp]),
                    ),
                }),
                18 => Some(RenderCommand::SetDepthBias {
                    depth_bias: random.f32(&[0.0, 2.5]),
                    slope_scale: random.f32(&[0.0, 1.0]),
                    clamp: random.f32(&[0.0]),
                }),
                19 => Some(if random.chance(50) {
                    RenderCommand::SetBlendColor([
                        random.f32(&[0.5]),
                        random.f32(&[0.5]),
                        random.f32(&[0.5]),
                        random.f32(&[0.5]),
                    ])
                } else {
                    RenderCommand::SetStencilReferenceValues {
                        front: random.u32(&[0, 3, 255]),
                        back: random.u32(&[0, 7, 256]),
                    }
                }),
                20 => Some(RenderCommand::SetVisibilityResultMode {
                    mode: random.pick(&[
                        VisibilityResultMode::Disabled,
                        VisibilityResultMode::Boolean,
                        VisibilityResultMode::Counting,
                    ]),
                    offset: random.u64(&[0, 8, 16, 262_136]),
                }),
                21 | 22 => {
                    let (draw, must) = self.draw(random, pipeline);
                    refuse |= must;
                    Some(draw)
                }
                _ => self.indexed_draw(random, pipeline).map(|(draw, must)| {
                    refuse |= must;
                    draw
                }),
            };
            commands.extend(command);
        }
        if random.chance(80) {
            let (draw, must) = self.draw(random, pipeline);
            refuse |= must;
            commands.push(draw);
        }
        (commands, allowed, refuse)
    }

    /// Returns a primitive type, of the pipeline's class most of the time.
    fn primitives(random: &mut Random, pipeline: Option<&Pipeline>) -> PrimitiveType {
        match pipeline.map(|p| p.topology).filter(|_| random.chance(80)) {
            Some(PrimitiveTopologyClass::Point) => PrimitiveType::Point,
            Some(PrimitiveTopologyClass::Line) => {
                random.pick(&[PrimitiveType::Line, PrimitiveType::LineStrip])
            }
            Some(PrimitiveTopologyClass::Triangle) => {
                random.pick(&[PrimitiveType::Triangle, PrimitiveType::TriangleStrip])
            }
            None => random.pick(PRIMITIVES),
        }
    }

    /// Returns a draw without indices, and whether it must be refused.
    ///
    /// Counts whose sums pass the largest `u32`, or no instances, reach the draw as drawn, and must be refused; a draw
    /// that passes those checks but would run a great many vertices is cut after them: the bounds, not the GPU's time,
    /// are under test.
    fn draw(
        &mut self,
        random: &mut Random,
        pipeline: Option<&Pipeline>,
    ) -> (RenderCommand<M>, bool) {
        let vertex_start = random.u32(&[0, 1]);
        let mut vertex_count = random.u32(&[3, 4, 6]);
        let mut instance_count = random.u32(&[1, 2]);
        let base_instance = random.u32(&[0, 1]);
        let refuse = vertex_start.checked_add(vertex_count).is_none()
            || base_instance.checked_add(instance_count).is_none()
            || instance_count == 0;
        if !refuse && u64::from(vertex_count).saturating_mul(u64::from(instance_count)) > 1 << 16 {
            vertex_count = vertex_count.min(1 << 12);
            instance_count = instance_count.min(4);
        }
        let draw = RenderCommand::DrawPrimitives {
            primitive_type: Self::primitives(random, pipeline),
            vertex_start,
            vertex_count,
            instance_count,
            base_instance,
        };
        (draw, refuse)
    }

    /// Returns a draw with indices into a random buffer, and whether it must be refused.
    ///
    /// Its index count is bounded by the buffer, which a batch keeps small; instances are as for `draw`.
    fn indexed_draw(
        &mut self,
        random: &mut Random,
        pipeline: Option<&Pipeline>,
    ) -> Option<(RenderCommand<M>, bool)> {
        let held = any(random, &self.buffers)?;
        let mut instance_count = random.u32(&[1, 2]);
        let index_count = random.u32(&[3, 6, 64]);
        let base_instance = random.u32(&[0]);
        let index_type = random.pick(&[IndexType::UInt16, IndexType::UInt32]);
        let index_buffer_offset = random.u64(&[0, 2, 4, 64]);
        let size = if index_type == IndexType::UInt16 {
            2
        } else {
            4
        };
        let end = u64::from(index_count)
            .checked_mul(size)
            .and_then(|bytes| bytes.checked_add(index_buffer_offset));
        let refuse = base_instance.checked_add(instance_count).is_none()
            || instance_count == 0
            || end.is_none_or(|end| end > held.shadow.borrow().length());
        if !refuse && u64::from(index_count).saturating_mul(u64::from(instance_count)) > 1 << 16 {
            instance_count = instance_count.min(4);
        }
        let draw = RenderCommand::DrawIndexedPrimitives {
            primitive_type: Self::primitives(random, pipeline),
            index_count,
            index_type,
            index_buffer: held.buffer,
            index_buffer_offset,
            instance_count,
            base_vertex: random.i32(),
            base_instance,
        };
        Some((draw, refuse))
    }

    /// Encodes one random render pass, its attachments those of a pipeline most of the time.
    fn encode_render(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let pipeline = if random.chance(85) {
            any(random, &self.pipelines)
        } else {
            None
        };
        let samples = pipeline.as_ref().map_or(1, |p| p.samples);
        let mut colors: Vec<Option<Texture>> = Vec::new();
        let mut resolves: Vec<Option<Texture>> = Vec::new();
        for at in 0..MAX_COLOR_ATTACHMENTS {
            let format = match &pipeline {
                Some(p) if random.chance(90) => p.colors[at],
                _ => random.chance(15).then(|| random.pick(FORMATS)),
            };
            colors.push(format.and_then(|f| self.attachment_texture(random, f, samples)));
            let resolve = format
                .filter(|_| samples > 1 && random.chance(40))
                .and_then(|f| self.attachment_texture(random, f, 1));
            resolves.push(resolve);
        }
        let aspect = |random: &mut Random, world: &mut World, format: Option<PixelFormat>| {
            format.and_then(|f| world.attachment_texture(random, f, samples))
        };
        let depth_format = pipeline.as_ref().and_then(|p| p.depth);
        let depth = aspect(random, self, depth_format);
        let stencil_format = pipeline.as_ref().and_then(|p| p.stencil);
        let stencil =
            if stencil_format.is_some() && stencil_format == depth_format && random.chance(80) {
                depth.clone()
            } else {
                aspect(random, self, stencil_format)
            };
        let visibility = random
            .chance(30)
            .then(|| any(random, &self.buffers))
            .flatten();
        let (commands, mut allowed, refuse) = self.render_commands(random, pipeline.as_ref());
        // Metal counts into the visibility buffer at each offset a command names.
        if let Some(held) = &visibility {
            for command in &commands {
                if let RenderCommand::SetVisibilityResultMode { mode, offset } = command
                    && *mode != VisibilityResultMode::Disabled
                {
                    allowed.push((
                        held.shadow.clone(),
                        *offset,
                        wcontainers_metal::VISIBILITY_RESULT_BYTES,
                    ));
                }
            }
        }
        let visibility = visibility.map(|held| held.buffer);
        let load = |random: &mut Random| {
            random.pick(&[LoadAction::Clear, LoadAction::Load, LoadAction::DontCare])
        };
        let store = |random: &mut Random| random.pick(&[StoreAction::Store, StoreAction::DontCare]);
        let mut color_attachments = [const { None }; MAX_COLOR_ATTACHMENTS];
        for (at, slot) in color_attachments.iter_mut().enumerate() {
            if let Some(texture) = &colors[at] {
                *slot = Some(RenderPassColorAttachment {
                    texture,
                    level: random.u32(&[0, 0, 0, 1]),
                    slice: random.u32(&[0, 0, 0, 1]),
                    depth_plane: random.u32(&[0, 0, 0, 1]),
                    load_action: load(random),
                    store_action: store(random),
                    clear_color: [random.f64(&[0.0, 1.0]), 0.5, 0.25, 1.0],
                    resolve: resolves[at].as_ref().map(|texture| ResolveTarget {
                        texture,
                        level: random.u32(&[0]),
                        slice: random.u32(&[0]),
                        depth_plane: random.u32(&[0]),
                    }),
                });
            }
        }
        let depth_attachment = depth
            .as_ref()
            .map(|texture| RenderPassDepthStencilAttachment {
                texture,
                level: random.u32(&[0, 0, 0]),
                slice: random.u32(&[0, 0, 0]),
                load_action: load(random),
                store_action: store(random),
                clear_value: random.f64(&[1.0, 0.0, 0.5]),
            });
        let stencil_attachment = stencil
            .as_ref()
            .map(|texture| RenderPassDepthStencilAttachment {
                texture,
                level: random.u32(&[0, 0, 0]),
                slice: random.u32(&[0, 0, 0]),
                load_action: load(random),
                store_action: store(random),
                clear_value: random.f64(&[0.0, 3.0, 255.0]),
            });
        let pass = RenderPassDescriptor {
            color_attachments,
            depth_attachment,
            stencil_attachment,
            visibility_result_buffer: visibility.as_ref(),
            render_target_width: random.u32(&[4, 8]),
            render_target_height: random.u32(&[4, 8]),
            default_raster_sample_count: random.u32(&[1, 4]),
        };
        let refused = self
            .metal
            .encode_render_commands(&command_buffer, &pass, &commands)
            .is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &allowed, "a render pass");
    }

    /// Notes what a call that Metal took may change, or counts a call taken that had to be refused.
    fn taken(&mut self, refused: bool, refuse: bool, allowed: &[Allowed], what: &str) {
        if refused {
            return;
        }
        if refuse {
            eprintln!("ACCEPTED {what} that had to be refused");
            self.counts.accepted = self.counts.accepted.saturating_add(1);
        }
        for (shadow, offset, size) in allowed {
            shadow.borrow_mut().allow(*offset, *size);
        }
    }

    /// Returns a random region of a random texture.
    ///
    /// The last of the tuple is the texture's format.
    fn region(
        &mut self,
        random: &mut Random,
    ) -> Option<(Texture, u32, u32, Origin, Size, PixelFormat)> {
        let (texture, made) = any(random, &self.textures)?;
        Some((
            texture,
            random.u32(&[0, 0, 1]),
            random.u32(&[0, 0, 1]),
            Origin {
                x: random.u32(&[0, 0, 1, 4]),
                y: random.u32(&[0, 0, 1, 4]),
                z: random.u32(&[0, 0, 1]),
            },
            Size {
                width: random.u32(&[1, 4, 8]),
                height: random.u32(&[1, 4, 8]),
                depth: random.u32(&[1, 1, 2]),
            },
            made.format,
        ))
    }

    /// Encodes random copies and fills.
    fn encode_blits(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let mut commands = Vec::new();
        let mut allowed: Vec<Allowed> = Vec::new();
        let options = [
            BlitOption::None,
            BlitOption::None,
            BlitOption::DepthFromDepthStencil,
            BlitOption::StencilFromDepthStencil,
        ];
        for _ in 0..1u64.saturating_add(random.below(4)) {
            let pitches = [16, 32, 64, 128, 256];
            let images = [0, 64, 256, 1024, 4096];
            let command = match random.below(6) {
                0 => match (self.buffer_range(random), self.buffer_range(random)) {
                    (
                        Some((source, source_offset, size)),
                        Some((destination, destination_offset, _)),
                    ) => {
                        allowed.push((destination.shadow, destination_offset, size));
                        Some(BlitCommand::CopyFromBufferToBuffer {
                            source_buffer: source.buffer,
                            source_offset,
                            destination_buffer: destination.buffer,
                            destination_offset,
                            size,
                        })
                    }
                    _ => None,
                },
                1 => match (self.buffer_range(random), self.region(random)) {
                    (
                        Some((source, source_offset, _)),
                        Some((
                            destination_texture,
                            destination_slice,
                            destination_level,
                            destination_origin,
                            source_size,
                            _,
                        )),
                    ) => Some(BlitCommand::CopyFromBufferToTexture {
                        source_buffer: source.buffer,
                        source_offset,
                        source_bytes_per_row: random.u64(&pitches),
                        source_bytes_per_image: random.u64(&images),
                        source_size,
                        destination_texture,
                        destination_slice,
                        destination_level,
                        destination_origin,
                        options: random.pick(&options),
                    }),
                    _ => None,
                },
                2 => match (self.region(random), self.buffer_range(random)) {
                    (
                        Some((
                            source_texture,
                            source_slice,
                            source_level,
                            source_origin,
                            source_size,
                            source_format,
                        )),
                        Some((destination, destination_offset, _)),
                    ) => {
                        let (row, image) = (random.u64(&pitches), random.u64(&images));
                        let option = random.pick(&options);
                        // One aspect of a depth and stencil format: four bytes of depth, or one of stencil.
                        let format = match option {
                            BlitOption::DepthFromDepthStencil => PixelFormat::Depth32Float,
                            BlitOption::StencilFromDepthStencil => PixelFormat::Stencil8,
                            BlitOption::None => source_format,
                        };
                        for (start, length) in
                            copy_rows(format, destination_offset, row, image, source_size)
                        {
                            allowed.push((destination.shadow.clone(), start, length));
                        }
                        Some(BlitCommand::CopyFromTextureToBuffer {
                            source_texture,
                            source_slice,
                            source_level,
                            source_origin,
                            source_size,
                            destination_buffer: destination.buffer,
                            destination_offset,
                            destination_bytes_per_row: row,
                            destination_bytes_per_image: image,
                            options: option,
                        })
                    }
                    _ => None,
                },
                3 => match (self.region(random), self.region(random)) {
                    (
                        Some((
                            source_texture,
                            source_slice,
                            source_level,
                            source_origin,
                            source_size,
                            _,
                        )),
                        Some((
                            destination_texture,
                            destination_slice,
                            destination_level,
                            destination_origin,
                            _,
                            _,
                        )),
                    ) => Some(BlitCommand::CopyFromTextureToTexture {
                        source_texture,
                        source_slice,
                        source_level,
                        source_origin,
                        source_size,
                        destination_texture,
                        destination_slice,
                        destination_level,
                        destination_origin,
                    }),
                    _ => None,
                },
                4 => self.buffer_range(random).map(|(held, offset, length)| {
                    allowed.push((held.shadow, offset, length));
                    BlitCommand::FillBuffer {
                        buffer: held.buffer,
                        offset,
                        length,
                        value: random.next() as u8,
                    }
                }),
                _ => any(random, &self.textures)
                    .map(|(texture, _)| BlitCommand::GenerateMipmaps { texture }),
            };
            commands.extend(command);
        }
        let refused = self
            .metal
            .encode_blit_commands(&command_buffer, &commands)
            .is_err();
        self.counts.call(refused);
        self.taken(refused, false, &allowed, "copies");
    }

    /// Encodes random compute commands, binding what the programs read, and dispatches.
    fn encode_compute(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let mut commands = Vec::new();
        let mut allowed: Vec<Allowed> = Vec::new();
        // The threads of the threadgroup of the pipeline the dispatch runs, the last one set.
        let mut threads = None;
        if let Some((state, of)) = any(random, &self.computes) {
            commands.push(ComputeCommand::SetComputePipelineState(state));
            threads = Some(of);
        }
        for _ in 0..random.below(9) {
            let command = match random.below(7) {
                0 => any(random, &self.computes).map(|(state, of)| {
                    threads = Some(of);
                    ComputeCommand::SetComputePipelineState(state)
                }),
                1..=3 => self.buffer_range(random).map(|(held, offset, length)| {
                    allowed.push((held.shadow, offset, length));
                    ComputeCommand::SetBuffer {
                        buffer: held.buffer,
                        offset,
                        length,
                        index: random.u32(&[1, 2, 3, 0]),
                    }
                }),
                4 => Some(ComputeCommand::SetBytes {
                    bytes: random.bytes(&[0, 4, 16, 64, 4096, 4097]),
                    index: random.u32(&[1, 2, 3]),
                }),
                5 => any(random, &self.textures).map(|(texture, _)| ComputeCommand::SetTexture {
                    texture,
                    index: random.u32(&[0, 1, 2, 5]),
                }),
                _ => any(random, &self.samplers).map(|sampler| ComputeCommand::SetSamplerState {
                    sampler,
                    index: random.u32(&[0, 1]),
                }),
            };
            commands.extend(command);
        }
        // Counts whose threads on one axis pass the largest `u32` reach the dispatch as drawn, and must be refused; a
        // dispatch that passes that check but runs a great many threadgroups is cut after it: the bounds, not the GPU's
        // time, are under test.
        let mut groups = [
            random.u32(&[1, 2, 4]),
            random.u32(&[1, 2, 4]),
            random.u32(&[1, 2]),
        ];
        let refuse = threads.is_some_and(|threads| {
            groups
                .iter()
                .zip(threads)
                .any(|(n, of)| u32::try_from(u64::from(*n).saturating_mul(u64::from(of))).is_err())
        });
        let total = groups
            .iter()
            .fold(1u64, |total, n| total.saturating_mul(u64::from(*n)));
        if !refuse && total > 4096 {
            groups = [groups[0].min(64), groups[1].min(16), groups[2].min(4)];
        }
        commands.push(ComputeCommand::DispatchThreadgroups(Size {
            width: groups[0],
            height: groups[1],
            depth: groups[2],
        }));
        let refused = self
            .metal
            .encode_compute_commands(&command_buffer, &commands)
            .is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &allowed, "a dispatch");
    }

    /// Commits a random command buffer.
    fn commit(&mut self, random: &mut Random) {
        let Some(command_buffer) = any(random, &self.command_buffers) else {
            return;
        };
        let committed = self.metal.commit(&command_buffer);
        self.counts.call(committed.is_err());
        if committed.is_ok() {
            self.last_committed = Some(command_buffer);
        }
    }

    /// Waits for a random command buffer.
    fn wait(&mut self, random: &mut Random) {
        let Some(command_buffer) = any(random, &self.command_buffers) else {
            return;
        };
        let refused = self.metal.wait_until_completed(&command_buffer).is_err();
        self.counts.call(refused);
        if !refused && self.metal.status(&command_buffer) == CommandBufferStatus::Error {
            eprintln!("GPU-ERROR a command buffer ended in an error");
            self.counts.gpu_errors = self.counts.gpu_errors.saturating_add(1);
        }
    }

    /// Makes, configures, draws into and presents a layer of the batch's window, or asks for a layer of none.
    fn layer_call(&mut self, random: &mut Random, window: Option<&AppkitTestWindow>) {
        let Some(window) = window else {
            let refused = self.metal.make_layer(&NoWindow).is_err();
            self.counts.call(refused);
            return;
        };
        match random.below(4) {
            0 => {
                let made = self.metal.make_layer(window);
                self.counts.call(made.is_err());
                if let Ok(layer) = made {
                    keep(random, &mut self.layers, layer);
                }
            }
            1 => {
                let Some(layer) = any(random, &self.layers) else {
                    return;
                };
                let (mut width, mut height) = (random.u32(&[4, 8]), random.u32(&[4, 8]));
                // A drawable within the limits stays small; one past them is refused.
                if width <= 32768
                    && height <= 32768
                    && u64::from(width).saturating_mul(u64::from(height)) > 1 << 16
                {
                    (width, height) = (width.min(256), height.min(256));
                }
                let d = LayerDescriptor {
                    pixel_format: if random.chance(70) {
                        PixelFormat::Bgra8Unorm
                    } else {
                        random.pick(FORMATS)
                    },
                    width,
                    height,
                    display_sync_enabled: false,
                };
                let refused = self.metal.configure_layer(&layer, &d).is_err();
                self.counts.call(refused);
            }
            2 => {
                let Some(layer) = any(random, &self.layers) else {
                    return;
                };
                if self.drawables.len() >= 2 {
                    return;
                }
                let made = self.metal.next_drawable(&layer);
                self.counts.call(made.is_err());
                if let Ok(drawable) = made {
                    let texture = self.metal.drawable_texture(&drawable);
                    self.counts.call(false);
                    keep(random, &mut self.textures, (texture, Made::drawable()));
                    self.drawables.push(drawable);
                }
            }
            _ => {
                if self.drawables.is_empty() {
                    return;
                }
                let drawable = self
                    .drawables
                    .swap_remove(random.index(self.drawables.len()));
                let Some(command_buffer) = self.command_buffer(random) else {
                    return;
                };
                let refused = self
                    .metal
                    .present_drawable(&command_buffer, &drawable)
                    .is_err();
                self.counts.call(refused);
                if random.chance(50) {
                    let refused = self
                        .metal
                        .present_drawable(&command_buffer, &drawable)
                        .is_err();
                    self.counts.call(refused);
                }
            }
        }
    }

    /// Drops random objects, whose last handles may be the ones encoded command buffers hold.
    fn drop_some(&mut self, random: &mut Random) {
        match random.below(6) {
            0 if !self.buffers.is_empty() => {
                let held = self.buffers.swap_remove(random.index(self.buffers.len()));
                self.check(&held);
            }
            1 if !self.textures.is_empty() => {
                self.textures.swap_remove(random.index(self.textures.len()));
            }
            2 if !self.command_buffers.is_empty() => {
                self.command_buffers
                    .swap_remove(random.index(self.command_buffers.len()));
            }
            3 if !self.pipelines.is_empty() => {
                self.pipelines
                    .swap_remove(random.index(self.pipelines.len()));
            }
            4 if !self.functions.is_empty() => {
                self.functions
                    .swap_remove(random.index(self.functions.len()));
            }
            _ if !self.samplers.is_empty() => {
                self.samplers.swap_remove(random.index(self.samplers.len()));
            }
            _ => {}
        }
    }

    /// Makes objects of a second device, which is then dropped while they stay in the pools.
    fn other_device(&mut self, random: &mut Random) {
        let mut other = M::new(());
        let format = random.pick(DRAWN_FORMATS);
        let buffer = other.make_buffer(256);
        let descriptor = TextureDescriptor {
            texture_type: TextureType::Type2D,
            pixel_format: format,
            width: 4,
            height: 4,
            depth: 1,
            mipmap_level_count: 1,
            sample_count: 1,
            array_length: 1,
            usage: TextureUsage {
                shader_read: true,
                shader_write: false,
                render_target: true,
                pixel_format_view: false,
            },
        };
        let texture = other.make_texture(&descriptor);
        let command_buffer = other.make_command_buffer();
        drop(other);
        self.counts.call(false);
        if let Ok(buffer) = buffer {
            let held = Held {
                buffer,
                shadow: Shadow::new(256),
            };
            self.keep_buffer(random, held);
        }
        if let Ok(texture) = texture {
            keep(random, &mut self.textures, (texture, Made::of(&descriptor)));
        }
        if let Ok(command_buffer) = command_buffer {
            keep(random, &mut self.command_buffers, command_buffer);
        }
    }

    /// Ends the batch: commits what is left, waits for every command buffer, and reads every canary back.
    fn finish(mut self) -> Counts {
        for command_buffer in std::mem::take(&mut self.command_buffers) {
            // After a call taken that had to be refused, nothing more is committed.
            if self.counts.accepted == 0 && self.metal.commit(&command_buffer).is_ok() {
                self.last_committed = Some(command_buffer);
            }
        }
        if let Some(last) = self.last_committed.take() {
            let _ = self.metal.wait_until_completed(&last);
            if self.metal.status(&last) == CommandBufferStatus::Error {
                eprintln!("GPU-ERROR the last command buffer ended in an error");
                self.counts.gpu_errors = self.counts.gpu_errors.saturating_add(1);
            }
        }
        for held in std::mem::take(&mut self.buffers) {
            self.check(&held);
        }
        self.counts
    }
}

/// What a texture or view of the batch is, which the calls the batch makes valid follow.
#[derive(Clone, Copy)]
struct Made {
    format: PixelFormat,
    texture_type: TextureType,
    width: u32,
    height: u32,
    depth: u32,
    levels: u32,
    /// Slices, each face of a cube one.
    slices: u32,
    samples: u32,
    usage: TextureUsage,
    /// A drawable's texture, which only passes draw into.
    drawable: bool,
}

impl Made {
    /// Returns what a texture made of `d` is.
    fn of(d: &TextureDescriptor) -> Made {
        let cube = matches!(d.texture_type, TextureType::TypeCube | TextureType::TypeCubeArray);
        Made {
            format: d.pixel_format,
            texture_type: d.texture_type,
            width: d.width,
            height: d.height,
            depth: d.depth,
            levels: d.mipmap_level_count,
            slices: if cube {
                d.array_length.saturating_mul(CUBE_FACES)
            } else {
                d.array_length
            },
            samples: d.sample_count,
            usage: d.usage,
            drawable: false,
        }
    }

    /// Returns what a drawable's texture is, of the size the batch configures its layers to at first.
    fn drawable() -> Made {
        Made {
            format: PixelFormat::Bgra8Unorm,
            texture_type: TextureType::Type2D,
            width: 4,
            height: 4,
            depth: 1,
            levels: 1,
            slices: 1,
            samples: 1,
            usage: TextureUsage {
                shader_read: false,
                shader_write: false,
                render_target: true,
                pixel_format_view: false,
            },
            drawable: true,
        }
    }

    /// Returns what a view of `d` of this texture is.
    fn view(&self, d: &TextureViewDescriptor) -> Made {
        let side = |n: u32| n.checked_shr(d.first_level).unwrap_or(0).max(1);
        Made {
            format: d.pixel_format,
            texture_type: d.texture_type,
            width: side(self.width),
            height: side(self.height),
            depth: side(self.depth),
            levels: d.level_count,
            slices: d.slice_count,
            samples: self.samples,
            usage: TextureUsage {
                pixel_format_view: false,
                ..self.usage
            },
            drawable: false,
        }
    }

    /// Returns the sides of a level: only a 3D texture is deeper than one.
    fn level(&self, level: u32) -> (u32, u32, u32) {
        let side = |n: u32| n.checked_shr(level).unwrap_or(0).max(1);
        let depth = if self.texture_type == TextureType::Type3D {
            side(self.depth)
        } else {
            1
        };
        (side(self.width), side(self.height), depth)
    }
}

/// The faces of a cube. https://developer.apple.com/documentation/metal/mtltexturetype/typecube
const CUBE_FACES: u32 = 6;

/// The longest side of a 2D texture on this Mac's GPU, and of a 3D one, and the most layers.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const MOST_SIDE: u32 = 32768;
const MOST_3D_SIDE: u32 = 2048;
const MOST_LAYERS: u64 = 2048;

/// The bytes of the visibility result buffer a pass counts into, and of one result.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const VISIBILITY_SPAN: u64 = 262_144;
const RESULT_BYTES: u64 = 8;

/// The number type a function reads a texture's texels as, and depth.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Float,
    Uint,
    Sint,
    Depth,
}

/// Returns how functions read a format's texels.
fn kind_of(format: PixelFormat) -> Kind {
    use PixelFormat as P;
    match format {
        P::R8Uint
        | P::R16Uint
        | P::Rg8Uint
        | P::R32Uint
        | P::Rg16Uint
        | P::Rgba8Uint
        | P::Rgb10a2Uint
        | P::Rg32Uint
        | P::Rgba16Uint
        | P::Rgba32Uint
        | P::Stencil8
        | P::X32Stencil8 => Kind::Uint,
        P::R8Sint
        | P::R16Sint
        | P::Rg8Sint
        | P::R32Sint
        | P::Rg16Sint
        | P::Rgba8Sint
        | P::Rg32Sint
        | P::Rgba16Sint
        | P::Rgba32Sint => Kind::Sint,
        P::Depth16Unorm | P::Depth32Float | P::Depth32FloatStencil8 => Kind::Depth,
        _ => Kind::Float,
    }
}

/// Returns whether a format is compressed in blocks.
fn compressed(format: PixelFormat) -> bool {
    block(format).0 > 1
}

/// Returns whether a format holds depth or stencil.
fn depth_or_stencil(format: PixelFormat) -> bool {
    matches!(
        format,
        PixelFormat::Depth16Unorm
            | PixelFormat::Depth32Float
            | PixelFormat::Stencil8
            | PixelFormat::Depth32FloatStencil8
            | PixelFormat::X32Stencil8
    )
}

/// The color formats of each size a view may reinterpret a texture made for such views as.
const SAME_SIZE: &[(u64, &[PixelFormat])] = &[
    (1, &[PixelFormat::R8Unorm, PixelFormat::R8Uint, PixelFormat::R8Sint, PixelFormat::R8Snorm]),
    (2, &[PixelFormat::R16Float, PixelFormat::Rg8Unorm, PixelFormat::R16Uint, PixelFormat::Rg8Sint]),
    (
        4,
        &[
            PixelFormat::Rgba8Unorm,
            PixelFormat::R32Float,
            PixelFormat::R32Uint,
            PixelFormat::Rgba8Sint,
            PixelFormat::Rg16Float,
            PixelFormat::Bgra8Unorm,
        ],
    ),
    (8, &[PixelFormat::Rg32Float, PixelFormat::Rgba16Float, PixelFormat::Rgba16Uint]),
    (16, &[PixelFormat::Rgba32Float, PixelFormat::Rgba32Uint]),
];

/// The formats a fragment function of each output kind draws into, and those of them a multisampled target resolves.
const FLOAT_TARGETS: &[PixelFormat] = &[
    PixelFormat::Rgba8Unorm,
    PixelFormat::Bgra8Unorm,
    PixelFormat::Rgba16Float,
    PixelFormat::R32Float,
    PixelFormat::Rgba8UnormSrgb,
    PixelFormat::Rg11b10Float,
    PixelFormat::Rgb10a2Unorm,
    PixelFormat::R8Unorm,
];
const UINT_TARGETS: &[PixelFormat] = &[PixelFormat::Rgba8Uint, PixelFormat::R32Uint, PixelFormat::Rg16Uint];
const SINT_TARGETS: &[PixelFormat] = &[PixelFormat::Rgba8Sint, PixelFormat::R32Sint];

/// What one function reads at a slot, which a valid draw or dispatch binds.
#[derive(Clone, Copy)]
enum Need {
    /// A buffer of at least `least` bytes at an offset that is a multiple of `align`, which the function may write.
    Buffer {
        slot: u32,
        least: u64,
        align: u64,
        written: bool,
    },
    Texture {
        slot: u32,
        texture_type: TextureType,
        kind: Kind,
    },
    Sampler {
        slot: u32,
    },
}

/// Returns what an entry point of the programs reads, as compiled with `SLOTS`.
fn needs(entry: &str) -> &'static [Need] {
    const SOURCE: Need = Need::Buffer {
        slot: 1,
        least: 4,
        align: 4,
        written: false,
    };
    const COLOR: Need = Need::Buffer {
        slot: 0,
        least: 16,
        align: 16,
        written: false,
    };
    const SINK: Need = Need::Buffer {
        slot: 2,
        least: 4,
        align: 4,
        written: true,
    };
    const TAIL: Need = Need::Buffer {
        slot: 3,
        least: 8,
        align: 4,
        written: true,
    };
    match entry {
        "vs_pulled" => &[SOURCE],
        "fs_color" | "fs_two" => &[COLOR],
        "fs_write" => &[SOURCE, SINK, TAIL],
        "fs_textures" => &[
            SOURCE,
            Need::Texture { slot: 0, texture_type: TextureType::Type2D, kind: Kind::Float },
            Need::Texture { slot: 1, texture_type: TextureType::Type2D, kind: Kind::Uint },
            Need::Texture { slot: 2, texture_type: TextureType::Type2D, kind: Kind::Sint },
            Need::Texture { slot: 3, texture_type: TextureType::Type2DArray, kind: Kind::Float },
            Need::Texture { slot: 4, texture_type: TextureType::TypeCube, kind: Kind::Float },
            Need::Texture { slot: 5, texture_type: TextureType::Type3D, kind: Kind::Float },
            Need::Texture { slot: 6, texture_type: TextureType::Type2D, kind: Kind::Depth },
            Need::Texture { slot: 7, texture_type: TextureType::Type2DMultisample, kind: Kind::Float },
            Need::Sampler { slot: 0 },
            Need::Sampler { slot: 1 },
        ],
        "fs_sample_f" => &[Need::Texture { slot: 0, texture_type: TextureType::Type2D, kind: Kind::Float }, Need::Sampler { slot: 0 }],
        "fs_load_u" => &[SOURCE, Need::Texture { slot: 1, texture_type: TextureType::Type2D, kind: Kind::Uint }],
        "fs_imm" => &[Need::Buffer {
            slot: IMMEDIATES_AT,
            least: 16,
            align: 4,
            written: false,
        }],
        "cs_scatter" => &[SOURCE, SINK, TAIL],
        "cs_shared" => &[SOURCE, SINK],
        "cs_textures" => &[
            SOURCE,
            SINK,
            Need::Texture { slot: 1, texture_type: TextureType::Type2D, kind: Kind::Uint },
            Need::Texture { slot: 2, texture_type: TextureType::Type2D, kind: Kind::Sint },
            Need::Texture { slot: 5, texture_type: TextureType::Type3D, kind: Kind::Float },
        ],
        _ => &[],
    }
}

/// Returns the kind of values a fragment entry point writes to each color attachment.
fn outputs(entry: &str) -> &'static [Kind] {
    match entry {
        "fs_uint" | "fs_load_u" => &[Kind::Uint],
        "fs_sint" => &[Kind::Sint],
        "fs_two" => &[Kind::Float, Kind::Float],
        _ => &[Kind::Float],
    }
}

/// Returns the levels of a full mipmap chain for a largest side.
fn full_chain(side: u32) -> u32 {
    u32::BITS.saturating_sub(side.leading_zeros())
}

/// Returns whether `direct` must refuse a texture of `d`, by Apple's rules as the batch knows them.
fn texture_refused(d: &TextureDescriptor) -> bool {
    let most = if d.texture_type == TextureType::Type3D {
        MOST_3D_SIDE
    } else {
        MOST_SIDE
    };
    let cube = matches!(d.texture_type, TextureType::TypeCube | TextureType::TypeCubeArray);
    let layers = u64::from(d.array_length).saturating_mul(if cube { 6 } else { 1 });
    let one_slice = !matches!(d.texture_type, TextureType::Type2DArray | TextureType::TypeCubeArray);
    let multisampled = d.texture_type == TextureType::Type2DMultisample;
    d.width > most
        || d.height > most
        || d.depth > most
        || layers > MOST_LAYERS
        || d.pixel_format == PixelFormat::X32Stencil8
        || d.width == 0
        || d.height == 0
        || d.depth == 0
        || d.texture_type != TextureType::Type3D && d.depth != 1
        || one_slice && d.array_length != 1
        || d.array_length == 0
        || cube && d.width != d.height
        || d.mipmap_level_count == 0
        || d.mipmap_level_count > full_chain(d.width.max(d.height).max(d.depth))
        || (d.sample_count > 1) != multisampled
        || ![1, 2, 4, 8].contains(&d.sample_count)
        || d.sample_count > 1 && d.mipmap_level_count != 1
        || compressed(d.pixel_format) && (d.texture_type == TextureType::Type3D || multisampled)
        || depth_or_stencil(d.pixel_format) && d.texture_type == TextureType::Type3D
}

/// One region of a texture a valid copy names, and what it moves a block of.
#[derive(Clone, Copy)]
struct Region {
    slice: u32,
    level: u32,
    origin: Origin,
    size: Size,
}


/// A render pipeline the batch made of unmangled functions, with what its draws must bind.
#[derive(Clone)]
struct PlainPipeline {
    state: <M as Metal>::RenderPipelineState,
    vertex: &'static str,
    fragment: Option<&'static str>,
    colors: [Option<PixelFormat>; MAX_COLOR_ATTACHMENTS],
    depth: Option<PixelFormat>,
    stencil: Option<PixelFormat>,
    samples: u32,
    topology: PrimitiveTopologyClass,
}

/// What a global of the programs is bound as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Buffer,
    Texture,
    Sampler,
}

/// One target a pass draws into: its texture, its level, slice and plane, and what it is.
struct Target {
    texture: Texture,
    made: Made,
    level: u32,
    slice: u32,
    plane: u32,
}

/// Returns a primitive type of a topology class, or of another class.
fn primitive_of(random: &mut Random, class: PrimitiveTopologyClass, other: bool) -> PrimitiveType {
    let of = |class| match class {
        PrimitiveTopologyClass::Point => &[PrimitiveType::Point][..],
        PrimitiveTopologyClass::Line => &[PrimitiveType::Line, PrimitiveType::LineStrip][..],
        PrimitiveTopologyClass::Triangle => &[PrimitiveType::Triangle, PrimitiveType::TriangleStrip][..],
    };
    if !other {
        return random.pick(of(class));
    }
    let others: Vec<PrimitiveType> = PRIMITIVES
        .iter()
        .copied()
        .filter(|p| !of(class).contains(p))
        .collect();
    random.pick(&others)
}

/// Returns whether a texture region is one `direct` must refuse, of a block `side` texels wide.
fn region_refused(made: &Made, region: &Region, side: u32) -> bool {
    if region.level >= made.levels || region.slice >= made.slices {
        return true;
    }
    let (w, h, d) = made.level(region.level);
    let past = |at: u32, n: u32, most: u32| u64::from(at).saturating_add(u64::from(n)) > u64::from(most);
    let (o, s) = (region.origin, region.size);
    let partial = |at: u32, n: u32, most: u32| {
        !at.is_multiple_of(side) || !(n.is_multiple_of(side) || at.checked_add(n) == Some(most))
    };
    past(o.x, s.width, w)
        || past(o.y, s.height, h)
        || past(o.z, s.depth, d)
        || partial(o.x, s.width, w)
        || partial(o.y, s.height, h)
        || s.width == 0
        || s.height == 0
        || s.depth == 0
}

/// Returns whether a copy between a buffer of `length` bytes and a texture is one `direct` must refuse.
fn buffer_copy_refused(
    made: &Made,
    option: BlitOption,
    region: &Region,
    bytes_per_row: u64,
    bytes_per_image: u64,
    offset: u64,
    length: u64,
) -> bool {
    if made.samples != 1 || made.drawable {
        return true;
    }
    let (bytes, side) = match (option, made.format) {
        (BlitOption::None, PixelFormat::Depth32FloatStencil8) => return true,
        (BlitOption::None, format) => (block(format).1, block(format).0),
        (BlitOption::DepthFromDepthStencil, PixelFormat::Depth32FloatStencil8) => (4, 1),
        (BlitOption::StencilFromDepthStencil, PixelFormat::Depth32FloatStencil8) => (1, 1),
        _ => return true,
    };
    if region_refused(made, region, side) {
        return true;
    }
    let s = region.size;
    let row = u128::from(s.width.div_ceil(side)).saturating_mul(u128::from(bytes));
    let rows = u128::from(s.height.div_ceil(side));
    let longest = u128::from(MOST_SIDE / side).saturating_mul(u128::from(bytes));
    let pitch = u128::from(bytes_per_row);
    if pitch < row || !bytes_per_row.is_multiple_of(bytes) || pitch > longest {
        return true;
    }
    let flat = s.depth == 1 && bytes_per_image == 0;
    if !flat && u128::from(bytes_per_image) < rows.saturating_mul(pitch) {
        return true;
    }
    if !offset.is_multiple_of(bytes) {
        return true;
    }
    let total = u128::from(s.depth.saturating_sub(1))
        .saturating_mul(u128::from(bytes_per_image))
        .saturating_add(rows.saturating_sub(1).saturating_mul(pitch))
        .saturating_add(row);
    total > u128::from(u64::MAX) || u128::from(offset).saturating_add(total) > u128::from(length)
}

/// Returns whether a view `d` of a texture is one `direct` must refuse; a format it may take as an sRGB twin is not.
fn view_refused(made: &Made, d: &TextureViewDescriptor) -> bool {
    let past = |first: u32, count: u32, most: u32| u64::from(first).saturating_add(u64::from(count)) > u64::from(most);
    if made.drawable
        || d.level_count == 0
        || d.slice_count == 0
        || past(d.first_level, d.level_count, made.levels)
        || past(d.first_slice, d.slice_count, made.slices)
    {
        return true;
    }
    let (from, to) = (made.format, d.pixel_format);
    let plain = |f: PixelFormat| !compressed(f) && !depth_or_stencil(f);
    let format_ok = from == to
        || made.usage.pixel_format_view && plain(from) && plain(to) && block(from).1 == block(to).1
        || made.usage.pixel_format_view
            && from == PixelFormat::Depth32FloatStencil8
            && to == PixelFormat::X32Stencil8
        // A twin is of the same size and kind of block.
        || block(from) == block(to) && depth_or_stencil(from) == depth_or_stencil(to) && !depth_or_stencil(from);
    let slices_fit = match d.texture_type {
        TextureType::Type2D | TextureType::Type2DMultisample | TextureType::Type3D => d.slice_count == 1,
        TextureType::TypeCube => d.slice_count == CUBE_FACES,
        TextureType::TypeCubeArray => d.slice_count.is_multiple_of(CUBE_FACES),
        TextureType::Type2DArray => true,
    };
    let cube = matches!(d.texture_type, TextureType::TypeCube | TextureType::TypeCubeArray);
    let type_fits = match made.texture_type {
        TextureType::Type3D => d.texture_type == TextureType::Type3D,
        TextureType::Type2DMultisample => d.texture_type == TextureType::Type2DMultisample,
        TextureType::Type2D => matches!(d.texture_type, TextureType::Type2D | TextureType::Type2DArray),
        TextureType::Type2DArray | TextureType::TypeCube | TextureType::TypeCubeArray => {
            matches!(d.texture_type, TextureType::Type2D | TextureType::Type2DArray) || cube
        }
    };
    !format_ok || !slices_fit || !type_fits || cube && made.width != made.height
}

impl World {
    /// Makes one call of every field valid but at most one, drawn so that the deep checks see it.
    fn precise(&mut self, random: &mut Random) {
        match random.below(100) {
            0..=10 => self.precise_texture(random),
            11..=17 => self.precise_view(random),
            18..=27 => self.precise_buffer_texture_copy(random),
            28..=37 => self.precise_texture_copy(random),
            38..=43 => self.precise_buffer_copy_or_fill(random),
            44..=46 => self.precise_mipmaps(random),
            47..=51 => self.precise_pipeline(random),
            52..=77 => self.precise_render(random),
            78..=89 => self.precise_compute(random),
            90..=98 => self.precise_function(random),
            _ if !self.layers.is_empty() && random.chance(70) => {
                // A layer's drawables configured valid but for one field.
                let Some(layer) = any(random, &self.layers) else {
                    return;
                };
                let mut d = LayerDescriptor {
                    pixel_format: random.pick(&[PixelFormat::Bgra8Unorm, PixelFormat::Rgba16Float, PixelFormat::Bgra8UnormSrgb]),
                    width: random.pick(&[4, 8, 16]),
                    height: random.pick(&[4, 8, 16]),
                    display_sync_enabled: false,
                };
                match random.broken(4) {
                    Some(0) => d.pixel_format = random.pick(&[PixelFormat::Rgba8Unorm, PixelFormat::R32Float, PixelFormat::Depth32Float]),
                    Some(1) => d.width = 0,
                    Some(2) => d.height = 0,
                    Some(_) => d.width = random.past(MOST_SIDE, 0),
                    None => {}
                }
                let refuse = d.width == 0
                    || d.height == 0
                    || d.width > MOST_SIDE
                    || !matches!(d.pixel_format, PixelFormat::Bgra8Unorm | PixelFormat::Rgba16Float | PixelFormat::Bgra8UnormSrgb);
                let refused = self.metal.configure_layer(&layer, &d).is_err();
                self.counts.call(refused);
                self.must(refused, refuse, "a layer's configuration");
            }
            _ => {
                // A layer asked for off the main thread, of a device made there.
                let refused = std::thread::spawn(|| M::new(()).make_layer(&NoWindow).is_err())
                    .join()
                    .unwrap_or(true);
                self.counts.call(refused);
                self.must(refused, true, "a layer off the main thread");
            }
        }
    }

    /// Counts a call taken that had to be refused.
    fn must(&mut self, refused: bool, refuse: bool, what: &str) {
        if refuse && !refused {
            eprintln!("ACCEPTED {what}, which had to be refused");
            self.counts.accepted = self.counts.accepted.saturating_add(1);
        }
    }

    /// Returns a buffer of the pool of at least `length` bytes, or a new one of them.
    fn buffer_of(&mut self, random: &mut Random, length: u64) -> Option<Held> {
        for _ in 0..4 {
            if let Some(held) = any(random, &self.buffers)
                && held.shadow.borrow().length() >= length
            {
                return Some(held);
            }
        }
        if length > LARGEST_MADE {
            return None;
        }
        let made = self.metal.make_buffer(length);
        self.counts.call(made.is_err());
        let held = Held {
            buffer: made.ok()?,
            shadow: Shadow::new(length),
        };
        let fill: Vec<u8> = (0..length.min(65536)).map(|_| random.next() as u8).collect();
        if self.metal.write_contents(&held.buffer, 0, &fill).is_ok() {
            held.shadow.borrow_mut().wrote(0, &fill);
        }
        self.keep_buffer(random, held.clone());
        Some(held)
    }

    /// Returns a texture of the pool that `fits`, or makes one of `d` and keeps it.
    fn texture_like(
        &mut self,
        random: &mut Random,
        fits: impl Fn(&Made) -> bool,
        d: &TextureDescriptor,
    ) -> Option<(Texture, Made)> {
        if !self.textures.is_empty() && random.chance(70) {
            let start = random.index(self.textures.len());
            for at in 0..self.textures.len() {
                let (texture, made) = &self.textures[at.saturating_add(start).checked_rem(self.textures.len()).unwrap_or(0)];
                if fits(made) {
                    return Some((texture.clone(), *made));
                }
            }
        }
        self.make_texture_of(random, d)
    }

    /// Makes a texture of `d`, which must be valid, and keeps it.
    fn make_texture_of(&mut self, random: &mut Random, d: &TextureDescriptor) -> Option<(Texture, Made)> {
        let made = self.metal.make_texture(d);
        self.counts.call(made.is_err());
        let texture = made.ok()?;
        keep(random, &mut self.textures, (texture.clone(), Made::of(d)));
        Some((texture, Made::of(d)))
    }

    /// Returns a texture a function reads at a slot of `texture_type` and `kind`, made to be read.
    fn readable(&mut self, random: &mut Random, texture_type: TextureType, kind: Kind) -> Option<(Texture, Made)> {
        let format = match kind {
            Kind::Float => random.pick(&[PixelFormat::Rgba8Unorm, PixelFormat::R32Float, PixelFormat::Rgba16Float]),
            Kind::Uint => random.pick(&[PixelFormat::Rgba8Uint, PixelFormat::R32Uint]),
            Kind::Sint => PixelFormat::Rgba8Sint,
            Kind::Depth => random.pick(&[PixelFormat::Depth32Float, PixelFormat::Depth16Unorm]),
        };
        let cube = matches!(texture_type, TextureType::TypeCube | TextureType::TypeCubeArray);
        let d = TextureDescriptor {
            texture_type,
            pixel_format: format,
            width: 4,
            height: 4,
            depth: if texture_type == TextureType::Type3D { 4 } else { 1 },
            mipmap_level_count: 1,
            sample_count: if texture_type == TextureType::Type2DMultisample { 4 } else { 1 },
            array_length: if texture_type == TextureType::Type2DArray { 2 } else { 1 },
            usage: TextureUsage {
                shader_read: true,
                shader_write: false,
                render_target: texture_type == TextureType::Type2DMultisample,
                pixel_format_view: false,
            },
        };
        let _ = cube;
        self.texture_like(
            random,
            |m| {
                m.texture_type == texture_type
                    && kind_of(m.format) == kind
                    && m.usage.shader_read
                    && !m.drawable
                    && (kind != Kind::Float || !depth_or_stencil(m.format))
            },
            &d,
        )
    }

    /// Returns a texture of the pool that `fits`.
    fn pick_texture(&self, random: &mut Random, fits: impl Fn(&Made) -> bool) -> Option<(Texture, Made)> {
        let of: Vec<&(Texture, Made)> = self.textures.iter().filter(|(_, m)| fits(m)).collect();
        (!of.is_empty()).then(|| of[random.index(of.len())].clone())
    }

    /// Makes a texture of every field valid but at most one, and counts one taken that had to be refused.
    fn precise_texture(&mut self, random: &mut Random) {
        let texture_type = random.pick(TEXTURE_TYPES);
        let multisampled = texture_type == TextureType::Type2DMultisample;
        let cube = matches!(texture_type, TextureType::TypeCube | TextureType::TypeCubeArray);
        let pixel_format = match texture_type {
            TextureType::Type2DMultisample => random.pick(&[
                PixelFormat::Rgba8Unorm,
                PixelFormat::Rgba16Float,
                PixelFormat::R32Float,
                PixelFormat::Rgba8Uint,
                PixelFormat::Depth32Float,
                PixelFormat::Depth32FloatStencil8,
            ]),
            TextureType::Type3D => random.pick(&[
                PixelFormat::Rgba8Unorm,
                PixelFormat::R32Float,
                PixelFormat::Rgba8Uint,
                PixelFormat::Rgba16Float,
                PixelFormat::R8Unorm,
                PixelFormat::Rgba32Float,
            ]),
            _ => match random.pick(FORMATS) {
                PixelFormat::X32Stencil8 => PixelFormat::Rgba8Unorm,
                format => format,
            },
        };
        let side = random.pick(&[1, 2, 3, 4, 5, 8, 16, 32]);
        let height = if cube { side } else { random.pick(&[side, 1, 2, 4, 7]) };
        let depth = if texture_type == TextureType::Type3D {
            random.pick(&[1, 2, 3, 4, 8])
        } else {
            1
        };
        let chain = full_chain(side.max(height).max(depth));
        let mut d = TextureDescriptor {
            texture_type,
            pixel_format,
            width: side,
            height,
            depth,
            mipmap_level_count: if multisampled {
                1
            } else {
                1u32.saturating_add(random.below(u64::from(chain)) as u32)
            },
            sample_count: if multisampled { random.pick(&[4, 2]) } else { 1 },
            array_length: match texture_type {
                TextureType::Type2DArray | TextureType::TypeCubeArray => random.pick(&[1, 2, 3, 4]),
                _ => 1,
            },
            usage: TextureUsage {
                shader_read: random.chance(85),
                shader_write: random.chance(15),
                render_target: (random.chance(70) || multisampled) && !compressed(pixel_format),
                pixel_format_view: random.chance(40),
            },
        };
        let most = if texture_type == TextureType::Type3D {
            MOST_3D_SIDE
        } else {
            MOST_SIDE
        };
        match random.broken(9) {
            Some(0) => d.width = random.past(most, 0),
            Some(1) if cube => d.height = side.saturating_add(1),
            Some(1) => d.height = random.past(most, 0),
            Some(2) if texture_type == TextureType::Type3D => d.depth = random.past(MOST_3D_SIDE, 0),
            Some(2) => d.depth = random.pick(&[2, 0, u32::MAX]),
            Some(3) => d.mipmap_level_count = random.pick(&[0, chain.saturating_add(1), u32::MAX, 32]),
            Some(4) if multisampled => d.sample_count = random.pick(&[0, 1, 3, 16, u32::MAX]),
            Some(4) => d.sample_count = random.pick(&[2, 4, 0, 3]),
            Some(5) => {
                d.array_length = match texture_type {
                    TextureType::Type2DArray => random.pick(&[0, 2049, 4096, u32::MAX]),
                    TextureType::TypeCubeArray => random.pick(&[0, 342, 2049, u32::MAX / 3]),
                    _ => random.pick(&[0, 2, 6]),
                }
            }
            Some(6) => {
                d.pixel_format = match texture_type {
                    TextureType::Type3D => random.pick(&[
                        PixelFormat::Bc1Rgba,
                        PixelFormat::Astc4x4Ldr,
                        PixelFormat::Depth32Float,
                        PixelFormat::Stencil8,
                    ]),
                    TextureType::Type2DMultisample => {
                        random.pick(&[PixelFormat::Bc1Rgba, PixelFormat::Etc2Rgb8, PixelFormat::X32Stencil8])
                    }
                    _ => PixelFormat::X32Stencil8,
                }
            }
            Some(7) if multisampled => d.texture_type = TextureType::Type2D,
            Some(7) => {
                d.texture_type = TextureType::Type2DMultisample;
                d.array_length = 1;
                d.depth = 1;
            }
            Some(_) => {
                if random.chance(50) {
                    d.width = 0;
                } else {
                    d.height = 0;
                }
            }
            None => {}
        }
        let refuse = texture_refused(&d);
        // Only a texture the checks take is cut, to keep each batch's memory small.
        if !refuse && texture_bytes(&d) > TEXTURE_BUDGET {
            d.depth = d.depth.min(1);
            d.array_length = d.array_length.min(1);
            d.mipmap_level_count = 1;
            if !cube {
                d.height = d.height.min(1);
            }
            if texture_bytes(&d) > TEXTURE_BUDGET {
                d.width = d.width.min(4096);
                d.height = d.height.min(4096);
            }
        }
        let made = self.metal.make_texture(&d);
        self.counts.call(made.is_err());
        self.must(made.is_err(), refuse, "a texture");
        if let Ok(texture) = made
            && !refuse
        {
            keep(random, &mut self.textures, (texture, Made::of(&d)));
        }
    }

    /// Makes a view of a texture of every field valid but at most one.
    fn precise_view(&mut self, random: &mut Random) {
        let Some((texture, made)) = self.pick_texture(random, |_| true) else {
            return;
        };
        let types: &[TextureType] = match made.texture_type {
            TextureType::Type3D => &[TextureType::Type3D],
            TextureType::Type2DMultisample => &[TextureType::Type2DMultisample],
            TextureType::Type2D => &[TextureType::Type2D, TextureType::Type2DArray],
            _ if made.width == made.height && made.slices >= CUBE_FACES => &[
                TextureType::Type2D,
                TextureType::Type2DArray,
                TextureType::TypeCube,
                TextureType::TypeCubeArray,
            ],
            _ => &[TextureType::Type2D, TextureType::Type2DArray],
        };
        let texture_type = random.pick(types);
        let slices = u64::from(made.slices.max(1));
        let (first_slice, slice_count) = match texture_type {
            TextureType::Type2DArray => {
                let first = random.below(slices);
                (first, 1u64.saturating_add(random.below(slices.saturating_sub(first))))
            }
            TextureType::TypeCube => (
                random.below(slices.saturating_sub(5)),
                u64::from(CUBE_FACES),
            ),
            TextureType::TypeCubeArray => (
                0,
                u64::from(CUBE_FACES).saturating_mul(1u64.saturating_add(random.below(slices / 6))),
            ),
            _ => (random.below(slices), 1),
        };
        let levels = u64::from(made.levels.max(1));
        let first_level = random.below(levels);
        let plain = |f: PixelFormat| !compressed(f) && !depth_or_stencil(f);
        let pixel_format = if made.usage.pixel_format_view && plain(made.format) && random.chance(40) {
            let bytes = block(made.format).1;
            SAME_SIZE
                .iter()
                .find(|(size, _)| *size == bytes)
                .map_or(made.format, |(_, formats)| random.pick(formats))
        } else if made.usage.pixel_format_view
            && made.format == PixelFormat::Depth32FloatStencil8
            && random.chance(30)
        {
            PixelFormat::X32Stencil8
        } else {
            made.format
        };
        let mut d = TextureViewDescriptor {
            pixel_format,
            texture_type,
            first_level: first_level as u32,
            level_count: 1u64.saturating_add(random.below(levels.saturating_sub(first_level))) as u32,
            first_slice: first_slice as u32,
            slice_count: slice_count as u32,
        };
        match random.broken(6) {
            Some(0) => d.first_level = random.past(made.levels.saturating_sub(d.level_count), d.level_count),
            Some(1) => d.level_count = random.pick(&[0, made.levels.saturating_add(1), u32::MAX]),
            Some(2) => d.first_slice = random.past(made.slices.saturating_sub(d.slice_count), d.slice_count),
            Some(3) => d.slice_count = random.pick(&[0, made.slices.saturating_add(1), u32::MAX, 5]),
            Some(4) => {
                let others: Vec<PixelFormat> = FORMATS
                    .iter()
                    .copied()
                    .filter(|f| block(*f) != block(made.format) || depth_or_stencil(*f) != depth_or_stencil(made.format))
                    .collect();
                d.pixel_format = random.pick(&others);
            }
            Some(_) => {
                d.texture_type = match made.texture_type {
                    TextureType::Type3D | TextureType::Type2DMultisample => TextureType::Type2D,
                    TextureType::Type2D => random.pick(&[TextureType::Type3D, TextureType::TypeCube]),
                    _ => random.pick(&[TextureType::Type3D, TextureType::Type2DMultisample]),
                }
            }
            None => {}
        }
        let refuse = view_refused(&made, &d);
        let view = self.metal.make_texture_view(&texture, &d);
        self.counts.call(view.is_err());
        self.must(view.is_err(), refuse, "a texture view");
        if let Ok(view) = view
            && !refuse
        {
            keep(random, &mut self.textures, (view, made.view(&d)));
        }
    }

    /// Returns a region of a texture inside one of its levels, in whole blocks of `side` texels.
    fn valid_region(random: &mut Random, made: &Made, side: u32) -> Region {
        let level = random.below(u64::from(made.levels.max(1))) as u32;
        let (w, h, d) = made.level(level);
        let slice = if made.texture_type == TextureType::Type3D {
            0
        } else {
            random.below(u64::from(made.slices.max(1))) as u32
        };
        let axis = |random: &mut Random, most: u32| {
            let blocks = most.div_ceil(side);
            let at = (random.below(u64::from(blocks)) as u32).saturating_mul(side);
            let rest = most.saturating_sub(at);
            let n = if random.chance(50) {
                rest
            } else {
                side.saturating_mul(1u32.saturating_add(random.below(u64::from(blocks)) as u32))
                    .min(rest)
            };
            (at, n.max(1).min(rest))
        };
        let (x, width) = axis(random, w);
        let (y, height) = axis(random, h);
        let z = random.below(u64::from(d)) as u32;
        let depth = 1u32.saturating_add(random.below(u64::from(d.saturating_sub(z))) as u32);
        Region {
            slice,
            level,
            origin: Origin { x, y, z },
            size: Size {
                width,
                height,
                depth,
            },
        }
    }

    /// Breaks one field of a region, as `field` names: its slice, level, origin or size.
    fn break_region(random: &mut Random, made: &Made, region: &mut Region, field: u64) {
        let (w, h, d) = made.level(region.level);
        let s = region.size;
        match field {
            0 => region.slice = random.past(made.slices.saturating_sub(1), 0),
            1 => region.level = random.past(made.levels.saturating_sub(1), 0),
            2 => region.origin.x = random.past(w.saturating_sub(s.width), s.width),
            3 => region.origin.y = random.past(h.saturating_sub(s.height), s.height),
            4 => region.origin.z = random.past(d.saturating_sub(s.depth), s.depth),
            5 => region.size.width = random.pick(&[0, w.saturating_sub(region.origin.x).saturating_add(1), u32::MAX]),
            6 => region.size.height = random.pick(&[0, h.saturating_sub(region.origin.y).saturating_add(1), u32::MAX]),
            _ => region.size.depth = random.pick(&[0, d.saturating_sub(region.origin.z).saturating_add(1), u32::MAX]),
        }
    }

    /// Copies between a buffer and a texture region, every field valid but at most one.
    fn precise_buffer_texture_copy(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let Some((mut texture, mut made)) = self.pick_texture(random, |m| {
            !m.drawable && m.samples == 1 && m.format != PixelFormat::X32Stencil8
        }) else {
            return;
        };
        let wrong = random.broken(14);
        let w = |k| wrong == Some(k);
        let mut option = BlitOption::None;
        let (mut bytes, mut side) = (block(made.format).1, block(made.format).0);
        if made.format == PixelFormat::Depth32FloatStencil8 {
            (option, bytes, side) = if random.chance(50) {
                (BlitOption::DepthFromDepthStencil, 4, 1)
            } else {
                (BlitOption::StencilFromDepthStencil, 1, 1)
            };
        }
        let mut region = Self::valid_region(random, &made, side);
        let s = region.size;
        let row = u64::from(s.width.div_ceil(side)).saturating_mul(bytes);
        let rows = u64::from(s.height.div_ceil(side));
        let mut bytes_per_row = row.saturating_mul(random.pick(&[1, 1, 2, 3]));
        let pitched = rows.saturating_mul(bytes_per_row);
        let mut bytes_per_image = if s.depth == 1 && random.chance(50) {
            0
        } else {
            pitched.saturating_mul(random.pick(&[1, 1, 2]))
        };
        let mut offset = bytes.saturating_mul(random.below(4));
        let span = u64::from(s.depth.saturating_sub(1))
            .saturating_mul(bytes_per_image)
            .saturating_add(rows.saturating_sub(1).saturating_mul(bytes_per_row))
            .saturating_add(row);
        let Some(mut held) = self.buffer_of(random, offset.saturating_add(span)) else {
            return;
        };
        if let Some(field) = wrong.filter(|k| *k < 8) {
            Self::break_region(random, &made, &mut region, field);
        }
        if w(8) {
            bytes_per_row = random.pick(&[
                row.saturating_sub(1),
                row.saturating_add(1),
                u64::from(MOST_SIDE / side).saturating_mul(bytes).saturating_add(bytes),
                u64::MAX,
                0,
            ]);
        }
        if w(9) {
            bytes_per_image = random.pick(&[pitched.saturating_sub(1), u64::MAX / 2 + 1, 1 << 63, u64::MAX]);
        }
        if w(10) {
            offset = random.pick(&[offset.saturating_add(1), u64::MAX - 3, held.shadow.borrow().length()]);
        }
        if w(11) {
            option = match option {
                BlitOption::None => random.pick(&[BlitOption::DepthFromDepthStencil, BlitOption::StencilFromDepthStencil]),
                _ => BlitOption::None,
            };
        }
        if w(12)
            && let Some((other, other_made)) =
                self.pick_texture(random, |m| m.samples > 1 || m.drawable)
        {
            (texture, made) = (other, other_made);
        }
        if w(13) {
            let short = offset.saturating_add(span).saturating_sub(1);
            let made_buffer = self.metal.make_buffer(short);
            self.counts.call(made_buffer.is_err());
            if let Ok(buffer) = made_buffer {
                held = Held {
                    buffer,
                    shadow: Shadow::new(short),
                };
                self.keep_buffer(random, held.clone());
            }
        }
        let length = held.shadow.borrow().length();
        let refuse = buffer_copy_refused(&made, option, &region, bytes_per_row, bytes_per_image, offset, length);
        let mut allowed: Vec<Allowed> = Vec::new();
        let command = if random.chance(50) {
            BlitCommand::CopyFromBufferToTexture {
                source_buffer: held.buffer.clone(),
                source_offset: offset,
                source_bytes_per_row: bytes_per_row,
                source_bytes_per_image: bytes_per_image,
                source_size: region.size,
                destination_texture: texture,
                destination_slice: region.slice,
                destination_level: region.level,
                destination_origin: region.origin,
                options: option,
            }
        } else {
            let format = match option {
                BlitOption::DepthFromDepthStencil => PixelFormat::Depth32Float,
                BlitOption::StencilFromDepthStencil => PixelFormat::Stencil8,
                BlitOption::None => made.format,
            };
            for (start, size) in copy_rows(format, offset, bytes_per_row, bytes_per_image, region.size) {
                allowed.push((held.shadow.clone(), start, size));
            }
            BlitCommand::CopyFromTextureToBuffer {
                source_texture: texture,
                source_slice: region.slice,
                source_level: region.level,
                source_origin: region.origin,
                source_size: region.size,
                destination_buffer: held.buffer.clone(),
                destination_offset: offset,
                destination_bytes_per_row: bytes_per_row,
                destination_bytes_per_image: bytes_per_image,
                options: option,
            }
        };
        let refused = self.metal.encode_blit_commands(&command_buffer, &[command]).is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &allowed, &format!("a copy between a buffer and a texture, break {wrong:?}"));
    }

    /// Copies a region between two textures, or within one, every field valid but at most one.
    fn precise_texture_copy(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let Some((mut source, mut source_made)) =
            self.pick_texture(random, |m| !m.drawable && m.format != PixelFormat::X32Stencil8)
        else {
            return;
        };
        let wrong = random.broken(11);
        let w = |k| wrong == Some(k);
        let side = block(source_made.format).0;
        let mut region = Self::valid_region(random, &source_made, side);
        // Into another slice of the texture itself, or into a texture of the same format and sample count.
        let within = (w(9) || random.chance(25))
            && source_made.slices > 1
            && source_made.texture_type != TextureType::Type3D;
        let (mut destination, mut destination_made, mut destination_region) = if within || w(9) {
            let mut other = region;
            if !w(9) {
                other.slice = region.slice.saturating_add(1).checked_rem(source_made.slices).unwrap_or(0);
            }
            (source.clone(), source_made, other)
        } else {
            let s = region.size;
            let grow = if random.chance(30) { side.saturating_mul(2) } else { 0 };
            let three_d = s.depth > 1;
            let format = if w(7) {
                let bytes = block(source_made.format).1;
                SAME_SIZE
                    .iter()
                    .find(|(size, _)| *size == bytes)
                    .map(|(_, formats)| random.pick(formats))
                    .filter(|f| *f != source_made.format)
                    .unwrap_or(if source_made.format == PixelFormat::Rgba8Unorm {
                        PixelFormat::Rgba8Uint
                    } else {
                        PixelFormat::Rgba8Unorm
                    })
            } else {
                source_made.format
            };
            let samples = if w(8) {
                if source_made.samples > 1 { 1 } else { 4 }
            } else {
                source_made.samples
            };
            let d = TextureDescriptor {
                texture_type: if three_d {
                    TextureType::Type3D
                } else if samples > 1 {
                    TextureType::Type2DMultisample
                } else {
                    TextureType::Type2D
                },
                pixel_format: format,
                width: s.width.saturating_add(grow),
                height: s.height.saturating_add(grow),
                depth: if three_d { s.depth } else { 1 },
                mipmap_level_count: 1,
                sample_count: samples,
                array_length: 1,
                usage: TextureUsage {
                    shader_read: true,
                    shader_write: false,
                    render_target: samples > 1 || random.chance(50),
                    pixel_format_view: false,
                },
            };
            let Some((texture, made)) = self.make_texture_of(random, &d) else {
                return;
            };
            let at = if grow > 0 { side } else { 0 };
            let to = Region {
                slice: 0,
                level: 0,
                origin: Origin { x: at, y: at, z: 0 },
                size: region.size,
            };
            (texture, made, to)
        };
        match wrong {
            Some(field @ 0..=3) => Self::break_region(random, &source_made, &mut region, [0, 1, 2, 5][field as usize]),
            Some(field @ 4..=6) => {
                Self::break_region(random, &destination_made, &mut destination_region, [0, 1, 3][usize::try_from(field).unwrap_or(4).saturating_sub(4)])
            }
            Some(10) => {
                // A drawable's texture and one of its format and size, so that only the drawable is wrong.
                let plain = TextureDescriptor {
                    texture_type: TextureType::Type2D,
                    pixel_format: PixelFormat::Bgra8Unorm,
                    width: 4,
                    height: 4,
                    depth: 1,
                    mipmap_level_count: 1,
                    sample_count: 1,
                    array_length: 1,
                    usage: TextureUsage {
                        shader_read: true,
                        shader_write: false,
                        render_target: true,
                        pixel_format_view: false,
                    },
                };
                if let Some(drawable) = self.pick_texture(random, |m| m.drawable)
                    && let Some(other) = self.make_texture_of(random, &plain)
                {
                    let first = Region {
                        slice: 0,
                        level: 0,
                        origin: Origin { x: 0, y: 0, z: 0 },
                        size: Size { width: 1, height: 1, depth: 1 },
                    };
                    (region, destination_region) = (first, first);
                    ((source, source_made), (destination, destination_made)) =
                        if random.chance(50) { (drawable, other) } else { (other, drawable) };
                }
            }
            _ => {}
        }
        let same = within || w(9);
        let overlap = |a: u32, b: u32, n: u32| {
            !(u64::from(a).saturating_add(u64::from(n)) <= u64::from(b)
                || u64::from(b).saturating_add(u64::from(n)) <= u64::from(a))
        };
        let s = region.size;
        let (o, p) = (region.origin, destination_region.origin);
        let overlapping = same
            && region.level == destination_region.level
            && region.slice == destination_region.slice
            && overlap(o.x, p.x, s.width)
            && overlap(o.y, p.y, s.height)
            && overlap(o.z, p.z, s.depth);
        let refuse = source_made.drawable
            || destination_made.drawable
            || source_made.format != destination_made.format
            || source_made.samples != destination_made.samples
            || region_refused(&source_made, &region, side)
            || region_refused(
                &destination_made,
                &Region {
                    size: region.size,
                    ..destination_region
                },
                block(destination_made.format).0,
            )
            || overlapping;
        let command = BlitCommand::CopyFromTextureToTexture {
            source_texture: source,
            source_slice: region.slice,
            source_level: region.level,
            source_origin: region.origin,
            source_size: region.size,
            destination_texture: destination,
            destination_slice: destination_region.slice,
            destination_level: destination_region.level,
            destination_origin: destination_region.origin,
        };
        let refused = self.metal.encode_blit_commands(&command_buffer, &[command]).is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &[], &format!("a copy between textures, break {wrong:?}"));
    }

    /// Copies bytes between buffers or fills a range, every field valid but at most one.
    fn precise_buffer_copy_or_fill(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let Some(source) = self.buffer_of(random, 64) else {
            return;
        };
        let same = random.chance(25);
        let Some(destination) = (if same { Some(source.clone()) } else { self.buffer_of(random, 64) }) else {
            return;
        };
        let (source_length, destination_length) = (source.shadow.borrow().length(), destination.shadow.borrow().length());
        let words = |n: u64| n / 4;
        let mut size = 4u64.saturating_mul(1u64.saturating_add(random.below(words(source_length.min(destination_length)).min(16))));
        let mut source_offset = 4u64.saturating_mul(random.below(words(source_length.saturating_sub(size)).saturating_add(1)));
        let mut destination_offset = 4u64.saturating_mul(random.below(words(destination_length.saturating_sub(size)).saturating_add(1)));
        if same && random.chance(90) {
            // Two ranges apart: the second after the first, if the buffer holds both.
            source_offset = 0;
            destination_offset = size;
            size = size.min(source_length / 2 / 4 * 4).max(4);
        }
        let fill = random.chance(40);
        match random.broken(6) {
            Some(0) => size = 0,
            Some(1) => size = size.saturating_add(random.pick(&[1, 2, 3])),
            Some(2) => source_offset = random.past_64(source_length.saturating_sub(size), size),
            Some(3) => destination_offset = random.past_64(destination_length.saturating_sub(size), size),
            Some(4) => destination_offset = destination_offset.saturating_add(2),
            Some(_) if same => destination_offset = source_offset.saturating_add(random.pick(&[0, 4]).min(size.saturating_sub(4))),
            _ => {}
        }
        let inside = |offset: u64, length: u64| offset.checked_add(size).is_some_and(|end| end <= length);
        let words_ok = [size, source_offset, destination_offset].iter().all(|n| n.is_multiple_of(4));
        let (command, refuse, allowed): (BlitCommand<M>, bool, Vec<Allowed>) = if fill {
            let refuse = size == 0 || !size.is_multiple_of(4) || !destination_offset.is_multiple_of(4)
                || !inside(destination_offset, destination_length);
            (
                BlitCommand::FillBuffer {
                    buffer: destination.buffer.clone(),
                    offset: destination_offset,
                    length: size,
                    value: random.next() as u8,
                },
                refuse,
                vec![(destination.shadow.clone(), destination_offset, size)],
            )
        } else {
            let apart = |a: u64, b: u64| a.checked_add(size).is_some_and(|end| end <= b);
            let refuse = size == 0
                || !words_ok
                || !inside(source_offset, source_length)
                || !inside(destination_offset, destination_length)
                || same && !(apart(source_offset, destination_offset) || apart(destination_offset, source_offset));
            (
                BlitCommand::CopyFromBufferToBuffer {
                    source_buffer: source.buffer.clone(),
                    source_offset,
                    destination_buffer: destination.buffer.clone(),
                    destination_offset,
                    size,
                },
                refuse,
                vec![(destination.shadow.clone(), destination_offset, size)],
            )
        };
        let refused = self.metal.encode_blit_commands(&command_buffer, &[command]).is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &allowed, "a copy or fill of bytes");
    }

    /// Generates the mipmaps of a texture that takes them, or of one that does not.
    fn precise_mipmaps(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let filtered = |m: &Made| {
            m.levels > 1 && kind_of(m.format) == Kind::Float && !compressed(m.format) && m.samples == 1 && !m.drawable
        };
        let wrong = random.broken(4);
        let found = match wrong {
            Some(0) => self.pick_texture(random, |m| m.levels == 1 && !m.drawable),
            Some(1) => self.pick_texture(random, |m| m.levels > 1 && kind_of(m.format) != Kind::Float),
            Some(2) => self.pick_texture(random, |m| m.levels > 1 && compressed(m.format)),
            Some(_) => self.pick_texture(random, |m| m.drawable),
            None => None,
        };
        let found = match found {
            Some(found) => Some(found),
            None => {
                let format = random.pick(&[
                    PixelFormat::Rgba8Unorm,
                    PixelFormat::Bgra8Unorm,
                    PixelFormat::Rgba16Float,
                    PixelFormat::R32Float,
                    PixelFormat::R8Unorm,
                    PixelFormat::Rgba8UnormSrgb,
                ]);
                let d = TextureDescriptor {
                    texture_type: random.pick(&[TextureType::Type2D, TextureType::Type2DArray, TextureType::Type3D]),
                    pixel_format: format,
                    width: 8,
                    height: 8,
                    depth: 1,
                    mipmap_level_count: 4,
                    sample_count: 1,
                    array_length: 1,
                    usage: TextureUsage {
                        shader_read: true,
                        shader_write: false,
                        render_target: true,
                        pixel_format_view: false,
                    },
                };
                self.texture_like(random, filtered, &d)
            }
        };
        let Some((texture, made)) = found else {
            return;
        };
        let refuse = !filtered(&made);
        let refused = self
            .metal
            .encode_blit_commands(&command_buffer, &[BlitCommand::GenerateMipmaps { texture }])
            .is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &[], "mipmaps");
    }

    /// Returns the function compiled unmangled of an entry point.
    fn plain(&self, entry: &str) -> Option<<M as Metal>::Function> {
        let at = ENTRIES.iter().position(|(name, _)| *name == entry)?;
        self.plain_functions.get(at).cloned().flatten()
    }

    /// Makes a render pipeline of unmangled functions, every field valid but at most one.
    fn precise_pipeline(&mut self, random: &mut Random) {
        let vertex = random.pick(&["vs_plain", "vs_pulled", "vs_inputs"]);
        let fragment = random.chance(85).then(|| {
            random.pick(&[
                "fs_color", "fs_uint", "fs_sint", "fs_two", "fs_write", "fs_textures", "fs_sample_f", "fs_load_u", "fs_imm",
            ])
        });
        let mut colors: [Option<RenderPipelineColorAttachmentDescriptor>; MAX_COLOR_ATTACHMENTS] = [None; MAX_COLOR_ATTACHMENTS];
        let kinds: &[Kind] = match fragment {
            Some(entry) => outputs(entry),
            None if random.chance(70) => &[Kind::Float],
            None => &[],
        };
        for (at, kind) in kinds.iter().enumerate() {
            let format = match kind {
                Kind::Uint => random.pick(UINT_TARGETS),
                Kind::Sint => random.pick(SINT_TARGETS),
                _ => random.pick(FLOAT_TARGETS),
            };
            let blends = *kind == Kind::Float && random.chance(25);
            colors[at] = Some(RenderPipelineColorAttachmentDescriptor {
                pixel_format: format,
                blending: blends.then(|| Blending {
                    rgb_blend_operation: random.pick(BLEND_OPERATIONS),
                    alpha_blend_operation: random.pick(BLEND_OPERATIONS),
                    source_rgb_blend_factor: random.pick(&BLEND_FACTORS[..9]),
                    destination_rgb_blend_factor: random.pick(&BLEND_FACTORS[..9]),
                    source_alpha_blend_factor: random.pick(&BLEND_FACTORS[..9]),
                    destination_alpha_blend_factor: random.pick(&BLEND_FACTORS[..9]),
                }),
                write_mask: ColorWriteMask {
                    red: true,
                    green: true,
                    blue: true,
                    alpha: random.chance(90),
                },
            });
        }
        let mut depth = random.chance(40).then(|| {
            random.pick(&[PixelFormat::Depth32Float, PixelFormat::Depth16Unorm, PixelFormat::Depth32FloatStencil8])
        });
        let mut stencil = match depth {
            Some(PixelFormat::Depth32FloatStencil8) if random.chance(70) => depth,
            None if random.chance(15) => Some(PixelFormat::Stencil8),
            _ => None,
        };
        let mut samples = if random.chance(75) { 1 } else { 4 };
        let topology = random.pick(&[
            PrimitiveTopologyClass::Triangle,
            PrimitiveTopologyClass::Triangle,
            PrimitiveTopologyClass::Triangle,
            PrimitiveTopologyClass::Line,
            PrimitiveTopologyClass::Point,
        ]);
        let first_float = colors[0].is_some_and(|c| kind_of(c.pixel_format) == Kind::Float);
        let mut alpha_to_coverage = first_float && random.chance(10);
        let (mut vertex_function, mut fragment_function) = (self.plain(vertex), fragment.and_then(|f| self.plain(f)));
        let wrong = random.broken(11);
        let mut refuse = wrong.is_some();
        match wrong {
            Some(0) => vertex_function = self.plain(random.pick(&["fs_color", "cs_scatter"])),
            Some(1) if fragment_function.is_some() => fragment_function = self.plain(random.pick(&["vs_plain", "cs_shared"])),
            Some(2) => samples = random.pick(&[3, 0, 16, u32::MAX]),
            Some(3) => depth = Some(random.pick(&[PixelFormat::Stencil8, PixelFormat::Rgba8Unorm, PixelFormat::X32Stencil8])),
            Some(4) => stencil = Some(random.pick(&[PixelFormat::Depth32Float, PixelFormat::X32Stencil8, PixelFormat::R8Uint])),
            Some(5) => (depth, stencil) = (Some(PixelFormat::Depth32FloatStencil8), Some(PixelFormat::Stencil8)),
            Some(6) => {
                colors[0] = Some(RenderPipelineColorAttachmentDescriptor {
                    pixel_format: PixelFormat::Rgba8Unorm,
                    blending: Some(Blending {
                        rgb_blend_operation: BlendOperation::Add,
                        alpha_blend_operation: BlendOperation::Add,
                        source_rgb_blend_factor: BlendFactor::Source1Color,
                        destination_rgb_blend_factor: BlendFactor::OneMinusSource1Alpha,
                        source_alpha_blend_factor: BlendFactor::One,
                        destination_alpha_blend_factor: BlendFactor::Zero,
                    }),
                    write_mask: ColorWriteMask { red: true, green: true, blue: true, alpha: true },
                });
                let at = 1usize.saturating_add(random.index(MAX_COLOR_ATTACHMENTS.saturating_sub(1)));
                colors[at] = Some(RenderPipelineColorAttachmentDescriptor { blending: None, ..colors[0].unwrap_or_else(|| unreachable!()) });
            }
            Some(7) => {
                let at = random.index(MAX_COLOR_ATTACHMENTS);
                colors[at] = Some(RenderPipelineColorAttachmentDescriptor {
                    pixel_format: random.pick(&[PixelFormat::Depth32Float, PixelFormat::Bc1Rgba, PixelFormat::Stencil8, PixelFormat::Astc4x4Ldr]),
                    blending: None,
                    write_mask: ColorWriteMask { red: true, green: true, blue: true, alpha: true },
                });
            }
            Some(8) => {
                colors[0] = Some(RenderPipelineColorAttachmentDescriptor {
                    pixel_format: PixelFormat::Rgb9e5Float,
                    blending: None,
                    write_mask: ColorWriteMask { red: true, green: false, blue: true, alpha: random.chance(50) },
                });
            }
            Some(9) => {
                colors[0] = Some(RenderPipelineColorAttachmentDescriptor {
                    pixel_format: random.pick(UINT_TARGETS),
                    blending: Some(Blending {
                        rgb_blend_operation: BlendOperation::Add,
                        alpha_blend_operation: BlendOperation::Add,
                        source_rgb_blend_factor: BlendFactor::One,
                        destination_rgb_blend_factor: BlendFactor::One,
                        source_alpha_blend_factor: BlendFactor::One,
                        destination_alpha_blend_factor: BlendFactor::One,
                    }),
                    write_mask: ColorWriteMask { red: true, green: true, blue: true, alpha: true },
                });
            }
            Some(_) => {
                colors[0] = Some(RenderPipelineColorAttachmentDescriptor {
                    pixel_format: random.pick(SINT_TARGETS),
                    blending: None,
                    write_mask: ColorWriteMask { red: true, green: true, blue: true, alpha: true },
                });
                alpha_to_coverage = true;
            }
            None => {}
        }
        if wrong == Some(1) && fragment.is_none() {
            refuse = false;
        }
        let Some(vertex_function) = vertex_function else {
            return;
        };
        let made = self.metal.make_render_pipeline_state(&RenderPipelineDescriptor {
            vertex_function: &vertex_function,
            fragment_function: fragment_function.as_ref(),
            color_attachments: colors,
            depth_attachment_pixel_format: depth,
            stencil_attachment_pixel_format: stencil,
            raster_sample_count: samples,
            alpha_to_coverage_enabled: alpha_to_coverage,
            rasterization_enabled: random.chance(95),
            input_primitive_topology: topology,
        });
        self.counts.call(made.is_err());
        self.must(made.is_err(), refuse, "a render pipeline");
        if let Ok(state) = made
            && !refuse
        {
            let pipeline = PlainPipeline {
                state,
                vertex,
                fragment,
                colors: colors.map(|c| c.map(|c| c.pixel_format)),
                depth,
                stencil,
                samples,
                topology,
            };
            keep(random, &mut self.plain_pipelines, pipeline);
        }
    }

    /// Makes a texture a pass draws into, as `layout` lays its targets out.
    fn target(
        &mut self,
        random: &mut Random,
        format: PixelFormat,
        samples: u32,
        side: u32,
        layout: u64,
        render_target: bool,
    ) -> Option<Target> {
        let aspect = depth_or_stencil(format);
        let layout = if samples > 1 { 0 } else if aspect && layout == 3 { 0 } else { layout };
        let (texture_type, width, levels, array_length, depth) = match layout {
            1 => (TextureType::Type2DArray, side, 1, 3, 1),
            2 => (TextureType::Type2D, side.saturating_mul(2), 2, 1, 1),
            3 => (TextureType::Type3D, side, 1, 1, 4),
            _ if samples > 1 => (TextureType::Type2DMultisample, side, 1, 1, 1),
            _ => (TextureType::Type2D, side, 1, 1, 1),
        };
        let d = TextureDescriptor {
            texture_type,
            pixel_format: format,
            width,
            height: width,
            depth,
            mipmap_level_count: levels,
            sample_count: samples,
            array_length,
            usage: TextureUsage {
                shader_read: random.chance(50),
                shader_write: false,
                render_target,
                pixel_format_view: false,
            },
        };
        let made = self.metal.make_texture(&d);
        self.counts.call(made.is_err());
        let texture = made.ok()?;
        keep(random, &mut self.textures, (texture.clone(), Made::of(&d)));
        Some(Target {
            texture,
            made: Made::of(&d),
            level: if layout == 2 { 1 } else { 0 },
            slice: if layout == 1 { 1u32.saturating_add(random.below(2) as u32) } else { 0 },
            plane: if layout == 3 { random.below(4) as u32 } else { 0 },
        })
    }

    /// Binds what an entry point reads, valid but for the slot `wrong` names when it names one of this stage.
    ///
    /// Returns whether it broke a read, so that the draw must be refused.
    fn bind_reads(
        &mut self,
        random: &mut Random,
        entry: &str,
        vertex: bool,
        wrong: Option<u64>,
        commands: &mut Vec<RenderCommand<M>>,
        allowed: &mut Vec<Allowed>,
    ) -> bool {
        let mut broke = false;
        let w = |k| wrong == Some(k);
        let all = needs(entry);
        // The one read the break is about: each kind of break takes the first read it applies to.
        let target = |pred: &dyn Fn(&Need) -> bool| all.iter().position(|n| pred(n));
        let buffer_at = target(&|n| matches!(n, Need::Buffer { .. }));
        let written_at = target(&|n| matches!(n, Need::Buffer { written: true, .. }));
        let texture_at = target(&|n| matches!(n, Need::Texture { .. }));
        let sampler_at = target(&|n| matches!(n, Need::Sampler { .. }));
        for (at, need) in all.iter().enumerate() {
            let here = Some(at);
            match *need {
                Need::Buffer { slot, least, align, written } => {
                    if w(3) && here == buffer_at {
                        broke = true;
                        continue;
                    }
                    let mut length = least.saturating_add(4u64.saturating_mul(random.below(16)));
                    if w(4) && here == buffer_at {
                        length = least.saturating_sub(1);
                        broke = true;
                    }
                    let bytes_here = (w(6) && here == written_at) || !written && random.chance(25);
                    if bytes_here {
                        broke |= w(6) && here == written_at;
                        let bytes = random.bytes(&[usize::try_from(length).unwrap_or(16)]);
                        commands.push(if vertex {
                            RenderCommand::SetVertexBytes { bytes, index: slot }
                        } else {
                            RenderCommand::SetFragmentBytes { bytes, index: slot }
                        });
                        continue;
                    }
                    let mut offset = align.saturating_mul(random.below(4));
                    if w(5) && here == buffer_at {
                        offset = offset.saturating_add(2);
                        broke = true;
                    }
                    let Some(held) = self.buffer_of(random, offset.saturating_add(length)) else {
                        continue;
                    };
                    if written {
                        allowed.push((held.shadow.clone(), offset, length));
                    }
                    commands.push(if vertex {
                        RenderCommand::SetVertexBuffer { buffer: held.buffer, offset, length, index: slot }
                    } else {
                        RenderCommand::SetFragmentBuffer { buffer: held.buffer, offset, length, index: slot }
                    });
                }
                Need::Texture { slot, texture_type, kind } => {
                    if w(7) && here == texture_at {
                        broke = true;
                        continue;
                    }
                    let found = if w(8) && here == texture_at {
                        broke = true;
                        let other = random.pick(&TEXTURE_TYPES.iter().copied().filter(|t| *t != texture_type).collect::<Vec<_>>());
                        self.readable(random, other, if kind == Kind::Depth { Kind::Float } else { kind })
                    } else if w(9) && here == texture_at {
                        broke = true;
                        let other = match kind {
                            Kind::Uint => Kind::Sint,
                            Kind::Depth => Kind::Uint,
                            _ => Kind::Uint,
                        };
                        self.readable(random, texture_type, other)
                    } else if w(10) && here == texture_at {
                        broke = true;
                        self.target(random, PixelFormat::Rgba8Unorm, 1, 4, 0, true)
                            .filter(|t| !t.made.usage.shader_read)
                            .map(|t| (t.texture, t.made))
                    } else {
                        self.readable(random, texture_type, kind)
                    };
                    let Some((texture, _)) = found else {
                        broke = false;
                        continue;
                    };
                    commands.push(if vertex {
                        RenderCommand::SetVertexTexture { texture, index: slot }
                    } else {
                        RenderCommand::SetFragmentTexture { texture, index: slot }
                    });
                }
                Need::Sampler { slot } => {
                    if w(11) && here == sampler_at {
                        broke = true;
                        continue;
                    }
                    let Some(sampler) = any(random, &self.samplers) else {
                        continue;
                    };
                    commands.push(if vertex {
                        RenderCommand::SetVertexSamplerState { sampler, index: slot }
                    } else {
                        RenderCommand::SetFragmentSamplerState { sampler, index: slot }
                    });
                }
            }
        }
        broke
    }

    /// Encodes a pass and a draw of a pipeline of unmangled functions, every field valid but at most one.
    fn precise_render(&mut self, random: &mut Random) {
        if self.plain_pipelines.is_empty() || random.chance(8) {
            self.precise_pipeline(random);
        }
        let Some(p) = any(random, &self.plain_pipelines) else {
            return;
        };
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let wrong = random.broken(31);
        let w = |k| wrong == Some(k);
        let mut refuse = false;
        let samples = if w(1) {
            refuse = true;
            if p.samples == 1 { 4 } else { 1 }
        } else {
            p.samples
        };
        let side: u32 = random.pick(&[1, 2, 4, 8]);
        let layout = random.below(10).saturating_sub(6);
        // The color targets, of the pipeline's formats.
        let mut colors: Vec<Option<Target>> = Vec::new();
        let first_color = p.colors.iter().position(Option::is_some);
        for (at, format) in p.colors.iter().enumerate() {
            let Some(mut format) = *format else {
                colors.push(None);
                continue;
            };
            if w(2) && Some(at) == first_color {
                format = match kind_of(format) {
                    Kind::Uint => PixelFormat::Rgba8Sint,
                    _ if format == PixelFormat::Rgba8Unorm => PixelFormat::Bgra8Unorm,
                    _ => PixelFormat::Rgba8Unorm,
                };
                refuse = true;
            }
            let render_target = !(w(22) && Some(at) == first_color);
            refuse |= !render_target;
            let grow = w(24) && at > 0;
            colors.push(self.target(random, format, samples, if grow { side.saturating_mul(2) } else { side }, layout, render_target));
            refuse |= grow;
        }
        if w(2) && first_color.is_none() {
            colors[0] = self.target(random, PixelFormat::Rgba8Unorm, samples, side, layout, true);
            refuse = true;
        }
        if w(28) && p.colors[0].is_some() && p.colors[1].is_some() {
            if let Some(first) = &colors[0] {
                colors[1] = Some(Target {
                    texture: first.texture.clone(),
                    made: first.made,
                    level: first.level,
                    slice: first.slice,
                    plane: first.plane,
                });
                refuse = true;
            }
        }
        // Resolves of a multisampled first target, into one of its format and size.
        let mut resolve: Option<Target> = None;
        let mut resolve_level = 0;
        if let Some(at) = first_color
            && let Some(first) = &colors[at]
        {
            let format = first.made.format;
            let resolves = kind_of(format) == Kind::Float;
            let want = (samples > 1 && resolves && random.chance(60)) || w(27);
            if want {
                let (level_side, _, _) = first.made.level(first.level);
                let (mut format_to, mut samples_to, mut side_to) = (format, 1, level_side);
                if w(27) {
                    refuse = true;
                    match random.below(4) {
                        0 => format_to = if format == PixelFormat::Rgba8Unorm { PixelFormat::Bgra8Unorm } else { PixelFormat::Rgba8Unorm },
                        1 => samples_to = 4,
                        2 => side_to = level_side.saturating_mul(2),
                        _ => resolve_level = random.past(0, 0),
                    }
                    // A resolve from one sample, or of a format that does not resolve, is refused whatever its target.
                    refuse |= samples == 1 || !resolves;
                }
                resolve = self.target(random, format_to, samples_to, side_to, 0, true);
                if w(27) && resolve.is_none() {
                    refuse = false;
                }
            }
        }
        // Depth and stencil, of one texture when the pipeline's formats are one.
        let mut depth: Option<Target> = None;
        let mut stencil: Option<Target> = None;
        if let Some(format) = p.depth {
            let format = if w(26) && p.stencil != p.depth { PixelFormat::Rgba8Unorm } else { format };
            refuse |= w(26) && p.stencil != p.depth;
            depth = self.target(random, format, samples, if w(24) { side.saturating_mul(2) } else { side }, layout, true);
            refuse |= w(24);
        }
        if let Some(format) = p.stencil {
            if p.stencil == p.depth && !w(26)
                && let Some(d) = &depth
            {
                stencil = Some(Target { texture: d.texture.clone(), made: d.made, level: d.level, slice: d.slice, plane: 0 });
            } else {
                stencil = self.target(random, format, samples, side, layout, true);
                refuse |= w(26) && p.stencil == p.depth;
            }
        }
        // The area the targets share, or of a pass without them.
        let level_side = |t: &Target| t.made.level(t.level).0;
        let area = colors
            .iter()
            .flatten()
            .chain(depth.iter())
            .chain(stencil.iter())
            .map(level_side)
            .next();
        let mut render_target_side = side;
        if area.is_none() && (w(23) || w(24)) {
            render_target_side = random.pick(&[0, MOST_SIDE.saturating_add(1), u32::MAX]);
            refuse = true;
        }
        let width = area.unwrap_or(render_target_side);
        if w(23) && area.is_some() {
            refuse = true;
            if let Some(Some(t)) = colors.iter_mut().find(|t| t.is_some()) {
                match random.below(3) {
                    0 => t.level = random.past(t.made.levels.saturating_sub(1), 0),
                    1 => t.slice = random.past(t.made.slices.saturating_sub(1), 0),
                    _ => t.plane = random.past(t.made.level(t.level).2.saturating_sub(1), 0),
                }
            } else if let Some(t) = &mut depth {
                t.level = random.past(t.made.levels.saturating_sub(1), 0);
            } else if let Some(t) = &mut stencil {
                t.slice = random.past(t.made.slices.saturating_sub(1), 0);
            }
        }
        let attachments = colors.iter().flatten().count()
            .saturating_add(depth.iter().count())
            .saturating_add(usize::from(stencil.is_some() && p.stencil != p.depth));
        if w(24) && area.is_some() && attachments < 2 {
            refuse = false;
        }
        // The commands: the pipeline, a depth and stencil state, the reads, the area, the counting and the draw.
        let mut commands: Vec<RenderCommand<M>> = Vec::new();
        let mut allowed: Vec<Allowed> = Vec::new();
        if w(0) {
            refuse = true;
        } else {
            commands.push(RenderCommand::SetRenderPipelineState(p.state.clone()));
        }
        let tests_depth = depth.is_some() && random.chance(60);
        let tests_stencil = stencil.is_some() && random.chance(60);
        let state = if w(21) && depth.is_none() {
            refuse = true;
            Some((true, false))
        } else if w(21) && stencil.is_none() {
            refuse = true;
            Some((false, true))
        } else {
            Some((tests_depth, tests_stencil))
        };
        if let Some((depth_test, stencil_test)) = state
            && let Some(ds) = self
                .ds_states
                .iter()
                .find(|(_, d, s)| *d == depth_test && *s == stencil_test)
                .map(|(state, _, _)| state.clone())
        {
            commands.push(RenderCommand::SetDepthStencilState(ds));
        }
        let read_break = wrong.filter(|k| (3..=11).contains(k));
        let vertex_breaks = random.chance(50);
        refuse |= self.bind_reads(random, p.vertex, true, read_break.filter(|_| vertex_breaks), &mut commands, &mut allowed);
        if let Some(fragment) = p.fragment {
            refuse |= self.bind_reads(random, fragment, false, read_break.filter(|_| !vertex_breaks), &mut commands, &mut allowed);
        }
        // Vertex inputs for the functions that read them, of the strides they were compiled for.
        if random.chance(50)
            && let Some(held) = self.buffer_of(random, 256)
        {
            commands.push(RenderCommand::SetVertexBuffer { buffer: held.buffer, offset: 0, length: 256, index: VERTEX_SLOTS[0] });
        }
        if w(12) {
            refuse = true;
            commands.push(match random.below(3) {
                0 => RenderCommand::SetFragmentBytes { bytes: vec![0; 16], index: random.past(30, 0) },
                1 => match any(random, &self.textures) {
                    Some((texture, _)) => RenderCommand::SetFragmentTexture { texture, index: random.past(127, 0) },
                    None => RenderCommand::SetVertexBytes { bytes: vec![0; 4], index: 31 },
                },
                _ => match any(random, &self.samplers) {
                    Some(sampler) => RenderCommand::SetVertexSamplerState { sampler, index: random.past(15, 0) },
                    None => RenderCommand::SetVertexBytes { bytes: vec![0; 4], index: u32::MAX },
                },
            });
        }
        if w(13)
            && let Some(held) = self.buffer_of(random, 64)
        {
            let length = held.shadow.borrow().length();
            let range = random.pick(&[4, 16, 64]);
            let offset = random.past_64(length.saturating_sub(range), range);
            commands.push(RenderCommand::SetFragmentBuffer {
                buffer: held.buffer,
                offset,
                length: range,
                index: 5,
            });
            refuse = true;
        }
        if w(14) {
            commands.push(RenderCommand::SetVertexBytes { bytes: vec![0; random.pick(&[4097, 8192, 65536])], index: 6 });
            refuse = true;
        }
        let viewports = if w(15) {
            refuse = true;
            match random.below(3) {
                0 => Vec::new(),
                1 => vec![Viewport { origin_x: 0.0, origin_y: 0.0, width: 4.0, height: 4.0, znear: 0.0, zfar: 1.0 }; 17],
                _ => vec![Viewport { origin_x: random.pick(&[f64::NAN, f64::INFINITY]), origin_y: 0.0, width: 4.0, height: 4.0, znear: 0.0, zfar: 1.0 }],
            }
        } else {
            vec![Viewport { origin_x: 0.0, origin_y: 0.0, width: f64::from(width), height: f64::from(width), znear: 0.0, zfar: 1.0 }; 1usize.saturating_add(random.index(2))]
        };
        if w(15) || random.chance(40) {
            commands.push(RenderCommand::SetViewports(viewports));
        }
        if w(16) || random.chance(40) {
            let inside = width.max(1);
            let mut rect = ScissorRect { x: 0, y: 0, width: inside, height: inside };
            let mut count = 1usize;
            if w(16) {
                refuse = true;
                match random.below(4) {
                    0 => count = random.pick(&[0, 17]),
                    1 => rect.x = random.past(inside.saturating_sub(rect.width), rect.width),
                    2 => rect.y = random.past(inside.saturating_sub(rect.height), rect.height),
                    _ => rect.width = inside.saturating_add(1),
                }
            }
            commands.push(RenderCommand::SetScissorRects(vec![rect; count]));
        }
        // Counting into the visibility buffer at offsets apart, or one wrong.
        let visibility = if w(29) {
            refuse = true;
            let short = self.metal.make_buffer(random.pick(&[0, 4, 7]));
            self.counts.call(short.is_err());
            short.ok().map(|buffer| Held { buffer, shadow: Shadow::new(0) })
        } else if w(17) || random.chance(25) {
            self.buffer_of(random, 64)
        } else {
            None
        };
        if let Some(held) = &visibility {
            let length = held.shadow.borrow().length();
            for at in 0..random.below(3).saturating_add(1) {
                let offset = at.saturating_mul(RESULT_BYTES);
                allowed.push((held.shadow.clone(), offset, RESULT_BYTES));
                commands.push(RenderCommand::SetVisibilityResultMode {
                    mode: random.pick(&[VisibilityResultMode::Boolean, VisibilityResultMode::Counting]),
                    offset,
                });
            }
            if w(17) {
                refuse = true;
                let offset = match random.below(4) {
                    0 => RESULT_BYTES.saturating_add(random.pick(&[1, 4])),
                    1 => random.pick(&[VISIBILITY_SPAN, u64::MAX - 7, 1 << 63]),
                    2 => length.next_multiple_of(RESULT_BYTES),
                    _ => 0,
                };
                commands.push(RenderCommand::SetVisibilityResultMode { mode: VisibilityResultMode::Counting, offset });
            }
        } else if w(17) {
            refuse = true;
            commands.push(RenderCommand::SetVisibilityResultMode { mode: VisibilityResultMode::Counting, offset: 0 });
        }
        // The draw.
        let other_class = w(20);
        refuse |= other_class;
        let primitive_type = primitive_of(random, p.topology, other_class);
        if random.chance(70) && !w(19) {
            let mut vertex_start = random.below(3) as u32;
            let vertex_count = 3u32.saturating_add(random.below(4) as u32);
            let mut instance_count = 1u32.saturating_add(random.below(2) as u32);
            let mut base_instance = random.below(2) as u32;
            if w(18) {
                refuse = true;
                match random.below(3) {
                    0 => vertex_start = random.past(u32::MAX.saturating_sub(vertex_count), vertex_count).max(u32::MAX - 2),
                    1 => base_instance = u32::MAX.saturating_sub(random.below(1) as u32),
                    _ => instance_count = 0,
                }
            }
            commands.push(RenderCommand::DrawPrimitives { primitive_type, vertex_start, vertex_count, instance_count, base_instance });
        } else {
            let index_type = random.pick(&[IndexType::UInt16, IndexType::UInt32]);
            let size: u64 = if index_type == IndexType::UInt16 { 2 } else { 4 };
            let mut index_count = 3u32.saturating_add(random.below(6) as u32);
            let mut offset = size.saturating_mul(random.below(4));
            let Some(held) = self.buffer_of(random, offset.saturating_add(u64::from(index_count).saturating_mul(size))) else {
                return;
            };
            let mut instance_count = 1u32.saturating_add(random.below(2) as u32);
            let mut base_instance = 0;
            if w(19) {
                refuse = true;
                match random.below(4) {
                    0 => offset = offset.saturating_add(1),
                    1 => index_count = u32::try_from(held.shadow.borrow().length() / size).unwrap_or(u32::MAX).saturating_add(1),
                    2 => index_count = 0,
                    _ => {
                        base_instance = u32::MAX;
                        instance_count = instance_count.max(2);
                    }
                }
            }
            if w(18) {
                refuse = true;
                instance_count = 0;
            }
            commands.push(RenderCommand::DrawIndexedPrimitives {
                primitive_type,
                index_count,
                index_type,
                index_buffer: held.buffer,
                index_buffer_offset: offset,
                instance_count,
                base_vertex: random.below(3) as i32,
                base_instance,
            });
        }
        // The pass.
        let load = |random: &mut Random| random.pick(&[LoadAction::Clear, LoadAction::Load, LoadAction::DontCare]);
        let store = |random: &mut Random| random.pick(&[StoreAction::Store, StoreAction::DontCare]);
        let mut color_attachments = [const { None }; MAX_COLOR_ATTACHMENTS];
        for (at, slot) in color_attachments.iter_mut().enumerate() {
            if let Some(t) = &colors[at] {
                *slot = Some(RenderPassColorAttachment {
                    texture: &t.texture,
                    level: t.level,
                    slice: t.slice,
                    depth_plane: t.plane,
                    load_action: load(random),
                    store_action: store(random),
                    clear_color: [0.25, 0.5, 0.75, 1.0],
                    resolve: (Some(at) == first_color)
                        .then_some(resolve.as_ref())
                        .flatten()
                        .map(|r| ResolveTarget { texture: &r.texture, level: resolve_level, slice: 0, depth_plane: 0 }),
                });
            }
        }
        let mut depth_clear = random.pick(&[0.0, 0.5, 1.0]);
        let mut stencil_clear = random.pick(&[0.0, 3.0, 255.0]);
        if w(25) {
            if depth.is_some() {
                depth_clear = random.pick(&[1.5, -0.25, f64::NAN, f64::INFINITY]);
                refuse = true;
            } else if stencil.is_some() {
                stencil_clear = random.pick(&[256.0, 0.5, -1.0, f64::NAN]);
                refuse = true;
            }
        }
        let pass = RenderPassDescriptor {
            color_attachments,
            depth_attachment: depth.as_ref().map(|t| RenderPassDepthStencilAttachment {
                texture: &t.texture,
                level: t.level,
                slice: t.slice,
                load_action: load(random),
                store_action: store(random),
                clear_value: depth_clear,
            }),
            stencil_attachment: stencil.as_ref().map(|t| RenderPassDepthStencilAttachment {
                texture: &t.texture,
                level: t.level,
                slice: t.slice,
                load_action: load(random),
                store_action: store(random),
                clear_value: stencil_clear,
            }),
            visibility_result_buffer: visibility.as_ref().map(|h| &h.buffer),
            render_target_width: render_target_side,
            render_target_height: render_target_side,
            default_raster_sample_count: samples,
        };
        if w(30) {
            refuse = true;
            let _ = self.metal.commit(&command_buffer);
        }
        let refused = self.metal.encode_render_commands(&command_buffer, &pass, &commands).is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &allowed, &format!("a precise render pass, break {wrong:?}"));
    }

    /// Encodes a dispatch of an unmangled compute function, every field valid but at most one.
    fn precise_compute(&mut self, random: &mut Random) {
        let Some(command_buffer) = self.command_buffer(random) else {
            return;
        };
        let entry = random.pick(&["cs_scatter", "cs_shared", "cs_textures", "cs_wide", "cs_big"]);
        let Some(function) = self.plain(entry) else {
            return;
        };
        let pipeline = self.metal.make_compute_pipeline_state(&function);
        self.counts.call(pipeline.is_err());
        let too_large = matches!(entry, "cs_wide" | "cs_big");
        if too_large && std::env::var("FUZZ_SHOW_PLAIN").is_ok() {
            eprintln!("PLAIN {entry}: {:?}", pipeline.as_ref().err());
        }
        self.must(pipeline.is_err(), too_large, "a compute pipeline past the device's threads or memory");
        let Ok(pipeline) = pipeline else {
            return;
        };
        if too_large {
            return;
        }
        let threads: [u32; 3] = match entry {
            "cs_scatter" => [64, 1, 1],
            "cs_shared" => [8, 8, 1],
            _ => [1, 1, 1],
        };
        let wrong = random.broken(13);
        let w = |k| wrong == Some(k);
        let mut refuse = false;
        let mut commands: Vec<ComputeCommand<M>> = Vec::new();
        let mut allowed: Vec<Allowed> = Vec::new();
        if w(0) {
            refuse = true;
        } else {
            commands.push(ComputeCommand::SetComputePipelineState(pipeline));
        }
        let all = needs(entry);
        let buffer_at = all.iter().position(|n| matches!(n, Need::Buffer { .. }));
        let written_at = all.iter().position(|n| matches!(n, Need::Buffer { written: true, .. }));
        let texture_at = all.iter().position(|n| matches!(n, Need::Texture { .. }));
        for (at, need) in all.iter().enumerate() {
            let here = Some(at);
            match *need {
                Need::Buffer { slot, least, align, written } => {
                    if w(1) && here == buffer_at {
                        refuse = true;
                        continue;
                    }
                    let length = if w(2) && here == buffer_at {
                        refuse = true;
                        least.saturating_sub(1)
                    } else {
                        least.saturating_add(4u64.saturating_mul(random.below(32)))
                    };
                    if (w(4) && here == written_at) || !written && random.chance(25) {
                        refuse |= w(4) && here == written_at;
                        commands.push(ComputeCommand::SetBytes { bytes: random.bytes(&[usize::try_from(length).unwrap_or(16)]), index: slot });
                        continue;
                    }
                    let mut offset = align.saturating_mul(random.below(4));
                    if w(3) && here == buffer_at {
                        offset = offset.saturating_add(2);
                        refuse = true;
                    }
                    let Some(held) = self.buffer_of(random, offset.saturating_add(length)) else {
                        continue;
                    };
                    if written {
                        allowed.push((held.shadow.clone(), offset, length));
                    }
                    commands.push(ComputeCommand::SetBuffer { buffer: held.buffer, offset, length, index: slot });
                }
                Need::Texture { slot, texture_type, kind } => {
                    if w(5) && here == texture_at {
                        refuse = true;
                        continue;
                    }
                    let found = if w(6) && here == texture_at {
                        refuse = true;
                        self.readable(random, TextureType::Type2DArray, kind)
                    } else if w(7) && here == texture_at {
                        refuse = true;
                        self.readable(random, texture_type, if kind == Kind::Float { Kind::Uint } else { Kind::Float })
                    } else {
                        self.readable(random, texture_type, kind)
                    };
                    match found {
                        Some((texture, _)) => commands.push(ComputeCommand::SetTexture { texture, index: slot }),
                        None => refuse = false,
                    }
                }
                Need::Sampler { slot } => {
                    if let Some(sampler) = any(random, &self.samplers) {
                        commands.push(ComputeCommand::SetSamplerState { sampler, index: slot });
                    }
                }
            }
        }
        if w(5) && texture_at.is_none() {
            refuse = false;
        }
        if (w(6) || w(7)) && texture_at.is_none() {
            refuse = false;
        }
        if w(8) {
            refuse = true;
            commands.push(match random.below(2) {
                0 => ComputeCommand::SetBytes { bytes: vec![0; 4], index: random.past(30, 0) },
                _ => match any(random, &self.textures) {
                    Some((texture, _)) => ComputeCommand::SetTexture { texture, index: random.past(127, 0) },
                    None => ComputeCommand::SetBytes { bytes: vec![0; 4], index: 31 },
                },
            });
        }
        if w(9)
            && let Some(held) = self.buffer_of(random, 64)
        {
            let length = held.shadow.borrow().length();
            commands.push(ComputeCommand::SetBuffer { buffer: held.buffer, offset: random.past_64(length.saturating_sub(16), 16), length: 16, index: 6 });
            refuse = true;
        }
        if w(10) {
            commands.push(ComputeCommand::SetBytes { bytes: vec![0; random.pick(&[4097, 8192])], index: 7 });
            refuse = true;
        }
        let mut groups = [1u32.saturating_add(random.below(4) as u32), 1u32.saturating_add(random.below(2) as u32), 1];
        if w(11) {
            refuse = true;
            let axis = random.index(3);
            groups[axis] = if random.chance(50) || threads[axis] == 1 {
                0
            } else {
                (u32::MAX / threads[axis]).saturating_add(1)
            };
        }
        commands.push(ComputeCommand::DispatchThreadgroups(Size { width: groups[0], height: groups[1], depth: groups[2] }));
        if w(12) {
            refuse = true;
            let _ = self.metal.commit(&command_buffer);
        }
        let refused = self.metal.encode_compute_commands(&command_buffer, &commands).is_err();
        self.counts.call(refused);
        self.taken(refused, refuse, &allowed, &format!("a precise dispatch, break {wrong:?}"));
    }

    /// Compiles an entry point, valid but for one field of its slots, layouts or module.
    fn precise_function(&mut self, random: &mut Random) {
        let index = random.index(ENTRIES.len());
        let (entry, mut stage) = ENTRIES[index];
        let mut entry_point = entry;
        let mut slots: Vec<ResourceSlot> = SLOTS
            .iter()
            .map(|&(binding, index)| ResourceSlot { group: 0, binding, index })
            .collect();
        let mut vertex_buffers = vertex_layouts(entry);
        let (mut immediates, mut sizes) = (IMMEDIATES_AT, LENGTHS_AT);
        let mut module: Option<naga::Module> = None;
        let used: Vec<(naga::Handle<naga::GlobalVariable>, Bound)> = self.used[index].clone();
        let of = |kind: Bound| used.iter().filter(|(_, k)| *k == kind).map(|(h, _)| *h).collect::<Vec<_>>();
        let (buffers, textures, samplers) = (of(Bound::Buffer), of(Bound::Texture), of(Bound::Sampler));
        let pick = |random: &mut Random, of: &[naga::Handle<naga::GlobalVariable>]| (!of.is_empty()).then(|| of[random.index(of.len())]);
        let slot_of = |slots: &[ResourceSlot], binding: u32| slots.iter().position(|s| s.binding == binding);
        let binding_of = |m: &naga::Module, h: naga::Handle<naga::GlobalVariable>| m.global_variables[h].binding.map_or(u32::MAX, |b| b.binding);
        let refuse = match random.broken(17) {
            None => false,
            Some(0) => {
                entry_point = "no_such_entry";
                true
            }
            Some(1) => {
                let m = copy(&mut module, &self.module);
                if let Some(e) = m.entry_points.iter_mut().find(|e| e.name == entry) {
                    e.stage = naga::ShaderStage::Task;
                }
                stage = naga::ShaderStage::Task;
                true
            }
            Some(2) => {
                if random.chance(50) {
                    sizes = immediates;
                } else {
                    immediates = random.pick(&[31, 32, u32::MAX]);
                }
                true
            }
            Some(3) => match pick(random, &[buffers.clone(), textures.clone(), samplers.clone()].concat()) {
                Some(h) => {
                    copy(&mut module, &self.module).global_variables[h].space = naga::AddressSpace::TaskPayload;
                    true
                }
                None => false,
            },
            Some(4) => match pick(random, &[buffers.clone(), textures.clone(), samplers.clone()].concat()) {
                Some(h) => {
                    copy(&mut module, &self.module).global_variables[h].binding = None;
                    true
                }
                None => false,
            },
            Some(5) => match pick(random, &[buffers.clone(), textures.clone(), samplers.clone()].concat()) {
                Some(h) => {
                    let binding = binding_of(&self.module, h);
                    if let Some(at) = slot_of(&slots, binding) {
                        slots.remove(at);
                    }
                    true
                }
                None => false,
            },
            Some(6) => match pick(random, &buffers) {
                Some(h) => {
                    let binding = binding_of(&self.module, h);
                    if let Some(at) = slot_of(&slots, binding) {
                        slots[at].index = random.pick(&[31, 32, IMMEDIATES_AT, LENGTHS_AT, VERTEX_SLOTS[0], u32::MAX]);
                    }
                    true
                }
                None => false,
            },
            Some(7) => match pick(random, &buffers) {
                Some(h) => {
                    let m = copy(&mut module, &self.module);
                    let base = m.global_variables[h].ty;
                    let size = naga::ArraySize::Constant(std::num::NonZeroU32::MIN.saturating_add(1));
                    let ty = m.types.insert(
                        naga::Type { name: None, inner: naga::TypeInner::BindingArray { base, size } },
                        naga::Span::UNDEFINED,
                    );
                    m.global_variables[h].ty = ty;
                    true
                }
                None => false,
            },
            Some(8) => match pick(random, &textures) {
                Some(h) => {
                    let binding = binding_of(&self.module, h);
                    if let Some(at) = slot_of(&slots, binding) {
                        slots[at].index = random.pick(&[128, 129, u32::MAX]);
                    }
                    true
                }
                None => false,
            },
            Some(9 | 10) => {
                let sampled: Vec<_> = textures
                    .iter()
                    .copied()
                    .filter(|h| matches!(self.module.types[self.module.global_variables[*h].ty].inner,
                        naga::TypeInner::Image { class: naga::ImageClass::Sampled { .. }, .. }))
                    .collect();
                match pick(random, &sampled) {
                    Some(h) => {
                        let external = random.chance(50);
                        let m = copy(&mut module, &self.module);
                        let inner = match m.types[m.global_variables[h].ty].inner {
                            naga::TypeInner::Image { dim, arrayed, class: naga::ImageClass::Sampled { multi, .. } } => {
                                naga::TypeInner::Image {
                                    dim,
                                    arrayed,
                                    class: if external {
                                        naga::ImageClass::External
                                    } else {
                                        naga::ImageClass::Sampled { kind: naga::ScalarKind::Bool, multi }
                                    },
                                }
                            }
                            ref other => other.clone(),
                        };
                        let ty = m.types.insert(naga::Type { name: None, inner }, naga::Span::UNDEFINED);
                        m.global_variables[h].ty = ty;
                        true
                    }
                    None => false,
                }
            }
            Some(11) => match pick(random, &samplers) {
                Some(h) => {
                    let binding = binding_of(&self.module, h);
                    if let Some(at) = slot_of(&slots, binding) {
                        slots[at].index = random.pick(&[16, 17, u32::MAX]);
                    }
                    true
                }
                None => false,
            },
            Some(12) => match pick(random, &samplers) {
                Some(h) => {
                    let m = copy(&mut module, &self.module);
                    let ty = m.types.insert(
                        naga::Type { name: None, inner: naga::TypeInner::Scalar(naga::Scalar::U32) },
                        naga::Span::UNDEFINED,
                    );
                    m.global_variables[h].ty = ty;
                    true
                }
                None => false,
            },
            Some(13) if stage == naga::ShaderStage::Vertex && !vertex_buffers.is_empty() => {
                vertex_buffers[0].buffer_index = random.pick(&[31, IMMEDIATES_AT, LENGTHS_AT, u32::MAX]);
                true
            }
            Some(14) if stage == naga::ShaderStage::Vertex && !vertex_buffers.is_empty() => {
                vertex_buffers[0].stride = random.pick(&[0, 2049, u32::MAX]);
                true
            }
            Some(15) if stage == naga::ShaderStage::Vertex && !vertex_buffers.is_empty() => {
                let stride = vertex_buffers[0].stride;
                vertex_buffers[0].attributes[0].offset = random.pick(&[stride.saturating_sub(1), u32::MAX - 3, stride]);
                true
            }
            Some(16) => {
                entry_point = random.pick(&["fs_store", "fs_line"]);
                stage = naga::ShaderStage::Fragment;
                true
            }
            Some(_) => false,
        };
        let module_ref = module.as_ref().unwrap_or(&self.module);
        let made = self.metal.make_function(&FunctionSource {
            module: module_ref,
            info: &self.info,
            stage,
            entry_point,
            resources: &slots,
            immediates_buffer_index: immediates,
            sizes_buffer_index: sizes,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        });
        self.counts.call(made.is_err());
        self.must(made.is_err(), refuse, "a function");
    }
}

/// Returns the vertex buffers an entry point is compiled with: its inputs at `VERTEX_SLOTS`.
fn vertex_layouts(entry: &str) -> Vec<VertexBufferLayout> {
    vec![
        VertexBufferLayout {
            buffer_index: VERTEX_SLOTS[0],
            stride: 16,
            step_function: VertexStepFunction::PerVertex,
            attributes: vec![VertexAttribute {
                location: 0,
                offset: 0,
                format: if entry == "vs_plain" { VertexFormat::Float2 } else { VertexFormat::Float4 },
            }],
        },
        VertexBufferLayout {
            buffer_index: VERTEX_SLOTS[1],
            stride: 24,
            step_function: VertexStepFunction::PerInstance,
            attributes: vec![
                VertexAttribute { location: 1, offset: 0, format: VertexFormat::UInt4 },
                VertexAttribute { location: 2, offset: 16, format: VertexFormat::Int2 },
            ],
        },
    ]
}

/// Returns, for every entry point, the globals it uses that are bound, and as what.
fn used_globals(module: &naga::Module, info: &naga::valid::ModuleInfo) -> Vec<Vec<(naga::Handle<naga::GlobalVariable>, Bound)>> {
    ENTRIES
        .iter()
        .map(|(name, stage)| {
            let Some(at) = module.entry_points.iter().position(|e| e.name == *name && e.stage == *stage) else {
                return Vec::new();
            };
            let uses = info.get_entry_point(at);
            module
                .global_variables
                .iter()
                .filter(|(h, var)| !uses[*h].is_empty() && var.binding.is_some())
                .filter_map(|(h, var)| {
                    let bound = match (var.space, &module.types[var.ty].inner) {
                        (naga::AddressSpace::Uniform | naga::AddressSpace::Storage { .. }, _) => Bound::Buffer,
                        (_, naga::TypeInner::Image { .. }) => Bound::Texture,
                        (_, naga::TypeInner::Sampler { .. }) => Bound::Sampler,
                        _ => return None,
                    };
                    Some((h, bound))
                })
                .collect()
        })
        .collect()
}

/// Returns the changed copy of the batch's module, made of `of` the first time.
fn copy<'a>(module: &'a mut Option<naga::Module>, of: &naga::Module) -> &'a mut naga::Module {
    module.get_or_insert_with(|| of.clone())
}
