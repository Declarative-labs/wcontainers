//! Texel formats. One list for every front: each front maps its own format names onto it, and each back
//! end says in [`crate::Caps`] what it can do with each one.

/// A texel format. Names follow the channel order in memory, then the number type.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[repr(u16)]
pub enum Format {
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
    /// Two pixels in four bytes, R, G0, B, G1: pixel 0 is (R, G0, B), pixel 1 is (R, G1, B)
    /// (Direct3D's R8G8_B8G8_UNORM). No browser has it: back ends store it decoded
    /// ([`crate::SlowPath::BlockDecoded`]).
    Rgbg8Unorm,
    /// Two pixels in four bytes, G0, R, G1, B (Direct3D's G8R8_G8B8_UNORM), stored as
    /// [`Format::Rgbg8Unorm`] is.
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
    /// 8-bit alpha only (Direct3D's A8_UNORM).
    A8Unorm,
    Stencil8,
    Depth16Unorm,
    /// 24-bit depth: stored as 24-bit unsigned normalised where [`crate::Features::DEPTH24_UNORM`],
    /// otherwise as 32-bit float (more bits, never fewer). The same holds for the depth of
    /// [`Format::Depth24PlusStencil8`].
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
}

/// What a shader reads a format as, which decides how it is bound and sampled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatKind {
    Float,
    Uint,
    Sint,
    Depth,
    DepthStencil,
    Stencil,
}

/// How many formats there are: every format, in declaration order, indexes [`FORMAT_FACTS`] and
/// [`crate::Caps::formats`] by `Format as usize`.
pub const FORMAT_COUNT: usize = Format::Astc4x4UnormSrgb as usize + 1;

/// What one [`Format`] is: [`FORMAT_FACTS`] holds one per format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatFacts {
    /// Width and height of one block in texels, and its size in bytes.
    pub block: (u32, u32, u32),
    /// How shaders see the format.
    pub kind: FormatKind,
    /// The components a view of this format reads from its texels, a bit each from red; the others
    /// read their defaults, zero and one for alpha. Depth and stencil read as red. The measure of
    /// [`crate::HelperValue::TextureComponents`].
    pub components: u32,
    /// The format holds depth, stencil or both: its kind is `Depth`, `DepthStencil` or `Stencil`.
    pub depth_stencil: bool,
    /// The same bits read as linear values, for formats that have an sRGB twin; the format itself
    /// otherwise.
    pub linear: Format,
    /// The integer format of the same bits a front stores this normalised format as where the back
    /// end lacks it ([`crate::SlowPath::NormAsInteger`]): unsigned for unsigned normalised, signed
    /// for signed normalised; None for formats that have no such twin.
    pub integer_twin: Option<Format>,
    /// What one sample of a colour target in this format counts toward
    /// [`crate::Limits::color_bytes_per_sample`]: its byte cost and the alignment it starts at, as
    /// WebGPU counts render targets (8 bytes for 8-bit normalised RGBA). Formats WebGPU does not
    /// render count as their nearest one there: the packed and two-pixel 8-bit formats as
    /// `rgba8unorm`, `A8Unorm` as `r8unorm`, `Rgb9e5Float` as `rg11b10ufloat`, three 32-bit channels
    /// as four. None for depth, stencil and block-compressed formats.
    pub target_bytes: Option<(u32, u32)>,
}

/// The facts of every format, indexed by `Format as usize`.
pub const FORMAT_FACTS: [FormatFacts; FORMAT_COUNT] = [
    // R8Unorm
    FormatFacts {
        block: (1, 1, 1),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R8Unorm,
        integer_twin: None,
        target_bytes: Some((1, 1)),
    },
    // R8Snorm
    FormatFacts {
        block: (1, 1, 1),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R8Snorm,
        integer_twin: Some(Format::R8Sint),
        target_bytes: Some((1, 1)),
    },
    // R8Uint
    FormatFacts {
        block: (1, 1, 1),
        kind: FormatKind::Uint,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R8Uint,
        integer_twin: None,
        target_bytes: Some((1, 1)),
    },
    // R8Sint
    FormatFacts {
        block: (1, 1, 1),
        kind: FormatKind::Sint,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R8Sint,
        integer_twin: None,
        target_bytes: Some((1, 1)),
    },
    // R16Unorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R16Unorm,
        integer_twin: Some(Format::R16Uint),
        target_bytes: Some((2, 2)),
    },
    // R16Snorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R16Snorm,
        integer_twin: Some(Format::R16Sint),
        target_bytes: Some((2, 2)),
    },
    // R16Uint
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Uint,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R16Uint,
        integer_twin: None,
        target_bytes: Some((2, 2)),
    },
    // R16Sint
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Sint,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R16Sint,
        integer_twin: None,
        target_bytes: Some((2, 2)),
    },
    // R16Float
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R16Float,
        integer_twin: None,
        target_bytes: Some((2, 2)),
    },
    // Rg8Unorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg8Unorm,
        integer_twin: None,
        target_bytes: Some((2, 1)),
    },
    // Rg8Snorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg8Snorm,
        integer_twin: Some(Format::Rg8Sint),
        target_bytes: Some((2, 1)),
    },
    // Rg8Uint
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Uint,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg8Uint,
        integer_twin: None,
        target_bytes: Some((2, 1)),
    },
    // Rg8Sint
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Sint,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg8Sint,
        integer_twin: None,
        target_bytes: Some((2, 1)),
    },
    // R32Uint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Uint,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R32Uint,
        integer_twin: None,
        target_bytes: Some((4, 4)),
    },
    // R32Sint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Sint,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R32Sint,
        integer_twin: None,
        target_bytes: Some((4, 4)),
    },
    // R32Float
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::R32Float,
        integer_twin: None,
        target_bytes: Some((4, 4)),
    },
    // Rg16Unorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg16Unorm,
        integer_twin: Some(Format::Rg16Uint),
        target_bytes: Some((4, 2)),
    },
    // Rg16Snorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg16Snorm,
        integer_twin: Some(Format::Rg16Sint),
        target_bytes: Some((4, 2)),
    },
    // Rg16Uint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Uint,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg16Uint,
        integer_twin: None,
        target_bytes: Some((4, 2)),
    },
    // Rg16Sint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Sint,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg16Sint,
        integer_twin: None,
        target_bytes: Some((4, 2)),
    },
    // Rg16Float
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg16Float,
        integer_twin: None,
        target_bytes: Some((4, 2)),
    },
    // Rgba8Unorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Rgba8UnormSrgb
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Rgba8Snorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba8Snorm,
        integer_twin: Some(Format::Rgba8Sint),
        target_bytes: Some((8, 1)),
    },
    // Rgba8Uint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Uint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba8Uint,
        integer_twin: None,
        target_bytes: Some((4, 1)),
    },
    // Rgba8Sint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Sint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba8Sint,
        integer_twin: None,
        target_bytes: Some((4, 1)),
    },
    // Bgra8Unorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bgra8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Bgra8UnormSrgb
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bgra8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Bgrx8Unorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Bgrx8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Bgrx8UnormSrgb
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Bgrx8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Rgb10a2Unorm
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgb10a2Unorm,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // Rgb10a2Uint
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Uint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgb10a2Uint,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // Rg11b10Float
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Rg11b10Float,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // Rgb9e5Float
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Rgb9e5Float,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // B5g6r5Unorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::B5g6r5Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Bgr5a1Unorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bgr5a1Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Bgra4Unorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bgra4Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Rgbg8Unorm
    FormatFacts {
        block: (2, 1, 4),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Rgbg8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Grgb8Unorm
    FormatFacts {
        block: (2, 1, 4),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Grgb8Unorm,
        integer_twin: None,
        target_bytes: Some((8, 1)),
    },
    // Rg32Uint
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Uint,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg32Uint,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // Rg32Sint
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Sint,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg32Sint,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // Rg32Float
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Rg32Float,
        integer_twin: None,
        target_bytes: Some((8, 4)),
    },
    // Rgba16Unorm
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba16Unorm,
        integer_twin: Some(Format::Rgba16Uint),
        target_bytes: Some((8, 2)),
    },
    // Rgba16Snorm
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba16Snorm,
        integer_twin: Some(Format::Rgba16Sint),
        target_bytes: Some((8, 2)),
    },
    // Rgba16Uint
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Uint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba16Uint,
        integer_twin: None,
        target_bytes: Some((8, 2)),
    },
    // Rgba16Sint
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Sint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba16Sint,
        integer_twin: None,
        target_bytes: Some((8, 2)),
    },
    // Rgba16Float
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba16Float,
        integer_twin: None,
        target_bytes: Some((8, 2)),
    },
    // Rgb32Uint
    FormatFacts {
        block: (1, 1, 12),
        kind: FormatKind::Uint,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Rgb32Uint,
        integer_twin: None,
        target_bytes: Some((16, 4)),
    },
    // Rgb32Sint
    FormatFacts {
        block: (1, 1, 12),
        kind: FormatKind::Sint,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Rgb32Sint,
        integer_twin: None,
        target_bytes: Some((16, 4)),
    },
    // Rgb32Float
    FormatFacts {
        block: (1, 1, 12),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Rgb32Float,
        integer_twin: None,
        target_bytes: Some((16, 4)),
    },
    // Rgba32Uint
    FormatFacts {
        block: (1, 1, 16),
        kind: FormatKind::Uint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba32Uint,
        integer_twin: None,
        target_bytes: Some((16, 4)),
    },
    // Rgba32Sint
    FormatFacts {
        block: (1, 1, 16),
        kind: FormatKind::Sint,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba32Sint,
        integer_twin: None,
        target_bytes: Some((16, 4)),
    },
    // Rgba32Float
    FormatFacts {
        block: (1, 1, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Rgba32Float,
        integer_twin: None,
        target_bytes: Some((16, 4)),
    },
    // A8Unorm
    FormatFacts {
        block: (1, 1, 1),
        kind: FormatKind::Float,
        components: 0b1000,
        depth_stencil: false,
        linear: Format::A8Unorm,
        integer_twin: None,
        target_bytes: Some((1, 1)),
    },
    // Stencil8
    FormatFacts {
        block: (1, 1, 1),
        kind: FormatKind::Stencil,
        components: 0b0001,
        depth_stencil: true,
        linear: Format::Stencil8,
        integer_twin: None,
        target_bytes: None,
    },
    // Depth16Unorm
    FormatFacts {
        block: (1, 1, 2),
        kind: FormatKind::Depth,
        components: 0b0001,
        depth_stencil: true,
        linear: Format::Depth16Unorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Depth24Plus
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Depth,
        components: 0b0001,
        depth_stencil: true,
        linear: Format::Depth24Plus,
        integer_twin: None,
        target_bytes: None,
    },
    // Depth24PlusStencil8
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::DepthStencil,
        components: 0b0001,
        depth_stencil: true,
        linear: Format::Depth24PlusStencil8,
        integer_twin: None,
        target_bytes: None,
    },
    // Depth32Float
    FormatFacts {
        block: (1, 1, 4),
        kind: FormatKind::Depth,
        components: 0b0001,
        depth_stencil: true,
        linear: Format::Depth32Float,
        integer_twin: None,
        target_bytes: None,
    },
    // Depth32FloatStencil8
    FormatFacts {
        block: (1, 1, 8),
        kind: FormatKind::DepthStencil,
        components: 0b0001,
        depth_stencil: true,
        linear: Format::Depth32FloatStencil8,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc1RgbaUnorm
    FormatFacts {
        block: (4, 4, 8),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc1RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc1RgbaUnormSrgb
    FormatFacts {
        block: (4, 4, 8),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc1RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc2RgbaUnorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc2RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc2RgbaUnormSrgb
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc2RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc3RgbaUnorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc3RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc3RgbaUnormSrgb
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc3RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc4RUnorm
    FormatFacts {
        block: (4, 4, 8),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::Bc4RUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc4RSnorm
    FormatFacts {
        block: (4, 4, 8),
        kind: FormatKind::Float,
        components: 0b0001,
        depth_stencil: false,
        linear: Format::Bc4RSnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc5RgUnorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Bc5RgUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc5RgSnorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b0011,
        depth_stencil: false,
        linear: Format::Bc5RgSnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc6hRgbUfloat
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Bc6hRgbUfloat,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc6hRgbFloat
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Bc6hRgbFloat,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc7RgbaUnorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc7RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Bc7RgbaUnormSrgb
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Bc7RgbaUnorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Etc2Rgb8Unorm
    FormatFacts {
        block: (4, 4, 8),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Etc2Rgb8Unorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Etc2Rgb8UnormSrgb
    FormatFacts {
        block: (4, 4, 8),
        kind: FormatKind::Float,
        components: 0b0111,
        depth_stencil: false,
        linear: Format::Etc2Rgb8Unorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Etc2Rgba8Unorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Etc2Rgba8Unorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Etc2Rgba8UnormSrgb
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Etc2Rgba8Unorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Astc4x4Unorm
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Astc4x4Unorm,
        integer_twin: None,
        target_bytes: None,
    },
    // Astc4x4UnormSrgb
    FormatFacts {
        block: (4, 4, 16),
        kind: FormatKind::Float,
        components: 0b1111,
        depth_stencil: false,
        linear: Format::Astc4x4Unorm,
        integer_twin: None,
        target_bytes: None,
    },
];

/// The bytes one sample of colour targets in these formats holds, in attachment order, as
/// [`crate::Limits::color_bytes_per_sample`] bounds it: each target starts at a multiple of its
/// alignment and adds its cost ([`FormatFacts::target_bytes`]). None where one cannot be a colour
/// target.
pub fn color_target_bytes(formats: impl IntoIterator<Item = Format>) -> Option<u32> {
    formats.into_iter().try_fold(0, |at: u32, f| {
        let (cost, align) = FORMAT_FACTS[f as usize].target_bytes?;
        Some(at.next_multiple_of(align) + cost)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec::Vec};
    use serde::Deserialize;

    /// Every format, by its number.
    fn all() -> Vec<Format> {
        (0..)
            .map_while(|n| {
                Format::deserialize(
                    serde::de::value::U32Deserializer::<serde::de::value::Error>::new(n),
                )
                .ok()
            })
            .collect()
    }

    /// A format's name lists the channels its texels hold, as DXGI's and WebGPU's names do
    /// (`x` padding, `e` a shared exponent; ASTC blocks hold all four); `components` names the same.
    #[test]
    fn components_are_the_channels_a_format_names() {
        let formats = all();
        assert_eq!(formats.last(), Some(&Format::Astc4x4UnormSrgb));
        for f in formats {
            let name = format!("{f:?}");
            let expected = if FORMAT_FACTS[f as usize].depth_stencil {
                0b0001
            } else if name.starts_with("Astc") {
                0b1111
            } else {
                let rest = ["Bc1", "Bc2", "Bc3", "Bc4", "Bc5", "Bc6h", "Bc7", "Etc2"]
                    .iter()
                    .find_map(|p| name.strip_prefix(p))
                    .unwrap_or(&name);
                let end = rest[1..]
                    .find(|c: char| c.is_ascii_uppercase())
                    .map_or(rest.len(), |i| i + 1);
                rest[..end]
                    .to_ascii_lowercase()
                    .chars()
                    .map(|c| match c {
                        'r' => 0b0001,
                        'g' => 0b0010,
                        'b' => 0b0100,
                        'a' => 0b1000,
                        _ => 0,
                    })
                    .fold(0, |m, b| m | b)
            };
            assert_eq!(FORMAT_FACTS[f as usize].components, expected, "{f:?}");
        }
    }
}
