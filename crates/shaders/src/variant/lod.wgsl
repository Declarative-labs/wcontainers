// Direct3D's level of detail of an implicit sample (`CalculateLevelOfDetail` and its unclamped form)
// from the coordinate's derivatives `dx` and `dy` in a texture of `size` texels (axes the texture
// lacks are zero), through a sampler of state `s0`, `s1` and `s2` (the helper block's
// `SamplerState`: the mip filter, anisotropy and the level range) and level-of-detail bias `bias`: x
// clamped to the sampler's level range, the resource's clamp `min_lod` and the view's `levels`, and
// the level a point mip filter picks; y unclamped.
fn level_of_detail(size: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>, levels: u32, s0: vec4<u32>, s1: vec4<u32>, s2: vec4<u32>, bias: f32, min_lod: f32) -> vec2<f32> {
    let px = length(dx * size);
    let py = length(dy * size);
    let longest = max(px, py);
    let shortest = min(px, py);
    var lod = log2(longest);
    if s1.z > 1u && shortest > 0.0 {
        lod = log2(longest / min(longest / shortest, f32(s1.z)));
    }
    let unclamped = lod + bias;
    let ranged = max(min(max(unclamped, bitcast<f32>(s1.w)), bitcast<f32>(s2.x)), min_lod);
    var clamped = clamp(ranged, 0.0, f32(levels) - 1.0);
    // The nearest level, from the level of detail at Direct3D's 8 fraction bits.
    if s0.z != 1u {
        clamped = floor(floor(clamped * 256.0 + 0.5) / 256.0 + 0.5);
    }
    return vec2<f32>(clamped, unclamped);
}

// The gradients a sample with Direct3D's per-sample clamp `clamp` takes, so that the device's own
// level of detail (adding the sampler's bias `bias` itself) is the larger of the sample's and the
// clamp: the coordinate's derivatives or the sample's gradients `dx` and `dy` in a texture of `size`
// texels through a sampler of anisotropy `s1.z` (as `level_of_detail`), scaled by two to the sample's
// own bias `shift` and to the distance from its level of detail up to the clamp where that is below.
// Where they are too short to scale (zero), one texel of the clamp's level along x and y.
fn clamped_gradients(size: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>, s1: vec4<u32>, bias: f32, shift: f32, clamp: f32) -> mat2x3<f32> {
    let px = length(dx * size);
    let py = length(dy * size);
    let longest = max(px, py);
    let shortest = min(px, py);
    var lod = log2(longest);
    if s1.z > 1u && shortest > 0.0 {
        lod = log2(longest / min(longest / shortest, f32(s1.z)));
    }
    let up = max(clamp - (lod + bias + shift), 0.0);
    // Two to this power still scales a nonzero gradient within the floats.
    if up <= 64.0 {
        let scale = exp2(shift + up);
        return mat2x3<f32>(dx * scale, dy * scale);
    }
    let texel = select(vec3<f32>(0.0), exp2(clamp - bias) / size, size > vec3<f32>(0.0));
    return mat2x3<f32>(vec3<f32>(texel.x, 0.0, 0.0), vec3<f32>(0.0, texel.y, 0.0));
}
