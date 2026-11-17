//! DXGI formats (their numbers in `dxgiformat.h`) as the interface's formats.

use wcontainers_gpu::{Format, IndexFormat, VertexFormat};

/// `DXGI_FORMAT_UNKNOWN`.
pub const UNKNOWN: u32 = 0;
pub const R16_UINT: u32 = 57;
pub const R32_UINT: u32 = 42;

/// The interface format a DXGI format is stored as, for the formats this front serves. Typeless
/// formats are not served yet.
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
