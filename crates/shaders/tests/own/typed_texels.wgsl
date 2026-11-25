// cs_texels (texels.hlsl) as another route hands it to wcontainers_shaders::typed: each typed buffer
// a storage buffer at its slot's binding, and stand-ins for the functions the program calls for it.

@group(0) @binding(32) var<storage, read> floats: array<u32>;
@group(0) @binding(33) var<storage, read> uints: array<u32>;
@group(0) @binding(34) var<storage, read> ints: array<u32>;
@group(0) @binding(35) var<storage, read> pairs: array<u32>;
@group(0) @binding(160) var<storage, read_write> written: array<u32>;
@group(0) @binding(161) var<storage, read_write> counts: array<u32>;
@group(0) @binding(162) var<storage, read_write> results: array<vec4<u32>>;

fn floats_load(index: u32) -> vec4<f32> { return vec4<f32>(); }
fn floats_length() -> u32 { return 0u; }
fn uints_load(index: u32) -> vec4<u32> { return vec4<u32>(); }
fn uints_length() -> u32 { return 0u; }
fn ints_load(index: u32) -> vec4<i32> { return vec4<i32>(); }
fn ints_length() -> u32 { return 0u; }
fn pairs_load(index: u32) -> vec4<f32> { return vec4<f32>(); }
fn pairs_length() -> u32 { return 0u; }
fn written_store(index: u32, value: vec4<f32>) {}
fn written_length() -> u32 { return 0u; }
fn counts_load(index: u32) -> vec4<u32> { return vec4<u32>(); }
fn counts_store(index: u32, value: vec4<u32>) {}
fn counts_length() -> u32 { return 0u; }
fn counts_add(index: u32, value: u32) -> u32 { return 0u; }
fn counts_max(index: u32, value: u32) -> u32 { return 0u; }
fn counts_or(index: u32, value: u32) -> u32 { return 0u; }

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    let n0 = floats_length();
    let n1 = uints_length();
    let n2 = ints_length();
    let n3 = pairs_length();
    let n4 = written_length();
    let n5 = counts_length();
    results[i * 6u + 0u] = bitcast<vec4<u32>>(floats_load(i));
    results[i * 6u + 1u] = uints_load(i);
    results[i * 6u + 2u] = bitcast<vec4<u32>>(ints_load(i));
    results[i * 6u + 3u] = vec4<u32>(bitcast<vec2<u32>>(pairs_load(i).xy), n0, n1);
    results[i * 6u + 4u] = vec4<u32>(n2, n3, n4, n5);

    // Values from tiny to past a half float's range, both signs, and infinities and a NaN (made at run
    // time, as WGSL refuses them as constants).
    let scale = bitcast<f32>((i % 40u + 103u) << 23u);
    var v = (f32(i) - 60.0) * vec4<f32>(0.013, 1.7, -0.29, 3.1) * scale;
    if i == 5u {
        v.x = bitcast<f32>(0x7f800000u + (i - 5u));
    }
    if i == 6u {
        v.y = bitcast<f32>(0xff800000u + (i - 6u));
    }
    if i == 7u {
        v.z = bitcast<f32>(0x7fc00000u + (i - 7u));
    }
    written_store(i, v);
    results[i * 6u + 5u] = bitcast<vec4<u32>>(v);

    _ = counts_add(i % 16u, i + 1u);
    _ = counts_max(16u + i % 8u, i * 2654435761u);
    _ = counts_or(24u + i % 8u, 1u << (i % 32u));
    if i >= 32u {
        counts_store(i, vec4<u32>(counts_load(i).x * 3u + 1u));
    }
}
