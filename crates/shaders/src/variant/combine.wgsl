// A target combined with its old value in the program (`TargetInShader::combine`). The codes are the
// discriminants of the interface's blend factors, operations and logic operations, declared ahead of
// this text by targets.rs.

fn blend_factor(code: u32, s: vec4<f32>, s1: vec4<f32>, d: vec4<f32>, k: vec4<f32>) -> vec4<f32> {
    switch code {
        case FACTOR_ZERO: { return vec4<f32>(0.0); }
        case FACTOR_ONE: { return vec4<f32>(1.0); }
        case FACTOR_SRC: { return s; }
        case FACTOR_ONE_MINUS_SRC: { return 1.0 - s; }
        case FACTOR_SRC_ALPHA: { return vec4<f32>(s.a); }
        case FACTOR_ONE_MINUS_SRC_ALPHA: { return vec4<f32>(1.0 - s.a); }
        case FACTOR_DST: { return d; }
        case FACTOR_ONE_MINUS_DST: { return 1.0 - d; }
        case FACTOR_DST_ALPHA: { return vec4<f32>(d.a); }
        case FACTOR_ONE_MINUS_DST_ALPHA: { return vec4<f32>(1.0 - d.a); }
        case FACTOR_SRC_ALPHA_SATURATED: { return vec4<f32>(vec3<f32>(min(s.a, 1.0 - d.a)), 1.0); }
        case FACTOR_CONSTANT: { return k; }
        case FACTOR_ONE_MINUS_CONSTANT: { return 1.0 - k; }
        case FACTOR_CONSTANT_ALPHA: { return vec4<f32>(k.a); }
        case FACTOR_ONE_MINUS_CONSTANT_ALPHA: { return vec4<f32>(1.0 - k.a); }
        case FACTOR_SRC1: { return s1; }
        case FACTOR_ONE_MINUS_SRC1: { return 1.0 - s1; }
        case FACTOR_SRC1_ALPHA: { return vec4<f32>(s1.a); }
        case FACTOR_ONE_MINUS_SRC1_ALPHA: { return vec4<f32>(1.0 - s1.a); }
        default: { return vec4<f32>(0.0); }
    }
}

// Minimum and maximum take the values unweighted, as Direct3D's do.
fn blend_operation(code: u32, sf: vec4<f32>, df: vec4<f32>, s: vec4<f32>, d: vec4<f32>) -> vec4<f32> {
    switch code {
        case OP_SUBTRACT: { return sf - df; }
        case OP_REVERSE_SUBTRACT: { return df - sf; }
        case OP_MIN: { return min(s, d); }
        case OP_MAX: { return max(s, d); }
        default: { return sf + df; }
    }
}

// The source `s` blended with the old value `d` by the codes of each half (source factor, destination
// factor, operation): the color half gives red, green and blue, the alpha half alpha.
fn blended(s: vec4<f32>, s1: vec4<f32>, d: vec4<f32>, k: vec4<f32>, color: vec3<u32>, alpha: vec3<u32>) -> vec4<f32> {
    let c = blend_operation(color.z, s * blend_factor(color.x, s, s1, d, k), d * blend_factor(color.y, s, s1, d, k), s, d);
    let a = blend_operation(alpha.z, s * blend_factor(alpha.x, s, s1, d, k), d * blend_factor(alpha.y, s, s1, d, k), s, d);
    return vec4<f32>(c.rgb, a.a);
}

// The logic operation on each component's low bits, those below the `past` high bits it lacks,
// sign-extended from them where `signed`.
fn logic_operation(code: u32, s: vec4<u32>, d: vec4<u32>, past: vec4<u32>, signed: bool) -> vec4<u32> {
    var r: vec4<u32>;
    switch code {
        case LOGIC_CLEAR: { r = vec4<u32>(0u); }
        case LOGIC_AND: { r = s & d; }
        case LOGIC_AND_REVERSE: { r = s & ~d; }
        case LOGIC_AND_INVERTED: { r = ~s & d; }
        case LOGIC_NOOP: { r = d; }
        case LOGIC_XOR: { r = s ^ d; }
        case LOGIC_OR: { r = s | d; }
        case LOGIC_NOR: { r = ~(s | d); }
        case LOGIC_EQUIVALENT: { r = ~(s ^ d); }
        case LOGIC_INVERT: { r = ~d; }
        case LOGIC_OR_REVERSE: { r = s | ~d; }
        case LOGIC_COPY_INVERTED: { r = ~s; }
        case LOGIC_OR_INVERTED: { r = ~s | d; }
        case LOGIC_NAND: { r = ~(s & d); }
        case LOGIC_SET: { r = ~vec4<u32>(0u); }
        default: { r = s; }
    }
    if signed {
        return bitcast<vec4<u32>>(bitcast<vec4<i32>>(r << past) >> past);
    }
    return (r << past) >> past;
}

// Each component of `s` where the write mask `writes` has its channel's bit in `channels`, else `d`'s.
fn write_masked(s: vec4<u32>, d: vec4<u32>, writes: u32, channels: vec4<u32>) -> vec4<u32> {
    return select(d, s, (vec4<u32>(writes) & channels) != vec4<u32>(0u));
}
