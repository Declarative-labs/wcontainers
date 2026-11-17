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
    /// Whether every target uses `targets[0]`; with independent blend absent, fronts always set this.
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
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
    /// Depth bounds test (Direct3D 12, Vulkan), as f32 bits of the low and high bound.
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
    /// Depth bias: constant, slope scale (f32 bits) and clamp (f32 bits).
    pub depth_bias: i32,
    pub depth_bias_slope: u32,
    pub depth_bias_clamp: u32,
    /// False clamps depth instead of clipping it.
    pub depth_clip: bool,
    pub scissor: bool,
    /// Multisampled rasterisation of lines (Direct3D's MultisampleEnable).
    pub multisample_lines: bool,
    /// Line width as f32 bits; one unless [`crate::Features::WIDE_LINES`].
    pub line_width: u32,
    pub provoking: ProvokingVertex,
    /// Clip space depth runs from minus one (OpenGL) instead of zero.
    pub depth_minus_one: bool,
    /// Bits of the sample mask, applied with the fragment's own coverage.
    pub sample_mask: u32,
}

/// How a vertex attribute is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    Uint8x1,
    Uint8x2,
    Uint8x3,
    Uint8x4,
    Sint8x4,
    Unorm8x1,
    Unorm8x2,
    Unorm8x3,
    Unorm8x4,
    Snorm8x2,
    Snorm8x4,
    Bgra8Unorm,
    Uint16x1,
    Uint16x2,
    Uint16x4,
    Sint16x2,
    Sint16x4,
    Unorm16x2,
    Unorm16x4,
    Snorm16x2,
    Snorm16x4,
    Float16x2,
    Float16x4,
    Float32,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32,
    Sint32x2,
    Sint32x3,
    Sint32x4,
    Unorm10_10_10_2,
    Uint10_10_10_2,
    Float11_11_10,
}

/// Whether a stream advances per vertex or per instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepMode {
    Vertex,
    /// Advances once every `n` instances (Direct3D's step rate; zero means never).
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
    /// the block from the layout and the bound strides together.
    pub stride: u32,
    pub step: StepMode,
}

/// The vertex layout block. Unused streams have stride zero and no attributes.
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
/// [`crate::Backend::draw`].
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
    U16,
    U32,
}

impl IndexFormat {
    /// Bytes per index.
    pub fn size(self) -> u32 {
        match self {
            IndexFormat::U16 => 2,
            IndexFormat::U32 => 4,
        }
    }
}
