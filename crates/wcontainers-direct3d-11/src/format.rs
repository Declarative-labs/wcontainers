//! DXGI formats (their numbers in `dxgiformat.h`) as the interface's formats.

use wcontainers_gpu::{Format, IndexFormat, VertexFormat};

use crate::error::{Error, Result, not_implemented};

/// `DXGI_FORMAT_UNKNOWN`.
pub const UNKNOWN: u32 = 0;
pub const R16_UINT: u32 = 57;
pub const R32_UINT: u32 = 42;

/// The interface format of a DXGI format a texture or view names: `InvalidArg` for a number that is
/// no DXGI format, `NotImplemented` for a DXGI format this front does not serve (typeless formats,
/// among others).
pub fn texture_format_of(dxgi: u32) -> Result<Format> {
    texture_format(dxgi).map_or_else(|| unserved(dxgi), Ok)
}

/// Not served for a defined DXGI format, invalid for others.
fn unserved<T>(dxgi: u32) -> Result<T> {
    // Defined DXGI formats: 1 to 132, the sampler feedback formats 189 and 190, and 191.
    if matches!(dxgi, 1..=132 | 189..=191) {
        not_implemented("a DXGI format this front does not serve")
    } else {
        Err(Error::InvalidArg("unknown format"))
    }
}

/// The interface format a texture of DXGI format `dxgi` is stored as. A typeless texture is stored
/// as a member of its group: the depth format where it is bound for depth (or the group holds only
/// depth with stencil), otherwise a colour format whose views as itself and its sRGB twin need no
/// reinterpreting.
pub fn storage_format(dxgi: u32, depth_bound: bool) -> Result<Format> {
    use Format::*;
    let typeless = match (dxgi, depth_bound) {
        (1, _) => Rgba32Float,
        (9, _) => Rgba16Float,
        (15, _) => Rg32Float,
        (19, _) => Depth32FloatStencil8,
        (23, _) => Rgb10a2Unorm,
        (27, _) => Rgba8Unorm,
        (33, _) => Rg16Float,
        (39, true) => Depth32Float,
        (39, false) => R32Float,
        (44, _) => Depth24PlusStencil8,
        (48, _) => Rg8Unorm,
        (53, true) => Depth16Unorm,
        (53, false) => R16Float,
        (60, _) => R8Unorm,
        (70, _) => Bc1RgbaUnorm,
        (73, _) => Bc2RgbaUnorm,
        (76, _) => Bc3RgbaUnorm,
        (79, _) => Bc4RUnorm,
        (82, _) => Bc5RgUnorm,
        (90, _) => Bgra8Unorm,
        (92, _) => Bgrx8Unorm,
        (94, _) => Bc6hRgbUfloat,
        (97, _) => Bc7RgbaUnorm,
        _ => return texture_format_of(dxgi),
    };
    Ok(typeless)
}

/// The interface format and aspect (true for stencil) of a view of DXGI format `view` of a texture
/// stored as `storage` with DXGI format `texture`. A typed texture is viewed as its own format; a
/// typeless one as any member of its group, and Direct3D's depth views (`R24_UNORM_X8_TYPELESS`,
/// `X24_TYPELESS_G8_UINT`, `R32_FLOAT` of `R32_TYPELESS` and the like) read one aspect of the depth
/// format. A member whose bits another format of the group stores is viewed through a texture of
/// its own format kept beside the texture (`ReinterpretCopy`, see `reinterprets`).
pub fn view_format(view: u32, texture: u32, storage: Format) -> Result<(Format, bool)> {
    use Format::*;
    if is_typeless(view) {
        return Err(Error::InvalidArg("a typeless view format"));
    }
    if view != texture && typeless_group(view) != texture {
        return Err(Error::InvalidArg(
            "a view format outside the texture's own or its typeless group",
        ));
    }
    let aspect = match (storage, view) {
        (Depth32FloatStencil8, 20 | 21) => Some(false),
        (Depth32FloatStencil8, 22) => Some(true),
        (Depth32Float, 40 | 41) | (Depth16Unorm, 55 | 56) => Some(false),
        (Depth24PlusStencil8, 45 | 46) => Some(false),
        (Depth24PlusStencil8, 47) => Some(true),
        _ => None,
    };
    if let Some(stencil) = aspect {
        return Ok((storage, stencil));
    }
    let format = texture_format(view).map_or_else(|| unserved(view), Ok)?;
    Ok((format, false))
}

/// Whether a view of `format` of a texture stored as `storage` reads other bits than the storage
/// format's, which no back end's view does (`ReinterpretCopy`).
pub fn reinterprets(format: Format, storage: Format) -> bool {
    format.linear() != storage.linear()
}

/// The sRGB format of a linear one, or the linear one of an sRGB one, where the pair exists.
pub fn srgb_twin(dxgi: u32) -> Option<u32> {
    match dxgi {
        28 => Some(29),
        29 => Some(28),
        87 => Some(91),
        91 => Some(87),
        88 => Some(93),
        93 => Some(88),
        _ => None,
    }
}

/// Whether `dxgi` is the typeless format heading a group.
pub fn is_typeless(dxgi: u32) -> bool {
    matches!(
        dxgi,
        1 | 5
            | 9
            | 15
            | 19
            | 23
            | 27
            | 33
            | 39
            | 44
            | 48
            | 53
            | 60
            | 70
            | 73
            | 76
            | 79
            | 82
            | 90
            | 92
            | 94
            | 97
    )
}

/// The interface format a DXGI format is stored as, for the typed formats this front serves.
pub fn texture_format(dxgi: u32) -> Option<Format> {
    use Format::*;
    Some(match dxgi {
        2 => Rgba32Float,
        3 => Rgba32Uint,
        4 => Rgba32Sint,
        10 => Rgba16Float,
        11 => Rgba16Unorm,
        12 => Rgba16Uint,
        13 => Rgba16Snorm,
        14 => Rgba16Sint,
        16 => Rg32Float,
        17 => Rg32Uint,
        18 => Rg32Sint,
        20 => Depth32FloatStencil8,
        24 => Rgb10a2Unorm,
        25 => Rgb10a2Uint,
        26 => Rg11b10Float,
        28 => Rgba8Unorm,
        29 => Rgba8UnormSrgb,
        30 => Rgba8Uint,
        31 => Rgba8Snorm,
        32 => Rgba8Sint,
        34 => Rg16Float,
        35 => Rg16Unorm,
        36 => Rg16Uint,
        37 => Rg16Snorm,
        38 => Rg16Sint,
        40 => Depth32Float,
        41 => R32Float,
        42 => R32Uint,
        43 => R32Sint,
        45 => Depth24PlusStencil8,
        49 => Rg8Unorm,
        50 => Rg8Uint,
        51 => Rg8Snorm,
        52 => Rg8Sint,
        54 => R16Float,
        55 => Depth16Unorm,
        56 => R16Unorm,
        57 => R16Uint,
        58 => R16Snorm,
        59 => R16Sint,
        61 => R8Unorm,
        62 => R8Uint,
        63 => R8Snorm,
        64 => R8Sint,
        65 => A8Unorm,
        67 => Rgb9e5Float,
        71 => Bc1RgbaUnorm,
        72 => Bc1RgbaUnormSrgb,
        74 => Bc2RgbaUnorm,
        75 => Bc2RgbaUnormSrgb,
        77 => Bc3RgbaUnorm,
        78 => Bc3RgbaUnormSrgb,
        80 => Bc4RUnorm,
        81 => Bc4RSnorm,
        83 => Bc5RgUnorm,
        84 => Bc5RgSnorm,
        85 => B5g6r5Unorm,
        86 => Bgr5a1Unorm,
        87 => Bgra8Unorm,
        88 => Bgrx8Unorm,
        91 => Bgra8UnormSrgb,
        93 => Bgrx8UnormSrgb,
        95 => Bc6hRgbUfloat,
        96 => Bc6hRgbFloat,
        98 => Bc7RgbaUnorm,
        99 => Bc7RgbaUnormSrgb,
        115 => Bgra4Unorm,
        _ => return None,
    })
}

/// The typeless format heading `dxgi`'s group (formats whose texels are the same bits), or `dxgi`
/// itself for a format in no group.
pub fn typeless_group(dxgi: u32) -> u32 {
    match dxgi {
        1..=4 => 1,
        5..=8 => 5,
        9..=14 => 9,
        15..=18 => 15,
        19..=22 => 19,
        23..=25 => 23,
        27..=32 => 27,
        33..=38 => 33,
        39..=43 => 39,
        44..=47 => 44,
        48..=52 => 48,
        53..=59 => 53,
        60..=64 => 60,
        70..=72 => 70,
        73..=75 => 73,
        76..=78 => 76,
        79..=81 => 79,
        82..=84 => 82,
        87 | 90 | 91 => 90,
        88 | 92 | 93 => 92,
        94..=96 => 94,
        97..=99 => 97,
        f => f,
    }
}

/// Whether Direct3D copies between `a` and `b`, of different groups, as the same bits (Direct3D 10.1's
/// format conversions): a 32-bit single-channel format and a shared-exponent or packed 4:2:2 one, and an
/// uncompressed colour format of 64 or 128 bits and a block-compressed format of that block size.
pub fn copy_cast(a: u32, b: u32) -> bool {
    let one = |x: u32, y: u32| match x {
        // R32: R9G9B9E5, R8G8_B8G8 and G8R8_G8B8.
        39 | 41..=43 => matches!(y, 67..=69),
        // R16G16B16A16 and R32G32: BC1 and BC4.
        9..=18 => matches!(y, 70..=72 | 79..=81),
        // R32G32B32A32: BC2, BC3, BC5, BC6H and BC7.
        1..=4 => matches!(y, 73..=78 | 82..=84 | 94..=99),
        _ => false,
    };
    one(a, b) || one(b, a)
}

/// The interface vertex format of a DXGI format an input element may have.
pub fn vertex_format(dxgi: u32) -> Option<VertexFormat> {
    use VertexFormat::*;
    Some(match dxgi {
        2 => Float32x4,
        3 => Uint32x4,
        4 => Sint32x4,
        6 => Float32x3,
        7 => Uint32x3,
        8 => Sint32x3,
        10 => Float16x4,
        11 => Unorm16x4,
        12 => Uint16x4,
        13 => Snorm16x4,
        14 => Sint16x4,
        16 => Float32x2,
        17 => Uint32x2,
        18 => Sint32x2,
        24 => Unorm10_10_10_2,
        25 => Uint10_10_10_2,
        26 => Float11_11_10,
        28 => Unorm8x4,
        30 => Uint8x4,
        31 => Snorm8x4,
        32 => Sint8x4,
        34 => Float16x2,
        35 => Unorm16x2,
        36 => Uint16x2,
        37 => Snorm16x2,
        38 => Sint16x2,
        41 => Float32x1,
        42 => Uint32x1,
        43 => Sint32x1,
        49 => Unorm8x2,
        50 => Uint8x2,
        51 => Snorm8x2,
        52 => Sint8x2,
        54 => Float16x1,
        56 => Unorm16x1,
        57 => Uint16x1,
        58 => Snorm16x1,
        59 => Sint16x1,
        61 => Unorm8x1,
        62 => Uint8x1,
        63 => Snorm8x1,
        64 => Sint8x1,
        87 => Bgra8Unorm,
        _ => return None,
    })
}

/// Bytes one element of a vertex format takes.
pub fn vertex_size(format: VertexFormat) -> u32 {
    use VertexFormat::*;
    match format {
        Uint8x1 | Sint8x1 | Unorm8x1 | Snorm8x1 => 1,
        Uint8x2 | Sint8x2 | Unorm8x2 | Snorm8x2 | Uint16x1 | Sint16x1 | Unorm16x1 | Snorm16x1
        | Float16x1 => 2,
        Uint8x3 | Sint8x3 | Unorm8x3 | Snorm8x3 => 3,
        Uint8x4 | Sint8x4 | Unorm8x4 | Snorm8x4 | Uint16x2 | Sint16x2 | Unorm16x2 | Snorm16x2
        | Float16x2 | Float32x1 | Uint32x1 | Sint32x1 | Bgra8Unorm | Unorm10_10_10_2
        | Uint10_10_10_2 | Float11_11_10 => 4,
        Uint16x3 | Sint16x3 | Unorm16x3 | Snorm16x3 | Float16x3 => 6,
        Uint16x4 | Sint16x4 | Unorm16x4 | Snorm16x4 | Float16x4 | Float32x2 | Uint32x2
        | Sint32x2 => 8,
        Float32x3 | Uint32x3 | Sint32x3 => 12,
        Float32x4 | Uint32x4 | Sint32x4 => 16,
    }
}

/// The alignment of a vertex element's offset: the size of its components, at most 4 bytes.
pub fn vertex_alignment(format: VertexFormat) -> u32 {
    use VertexFormat::*;
    match format {
        Uint8x1 | Sint8x1 | Unorm8x1 | Snorm8x1 | Uint8x2 | Sint8x2 | Unorm8x2 | Snorm8x2
        | Uint8x3 | Sint8x3 | Unorm8x3 | Snorm8x3 | Uint8x4 | Sint8x4 | Unorm8x4 | Snorm8x4 => 1,
        Uint16x1 | Sint16x1 | Unorm16x1 | Snorm16x1 | Float16x1 | Uint16x2 | Sint16x2
        | Unorm16x2 | Snorm16x2 | Float16x2 | Uint16x3 | Sint16x3 | Unorm16x3 | Snorm16x3
        | Float16x3 | Uint16x4 | Sint16x4 | Unorm16x4 | Snorm16x4 | Float16x4 => 2,
        _ => 4,
    }
}

/// The index format of `IASetIndexBuffer`'s format (16 or 32-bit unsigned only, as Direct3D 11 allows).
pub fn index_format(dxgi: u32) -> Option<IndexFormat> {
    match dxgi {
        R16_UINT => Some(IndexFormat::U16),
        R32_UINT => Some(IndexFormat::U32),
        _ => None,
    }
}

/// Bytes a row of `width` texels of `format` takes, and how many rows of blocks `height` texels make.
pub fn row_bytes(format: Format, width: u32, height: u32) -> (u32, u32) {
    let (bw, bh, bytes) = format.block();
    (width.div_ceil(bw) * bytes, height.div_ceil(bh))
}

/// The components `format` has, a bit each from red, as a view reads them: a border colour is read
/// as a texel of the view's format (`HelperValue::TextureComponents`). Depth reads as red.
pub fn components(format: Format) -> u32 {
    use Format::*;
    match format {
        R8Unorm | R8Snorm | R8Uint | R8Sint | R16Unorm | R16Snorm | R16Uint | R16Sint
        | R16Float | R32Uint | R32Sint | R32Float | Bc4RUnorm | Bc4RSnorm | Stencil8
        | Depth16Unorm | Depth24Plus | Depth24PlusStencil8 | Depth32Float
        | Depth32FloatStencil8 => 0b0001,
        Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint | Rg16Unorm | Rg16Snorm | Rg16Uint | Rg16Sint
        | Rg16Float | Rg32Uint | Rg32Sint | Rg32Float | Bc5RgUnorm | Bc5RgSnorm => 0b0011,
        Bgrx8Unorm | Bgrx8UnormSrgb | Rg11b10Float | Rgb9e5Float | B5g6r5Unorm | Rgbg8Unorm
        | Grgb8Unorm | Rgb32Uint | Rgb32Sint | Rgb32Float | Bc6hRgbUfloat | Bc6hRgbFloat
        | Etc2Rgb8Unorm | Etc2Rgb8UnormSrgb => 0b0111,
        A8Unorm => 0b1000,
        _ => 0b1111,
    }
}
