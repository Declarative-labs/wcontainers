//! Hand-written WGSL standing in for known DXIL shaders until the shaders crate converts DXIL. A
//! shader is chosen by the BLAKE3 hash of its exact bytes; every use is counted and warned by the
//! device, and any other DXIL is refused at pipeline creation.

use wcontainers_gpu::ShaderStage;

/// One stand-in: the WGSL, its entry point and stage, and for a vertex shader the input semantics
/// (name and index) in the order of its locations.
#[derive(Debug)]
pub struct StandIn {
    pub hash: [u8; 32],
    pub wgsl: &'static str,
    pub entry: &'static str,
    pub stage: ShaderStage,
    pub inputs: &'static [(&'static str, u32)],
}

const TRIANGLE: &str = "
struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) color: vec3f,
}

@vertex
fn vs(@location(0) pos: vec2f, @location(1) color: vec3f) -> VsOut {
    return VsOut(vec4f(pos, 0.0, 1.0), color);
}

@fragment
fn ps(i: VsOut) -> @location(0) vec4f {
    return vec4f(i.color, 1.0);
}
";

/// The rig's `triangle.hlsl`, compiled by dxc as `vs_6_0 vs` and `ps_6_0 ps`.
pub const STAND_INS: &[StandIn] = &[
    StandIn {
        hash: hex("ad39e060e3e9edfc54849411b517576e5e0db5913d03ff3857109fb9926f0bd3"),
        wgsl: TRIANGLE,
        entry: "vs",
        stage: ShaderStage::Vertex,
        inputs: &[("POSITION", 0), ("COLOR", 0)],
    },
    StandIn {
        hash: hex("b6ed9d0d30ce5cc688bff39510ba1a2503cf8b90e8edb8b6cc5e4fb3b009c8a5"),
        wgsl: TRIANGLE,
        entry: "ps",
        stage: ShaderStage::Fragment,
        inputs: &[],
    },
];

/// The stand-in for `dxil`, if there is one.
pub fn find(dxil: &[u8]) -> Option<&'static StandIn> {
    let hash = *blake3::hash(dxil).as_bytes();
    STAND_INS.iter().find(|s| s.hash == hash)
}

const fn hex(s: &str) -> [u8; 32] {
    let b = s.as_bytes();
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = (nibble(b[2 * i]) << 4) | nibble(b[2 * i + 1]);
        i += 1;
    }
    out
}

const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => panic!("not a hex digit"),
    }
}
