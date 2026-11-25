//! Read-back rows of a DXGI format turned into a lossless picture: 8-bit formats into 8-bit RGBA,
//! 16-bit and packed integer formats into 16-bit RGBA holding each channel's raw bits, float, depth
//! and 32-bit integer formats (whose values must be at most 2^24) into float RGBA. Missing channels read 0, missing alpha the maximum.

use anyhow::{Result, bail};
use image::{DynamicImage, ImageBuffer, Rgba};
use windows::Win32::Graphics::Dxgi::Common::*;

/// Bytes per texel of the formats `picture` reads.
pub fn texel_size(format: DXGI_FORMAT) -> Result<usize> {
    Ok(match kind(format)? {
        Kind::Eight(n) => n,
        Kind::Sixteen(n) | Kind::Half(n) => 2 * n,
        Kind::Float(n) | Kind::Uint(n) | Kind::Sint(n) => 4 * n,
        Kind::Bgra8 | Kind::Rgb10a2 | Kind::Rg11b10 | Kind::Depth24 => 4,
        Kind::B5g6r5 => 2,
        Kind::Depth32s8 => 8,
    })
}

enum Kind {
    /// n 8-bit channels.
    Eight(usize),
    Bgra8,
    /// n 16-bit integer channels, raw bits.
    Sixteen(usize),
    Half(usize),
    Float(usize),
    Uint(usize),
    Sint(usize),
    Rgb10a2,
    Rg11b10,
    B5g6r5,
    /// 24-bit normalised depth and 8-bit stencil: depth in red, stencil in green.
    Depth24,
    Depth32s8,
}

fn kind(format: DXGI_FORMAT) -> Result<Kind> {
    Ok(match format {
        DXGI_FORMAT_R8G8B8A8_UNORM
        | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
        | DXGI_FORMAT_R8G8B8A8_UINT
        | DXGI_FORMAT_R8G8B8A8_SNORM
        | DXGI_FORMAT_R8G8B8A8_SINT
        | DXGI_FORMAT_R8G8B8A8_TYPELESS => Kind::Eight(4),
        DXGI_FORMAT_R8G8_UNORM
        | DXGI_FORMAT_R8G8_UINT
        | DXGI_FORMAT_R8G8_SNORM
        | DXGI_FORMAT_R8G8_SINT => Kind::Eight(2),
        DXGI_FORMAT_R8_UNORM
        | DXGI_FORMAT_R8_UINT
        | DXGI_FORMAT_R8_SNORM
        | DXGI_FORMAT_R8_SINT
        | DXGI_FORMAT_A8_UNORM
        | DXGI_FORMAT_R8_TYPELESS => Kind::Eight(1),
        DXGI_FORMAT_B8G8R8A8_UNORM
        | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        | DXGI_FORMAT_B8G8R8X8_UNORM => Kind::Bgra8,
        DXGI_FORMAT_R16G16B16A16_UNORM
        | DXGI_FORMAT_R16G16B16A16_UINT
        | DXGI_FORMAT_R16G16B16A16_SNORM
        | DXGI_FORMAT_R16G16B16A16_SINT => Kind::Sixteen(4),
        DXGI_FORMAT_R16G16_UNORM
        | DXGI_FORMAT_R16G16_UINT
        | DXGI_FORMAT_R16G16_SNORM
        | DXGI_FORMAT_R16G16_SINT => Kind::Sixteen(2),
        DXGI_FORMAT_R16_UNORM
        | DXGI_FORMAT_R16_UINT
        | DXGI_FORMAT_R16_SNORM
        | DXGI_FORMAT_R16_SINT
        | DXGI_FORMAT_D16_UNORM
        | DXGI_FORMAT_R16_TYPELESS => Kind::Sixteen(1),
        DXGI_FORMAT_R16G16B16A16_FLOAT => Kind::Half(4),
        DXGI_FORMAT_R16G16_FLOAT => Kind::Half(2),
        DXGI_FORMAT_R16_FLOAT => Kind::Half(1),
        DXGI_FORMAT_R32G32B32A32_FLOAT => Kind::Float(4),
        DXGI_FORMAT_R32G32B32_FLOAT => Kind::Float(3),
        DXGI_FORMAT_R32G32_FLOAT => Kind::Float(2),
        DXGI_FORMAT_R32_FLOAT | DXGI_FORMAT_D32_FLOAT | DXGI_FORMAT_R32_TYPELESS => Kind::Float(1),
        DXGI_FORMAT_R32G32B32A32_UINT => Kind::Uint(4),
        DXGI_FORMAT_R32G32_UINT => Kind::Uint(2),
        DXGI_FORMAT_R32_UINT => Kind::Uint(1),
        DXGI_FORMAT_R32G32B32A32_SINT => Kind::Sint(4),
        DXGI_FORMAT_R32G32_SINT => Kind::Sint(2),
        DXGI_FORMAT_R32_SINT => Kind::Sint(1),
        DXGI_FORMAT_R10G10B10A2_UNORM | DXGI_FORMAT_R10G10B10A2_UINT => Kind::Rgb10a2,
        DXGI_FORMAT_R11G11B10_FLOAT => Kind::Rg11b10,
        DXGI_FORMAT_B5G6R5_UNORM => Kind::B5g6r5,
        DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_R24G8_TYPELESS => Kind::Depth24,
        DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_R32G8X24_TYPELESS => Kind::Depth32s8,
        other => bail!("reading back format {} is not supported", other.0),
    })
}

/// The picture of `height` rows of `width` texels of `format`, rows `pitch` bytes apart.
pub fn picture(
    format: DXGI_FORMAT,
    width: u32,
    height: u32,
    rows: &[u8],
    pitch: usize,
) -> Result<DynamicImage> {
    let kind = kind(format)?;
    let size = texel_size(format)?;
    let texels = (0..height as usize).flat_map(|y| {
        let row = &rows[y * pitch..y * pitch + width as usize * size];
        row.chunks_exact(size)
    });
    let le16 = |t: &[u8], i: usize| u16::from_le_bytes([t[2 * i], t[2 * i + 1]]);
    let le32 = |t: &[u8], i: usize| u32::from_le_bytes(t[4 * i..4 * i + 4].try_into().unwrap());
    let fill = |n: usize, values: &mut dyn FnMut(usize) -> f32, one: f32| -> [f32; 4] {
        std::array::from_fn(|c| {
            if c < n {
                values(c)
            } else if c == 3 {
                one
            } else {
                0.0
            }
        })
    };
    Ok(match kind {
        Kind::Eight(n) => {
            let data = texels
                .flat_map(|t| {
                    std::array::from_fn::<u8, 4, _>(|c| {
                        if c < n {
                            t[c]
                        } else if c == 3 {
                            255
                        } else {
                            0
                        }
                    })
                })
                .collect();
            DynamicImage::ImageRgba8(ImageBuffer::from_raw(width, height, data).unwrap())
        }
        Kind::Bgra8 => {
            let alpha = format != DXGI_FORMAT_B8G8R8X8_UNORM;
            let data = texels
                .flat_map(|t| [t[2], t[1], t[0], if alpha { t[3] } else { 255 }])
                .collect();
            DynamicImage::ImageRgba8(ImageBuffer::from_raw(width, height, data).unwrap())
        }
        Kind::Sixteen(n) => {
            let data = texels
                .flat_map(|t| {
                    std::array::from_fn::<u16, 4, _>(|c| {
                        if c < n {
                            le16(t, c)
                        } else if c == 3 {
                            u16::MAX
                        } else {
                            0
                        }
                    })
                })
                .collect();
            DynamicImage::ImageRgba16(ImageBuffer::from_raw(width, height, data).unwrap())
        }
        Kind::Rgb10a2 => {
            let data = texels
                .flat_map(|t| {
                    let v = le32(t, 0);
                    [v & 0x3ff, (v >> 10) & 0x3ff, (v >> 20) & 0x3ff, v >> 30].map(|c| c as u16)
                })
                .collect();
            DynamicImage::ImageRgba16(ImageBuffer::from_raw(width, height, data).unwrap())
        }
        Kind::B5g6r5 => {
            let data = texels
                .flat_map(|t| {
                    let v = le16(t, 0);
                    [v >> 11, (v >> 5) & 0x3f, v & 0x1f, u16::MAX]
                })
                .collect();
            DynamicImage::ImageRgba16(ImageBuffer::from_raw(width, height, data).unwrap())
        }
        _ => {
            let mut data = Vec::with_capacity(width as usize * height as usize * 4);
            for t in texels {
                if let Kind::Uint(n) | Kind::Sint(n) = kind {
                    for c in 0..n {
                        let v = le32(t, c);
                        let v = if let Kind::Sint(_) = kind {
                            (v as i32).unsigned_abs()
                        } else {
                            v
                        };
                        if v > 1 << 24 {
                            bail!(
                                "a 32-bit integer texel holds {v:#x}, more than a float picture holds exactly"
                            );
                        }
                    }
                }
                data.extend(match kind {
                    Kind::Half(n) => {
                        fill(n, &mut |c| half::f16::from_bits(le16(t, c)).to_f32(), 1.0)
                    }
                    Kind::Float(n) => fill(n, &mut |c| f32::from_bits(le32(t, c)), 1.0),
                    Kind::Uint(n) => fill(n, &mut |c| le32(t, c) as f32, 1.0),
                    Kind::Sint(n) => fill(n, &mut |c| le32(t, c) as i32 as f32, 1.0),
                    Kind::Rg11b10 => {
                        let v = le32(t, 0);
                        [
                            small_float(v & 0x7ff, 6),
                            small_float((v >> 11) & 0x7ff, 6),
                            small_float(v >> 22, 5),
                            1.0,
                        ]
                    }
                    Kind::Depth24 => {
                        let v = le32(t, 0);
                        [
                            (v & 0xff_ffff) as f32 / 16_777_215.0,
                            (v >> 24) as f32,
                            0.0,
                            1.0,
                        ]
                    }
                    Kind::Depth32s8 => [
                        f32::from_bits(le32(t, 0)),
                        (le32(t, 1) & 0xff) as f32,
                        0.0,
                        1.0,
                    ],
                    _ => unreachable!(),
                });
            }
            DynamicImage::ImageRgba32F(
                ImageBuffer::<Rgba<f32>, _>::from_raw(width, height, data).unwrap(),
            )
        }
    })
}

/// An unsigned float of the packed R11G11B10 format: 5 exponent bits and `mantissa` mantissa bits.
fn small_float(bits: u32, mantissa: u32) -> f32 {
    let exponent = bits >> mantissa;
    let fraction = (bits & ((1 << mantissa) - 1)) as f32 / (1 << mantissa) as f32;
    match exponent {
        0 => fraction * 2f32.powi(-14),
        31 if fraction == 0.0 => f32::INFINITY,
        31 => f32::NAN,
        e => (1.0 + fraction) * 2f32.powi(e as i32 - 15),
    }
}
