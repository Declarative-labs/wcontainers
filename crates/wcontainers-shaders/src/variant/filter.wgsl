// Filtering and addressing from texel loads, by Direct3D's rules (functional specification 7.18), for
// textures the back end cannot filter and samplers whose border or address mode it lacks. One copy is
// made per texture dimension: `TEXTURE`, `AXES` (one for each axis the texture has), `TAPS` (two to the
// number of axes) and the function `load_at` are filled in for it. `size` is the size of level 0 (one
// past the texture's axes), which the caller asks of the texture.
//
// A sampler's state is `HelperValue::SamplerState` as three vectors: s0 = (min, mag, mip filter, address
// u), s1 = (address v, address w, anisotropy, smallest level of detail), s2.x = largest level of detail.

// The size of `level`.
fn filter_size(size: vec3<u32>, level: i32) -> vec3<i32> {
    return vec3<i32>(max(size >> vec3<u32>(u32(level)), vec3<u32>(1u)));
}

// The border colour as a texel of a format with `components` (a bit each from red): a component the
// format lacks reads its default.
fn filter_border(border: vec4<f32>, components: u32) -> vec4<f32> {
    let has = (vec4<u32>(components) >> vec4<u32>(0u, 1u, 2u, 3u)) & vec4<u32>(1u);
    return select(vec4<f32>(0.0, 0.0, 0.0, 1.0), border, has == vec4<u32>(1u));
}

// One axis's texel index under an address mode (`AddressMode`'s order), and whether it is inside the
// texture (zero only for a border outside).
fn filter_address(i: i32, n: i32, mode: u32) -> vec2<i32> {
    switch mode {
        case 0u: {
            return vec2<i32>(((i % n) + n) % n, 1);
        }
        case 1u: {
            let period = 2 * n;
            let m = ((i % period) + period) % period;
            return vec2<i32>(select(m, period - 1 - m, m >= n), 1);
        }
        case 3u: {
            return vec2<i32>(clamp(i, 0, n - 1), select(0, 1, i >= 0 && i < n));
        }
        case 4u: {
            return vec2<i32>(min(select(i, -1 - i, i < 0), n - 1), 1);
        }
        default: {
            return vec2<i32>(clamp(i, 0, n - 1), 1);
        }
    }
}

// The texel at `c` of a level `n` texels in size, addressed, or the border colour.
fn filter_texel(t: TEXTURE, c: vec3<i32>, n: vec3<i32>, layer: i32, level: i32, modes: vec3<u32>, border: vec4<f32>) -> vec4<f32> {
    let x = filter_address(c.x, n.x, modes.x);
    let y = filter_address(c.y, n.y, modes.y);
    let z = filter_address(c.z, n.z, modes.z);
    if (x.y & y.y & z.y) == 0 {
        return border;
    }
    return load_at(t, vec3<i32>(x.x, y.x, z.x), layer, level);
}

// Where the linear footprint around `coord` starts on a level `n` texels in size, in 256ths of a texel:
// the 8 bits of subtexel precision hardware filters with.
fn filter_fixed(coord: vec3<f32>, n: vec3<i32>) -> vec3<f32> {
    return floor((coord * vec3<f32>(n) - 0.5) * AXES * 256.0 + 0.5);
}

// One level read with point or linear filtering.
fn filter_level(t: TEXTURE, size: vec3<u32>, coord: vec3<f32>, offset: vec3<i32>, layer: i32, level: i32, linear: bool, modes: vec3<u32>, border: vec4<f32>) -> vec4<f32> {
    let n = filter_size(size, level);
    if !linear {
        let c = vec3<i32>(floor(coord * vec3<f32>(n))) + offset;
        return filter_texel(t, c, n, layer, level, modes, border);
    }
    let fixed = filter_fixed(coord, n);
    let whole = floor(fixed / 256.0);
    let first = vec3<i32>(whole) + offset;
    let f = (fixed - whole * 256.0) / 256.0;
    var sum = vec4<f32>(0.0);
    for (var k = 0u; k < TAPS; k++) {
        let o = vec3<u32>(k & 1u, (k >> 1u) & 1u, k >> 2u);
        let w = select(1.0 - f, f, o == vec3<u32>(1u));
        sum += w.x * w.y * w.z * filter_texel(t, first + vec3<i32>(o), n, layer, level, modes, border);
    }
    return sum;
}

// One level read with `count` taps spread evenly along `major` (anisotropic filtering), or one.
fn filter_taps(t: TEXTURE, size: vec3<u32>, coord: vec3<f32>, major: vec3<f32>, count: u32, offset: vec3<i32>, layer: i32, level: i32, linear: bool, modes: vec3<u32>, border: vec4<f32>) -> vec4<f32> {
    if count <= 1u {
        return filter_level(t, size, coord, offset, layer, level, linear, modes, border);
    }
    var sum = vec4<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        let along = (f32(i) + 0.5) / f32(count) - 0.5;
        sum += filter_level(t, size, coord + major * along, offset, layer, level, linear, modes, border);
    }
    return sum / f32(count);
}

// A sample: the level of detail from the gradients `dx` and `dy` plus `lod`, or `lod` itself when
// `given`; clamped to the sampler's range and to `min_lod` (a resource's clamp); magnification at
// zero or below; mip levels chosen and blended by the mip filter among `levels`.
fn filter_sample(t: TEXTURE, size: vec3<u32>, coord: vec3<f32>, dx: vec3<f32>, dy: vec3<f32>, given: bool, lod: f32, offset: vec3<i32>, layer: i32, s0: vec4<u32>, s1: vec4<u32>, s2: vec4<u32>, colour: vec4<f32>, components: u32, levels: u32, min_lod: f32) -> vec4<f32> {
    let modes = vec3<u32>(s0.w, s1.x, s1.y);
    let border = filter_border(colour, components);
    var at = lod;
    var count = 1u;
    var major = vec3<f32>(0.0);
    if !given {
        let texels = vec3<f32>(size) * AXES;
        let px = length(dx * texels);
        let py = length(dy * texels);
        let longest = max(px, py);
        let shortest = min(px, py);
        if s1.z > 1u && shortest > 0.0 {
            count = min(u32(ceil(longest / shortest)), s1.z);
            major = select(dy, dx, px >= py);
            at += log2(longest / f32(count));
        } else {
            at += log2(longest);
        }
    }
    at = max(min(max(at, bitcast<f32>(s1.w)), bitcast<f32>(s2.x)), min_lod);
    let magnified = at <= 0.0;
    let linear = select(s0.x, s0.y, magnified) == 1u;
    let last = f32(levels) - 1.0;
    if s0.z == 1u && !magnified {
        let l = clamp(at, 0.0, last);
        let below = floor(l);
        let f = floor((l - below) * 256.0 + 0.5) / 256.0;
        let near = filter_taps(t, size, coord, major, count, offset, layer, i32(below), linear, modes, border);
        if f == 0.0 {
            return near;
        }
        let far = filter_taps(t, size, coord, major, count, offset, layer, i32(min(below + 1.0, last)), linear, modes, border);
        return mix(near, far, f);
    }
    let level = select(clamp(floor(at + 0.5), 0.0, last), 0.0, magnified);
    return filter_taps(t, size, coord, major, count, offset, layer, i32(level), linear, modes, border);
}

// A gather of component `component` of the linear footprint at level 0, in Direct3D's order.
fn filter_gather(t: TEXTURE, size: vec3<u32>, coord: vec3<f32>, component: u32, offset: vec3<i32>, layer: i32, s0: vec4<u32>, s1: vec4<u32>, colour: vec4<f32>, components: u32) -> vec4<f32> {
    let modes = vec3<u32>(s0.w, s1.x, s1.y);
    let border = filter_border(colour, components);
    let n = vec3<i32>(size);
    let first = vec3<i32>(floor(filter_fixed(coord, n) / 256.0)) + offset;
    let a = filter_texel(t, first + vec3<i32>(0, 1, 0), n, layer, 0, modes, border);
    let b = filter_texel(t, first + vec3<i32>(1, 1, 0), n, layer, 0, modes, border);
    let c = filter_texel(t, first + vec3<i32>(1, 0, 0), n, layer, 0, modes, border);
    let d = filter_texel(t, first, n, layer, 0, modes, border);
    return vec4<f32>(a[component], b[component], c[component], d[component]);
}
