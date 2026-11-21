//! Texels: each format's bytes to values and back, by Direct3D's and WebGPU's conversion rules.
//! Colour values are four f64: floats for float and normalised formats, exact integers for integer
//! ones.

use wcontainers_gpu::Format;

/// How a format's channels are numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Num {
    Unorm,
    Snorm,
    Srgb,
    Uint,
    Sint,
    Float,
}

/// The channel layout of a colour format the recording back end holds: channel count, bytes per
/// channel, number type, and whether red and blue are swapped in memory.
fn layout(format: Format) -> Option<(usize, usize, Num, bool)> {
    use Format::*;
    use Num::*;
    Some(match format {
        R8Unorm => (1, 1, Unorm, false),
        R8Snorm => (1, 1, Snorm, false),
        R8Uint => (1, 1, Uint, false),
        R8Sint => (1, 1, Sint, false),
        Rg8Unorm => (2, 1, Unorm, false),
        Rg8Snorm => (2, 1, Snorm, false),
        Rg8Uint => (2, 1, Uint, false),
        Rg8Sint => (2, 1, Sint, false),
        Rgba8Unorm => (4, 1, Unorm, false),
        Rgba8UnormSrgb => (4, 1, Srgb, false),
        Rgba8Snorm => (4, 1, Snorm, false),
        Rgba8Uint => (4, 1, Uint, false),
        Rgba8Sint => (4, 1, Sint, false),
        Bgra8Unorm => (4, 1, Unorm, true),
        Bgra8UnormSrgb => (4, 1, Srgb, true),
        R16Unorm => (1, 2, Unorm, false),
        R16Snorm => (1, 2, Snorm, false),
        R16Uint => (1, 2, Uint, false),
        R16Sint => (1, 2, Sint, false),
        R16Float => (1, 2, Float, false),
        Rg16Unorm => (2, 2, Unorm, false),
        Rg16Snorm => (2, 2, Snorm, false),
        Rg16Uint => (2, 2, Uint, false),
        Rg16Sint => (2, 2, Sint, false),
        Rg16Float => (2, 2, Float, false),
        Rgba16Unorm => (4, 2, Unorm, false),
        Rgba16Snorm => (4, 2, Snorm, false),
        Rgba16Uint => (4, 2, Uint, false),
        Rgba16Sint => (4, 2, Sint, false),
        Rgba16Float => (4, 2, Float, false),
        R32Uint => (1, 4, Uint, false),
        R32Sint => (1, 4, Sint, false),
        R32Float => (1, 4, Float, false),
        Rg32Uint => (2, 4, Uint, false),
        Rg32Sint => (2, 4, Sint, false),
        Rg32Float => (2, 4, Float, false),
        Rgba32Uint => (4, 4, Uint, false),
        Rgba32Sint => (4, 4, Sint, false),
        Rgba32Float => (4, 4, Float, false),
        _ => return None,
    })
}

/// Whether the recording back end holds textures of `format`: the colour formats above, and depth and
/// stencil formats.
pub fn held(format: Format) -> bool {
    layout(format).is_some()
        || matches!(
            format,
            Format::Stencil8
                | Format::Depth16Unorm
                | Format::Depth24Plus
                | Format::Depth24PlusStencil8
                | Format::Depth32Float
                | Format::Depth32FloatStencil8
        )
}

/// Whether a colour format's values are clamped to zero to one (or minus one to one) when written,
/// so blending clamps its inputs too.
pub fn normalised(format: Format) -> bool {
    matches!(
        layout(format),
        Some((_, _, Num::Unorm | Num::Snorm | Num::Srgb, _))
    )
}

/// Whether a colour format holds integers, which are never blended.
pub fn integer(format: Format) -> bool {
    matches!(layout(format), Some((_, _, Num::Uint | Num::Sint, _)))
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Reads one colour texel. Missing channels read as 0, 0, 0, 1. sRGB formats convert red, green and
/// blue; alpha is linear.
pub fn decode(format: Format, bytes: &[u8]) -> [f64; 4] {
    let (n, size, num, swap) = layout(format).expect("decode of a format not held");
    let mut out = [0.0, 0.0, 0.0, 1.0];
    for (c, v) in out.iter_mut().enumerate().take(n) {
        let b = &bytes[c * size..c * size + size];
        let bits = match size {
            1 => u64::from(b[0]),
            2 => u64::from(u16::from_le_bytes([b[0], b[1]])),
            _ => u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        };
        let max = ((1u64 << (size * 8 - 1)) - 1) as f64;
        let signed = |bits: u64| -> i64 {
            let shift = 64 - size * 8;
            ((bits << shift) as i64) >> shift
        };
        *v = match num {
            Num::Unorm => bits as f64 / ((1u64 << (size * 8)) - 1) as f64,
            Num::Srgb if c == 3 => bits as f64 / 255.0,
            Num::Srgb => srgb_to_linear(bits as f64 / 255.0),
            Num::Snorm => (signed(bits) as f64 / max).max(-1.0),
            Num::Uint => bits as f64,
            Num::Sint => signed(bits) as f64,
            Num::Float if size == 2 => f64::from(half::f16::from_bits(bits as u16).to_f32()),
            Num::Float => f64::from(f32::from_bits(bits as u32)),
        };
    }
    if swap {
        out.swap(0, 2);
    }
    out
}

/// Writes one colour texel, converting as the APIs do: normalised values clamped and rounded to the
/// nearest step (ties to even), floats rounded to the format's precision, integers kept in range.
pub fn encode(format: Format, mut value: [f64; 4], bytes: &mut [u8]) {
    let (n, size, num, swap) = layout(format).expect("encode of a format not held");
    if swap {
        value.swap(0, 2);
    }
    let bits = (size * 8) as u32;
    for (c, &v) in value.iter().enumerate().take(n) {
        let word: u64 = match num {
            Num::Unorm => {
                let max = ((1u64 << bits) - 1) as f64;
                (v.clamp(0.0, 1.0) * max).round_ties_even() as u64
            }
            Num::Srgb if c == 3 => (v.clamp(0.0, 1.0) * 255.0).round_ties_even() as u64,
            Num::Srgb => (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round_ties_even() as u64,
            Num::Snorm => {
                let max = ((1u64 << (bits - 1)) - 1) as f64;
                ((v.clamp(-1.0, 1.0) * max).round_ties_even() as i64) as u64
            }
            Num::Uint => v.clamp(0.0, ((1u64 << bits) - 1) as f64) as u64,
            Num::Sint => {
                let half = (1i64 << (bits - 1)) as f64;
                (v.clamp(-half, half - 1.0) as i64) as u64
            }
            Num::Float if size == 2 => u64::from(half::f16::from_f64(v).to_bits()),
            Num::Float => u64::from((v as f32).to_bits()),
        };
        let b = &mut bytes[c * size..c * size + size];
        b.copy_from_slice(&word.to_le_bytes()[..size]);
    }
}

/// Reads the depth of a depth texel.
pub fn decode_depth(format: Format, bytes: &[u8]) -> f64 {
    let word =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    match format {
        Format::Depth16Unorm => f64::from(u16::from_le_bytes([bytes[0], bytes[1]])) / 65535.0,
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            f64::from(word(0) & 0xff_ffff) / 16_777_215.0
        }
        Format::Depth32Float | Format::Depth32FloatStencil8 => f64::from(f32::from_bits(word(0))),
        _ => 0.0,
    }
}

/// Writes the depth of a depth texel, leaving its stencil as it is.
pub fn encode_depth(format: Format, depth: f64, bytes: &mut [u8]) {
    let depth = depth.clamp(0.0, 1.0);
    match format {
        Format::Depth16Unorm => {
            let v = (depth * 65535.0).round_ties_even() as u16;
            bytes[..2].copy_from_slice(&v.to_le_bytes());
        }
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            let v = (depth * 16_777_215.0).round_ties_even() as u32;
            bytes[..3].copy_from_slice(&v.to_le_bytes()[..3]);
        }
        Format::Depth32Float | Format::Depth32FloatStencil8 => {
            bytes[..4].copy_from_slice(&(depth as f32).to_bits().to_le_bytes());
        }
        _ => {}
    }
}

/// Where a texel's stencil byte is, for formats with stencil.
pub fn stencil_byte(format: Format) -> Option<usize> {
    match format {
        Format::Stencil8 => Some(0),
        Format::Depth24PlusStencil8 => Some(3),
        Format::Depth32FloatStencil8 => Some(4),
        _ => None,
    }
}

/// The typeless group of a format: the formats whose texels are the same bits seen differently
/// (Direct3D's typeless groups, such as `R8G8B8A8_UNORM` and `R8G8B8A8_UINT`, or `R32_FLOAT` and
/// `D32_FLOAT`), between which copies move texels.
pub fn bits_group(format: Format) -> String {
    let name = format!("{format:?}");
    let name = match name.as_str() {
        "Depth32Float" => "R32Float",
        "Depth16Unorm" => "R16Unorm",
        n => n,
    };
    ["UnormSrgb", "Unorm", "Snorm", "Uint", "Sint", "Float"]
        .iter()
        .find_map(|s| name.strip_suffix(s))
        .unwrap_or(name)
        .into()
}
