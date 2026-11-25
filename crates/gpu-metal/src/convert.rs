//! The interface's formats and state blocks in Metal's terms, and what this implementation declares it does with each format.

use wcontainers_gpu::{
    AddressMode, BlendFactor as GpuBlendFactor, BlendOp, BorderColor, CompareFn,
    CullMode as GpuCullMode, FORMAT_COUNT, Features, Filter, Format, FormatCaps, FormatUse,
    FrontFace, GpuError, StencilFace, StencilOp, VertexFormat as GpuVertexFormat,
};
use wcontainers_metal::{
    BlendFactor, BlendOperation, CompareFunction, CullMode, PixelFormat, SamplerAddressMode,
    SamplerBorderColor, SamplerMinMagFilter, StencilDescriptor, StencilOperation, VertexFormat,
    Winding,
};

/// Returns the Metal format a texture of an interface format is held in, if this implementation holds it.
pub(crate) fn pixel_format(format: Format) -> Option<PixelFormat> {
    use Format as F;
    use PixelFormat as P;
    Some(match format {
        F::R8Unorm => P::R8Unorm,
        F::R8Snorm => P::R8Snorm,
        F::R8Uint => P::R8Uint,
        F::R8Sint => P::R8Sint,
        F::R16Unorm => P::R16Unorm,
        F::R16Snorm => P::R16Snorm,
        F::R16Uint => P::R16Uint,
        F::R16Sint => P::R16Sint,
        F::R16Float => P::R16Float,
        F::Rg8Unorm => P::Rg8Unorm,
        F::Rg8Snorm => P::Rg8Snorm,
        F::Rg8Uint => P::Rg8Uint,
        F::Rg8Sint => P::Rg8Sint,
        F::R32Uint => P::R32Uint,
        F::R32Sint => P::R32Sint,
        F::R32Float => P::R32Float,
        F::Rg16Unorm => P::Rg16Unorm,
        F::Rg16Snorm => P::Rg16Snorm,
        F::Rg16Uint => P::Rg16Uint,
        F::Rg16Sint => P::Rg16Sint,
        F::Rg16Float => P::Rg16Float,
        F::Rgba8Unorm => P::Rgba8Unorm,
        F::Rgba8UnormSrgb => P::Rgba8UnormSrgb,
        F::Rgba8Snorm => P::Rgba8Snorm,
        F::Rgba8Uint => P::Rgba8Uint,
        F::Rgba8Sint => P::Rgba8Sint,
        F::Bgra8Unorm => P::Bgra8Unorm,
        F::Bgra8UnormSrgb => P::Bgra8UnormSrgb,
        F::Rgb10a2Unorm => P::Rgb10a2Unorm,
        F::Rgb10a2Uint => P::Rgb10a2Uint,
        F::Rg11b10Float => P::Rg11b10Float,
        F::Rgb9e5Float => P::Rgb9e5Float,
        F::B5g6r5Unorm => P::B5g6r5Unorm,
        F::Rg32Uint => P::Rg32Uint,
        F::Rg32Sint => P::Rg32Sint,
        F::Rg32Float => P::Rg32Float,
        F::Rgba16Unorm => P::Rgba16Unorm,
        F::Rgba16Snorm => P::Rgba16Snorm,
        F::Rgba16Uint => P::Rgba16Uint,
        F::Rgba16Sint => P::Rgba16Sint,
        F::Rgba16Float => P::Rgba16Float,
        F::Rgba32Uint => P::Rgba32Uint,
        F::Rgba32Sint => P::Rgba32Sint,
        F::Rgba32Float => P::Rgba32Float,
        F::A8Unorm => P::A8Unorm,
        F::Stencil8 => P::Stencil8,
        F::Depth16Unorm => P::Depth16Unorm,
        F::Depth24Plus | F::Depth32Float => P::Depth32Float,
        F::Depth24PlusStencil8 | F::Depth32FloatStencil8 => P::Depth32FloatStencil8,
        F::Bc1RgbaUnorm => P::Bc1Rgba,
        F::Bc1RgbaUnormSrgb => P::Bc1RgbaSrgb,
        F::Bc2RgbaUnorm => P::Bc2Rgba,
        F::Bc2RgbaUnormSrgb => P::Bc2RgbaSrgb,
        F::Bc3RgbaUnorm => P::Bc3Rgba,
        F::Bc3RgbaUnormSrgb => P::Bc3RgbaSrgb,
        F::Bc4RUnorm => P::Bc4RUnorm,
        F::Bc4RSnorm => P::Bc4RSnorm,
        F::Bc5RgUnorm => P::Bc5RgUnorm,
        F::Bc5RgSnorm => P::Bc5RgSnorm,
        F::Bc6hRgbUfloat => P::Bc6hRgbUfloat,
        F::Bc6hRgbFloat => P::Bc6hRgbFloat,
        F::Bc7RgbaUnorm => P::Bc7RgbaUnorm,
        F::Bc7RgbaUnormSrgb => P::Bc7RgbaUnormSrgb,
        F::Etc2Rgb8Unorm => P::Etc2Rgb8,
        F::Etc2Rgb8UnormSrgb => P::Etc2Rgb8Srgb,
        F::Etc2Rgba8Unorm => P::EacRgba8,
        F::Etc2Rgba8UnormSrgb => P::EacRgba8Srgb,
        F::Astc4x4Unorm => P::Astc4x4Ldr,
        F::Astc4x4UnormSrgb => P::Astc4x4Srgb,
        F::Bgrx8Unorm
        | F::Bgrx8UnormSrgb
        | F::Bgr5a1Unorm
        | F::Bgra4Unorm
        | F::Rgbg8Unorm
        | F::Grgb8Unorm
        | F::Rgb32Uint
        | F::Rgb32Sint
        | F::Rgb32Float => return None,
    })
}

/// Returns whether a format's depth is held as 32-bit float while its buffers hold 24-bit unorm in the low bits of a word.
pub(crate) fn depth_converted(format: Format) -> bool {
    matches!(format, Format::Depth24Plus | Format::Depth24PlusStencil8)
}

/// The sample counts every format drawn into takes: one and four.
const DRAWN_SAMPLES: u8 = 0b101;

/// Returns what this implementation does with each format, on Apple's GPUs of macOS.
pub(crate) fn format_caps() -> FormatCaps {
    let mut uses = [FormatUse::empty(); FORMAT_COUNT];
    let mut samples = [1; FORMAT_COUNT];
    let filtered = FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT;
    let drawn = filtered | FormatUse::RENDER | FormatUse::BLEND | FormatUse::RESOLVE;
    let integer = FormatUse::SAMPLE | FormatUse::RENDER | FormatUse::COPY_OUT;
    let depth =
        FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::DEPTH_STENCIL | FormatUse::COPY_OUT;
    let stencil = FormatUse::SAMPLE | FormatUse::DEPTH_STENCIL | FormatUse::COPY_OUT;
    use Format as F;
    let table: &[(&[Format], FormatUse, bool)] = &[
        (
            &[
                F::R8Unorm,
                F::R8Snorm,
                F::R16Unorm,
                F::R16Snorm,
                F::R16Float,
                F::Rg8Unorm,
                F::Rg8Snorm,
                F::Rg16Unorm,
                F::Rg16Snorm,
                F::Rg16Float,
                F::Rgba8UnormSrgb,
                F::Rgba8Snorm,
                F::Rgb10a2Unorm,
                F::Rg11b10Float,
                F::B5g6r5Unorm,
                F::Rgba16Unorm,
                F::Rgba16Snorm,
                F::Rgba16Float,
            ],
            drawn,
            true,
        ),
        (&[F::Rgba8Unorm], drawn | FormatUse::PRESENT, true),
        (
            &[F::Bgra8Unorm, F::Bgra8UnormSrgb],
            drawn | FormatUse::PRESENT,
            true,
        ),
        (
            &[F::R32Float, F::Rg32Float, F::Rgba32Float],
            filtered | FormatUse::RENDER | FormatUse::BLEND,
            true,
        ),
        (
            &[
                F::R8Uint,
                F::R8Sint,
                F::R16Uint,
                F::R16Sint,
                F::Rg8Uint,
                F::Rg8Sint,
                F::R32Uint,
                F::R32Sint,
                F::Rg16Uint,
                F::Rg16Sint,
                F::Rgba8Uint,
                F::Rgba8Sint,
                F::Rgb10a2Uint,
                F::Rg32Uint,
                F::Rg32Sint,
                F::Rgba16Uint,
                F::Rgba16Sint,
                F::Rgba32Uint,
                F::Rgba32Sint,
            ],
            integer,
            true,
        ),
        (&[F::Rgb9e5Float, F::A8Unorm], filtered, false),
        (
            &[F::Depth16Unorm, F::Depth24Plus, F::Depth32Float],
            depth,
            true,
        ),
        (
            &[F::Stencil8, F::Depth24PlusStencil8, F::Depth32FloatStencil8],
            stencil,
            true,
        ),
        (
            &[
                F::Bc1RgbaUnorm,
                F::Bc1RgbaUnormSrgb,
                F::Bc2RgbaUnorm,
                F::Bc2RgbaUnormSrgb,
                F::Bc3RgbaUnorm,
                F::Bc3RgbaUnormSrgb,
                F::Bc4RUnorm,
                F::Bc4RSnorm,
                F::Bc5RgUnorm,
                F::Bc5RgSnorm,
                F::Bc6hRgbUfloat,
                F::Bc6hRgbFloat,
                F::Bc7RgbaUnorm,
                F::Bc7RgbaUnormSrgb,
                F::Etc2Rgb8Unorm,
                F::Etc2Rgb8UnormSrgb,
                F::Etc2Rgba8Unorm,
                F::Etc2Rgba8UnormSrgb,
                F::Astc4x4Unorm,
                F::Astc4x4UnormSrgb,
            ],
            filtered,
            false,
        ),
    ];
    for (formats, use_, multisampled) in table {
        for format in *formats {
            uses[*format as usize] = *use_;
            if *multisampled {
                samples[*format as usize] = DRAWN_SAMPLES;
            }
        }
    }
    FormatCaps { uses, samples }
}

/// Returns Metal's comparison of an interface comparison.
pub(crate) fn compare(compare: CompareFn) -> CompareFunction {
    match compare {
        CompareFn::Never => CompareFunction::Never,
        CompareFn::Less => CompareFunction::Less,
        CompareFn::Equal => CompareFunction::Equal,
        CompareFn::LessEqual => CompareFunction::LessEqual,
        CompareFn::Greater => CompareFunction::Greater,
        CompareFn::NotEqual => CompareFunction::NotEqual,
        CompareFn::GreaterEqual => CompareFunction::GreaterEqual,
        CompareFn::Always => CompareFunction::Always,
    }
}

/// Returns Metal's stencil operation of an interface one.
fn stencil_op(op: StencilOp) -> StencilOperation {
    match op {
        StencilOp::Keep => StencilOperation::Keep,
        StencilOp::Zero => StencilOperation::Zero,
        StencilOp::Replace => StencilOperation::Replace,
        StencilOp::IncrementClamp => StencilOperation::IncrementClamp,
        StencilOp::DecrementClamp => StencilOperation::DecrementClamp,
        StencilOp::Invert => StencilOperation::Invert,
        StencilOp::IncrementWrap => StencilOperation::IncrementWrap,
        StencilOp::DecrementWrap => StencilOperation::DecrementWrap,
    }
}

/// Returns Metal's stencil test of one face, with its masks.
pub(crate) fn stencil(face: &StencilFace, read_mask: u8, write_mask: u8) -> StencilDescriptor {
    StencilDescriptor {
        stencil_compare_function: compare(face.compare),
        stencil_failure_operation: stencil_op(face.fail),
        depth_failure_operation: stencil_op(face.depth_fail),
        depth_stencil_pass_operation: stencil_op(face.pass),
        read_mask: u32::from(read_mask),
        write_mask: u32::from(write_mask),
    }
}

/// Returns Metal's blend factor of an interface one.
pub(crate) fn blend_factor(factor: GpuBlendFactor) -> BlendFactor {
    use GpuBlendFactor as G;
    match factor {
        G::Zero => BlendFactor::Zero,
        G::One => BlendFactor::One,
        G::Src => BlendFactor::SourceColor,
        G::OneMinusSrc => BlendFactor::OneMinusSourceColor,
        G::SrcAlpha => BlendFactor::SourceAlpha,
        G::OneMinusSrcAlpha => BlendFactor::OneMinusSourceAlpha,
        G::Dst => BlendFactor::DestinationColor,
        G::OneMinusDst => BlendFactor::OneMinusDestinationColor,
        G::DstAlpha => BlendFactor::DestinationAlpha,
        G::OneMinusDstAlpha => BlendFactor::OneMinusDestinationAlpha,
        G::SrcAlphaSaturated => BlendFactor::SourceAlphaSaturated,
        G::Constant => BlendFactor::BlendColor,
        G::OneMinusConstant => BlendFactor::OneMinusBlendColor,
        G::ConstantAlpha => BlendFactor::BlendAlpha,
        G::OneMinusConstantAlpha => BlendFactor::OneMinusBlendAlpha,
        G::Src1 => BlendFactor::Source1Color,
        G::OneMinusSrc1 => BlendFactor::OneMinusSource1Color,
        G::Src1Alpha => BlendFactor::Source1Alpha,
        G::OneMinusSrc1Alpha => BlendFactor::OneMinusSource1Alpha,
    }
}

/// Returns Metal's blend operation of an interface one.
pub(crate) fn blend_op(op: BlendOp) -> BlendOperation {
    match op {
        BlendOp::Add => BlendOperation::Add,
        BlendOp::Subtract => BlendOperation::Subtract,
        BlendOp::ReverseSubtract => BlendOperation::ReverseSubtract,
        BlendOp::Min => BlendOperation::Min,
        BlendOp::Max => BlendOperation::Max,
    }
}

/// Returns Metal's cull mode of an interface one.
pub(crate) fn cull(mode: GpuCullMode) -> CullMode {
    match mode {
        GpuCullMode::None => CullMode::None,
        GpuCullMode::Front => CullMode::Front,
        GpuCullMode::Back => CullMode::Back,
    }
}

/// Returns Metal's front-facing winding of an interface front face.
pub(crate) fn winding(front: FrontFace) -> Winding {
    match front {
        FrontFace::CounterClockwise => Winding::CounterClockwise,
        FrontFace::Clockwise => Winding::Clockwise,
    }
}

/// Returns Metal's filter of an interface filter.
pub(crate) fn filter(filter: Filter) -> SamplerMinMagFilter {
    match filter {
        Filter::Nearest => SamplerMinMagFilter::Nearest,
        Filter::Linear => SamplerMinMagFilter::Linear,
    }
}

/// Returns Metal's address mode of an interface one and its border, or the feature it needs.
pub(crate) fn address(
    mode: AddressMode,
    border: BorderColor,
) -> Result<(SamplerAddressMode, SamplerBorderColor), GpuError> {
    let none = SamplerBorderColor::TransparentBlack;
    Ok(match (mode, border) {
        (AddressMode::Repeat, _) => (SamplerAddressMode::Repeat, none),
        (AddressMode::Mirror, _) => (SamplerAddressMode::MirrorRepeat, none),
        (AddressMode::Clamp, _) => (SamplerAddressMode::ClampToEdge, none),
        (AddressMode::Border, BorderColor::TransparentBlack) => {
            (SamplerAddressMode::ClampToZero, none)
        }
        (AddressMode::Border, _) => return Err(GpuError::Absent(Features::BORDER_COLOR)),
        (AddressMode::MirrorOnce, _) => return Err(GpuError::Absent(Features::MIRROR_ONCE)),
    })
}

/// Returns Metal's vertex format of an interface one, if naga's vertex reads have it.
pub(crate) fn vertex_format(format: GpuVertexFormat) -> Option<VertexFormat> {
    use GpuVertexFormat as G;
    use VertexFormat as V;
    Some(match format {
        G::Uint8x1 => V::UChar,
        G::Uint8x2 => V::UChar2,
        G::Uint8x4 => V::UChar4,
        G::Sint8x1 => V::Char,
        G::Sint8x2 => V::Char2,
        G::Sint8x4 => V::Char4,
        G::Unorm8x1 => V::UCharNormalized,
        G::Unorm8x2 => V::UChar2Normalized,
        G::Unorm8x4 => V::UChar4Normalized,
        G::Snorm8x1 => V::CharNormalized,
        G::Snorm8x2 => V::Char2Normalized,
        G::Snorm8x4 => V::Char4Normalized,
        G::Uint16x1 => V::UShort,
        G::Uint16x2 => V::UShort2,
        G::Uint16x4 => V::UShort4,
        G::Sint16x1 => V::Short,
        G::Sint16x2 => V::Short2,
        G::Sint16x4 => V::Short4,
        G::Unorm16x1 => V::UShortNormalized,
        G::Unorm16x2 => V::UShort2Normalized,
        G::Unorm16x4 => V::UShort4Normalized,
        G::Snorm16x1 => V::ShortNormalized,
        G::Snorm16x2 => V::Short2Normalized,
        G::Snorm16x4 => V::Short4Normalized,
        G::Float16x1 => V::Half,
        G::Float16x2 => V::Half2,
        G::Float16x4 => V::Half4,
        G::Float32x1 => V::Float,
        G::Float32x2 => V::Float2,
        G::Float32x3 => V::Float3,
        G::Float32x4 => V::Float4,
        G::Uint32x1 => V::UInt,
        G::Uint32x2 => V::UInt2,
        G::Uint32x3 => V::UInt3,
        G::Uint32x4 => V::UInt4,
        G::Sint32x1 => V::Int,
        G::Sint32x2 => V::Int2,
        G::Sint32x3 => V::Int3,
        G::Sint32x4 => V::Int4,
        G::Bgra8Unorm => V::UChar4NormalizedBgra,
        G::Unorm10_10_10_2 => V::UInt1010102Normalized,
        G::Uint8x3
        | G::Sint8x3
        | G::Unorm8x3
        | G::Snorm8x3
        | G::Uint16x3
        | G::Sint16x3
        | G::Unorm16x3
        | G::Snorm16x3
        | G::Float16x3
        | G::Uint10_10_10_2
        | G::Float11_11_10
        | G::Bgrx8Unorm => return None,
    })
}

/// Returns the bytes one attribute of an interface vertex format holds.
pub(crate) fn vertex_bytes(format: GpuVertexFormat) -> u32 {
    use GpuVertexFormat as G;
    match format {
        G::Uint8x1 | G::Sint8x1 | G::Unorm8x1 | G::Snorm8x1 => 1,
        G::Uint8x2 | G::Sint8x2 | G::Unorm8x2 | G::Snorm8x2 => 2,
        G::Uint8x3 | G::Sint8x3 | G::Unorm8x3 | G::Snorm8x3 => 3,
        G::Uint16x1 | G::Sint16x1 | G::Unorm16x1 | G::Snorm16x1 | G::Float16x1 => 2,
        G::Uint16x3 | G::Sint16x3 | G::Unorm16x3 | G::Snorm16x3 | G::Float16x3 => 6,
        G::Uint16x4 | G::Sint16x4 | G::Unorm16x4 | G::Snorm16x4 | G::Float16x4 => 8,
        G::Float32x2 | G::Uint32x2 | G::Sint32x2 => 8,
        G::Float32x3 | G::Uint32x3 | G::Sint32x3 => 12,
        G::Float32x4 | G::Uint32x4 | G::Sint32x4 => 16,
        _ => 4,
    }
}
