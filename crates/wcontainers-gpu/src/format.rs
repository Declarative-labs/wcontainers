//! Texel formats. One list for every front: each front maps its own format names onto it, and each back
//! end says in [`crate::Caps`] what it can do with each one.

/// A texel format. Names follow the channel order in memory, then the number type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl Format {
    /// Every format, in declaration order; indexes [`crate::Caps::formats`].
    pub const COUNT: usize = Format::Astc4x4UnormSrgb as usize + 1;

    /// The format's position in [`crate::Caps::formats`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Width and height of one block in texels, and its size in bytes.
    pub fn block(self) -> (u32, u32, u32) {
        use Format::*;
        match self {
            R8Unorm | R8Snorm | R8Uint | R8Sint | A8Unorm | Stencil8 => (1, 1, 1),
            R16Unorm | R16Snorm | R16Uint | R16Sint | R16Float | Rg8Unorm | Rg8Snorm | Rg8Uint
            | Rg8Sint | B5g6r5Unorm | Bgr5a1Unorm | Bgra4Unorm | Depth16Unorm => (1, 1, 2),
            R32Uint | R32Sint | R32Float | Rg16Unorm | Rg16Snorm | Rg16Uint | Rg16Sint
            | Rg16Float | Rgba8Unorm | Rgba8UnormSrgb | Rgba8Snorm | Rgba8Uint | Rgba8Sint
            | Bgra8Unorm | Bgra8UnormSrgb | Bgrx8Unorm | Bgrx8UnormSrgb | Rgb10a2Unorm
            | Rgb10a2Uint | Rg11b10Float | Rgb9e5Float | Depth24Plus | Depth24PlusStencil8
            | Depth32Float => (1, 1, 4),
            Depth32FloatStencil8 => (1, 1, 8),
            Rgbg8Unorm | Grgb8Unorm => (2, 1, 4),
            Rg32Uint | Rg32Sint | Rg32Float | Rgba16Unorm | Rgba16Snorm | Rgba16Uint
            | Rgba16Sint | Rgba16Float => (1, 1, 8),
            Rgb32Uint | Rgb32Sint | Rgb32Float => (1, 1, 12),
            Rgba32Uint | Rgba32Sint | Rgba32Float => (1, 1, 16),
            Bc1RgbaUnorm | Bc1RgbaUnormSrgb | Bc4RUnorm | Bc4RSnorm | Etc2Rgb8Unorm
            | Etc2Rgb8UnormSrgb => (4, 4, 8),
            Bc2RgbaUnorm | Bc2RgbaUnormSrgb | Bc3RgbaUnorm | Bc3RgbaUnormSrgb | Bc5RgUnorm
            | Bc5RgSnorm | Bc6hRgbUfloat | Bc6hRgbFloat | Bc7RgbaUnorm | Bc7RgbaUnormSrgb
            | Etc2Rgba8Unorm | Etc2Rgba8UnormSrgb | Astc4x4Unorm | Astc4x4UnormSrgb => (4, 4, 16),
        }
    }

    /// How shaders see the format.
    pub fn kind(self) -> FormatKind {
        use Format::*;
        match self {
            R8Uint | R16Uint | Rg8Uint | R32Uint | Rg16Uint | Rgba8Uint | Rgb10a2Uint
            | Rg32Uint | Rgba16Uint | Rgb32Uint | Rgba32Uint => FormatKind::Uint,
            R8Sint | R16Sint | Rg8Sint | R32Sint | Rg16Sint | Rgba8Sint | Rg32Sint | Rgba16Sint
            | Rgb32Sint | Rgba32Sint => FormatKind::Sint,
            Depth16Unorm | Depth24Plus | Depth32Float => FormatKind::Depth,
            Depth24PlusStencil8 | Depth32FloatStencil8 => FormatKind::DepthStencil,
            Stencil8 => FormatKind::Stencil,
            _ => FormatKind::Float,
        }
    }

    /// Whether this format holds depth, stencil or both.
    pub fn is_depth_stencil(self) -> bool {
        matches!(
            self.kind(),
            FormatKind::Depth | FormatKind::DepthStencil | FormatKind::Stencil
        )
    }

    /// The same bits read as linear values, for formats that have an sRGB twin.
    pub fn linear(self) -> Format {
        use Format::*;
        match self {
            Rgba8UnormSrgb => Rgba8Unorm,
            Bgra8UnormSrgb => Bgra8Unorm,
            Bgrx8UnormSrgb => Bgrx8Unorm,
            Bc1RgbaUnormSrgb => Bc1RgbaUnorm,
            Bc2RgbaUnormSrgb => Bc2RgbaUnorm,
            Bc3RgbaUnormSrgb => Bc3RgbaUnorm,
            Bc7RgbaUnormSrgb => Bc7RgbaUnorm,
            Etc2Rgb8UnormSrgb => Etc2Rgb8Unorm,
            Etc2Rgba8UnormSrgb => Etc2Rgba8Unorm,
            Astc4x4UnormSrgb => Astc4x4Unorm,
            f => f,
        }
    }
}
