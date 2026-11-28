//! Copies between formats of one typeless group whose texels `readPixels` reads wider than stored
//! (integers and 32-bit floats), or that are depth formats: a full-screen draw fetches each source
//! texel, recovers its stored bits and writes them as the destination format's values
//! ([`wcontainers_gpu::slow::SlowPath::CopyThroughBuffer`]), or as depth
//! ([`wcontainers_gpu::slow::SlowPath::DepthWrittenByDraw`]).

use wcontainers_gpu::format::Format;

/// How a colour format's components hold their bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unorm,
    /// Unorm whose colour components are sRGB-encoded; alpha is linear.
    Srgb,
    Snorm,
    Half,
    /// R11G11B10's unsigned floats: 6, 6 and 5 bits of mantissa, packed into one 32-bit word.
    SmallFloat,
    Single,
    Uint,
    Sint,
    /// Depth stored as unorm of the given bits, held in the low bits of an unsigned integer.
    Depth(u32),
    DepthFloat,
}

/// A format's kind and the width in bits of each of its four components.
fn bits(format: Format) -> Option<(Kind, [u32; 4])> {
    use Format as F;
    use Kind::*;
    let n = |kind, width| Some((kind, [width; 4]));
    match format {
        F::R8Unorm | F::Rg8Unorm | F::Rgba8Unorm | F::Bgra8Unorm => n(Unorm, 8),
        F::Rgba8UnormSrgb | F::Bgra8UnormSrgb => n(Srgb, u8::BITS),
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
        F::Rg11b10Float => Some((SmallFloat, [11, 11, 10, 0])),
        F::Depth16Unorm => n(Depth(16), 16),
        // A buffer's 24-bit depth is a 32-bit word.
        F::Depth24Plus | F::Depth24PlusStencil8 => n(Depth(Format::UNORM24_BITS), 32),
        F::Depth32Float | F::Depth32FloatStencil8 => n(DepthFloat, 32),
        _ => None,
    }
}

/// GLSL functions between sRGB-encoded and linear values (IEC 61966-2-1).
const SRGB_FUNCTIONS: &str = "highp vec3 srgb_encode(highp vec3 c) {
    return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, greaterThan(c, vec3(0.0031308)));
}
highp vec3 srgb_decode(highp vec3 c) {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), greaterThan(c, vec3(0.04045)));
}
";

/// The bits of an 8-bit sRGB texel `v`, sampled as its linear value.
const SRGB_BITS: &str = "uvec4(vec4(srgb_encode(v.rgb), v.a) * 255.0 + 0.5)";

/// The linear value an 8-bit sRGB target encodes as the bits `r`.
const SRGB_VALUE: &str = "vec4(srgb_decode(vec3(r.rgb) / 255.0), float(r.a) / 255.0)";

/// The GLSL vector of `w`'s values as unsigned integers.
fn uvec(w: [u32; 4], f: impl Fn(u32) -> u64) -> String {
    let c: Vec<String> = w.iter().map(|w| format!("{}u", f(*w))).collect();
    format!("uvec4({})", c.join(", "))
}

/// The uniform `unit` of a program writing the unorm depth of `format`, stored in b bits: 2^-b and
/// 2^-2b, whose products with a stored value v add up to v / (2^b - 1) rounded once for 16 bits (a
/// division would round twice on GPUs whose divide is a reciprocal and a product); 24-bit depth
/// takes the first alone ([`program`]). None for other formats.
pub(crate) fn depth_unit(format: Format) -> Option<[f32; 2]> {
    match bits(format)?.0 {
        Kind::Depth(b) => {
            let one = (1u64 << b) as f32;
            Some([1.0 / one, 1.0 / (one * one)])
        }
        _ => None,
    }
}

/// The fragment program copying `src` (sampled as `sampler`: "2D", "2DArray", "3D" or "Cube", where
/// `at.z` is the layer, slice or face) into a target
/// of `dst`, or None where a format's bits are not recovered this way (packed floats, formats
/// GL ES lacks) or `dst` cannot be drawn into. From a signed normalised format the bits are exact
/// only where it holds no smallest value (-128 samples as -127 does); into one, never (a draw writes
/// -127 for -1), so it is refused.
/// Into a depth format the program writes depth only, from the uniform `unit` where it is unorm. From
/// 24-bit depth there is none: no copy the interface allows draws from it (it has no colour partner),
/// so it is refused.
pub(crate) fn program(src: Format, dst: Format, sampler: &str) -> Option<String> {
    let (from, wf) = bits(src)?;
    if from == Kind::Depth(Format::UNORM24_BITS) {
        return None;
    }
    let (to, wt) = bits(dst)?;
    // R11G11B10 goes only into its 32-bit twin, as one word.
    let packed = from == Kind::SmallFloat && dst == Format::R32Uint;
    if (wf != wt && !packed) || to == Kind::Snorm || to == Kind::SmallFloat {
        return None;
    }
    let max = uvec(wf, |w| (1u64 << w) - 1);
    let prefix = match from {
        Kind::Uint => "u",
        Kind::Sint => "i",
        _ => "",
    };
    // GLSL ES 3.00 fetches no texel of a cube: a cube is sampled, nearest, towards the texel's centre,
    // at the one level the draw leaves it (its base and most level both the region's).
    let (fetch, towards) = match sampler {
        "2D" => ("texelFetch(src, q, 0)", ""),
        "Cube" => ("texture(src, towards(q))", crate::depth::TOWARDS),
        _ => ("texelFetch(src, ivec3(q, at.z), 0)", ""),
    };
    let srgb = if [from, to].contains(&Kind::Srgb) {
        SRGB_FUNCTIONS
    } else {
        Default::default()
    };
    let towards = [towards, srgb].concat();
    let half = |c: &str| format!("half_bits(v.{c})");
    let decode = match from {
        Kind::Unorm => format!("uvec4(v * vec4({max}) + 0.5)"),
        Kind::Srgb => SRGB_BITS.into(),
        // As the depth read back by draw is: rounded once (a sampled depth is at most 1, so the
        // product at most the largest word).
        Kind::Depth(b) => {
            let one = (1u64 << b) - 1;
            format!("uvec4(uint(round(v.x * {one}.0)))")
        }
        Kind::DepthFloat => "floatBitsToUint(v)".into(),
        Kind::Half => format!(
            "uvec4({}, {}, {}, {})",
            half("x"),
            half("y"),
            half("z"),
            half("w")
        ),
        Kind::Single => "floatBitsToUint(v)".into(),
        // One word, into a one-channel target.
        Kind::SmallFloat => "uvec4(small_bits(v.x, 6u) | small_bits(v.y, 6u) << 11u \
                             | small_bits(v.z, 5u) << 22u)"
            .into(),
        Kind::Uint => "v".into(),
        Kind::Sint => format!("uvec4(v) & {max}"),
        Kind::Snorm => format!(
            "uvec4(ivec4(round(v * {}.0))) & {max}",
            (1u64 << (wf[0] - 1)) - 1
        ),
    };
    let unhalf = |c: &str| format!("unpackHalf2x16(r.{c}).x");
    let shift = uvec(wt, |w| u64::from(32 - w));
    let (out, encode) = match to {
        Kind::Unorm | Kind::Srgb => (
            "vec4",
            match to {
                Kind::Srgb => SRGB_VALUE.into(),
                _ => format!("vec4(r) / vec4({max})"),
            },
        ),
        // v / (2^24 - 1) is v 2^-24 (exact) and a remainder of more than half and at most one of
        // that float's steps, so the nearest float is v 2^-24's bits plus one; for zero that is the
        // smallest denormal, which is stored as zero too. In integers, so that no compiler regroups
        // the sum below (Mesa makes v (2^-24 + 2^-48) of it, whose sum rounds to 2^-24, one step
        // low).
        Kind::Depth(Format::UNORM24_BITS) => (
            "",
            format!(
                "uintBitsToFloat(floatBitsToUint(float(r.x & {max}u) * unit.x) + 1u)",
                max = (1u64 << Format::UNORM24_BITS) - 1
            ),
        ),
        // 2^-16 + 2^-32 is a float, so no regrouping rounds it.
        Kind::Depth(b) => (
            "",
            format!(
                "float(r.x & {}u) * unit.x + float(r.x & {}u) * unit.y",
                (1u64 << b) - 1,
                (1u64 << b) - 1
            ),
        ),
        Kind::DepthFloat => ("", "uintBitsToFloat(r.x)".into()),
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
        Kind::Snorm | Kind::SmallFloat => unreachable!("refused above"),
    };
    Some(format!(
        "#version 300 es
precision highp float;
precision highp int;
uniform highp {prefix}sampler{sampler} src;
uniform ivec3 at;
{output}
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
// The bits of an unsigned float of `mb` mantissa bits and a 5-bit exponent sampled as `f`.
uint small_bits(float f, uint mb) {{
    uint u = floatBitsToUint(f);
    uint e = (u >> 23u) & 0xffu;
    uint m = u & 0x7fffffu;
    if (e == 0xffu) return (0x1fu << mb) | (m >> (23u - mb));
    if (e > 112u) return ((e - 112u) << mb) | (m >> (23u - mb));
    if (e < 113u - mb) return 0u;
    return (m | 0x800000u) >> (136u - mb - e);
}}
{towards}
void main() {{
    ivec2 q = ivec2(gl_FragCoord.xy) + at.xy;
    highp {prefix}vec4 v = {fetch};
    uvec4 r = {decode};
    {target} = {encode};
}}
",
        output = match out {
            "" => "uniform highp vec2 unit;".to_string(),
            out => format!("out highp {out} o;"),
        },
        target = if out.is_empty() { "gl_FragDepth" } else { "o" },
    ))
}
