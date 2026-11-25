//! DXGI format numbers (`DXGI_FORMAT`) and the interface formats that hold them.

use wcontainers_gpu::{Caps, FORMAT_FACTS, Format, FormatUse, VertexFormat};

/// `DXGI_FORMAT` values (dxgiformat.h), named as there without the prefix (`420_OPAQUE` as
/// `OPAQUE_420`).
pub mod dxgi {
    pub const UNKNOWN: u32 = 0;
    pub const R32G32B32A32_TYPELESS: u32 = 1;
    pub const R32G32B32A32_FLOAT: u32 = 2;
    pub const R32G32B32A32_UINT: u32 = 3;
    pub const R32G32B32A32_SINT: u32 = 4;
    pub const R32G32B32_TYPELESS: u32 = 5;
    pub const R32G32B32_FLOAT: u32 = 6;
    pub const R32G32B32_UINT: u32 = 7;
    pub const R32G32B32_SINT: u32 = 8;
    pub const R16G16B16A16_TYPELESS: u32 = 9;
    pub const R16G16B16A16_FLOAT: u32 = 10;
    pub const R16G16B16A16_UNORM: u32 = 11;
    pub const R16G16B16A16_UINT: u32 = 12;
    pub const R16G16B16A16_SNORM: u32 = 13;
    pub const R16G16B16A16_SINT: u32 = 14;
    pub const R32G32_TYPELESS: u32 = 15;
    pub const R32G32_FLOAT: u32 = 16;
    pub const R32G32_UINT: u32 = 17;
    pub const R32G32_SINT: u32 = 18;
    pub const R32G8X24_TYPELESS: u32 = 19;
    pub const D32_FLOAT_S8X24_UINT: u32 = 20;
    pub const R32_FLOAT_X8X24_TYPELESS: u32 = 21;
    pub const X32_TYPELESS_G8X24_UINT: u32 = 22;
    pub const R10G10B10A2_TYPELESS: u32 = 23;
    pub const R10G10B10A2_UNORM: u32 = 24;
    pub const R10G10B10A2_UINT: u32 = 25;
    pub const R11G11B10_FLOAT: u32 = 26;
    pub const R8G8B8A8_TYPELESS: u32 = 27;
    pub const R8G8B8A8_UNORM: u32 = 28;
    pub const R8G8B8A8_UNORM_SRGB: u32 = 29;
    pub const R8G8B8A8_UINT: u32 = 30;
    pub const R8G8B8A8_SNORM: u32 = 31;
    pub const R8G8B8A8_SINT: u32 = 32;
    pub const R16G16_TYPELESS: u32 = 33;
    pub const R16G16_FLOAT: u32 = 34;
    pub const R16G16_UNORM: u32 = 35;
    pub const R16G16_UINT: u32 = 36;
    pub const R16G16_SNORM: u32 = 37;
    pub const R16G16_SINT: u32 = 38;
    pub const R32_TYPELESS: u32 = 39;
    pub const D32_FLOAT: u32 = 40;
    pub const R32_FLOAT: u32 = 41;
    pub const R32_UINT: u32 = 42;
    pub const R32_SINT: u32 = 43;
    pub const R24G8_TYPELESS: u32 = 44;
    pub const D24_UNORM_S8_UINT: u32 = 45;
    pub const R24_UNORM_X8_TYPELESS: u32 = 46;
    pub const X24_TYPELESS_G8_UINT: u32 = 47;
    pub const R8G8_TYPELESS: u32 = 48;
    pub const R8G8_UNORM: u32 = 49;
    pub const R8G8_UINT: u32 = 50;
    pub const R8G8_SNORM: u32 = 51;
    pub const R8G8_SINT: u32 = 52;
    pub const R16_TYPELESS: u32 = 53;
    pub const R16_FLOAT: u32 = 54;
    pub const D16_UNORM: u32 = 55;
    pub const R16_UNORM: u32 = 56;
    pub const R16_UINT: u32 = 57;
    pub const R16_SNORM: u32 = 58;
    pub const R16_SINT: u32 = 59;
    pub const R8_TYPELESS: u32 = 60;
    pub const R8_UNORM: u32 = 61;
    pub const R8_UINT: u32 = 62;
    pub const R8_SNORM: u32 = 63;
    pub const R8_SINT: u32 = 64;
    pub const A8_UNORM: u32 = 65;
    pub const R1_UNORM: u32 = 66;
    pub const R9G9B9E5_SHAREDEXP: u32 = 67;
    pub const R8G8_B8G8_UNORM: u32 = 68;
    pub const G8R8_G8B8_UNORM: u32 = 69;
    pub const BC1_TYPELESS: u32 = 70;
    pub const BC1_UNORM: u32 = 71;
    pub const BC1_UNORM_SRGB: u32 = 72;
    pub const BC2_TYPELESS: u32 = 73;
    pub const BC2_UNORM: u32 = 74;
    pub const BC2_UNORM_SRGB: u32 = 75;
    pub const BC3_TYPELESS: u32 = 76;
    pub const BC3_UNORM: u32 = 77;
    pub const BC3_UNORM_SRGB: u32 = 78;
    pub const BC4_TYPELESS: u32 = 79;
    pub const BC4_UNORM: u32 = 80;
    pub const BC4_SNORM: u32 = 81;
    pub const BC5_TYPELESS: u32 = 82;
    pub const BC5_UNORM: u32 = 83;
    pub const BC5_SNORM: u32 = 84;
    pub const B5G6R5_UNORM: u32 = 85;
    pub const B5G5R5A1_UNORM: u32 = 86;
    pub const B8G8R8A8_UNORM: u32 = 87;
    pub const B8G8R8X8_UNORM: u32 = 88;
    pub const R10G10B10_XR_BIAS_A2_UNORM: u32 = 89;
    pub const B8G8R8A8_TYPELESS: u32 = 90;
    pub const B8G8R8A8_UNORM_SRGB: u32 = 91;
    pub const B8G8R8X8_TYPELESS: u32 = 92;
    pub const B8G8R8X8_UNORM_SRGB: u32 = 93;
    pub const BC6H_TYPELESS: u32 = 94;
    pub const BC6H_UF16: u32 = 95;
    pub const BC6H_SF16: u32 = 96;
    pub const BC7_TYPELESS: u32 = 97;
    pub const BC7_UNORM: u32 = 98;
    pub const BC7_UNORM_SRGB: u32 = 99;
    pub const AYUV: u32 = 100;
    pub const Y410: u32 = 101;
    pub const Y416: u32 = 102;
    pub const NV12: u32 = 103;
    pub const P010: u32 = 104;
    pub const P016: u32 = 105;
    pub const OPAQUE_420: u32 = 106;
    pub const YUY2: u32 = 107;
    pub const Y210: u32 = 108;
    pub const Y216: u32 = 109;
    pub const NV11: u32 = 110;
    pub const AI44: u32 = 111;
    pub const IA44: u32 = 112;
    pub const P8: u32 = 113;
    pub const A8P8: u32 = 114;
    pub const B4G4R4A4_UNORM: u32 = 115;
    pub const P208: u32 = 130;
    pub const V208: u32 = 131;
    pub const V408: u32 = 132;
}

/// The interface format a `DXGI_FORMAT` value stores as, and whether the DXGI format is typeless
/// (its texels are then stored in the group's first typed member, which views reinterpret). The
/// depth and stencil views of a depth format (`R24_UNORM_X8_TYPELESS`, `X24_TYPELESS_G8_UINT`,
/// `R32_FLOAT_X8X24_TYPELESS`, `X32_TYPELESS_G8X24_UINT`) give the depth format itself; which aspect
/// they read is the front's to say. None for formats the interface has no storage for (`R1_UNORM`,
/// `R10G10B10_XR_BIAS_A2_UNORM`, the video formats), which fronts report through their own format
/// support queries.
pub fn dxgi_format(value: u32) -> Option<(Format, bool)> {
    use Format::*;
    let typed = |f| Some((f, false));
    let typeless = |f| Some((f, true));
    match value {
        dxgi::R32G32B32A32_TYPELESS => typeless(Rgba32Float),
        dxgi::R32G32B32A32_FLOAT => typed(Rgba32Float),
        dxgi::R32G32B32A32_UINT => typed(Rgba32Uint),
        dxgi::R32G32B32A32_SINT => typed(Rgba32Sint),
        dxgi::R32G32B32_TYPELESS => typeless(Rgb32Float),
        dxgi::R32G32B32_FLOAT => typed(Rgb32Float),
        dxgi::R32G32B32_UINT => typed(Rgb32Uint),
        dxgi::R32G32B32_SINT => typed(Rgb32Sint),
        dxgi::R16G16B16A16_TYPELESS => typeless(Rgba16Float),
        dxgi::R16G16B16A16_FLOAT => typed(Rgba16Float),
        dxgi::R16G16B16A16_UNORM => typed(Rgba16Unorm),
        dxgi::R16G16B16A16_UINT => typed(Rgba16Uint),
        dxgi::R16G16B16A16_SNORM => typed(Rgba16Snorm),
        dxgi::R16G16B16A16_SINT => typed(Rgba16Sint),
        dxgi::R32G32_TYPELESS => typeless(Rg32Float),
        dxgi::R32G32_FLOAT => typed(Rg32Float),
        dxgi::R32G32_UINT => typed(Rg32Uint),
        dxgi::R32G32_SINT => typed(Rg32Sint),
        dxgi::R32G8X24_TYPELESS => typeless(Depth32FloatStencil8),
        dxgi::D32_FLOAT_S8X24_UINT..=dxgi::X32_TYPELESS_G8X24_UINT => typed(Depth32FloatStencil8),
        dxgi::R10G10B10A2_TYPELESS => typeless(Rgb10a2Unorm),
        dxgi::R10G10B10A2_UNORM => typed(Rgb10a2Unorm),
        dxgi::R10G10B10A2_UINT => typed(Rgb10a2Uint),
        dxgi::R11G11B10_FLOAT => typed(Rg11b10Float),
        dxgi::R8G8B8A8_TYPELESS => typeless(Rgba8Unorm),
        dxgi::R8G8B8A8_UNORM => typed(Rgba8Unorm),
        dxgi::R8G8B8A8_UNORM_SRGB => typed(Rgba8UnormSrgb),
        dxgi::R8G8B8A8_UINT => typed(Rgba8Uint),
        dxgi::R8G8B8A8_SNORM => typed(Rgba8Snorm),
        dxgi::R8G8B8A8_SINT => typed(Rgba8Sint),
        dxgi::R16G16_TYPELESS => typeless(Rg16Float),
        dxgi::R16G16_FLOAT => typed(Rg16Float),
        dxgi::R16G16_UNORM => typed(Rg16Unorm),
        dxgi::R16G16_UINT => typed(Rg16Uint),
        dxgi::R16G16_SNORM => typed(Rg16Snorm),
        dxgi::R16G16_SINT => typed(Rg16Sint),
        dxgi::R32_TYPELESS => typeless(R32Float),
        dxgi::D32_FLOAT => typed(Depth32Float),
        dxgi::R32_FLOAT => typed(R32Float),
        dxgi::R32_UINT => typed(R32Uint),
        dxgi::R32_SINT => typed(R32Sint),
        dxgi::R24G8_TYPELESS => typeless(Depth24PlusStencil8),
        dxgi::D24_UNORM_S8_UINT..=dxgi::X24_TYPELESS_G8_UINT => typed(Depth24PlusStencil8),
        dxgi::R8G8_TYPELESS => typeless(Rg8Unorm),
        dxgi::R8G8_UNORM => typed(Rg8Unorm),
        dxgi::R8G8_UINT => typed(Rg8Uint),
        dxgi::R8G8_SNORM => typed(Rg8Snorm),
        dxgi::R8G8_SINT => typed(Rg8Sint),
        dxgi::R16_TYPELESS => typeless(R16Float),
        dxgi::R16_FLOAT => typed(R16Float),
        dxgi::D16_UNORM => typed(Depth16Unorm),
        dxgi::R16_UNORM => typed(R16Unorm),
        dxgi::R16_UINT => typed(R16Uint),
        dxgi::R16_SNORM => typed(R16Snorm),
        dxgi::R16_SINT => typed(R16Sint),
        dxgi::R8_TYPELESS => typeless(R8Unorm),
        dxgi::R8_UNORM => typed(R8Unorm),
        dxgi::R8_UINT => typed(R8Uint),
        dxgi::R8_SNORM => typed(R8Snorm),
        dxgi::R8_SINT => typed(R8Sint),
        dxgi::A8_UNORM => typed(A8Unorm),
        dxgi::R9G9B9E5_SHAREDEXP => typed(Rgb9e5Float),
        dxgi::R8G8_B8G8_UNORM => typed(Rgbg8Unorm),
        dxgi::G8R8_G8B8_UNORM => typed(Grgb8Unorm),
        dxgi::BC1_TYPELESS => typeless(Bc1RgbaUnorm),
        dxgi::BC1_UNORM => typed(Bc1RgbaUnorm),
        dxgi::BC1_UNORM_SRGB => typed(Bc1RgbaUnormSrgb),
        dxgi::BC2_TYPELESS => typeless(Bc2RgbaUnorm),
        dxgi::BC2_UNORM => typed(Bc2RgbaUnorm),
        dxgi::BC2_UNORM_SRGB => typed(Bc2RgbaUnormSrgb),
        dxgi::BC3_TYPELESS => typeless(Bc3RgbaUnorm),
        dxgi::BC3_UNORM => typed(Bc3RgbaUnorm),
        dxgi::BC3_UNORM_SRGB => typed(Bc3RgbaUnormSrgb),
        dxgi::BC4_TYPELESS => typeless(Bc4RUnorm),
        dxgi::BC4_UNORM => typed(Bc4RUnorm),
        dxgi::BC4_SNORM => typed(Bc4RSnorm),
        dxgi::BC5_TYPELESS => typeless(Bc5RgUnorm),
        dxgi::BC5_UNORM => typed(Bc5RgUnorm),
        dxgi::BC5_SNORM => typed(Bc5RgSnorm),
        dxgi::B5G6R5_UNORM => typed(B5g6r5Unorm),
        dxgi::B5G5R5A1_UNORM => typed(Bgr5a1Unorm),
        dxgi::B8G8R8A8_UNORM => typed(Bgra8Unorm),
        dxgi::B8G8R8X8_UNORM => typed(Bgrx8Unorm),
        dxgi::B8G8R8A8_TYPELESS => typeless(Bgra8Unorm),
        dxgi::B8G8R8A8_UNORM_SRGB => typed(Bgra8UnormSrgb),
        dxgi::B8G8R8X8_TYPELESS => typeless(Bgrx8Unorm),
        dxgi::B8G8R8X8_UNORM_SRGB => typed(Bgrx8UnormSrgb),
        dxgi::BC6H_TYPELESS => typeless(Bc6hRgbUfloat),
        dxgi::BC6H_UF16 => typed(Bc6hRgbUfloat),
        dxgi::BC6H_SF16 => typed(Bc6hRgbFloat),
        dxgi::BC7_TYPELESS => typeless(Bc7RgbaUnorm),
        dxgi::BC7_UNORM => typed(Bc7RgbaUnorm),
        dxgi::BC7_UNORM_SRGB => typed(Bc7RgbaUnormSrgb),
        dxgi::B4G4R4A4_UNORM => typed(Bgra4Unorm),
        _ => None,
    }
}

/// The typeless member of the group whose members share `value`'s bit layout, which views of one
/// another's resources may take; None for a format in no such group.
pub fn typeless_group(value: u32) -> Option<u32> {
    use dxgi::*;
    Some(match value {
        R32G32B32A32_TYPELESS..=R32G32B32A32_SINT => R32G32B32A32_TYPELESS,
        R32G32B32_TYPELESS..=R32G32B32_SINT => R32G32B32_TYPELESS,
        R16G16B16A16_TYPELESS..=R16G16B16A16_SINT => R16G16B16A16_TYPELESS,
        R32G32_TYPELESS..=R32G32_SINT => R32G32_TYPELESS,
        R32G8X24_TYPELESS..=X32_TYPELESS_G8X24_UINT => R32G8X24_TYPELESS,
        R10G10B10A2_TYPELESS..=R10G10B10A2_UINT => R10G10B10A2_TYPELESS,
        R8G8B8A8_TYPELESS..=R8G8B8A8_SINT => R8G8B8A8_TYPELESS,
        R16G16_TYPELESS..=R16G16_SINT => R16G16_TYPELESS,
        R32_TYPELESS..=R32_SINT => R32_TYPELESS,
        R24G8_TYPELESS..=X24_TYPELESS_G8_UINT => R24G8_TYPELESS,
        R8G8_TYPELESS..=R8G8_SINT => R8G8_TYPELESS,
        R16_TYPELESS..=R16_SINT => R16_TYPELESS,
        R8_TYPELESS..=R8_SINT => R8_TYPELESS,
        BC1_TYPELESS..=BC1_UNORM_SRGB => BC1_TYPELESS,
        BC2_TYPELESS..=BC2_UNORM_SRGB => BC2_TYPELESS,
        BC3_TYPELESS..=BC3_UNORM_SRGB => BC3_TYPELESS,
        BC4_TYPELESS..=BC4_SNORM => BC4_TYPELESS,
        BC5_TYPELESS..=BC5_SNORM => BC5_TYPELESS,
        B8G8R8A8_UNORM | B8G8R8A8_TYPELESS | B8G8R8A8_UNORM_SRGB => B8G8R8A8_TYPELESS,
        B8G8R8X8_UNORM | B8G8R8X8_TYPELESS | B8G8R8X8_UNORM_SRGB => B8G8R8X8_TYPELESS,
        BC6H_TYPELESS..=BC6H_SF16 => BC6H_TYPELESS,
        BC7_TYPELESS..=BC7_UNORM_SRGB => BC7_TYPELESS,
        _ => return None,
    })
}

/// The back buffer formats a swap chain takes (DXGI's display formats): `R16G16B16A16_FLOAT`,
/// `R10G10B10A2_UNORM`, `R8G8B8A8_UNORM` and its sRGB twin, `B8G8R8A8_UNORM` and its sRGB twin, and
/// `R10G10B10_XR_BIAS_A2_UNORM`. The sRGB ones and the extended-range one only in the blit model, as
/// DXGI rules.
pub const BACK_BUFFER_FORMATS: [u32; 7] = [
    dxgi::R16G16B16A16_FLOAT,
    dxgi::R10G10B10A2_UNORM,
    dxgi::R8G8B8A8_UNORM,
    dxgi::R8G8B8A8_UNORM_SRGB,
    dxgi::B8G8R8A8_UNORM,
    dxgi::B8G8R8A8_UNORM_SRGB,
    dxgi::R10G10B10_XR_BIAS_A2_UNORM,
];

/// The interface format a back buffer of DXGI format `value` is held in: its own, and for the
/// extended-range format the bits of `R10G10B10A2_UNORM`, which the present copy decodes.
pub fn back_buffer_format(value: u32) -> Option<Format> {
    if value == dxgi::R10G10B10_XR_BIAS_A2_UNORM {
        return Some(Format::Rgb10a2Unorm);
    }
    dxgi_format(value).map(|(format, _)| format)
}

/// The interface format a surface is configured with for a back buffer of `format`. Swap chains
/// present in SDR sRGB only (`presents_in`), so a float surface would add nothing: a float back
/// buffer is presented encoded, like the others, into `rgb10a2unorm` where presented. Other formats
/// use their own where presented, else `rgb10a2unorm` for 10-bit buffers, else `rgba8unorm`, which
/// every back end presents.
pub fn present_format(caps: &Caps, format: Format) -> Format {
    let presents = |f: Format| caps.formats.get(f).contains(FormatUse::PRESENT);
    let own = FORMAT_FACTS[format as usize].linear;
    if own != Format::Rgba16Float && presents(own) {
        own
    } else if matches!(own, Format::Rgba16Float | Format::Rgb10a2Unorm)
        && presents(Format::Rgb10a2Unorm)
    {
        Format::Rgb10a2Unorm
    } else {
        Format::Rgba8Unorm
    }
}

/// The DXGI back buffer formats with what each is presented as on this back end.
pub fn present_formats(caps: &Caps) -> impl Iterator<Item = (u32, Format)> + '_ {
    BACK_BUFFER_FORMATS.into_iter().map(|value| {
        let format = back_buffer_format(value).expect("back buffer formats are held");
        (value, present_format(caps, format))
    })
}

/// The vertex format of a `DXGI_FORMAT` value and its size in bytes; None for a format that is no
/// vertex format. Back ends serve every [`VertexFormat`] (some by a slow path).
pub fn dxgi_vertex_format(value: u32) -> Option<(VertexFormat, u32)> {
    use VertexFormat::*;
    Some(match value {
        dxgi::R32G32B32A32_FLOAT => (Float32x4, 16),
        dxgi::R32G32B32A32_UINT => (Uint32x4, 16),
        dxgi::R32G32B32A32_SINT => (Sint32x4, 16),
        dxgi::R32G32B32_FLOAT => (Float32x3, 12),
        dxgi::R32G32B32_UINT => (Uint32x3, 12),
        dxgi::R32G32B32_SINT => (Sint32x3, 12),
        dxgi::R16G16B16A16_FLOAT => (Float16x4, 8),
        dxgi::R16G16B16A16_UNORM => (Unorm16x4, 8),
        dxgi::R16G16B16A16_UINT => (Uint16x4, 8),
        dxgi::R16G16B16A16_SNORM => (Snorm16x4, 8),
        dxgi::R16G16B16A16_SINT => (Sint16x4, 8),
        dxgi::R32G32_FLOAT => (Float32x2, 8),
        dxgi::R32G32_UINT => (Uint32x2, 8),
        dxgi::R32G32_SINT => (Sint32x2, 8),
        dxgi::R10G10B10A2_UNORM => (Unorm10_10_10_2, 4),
        dxgi::R10G10B10A2_UINT => (Uint10_10_10_2, 4),
        dxgi::R11G11B10_FLOAT => (Float11_11_10, 4),
        dxgi::R8G8B8A8_UNORM => (Unorm8x4, 4),
        dxgi::R8G8B8A8_UINT => (Uint8x4, 4),
        dxgi::R8G8B8A8_SNORM => (Snorm8x4, 4),
        dxgi::R8G8B8A8_SINT => (Sint8x4, 4),
        dxgi::R16G16_FLOAT => (Float16x2, 4),
        dxgi::R16G16_UNORM => (Unorm16x2, 4),
        dxgi::R16G16_UINT => (Uint16x2, 4),
        dxgi::R16G16_SNORM => (Snorm16x2, 4),
        dxgi::R16G16_SINT => (Sint16x2, 4),
        dxgi::R32_FLOAT => (Float32x1, 4),
        dxgi::R32_UINT => (Uint32x1, 4),
        dxgi::R32_SINT => (Sint32x1, 4),
        dxgi::R8G8_UNORM => (Unorm8x2, 2),
        dxgi::R8G8_UINT => (Uint8x2, 2),
        dxgi::R8G8_SNORM => (Snorm8x2, 2),
        dxgi::R8G8_SINT => (Sint8x2, 2),
        dxgi::R16_FLOAT => (Float16x1, 2),
        dxgi::R16_UNORM => (Unorm16x1, 2),
        dxgi::R16_UINT => (Uint16x1, 2),
        dxgi::R16_SNORM => (Snorm16x1, 2),
        dxgi::R16_SINT => (Sint16x1, 2),
        dxgi::R8_UNORM => (Unorm8x1, 1),
        dxgi::R8_UINT => (Uint8x1, 1),
        dxgi::R8_SNORM => (Snorm8x1, 1),
        dxgi::R8_SINT => (Sint8x1, 1),
        dxgi::B8G8R8A8_UNORM => (Bgra8Unorm, 4),
        dxgi::B8G8R8X8_UNORM => (
            Bgrx8Unorm,
            FORMAT_FACTS[Format::Bgrx8Unorm as usize].block.2,
        ),
        _ => return None,
    })
}
