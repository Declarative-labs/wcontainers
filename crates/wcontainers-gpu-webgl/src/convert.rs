//! Copies between formats of one typeless group whose texels `readPixels` reads wider than stored
//! (integers and 32-bit floats): a full-screen draw fetches each source texel, recovers its stored
//! bits and writes them as the destination format's values
//! ([`wcontainers_gpu::SlowPath::CopyThroughBuffer`]).

use wcontainers_gpu::Format;

/// How a colour format's components hold their bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unorm,
    Snorm,
    Half,
    Single,
    Uint,
    Sint,
}

/// A format's kind and the width in bits of each of its four components.
fn bits(format: Format) -> Option<(Kind, [u32; 4])> {
    use Format as F;
    use Kind::*;
    let n = |kind, width| Some((kind, [width; 4]));
    match format {
        F::R8Unorm | F::Rg8Unorm | F::Rgba8Unorm => n(Unorm, 8),
        F::R8Snorm | F::Rg8Snorm | F::Rgba8Snorm => n(Snorm, 8),
        F::R8Uint | F::Rg8Uint | F::Rgba8Uint => n(Uint, 8),
        F::R8Sint | F::Rg8Sint | F::Rgba8Sint => n(Sint, 8),
        F::R16Uint | F::Rg16Uint | F::Rgba16Uint => n(Uint, 16),
        F::R16Sint | F::Rg16Sint | F::Rgba16Sint => n(Sint, 16),
        F::R16Float | F::Rg16Float | F::Rgba16Float => n(Half, 16),
        F::R16Unorm | F::Rg16Unorm | F::Rgba16Unorm => n(Unorm, 16),
        F::R16Snorm | F::Rg16Snorm | F::Rgba16Snorm => n(Snorm, 16),
        F::R32Uint | F::Rg32Uint | F::Rgba32Uint => n(Uint, 32),
        F::R32Sint | F::Rg32Sint | F::Rgba32Sint => n(Sint, 32),
        F::R32Float | F::Rg32Float | F::Rgba32Float => n(Single, 32),
        F::Rgb10a2Unorm => Some((Unorm, [10, 10, 10, 2])),
        F::Rgb10a2Uint => Some((Uint, [10, 10, 10, 2])),
        _ => None,
    }
}

/// The GLSL vector of `w`'s values as unsigned integers.
fn uvec(w: [u32; 4], f: impl Fn(u32) -> u64) -> String {
    let c: Vec<String> = w.iter().map(|w| format!("{}u", f(*w))).collect();
    format!("uvec4({})", c.join(", "))
}

/// The fragment program copying `src` (sampled as `sampler`: "2D", "2DArray" or "3D") into a target
/// of `dst`, or None where a format's bits are not recovered this way (sRGB, packed floats, formats
/// GL ES lacks, signed normalised ones, whose -128 samples as -127 does) or `dst` cannot be drawn into.
pub fn program(src: Format, dst: Format, sampler: &str) -> Option<String> {
    let (from, wf) = bits(src)?;
    let (to, wt) = bits(dst)?;
    if wf != wt || from == Kind::Snorm || to == Kind::Snorm {
        return None;
    }
    let max = uvec(wf, |w| (1u64 << w) - 1);
    let prefix = match from {
        Kind::Uint => "u",
        Kind::Sint => "i",
        _ => "",
    };
    let coord = if sampler == "2D" {
        "q"
    } else {
        "ivec3(q, at.z)"
    };
    let half = |c: &str| format!("half_bits(v.{c})");
    let decode = match from {
        Kind::Unorm => format!("uvec4(v * vec4({max}) + 0.5)"),
        Kind::Half => format!(
            "uvec4({}, {}, {}, {})",
            half("x"),
            half("y"),
            half("z"),
            half("w")
        ),
        Kind::Single => "floatBitsToUint(v)".into(),
        Kind::Uint => "v".into(),
        Kind::Sint => format!("uvec4(v) & {max}"),
        Kind::Snorm => unreachable!("refused above"),
    };
    let unhalf = |c: &str| format!("unpackHalf2x16(r.{c}).x");
    let shift = uvec(wt, |w| u64::from(32 - w));
    let (out, encode) = match to {
        Kind::Unorm => ("vec4", format!("vec4(r) / vec4({max})")),
        Kind::Half => (
            "vec4",
            format!(
                "vec4({}, {}, {}, {})",
                unhalf("x"),
                unhalf("y"),
                unhalf("z"),
                unhalf("w")
            ),
        ),
        Kind::Single => ("vec4", "uintBitsToFloat(r)".into()),
        Kind::Uint => ("uvec4", "r".into()),
        Kind::Sint => ("ivec4", format!("ivec4(r << {shift}) >> ivec4({shift})")),
        Kind::Snorm => unreachable!("refused above"),
    };
    Some(format!(
        "#version 300 es
precision highp float;
precision highp int;
uniform highp {prefix}sampler{sampler} src;
uniform ivec3 at;
out highp {out} o;
// The bits of a half float sampled as `f`, NaN payloads kept (packHalf2x16 may make them canonical).
uint half_bits(float f) {{
    uint u = floatBitsToUint(f);
    uint sign = (u >> 16u) & 0x8000u;
    uint e = (u >> 23u) & 0xffu;
    uint m = u & 0x7fffffu;
    if (e == 0xffu) return sign | 0x7c00u | (m >> 13u);
    if (e > 112u) return sign | ((e - 112u) << 10u) | (m >> 13u);
    if (e < 103u) return sign;
    return sign | ((m | 0x800000u) >> (126u - e));
}}
void main() {{
    ivec2 q = ivec2(gl_FragCoord.xy) + at.xy;
    highp {prefix}vec4 v = texelFetch(src, {coord}, 0);
    uvec4 r = {decode};
    o = {encode};
}}
"
    ))
}
