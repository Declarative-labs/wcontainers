//! DXGI format numbers (`DXGI_FORMAT`) and the interface formats that hold them.

use wcontainers_gpu::{Caps, Format, FormatUse, VertexFormat};

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
        1 => typeless(Rgba32Float),
        2 => typed(Rgba32Float),
        3 => typed(Rgba32Uint),
        4 => typed(Rgba32Sint),
        5 => typeless(Rgb32Float),
        6 => typed(Rgb32Float),
        7 => typed(Rgb32Uint),
        8 => typed(Rgb32Sint),
        9 => typeless(Rgba16Float),
        10 => typed(Rgba16Float),
        11 => typed(Rgba16Unorm),
        12 => typed(Rgba16Uint),
        13 => typed(Rgba16Snorm),
        14 => typed(Rgba16Sint),
        15 => typeless(Rg32Float),
        16 => typed(Rg32Float),
        17 => typed(Rg32Uint),
        18 => typed(Rg32Sint),
        19 => typeless(Depth32FloatStencil8),
        20..=22 => typed(Depth32FloatStencil8),
        23 => typeless(Rgb10a2Unorm),
        24 => typed(Rgb10a2Unorm),
        25 => typed(Rgb10a2Uint),
        26 => typed(Rg11b10Float),
        27 => typeless(Rgba8Unorm),
        28 => typed(Rgba8Unorm),
        29 => typed(Rgba8UnormSrgb),
        30 => typed(Rgba8Uint),
        31 => typed(Rgba8Snorm),
        32 => typed(Rgba8Sint),
        33 => typeless(Rg16Float),
        34 => typed(Rg16Float),
        35 => typed(Rg16Unorm),
        36 => typed(Rg16Uint),
        37 => typed(Rg16Snorm),
        38 => typed(Rg16Sint),
        39 => typeless(R32Float),
        40 => typed(Depth32Float),
        41 => typed(R32Float),
        42 => typed(R32Uint),
        43 => typed(R32Sint),
        44 => typeless(Depth24PlusStencil8),
        45..=47 => typed(Depth24PlusStencil8),
        48 => typeless(Rg8Unorm),
        49 => typed(Rg8Unorm),
        50 => typed(Rg8Uint),
        51 => typed(Rg8Snorm),
        52 => typed(Rg8Sint),
        53 => typeless(R16Float),
        54 => typed(R16Float),
        55 => typed(Depth16Unorm),
        56 => typed(R16Unorm),
        57 => typed(R16Uint),
        58 => typed(R16Snorm),
        59 => typed(R16Sint),
        60 => typeless(R8Unorm),
        61 => typed(R8Unorm),
        62 => typed(R8Uint),
        63 => typed(R8Snorm),
        64 => typed(R8Sint),
        65 => typed(A8Unorm),
        67 => typed(Rgb9e5Float),
        68 => typed(Rgbg8Unorm),
        69 => typed(Grgb8Unorm),
        70 => typeless(Bc1RgbaUnorm),
        71 => typed(Bc1RgbaUnorm),
        72 => typed(Bc1RgbaUnormSrgb),
        73 => typeless(Bc2RgbaUnorm),
        74 => typed(Bc2RgbaUnorm),
        75 => typed(Bc2RgbaUnormSrgb),
        76 => typeless(Bc3RgbaUnorm),
        77 => typed(Bc3RgbaUnorm),
        78 => typed(Bc3RgbaUnormSrgb),
        79 => typeless(Bc4RUnorm),
        80 => typed(Bc4RUnorm),
        81 => typed(Bc4RSnorm),
        82 => typeless(Bc5RgUnorm),
        83 => typed(Bc5RgUnorm),
        84 => typed(Bc5RgSnorm),
        85 => typed(B5g6r5Unorm),
        86 => typed(Bgr5a1Unorm),
        87 => typed(Bgra8Unorm),
        88 => typed(Bgrx8Unorm),
        90 => typeless(Bgra8Unorm),
        91 => typed(Bgra8UnormSrgb),
        92 => typeless(Bgrx8Unorm),
        93 => typed(Bgrx8UnormSrgb),
        94 => typeless(Bc6hRgbUfloat),
        95 => typed(Bc6hRgbUfloat),
        96 => typed(Bc6hRgbFloat),
        97 => typeless(Bc7RgbaUnorm),
        98 => typed(Bc7RgbaUnorm),
        99 => typed(Bc7RgbaUnormSrgb),
        115 => typed(Bgra4Unorm),
        _ => None,
    }
}

/// The back buffer formats a swap chain takes (DXGI's display formats): `R16G16B16A16_FLOAT`,
/// `R10G10B10A2_UNORM`, `R8G8B8A8_UNORM` and its sRGB twin, `B8G8R8A8_UNORM` and its sRGB twin. The
/// sRGB ones only in the blit model, as DXGI rules.
pub const BACK_BUFFER_FORMATS: [u32; 6] = [10, 24, 28, 29, 87, 91];

/// The interface format a surface is configured with for a back buffer of `format`. Swap chains
/// present in SDR sRGB only (`presents_in`), so a float surface would add nothing: a float back
/// buffer is presented encoded, like the others, into `rgb10a2unorm` where presented. Other formats
/// use their own where presented, else `rgb10a2unorm` for 10-bit buffers, else `rgba8unorm`, which
/// every back end presents.
pub fn present_format(caps: &Caps, format: Format) -> Format {
    let presents = |f: Format| caps.formats.get(f).contains(FormatUse::PRESENT);
    let own = format.linear();
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
        let (format, _) = dxgi_format(value).expect("back buffer formats are mapped");
        (value, present_format(caps, format))
    })
}

/// The vertex format of a `DXGI_FORMAT` value and its size in bytes; None for a format that is no
/// vertex format. Back ends serve every [`VertexFormat`] (some by a slow path).
pub fn dxgi_vertex_format(value: u32) -> Option<(VertexFormat, u32)> {
    use VertexFormat::*;
    Some(match value {
        2 => (Float32x4, 16),
        3 => (Uint32x4, 16),
        4 => (Sint32x4, 16),
        6 => (Float32x3, 12),
        7 => (Uint32x3, 12),
        8 => (Sint32x3, 12),
        10 => (Float16x4, 8),
        11 => (Unorm16x4, 8),
        12 => (Uint16x4, 8),
        13 => (Snorm16x4, 8),
        14 => (Sint16x4, 8),
        16 => (Float32x2, 8),
        17 => (Uint32x2, 8),
        18 => (Sint32x2, 8),
        24 => (Unorm10_10_10_2, 4),
        25 => (Uint10_10_10_2, 4),
        26 => (Float11_11_10, 4),
        28 => (Unorm8x4, 4),
        30 => (Uint8x4, 4),
        31 => (Snorm8x4, 4),
        32 => (Sint8x4, 4),
        34 => (Float16x2, 4),
        35 => (Unorm16x2, 4),
        36 => (Uint16x2, 4),
        37 => (Snorm16x2, 4),
        38 => (Sint16x2, 4),
        41 => (Float32x1, 4),
        42 => (Uint32x1, 4),
        43 => (Sint32x1, 4),
        49 => (Unorm8x2, 2),
        50 => (Uint8x2, 2),
        51 => (Snorm8x2, 2),
        52 => (Sint8x2, 2),
        54 => (Float16x1, 2),
        56 => (Unorm16x1, 2),
        57 => (Uint16x1, 2),
        58 => (Snorm16x1, 2),
        59 => (Sint16x1, 2),
        61 => (Unorm8x1, 1),
        62 => (Uint8x1, 1),
        63 => (Snorm8x1, 1),
        64 => (Sint8x1, 1),
        87 => (Bgra8Unorm, 4),
        _ => return None,
    })
}
