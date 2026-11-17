//! The immutable state blocks a draw names. Each is plain data a front builds once; the back end turns it
//! into a handle with the matching `create_*` call. Floats are stored as their bits so every block can be
//! hashed and compared, which is how back ends find a combination they have seen before.

use crate::format::Format;
use crate::resource::CompareFn;
use crate::{MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS};
use bitflags::bitflags;

/// Blend factors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
    /// The blend constant's alpha in every channel (OpenGL's and Vulkan's); needs
    /// [`crate::Features::BLEND_CONSTANT_ALPHA`].
    ConstantAlpha,
    OneMinusConstantAlpha,
    Src1,
    OneMinusSrc1,
    Src1Alpha,
    OneMinusSrc1Alpha,
}

/// Blend operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendOp {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

/// One half (color or alpha) of a target's blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendComponent {
    pub src: BlendFactor,
    pub dst: BlendFactor,
    pub op: BlendOp,
}

bitflags! {
    /// Which channels a target keeps.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ColorWrites: u8 {
        const R = 1;
        const G = 2;
        const B = 4;
        const A = 8;
    }
}

/// Logic operations, which replace blending on integer and normalized targets where present.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogicOp {
    Clear,
    And,
    AndReverse,
    Copy,
    AndInverted,
    Noop,
    Xor,
    Or,
    Nor,
    Equivalent,
    Invert,
    OrReverse,
    CopyInverted,
    OrInverted,
    Nand,
    Set,
}

/// One color target's blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderTargetBlend {
    /// None writes the fragment's value unchanged.
    pub blend: Option<(BlendComponent, BlendComponent)>,
    pub writes: ColorWrites,
}

/// The blend block: Direct3D's blend state, OpenGL's blend enables and functions, Vulkan's color blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendDesc {
    pub targets: [RenderTargetBlend; MAX_COLOR_TARGETS],
    /// Whether every target uses `targets[0]`. Back ends without [`crate::Features::INDEPENDENT_BLEND`]
    /// are only given blocks with it set: a front whose targets differ in blend or write mask draws once
    /// per group of targets that agree, each group in a block of its own (slow path
    /// [`crate::SlowPath::TargetsSplit`]).
    pub same_for_all: bool,
    pub alpha_to_coverage: bool,
    /// Present only where [`crate::Features::LOGIC_OP`] is.
    pub logic_op: Option<LogicOp>,
}

/// What a stencil test does on each outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StencilOp {
    Keep,
    Zero,
    Replace,
    IncrementClamp,
    DecrementClamp,
    Invert,
    IncrementWrap,
    DecrementWrap,
}

/// One face's stencil test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StencilFace {
    pub compare: CompareFn,
    pub fail: StencilOp,
    pub depth_fail: StencilOp,
    pub pass: StencilOp,
}

/// The depth and stencil block. The stencil reference is not here: it changes per draw
/// ([`crate::DrawCall::stencil_reference`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepthStencilDesc {
    /// None turns the depth test off.
    pub depth_compare: Option<CompareFn>,
    pub depth_write: bool,
    /// None turns the stencil test off.
    pub stencil: Option<(StencilFace, StencilFace)>,
    /// Read and write masks for front and back faces; they differ only with
    /// [`crate::Features::STENCIL_PER_FACE`].
    pub stencil_read_mask: [u8; 2],
    pub stencil_write_mask: [u8; 2],
    /// Depth bounds test (Direct3D 12, Vulkan), as f32 bits of the low and high bound. Present only with
    /// [`crate::Features::DEPTH_BOUNDS`].
    pub depth_bounds: Option<[u32; 2]>,
}

/// Which way triangles are filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FillMode {
    Solid,
    Line,
    Point,
}

/// Which faces are dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
}

/// Which winding faces the viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrontFace {
    CounterClockwise,
    Clockwise,
}

/// Which vertex of a primitive gives flat-shaded values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProvokingVertex {
    First,
    Last,
}

/// The rasterising block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterDesc {
    pub fill: FillMode,
    pub cull: CullMode,
    pub front: FrontFace,
    /// Depth bias: constant (in steps of the depth format), slope scale and clamp, each as f32 bits. The
    /// bias applies to triangles only (WebGPU allows none on lines and points, GLES offsets only filled
    /// polygons); a non-zero clamp needs [`crate::Features::DEPTH_BIAS_CLAMP`], and a constant with a
    /// fraction needs [`crate::Features::FRACTIONAL_DEPTH_BIAS`]. A front whose API biases lines and
    /// points, or needs what the back end lacks, passes zero and binds the program variant that adds the
    /// bias to the fragment's depth (slow path [`crate::SlowPath::DepthBiasInShader`]).
    pub depth_bias: u32,
    pub depth_bias_slope: u32,
    pub depth_bias_clamp: u32,
    /// False clamps depth instead of clipping it.
    pub depth_clip: bool,
    pub scissor: bool,
    /// Line width as f32 bits; one unless [`crate::Features::WIDE_LINES`].
    pub line_width: u32,
    pub provoking: ProvokingVertex,
    /// Clip space depth runs from minus one (OpenGL) instead of zero (Direct3D, Vulkan). Native only
    /// with [`crate::Features::CLIP_DEPTH_MINUS_ONE`] or [`crate::Features::CLIP_DEPTH_ZERO_ONE`]
    /// respectively; otherwise the vertex program rewrites depth (slow path
    /// [`crate::SlowPath::ClipDepthRewrite`]).
    pub depth_minus_one: bool,
    /// Bits of the sample mask, applied with the fragment's own coverage. Back ends without
    /// [`crate::Features::SAMPLE_MASK`] are only given all ones: a front whose mask clears a sample of a
    /// multisampled target binds the program variant that writes the mask instead (slow path
    /// [`crate::SlowPath::SampleMaskInShader`]), and with single-sample targets drops a draw whose bit 0
    /// is clear, since it writes nothing.
    pub sample_mask: u32,
}

/// How a vertex attribute is stored: every component count of every 8, 16 and 32-bit type, and the packed
/// formats Direct3D and OpenGL use. A back end that lacks one (WebGPU has no three-component 8 or 16-bit
/// formats) fetches it in the vertex program from a storage buffer or converts it on upload (slow path
/// [`crate::SlowPath::VertexFormatConverted`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    Uint8x1,
    Uint8x2,
    Uint8x3,
    Uint8x4,
    Sint8x1,
    Sint8x2,
    Sint8x3,
    Sint8x4,
    Unorm8x1,
    Unorm8x2,
    Unorm8x3,
    Unorm8x4,
    Snorm8x1,
    Snorm8x2,
    Snorm8x3,
    Snorm8x4,
    Uint16x1,
    Uint16x2,
    Uint16x3,
    Uint16x4,
    Sint16x1,
    Sint16x2,
    Sint16x3,
    Sint16x4,
    Unorm16x1,
    Unorm16x2,
    Unorm16x3,
    Unorm16x4,
    Snorm16x1,
    Snorm16x2,
    Snorm16x3,
    Snorm16x4,
    Float16x1,
    Float16x2,
    Float16x3,
    Float16x4,
    Float32x1,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32x1,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32x1,
    Sint32x2,
    Sint32x3,
    Sint32x4,
    /// Direct3D 9's D3DCOLOR: bytes in B, G, R, A order, read as RGBA.
    Bgra8Unorm,
    Unorm10_10_10_2,
    Uint10_10_10_2,
    Float11_11_10,
}

/// Whether a stream advances per vertex or per instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepMode {
    Vertex,
    /// Advances once every `n` instances (Direct3D's step rate; zero means never): the instance of
    /// index `i` reads element `first_instance + (i - first_instance) / n`, or `first_instance` for
    /// zero.
    /// Rates other than one need [`crate::Features::INSTANCE_STEP_RATE`] (WebGPU steps by one only);
    /// without it they are fetched in the vertex program from the instance index (slow path
    /// [`crate::SlowPath::InstanceRateInShader`]).
    Instance(u32),
}

/// One attribute the vertex program reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexAttribute {
    /// The program input it feeds.
    pub location: u32,
    pub stream: u8,
    pub offset: u32,
    pub format: VertexFormat,
}

/// One vertex stream's layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexStream {
    /// Bytes between elements. Not part of Direct3D 11's input layout, so the Direct3D 11 front builds
    /// the block from the layout and the bound strides together. Zero means every vertex and instance
    /// reads the same element, as Direct3D and Vulkan define it; WebGL 2, where zero means tightly
    /// packed, draws such a stream with an instance divisor of 2^31.
    pub stride: u32,
    pub step: StepMode,
}

/// The vertex layout block. Unused streams are `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexLayoutDesc {
    pub streams: [Option<VertexStream>; MAX_VERTEX_STREAMS],
    pub attributes: [Option<VertexAttribute>; MAX_VERTEX_STREAMS],
}

/// The formats a pass's targets have, which programs and pipelines are built against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetLayout {
    pub colors: [Option<Format>; MAX_COLOR_TARGETS],
    pub depth_stencil: Option<Format>,
    pub samples: u32,
}

/// Primitive topology. Fans and loops go through [`crate::Backend::draw_fan_or_loop`], never
/// [`crate::Backend::draw`]. Adjacency topologies (Direct3D 10 and 11, OpenGL) are not here: with a
/// geometry shader the stage helper reads them; without one, as OpenGL allows, the front draws the list
/// or strip from indices that leave out the adjacent vertices (slow path
/// [`crate::SlowPath::AdjacencyDropped`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Topology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
    TriangleFan,
    LineLoop,
}

/// Width of index values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexFormat {
    /// 8-bit indices (OpenGL's `GL_UNSIGNED_BYTE`), where [`crate::Features::INDEX_U8`] is present;
    /// otherwise a front widens them to 16 bits in the index ring ([`crate::SlowPath::IndexWidened`]).
    U8,
    U16,
    U32,
}

impl IndexFormat {
    /// Bytes per index.
    pub fn size(self) -> u32 {
        match self {
            IndexFormat::U8 => 1,
            IndexFormat::U16 => 2,
            IndexFormat::U32 => 4,
        }
    }

    /// The largest value, which restarts strips when restart is on.
    pub fn restart_value(self) -> u32 {
        match self {
            IndexFormat::U8 => 0xff,
            IndexFormat::U16 => 0xffff,
            IndexFormat::U32 => 0xffff_ffff,
        }
    }
}
