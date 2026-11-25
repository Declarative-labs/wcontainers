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
];

/// The slot of each binding of the programs: buffers 0 to 3, samplers 0 and 1, textures 0 to 7.
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
    /// Every texture and view the batch keeps, with its format.
    textures: Vec<(Texture, PixelFormat)>,
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
    let mut world = World {
        metal: M::new(()),
        module,
        info,
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
    for (index, _) in ENTRIES.iter().enumerate() {
        world.make_function(random, index, false);
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
        if texture_bytes(&d) > TEXTURE_BUDGET {
            // A texture past the budget keeps its widest side, which the checks of each side meet, and loses the rest.
            d.height = d.height.min(1);
            d.depth = d.depth.min(1);
            d.array_length = d.array_length.min(1);
            if texture_bytes(&d) > TEXTURE_BUDGET {
                d.width = d.width.min(4096);
            }
        }
        let made = self.metal.make_texture(&d);
        self.counts.call(made.is_err());
        if let Ok(texture) = made {
            keep(random, &mut self.textures, (texture, d.pixel_format));
        }
    }

    /// Makes a view of a random texture.
    fn make_view(&mut self, random: &mut Random) {
        let Some((texture, _)) = any(random, &self.textures) else {
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
        let made = self.metal.make_texture_view(&texture, &d);
        self.counts.call(made.is_err());
        if let Ok(view) = made {
            keep(random, &mut self.textures, (view, d.pixel_format));
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
        let (texture, format) = any(random, &self.textures)?;
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
            format,
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
                    keep(
                        random,
                        &mut self.textures,
                        (texture, PixelFormat::Bgra8Unorm),
                    );
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
        let texture = other.make_texture(&TextureDescriptor {
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
        });
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
            keep(random, &mut self.textures, (texture, format));
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
