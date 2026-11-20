//! The GLSL comparison of the corpus run on one compute program: llvmpipe runs it through OpenGL,
//! route B's module runs on lavapipe, on the same seeded uniforms and blocks, and every word of its
//! storage blocks agrees.
#![cfg(mesa)]

#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/compare.rs"]
mod compare;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/glsl.rs"]
mod glsl;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/interface.rs"]
mod interface;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/lavapipe.rs"]
mod lavapipe;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/llvmpipe.rs"]
mod llvmpipe;

use compare::{CompareOptions, DEFAULT_SEED, Verdict};
use wcontainers_nir::mesa::{Nir, Stage};

const PROGRAM: &str = "#version 430
layout(local_size_x = 8) in;
uniform float scale;
uniform ivec3 shift;
uniform mat2 turn;
uniform uint picks[3];
uniform bool negate;
struct Light { vec3 direction; float power; };
layout(std140) uniform Scene { Light lights[2]; mat3 frame; };
struct Sample { vec2 at; int count; };
layout(std430) buffer Samples { uint written; Sample samples[]; };
void main() {
    uint i = gl_GlobalInvocationID.x;
    Light l = lights[i % 2u];
    vec2 at = turn * (frame * l.direction).xy * scale * l.power;
    samples[i].at = negate ? -at : at;
    samples[i].count = shift[int(i % 3u)] + int(picks[i % 3u]);
    atomicAdd(written, 1u);
}
";

const SAMPLING: &str = "#version 430
layout(local_size_x = 8) in;
uniform sampler2D colours;
uniform usampler3D counts;
uniform samplerCube sky;
uniform sampler2DShadow depths;
uniform isampler2DArray layered;
layout(r32ui) uniform uimage2D marks;
layout(rgba32f) uniform image2D tints;
layout(std430) buffer Sampled { vec4 c[16]; uvec4 n[16]; vec4 s[16]; float d[16]; ivec4 l[16]; };
void main() {
    uint i = gl_GlobalInvocationID.x;
    vec2 at = vec2(float(i) / 8.0 + 0.0625, 0.4);
    c[i] = textureLod(colours, at, 0.0) + texelFetch(colours, ivec2(i % 4u, 1), 0);
    n[i] = texelFetch(counts, ivec3(i % 4u, 2, 3), 0);
    s[i] = textureLod(sky, vec3(1.0, at * 2.0 - 1.0), 0.0);
    d[i] = textureLod(depths, vec3(at, 0.5), 0.0);
    l[i] = texelFetch(layered, ivec3(i % 4u, 3, i % 2u), 0);
    ivec2 own = ivec2(i % 4u, i / 4u);
    imageStore(marks, own, uvec4(imageLoad(marks, own).x * 3u + i));
    imageStore(tints, own, imageLoad(tints, own) * c[i]);
}
";

const BITS: &str = "#version 430
layout(local_size_x = 8) in;
uniform uvec4 words;
uniform ivec4 signed_words;
uniform int width;
uniform vec4 values;
layout(std430) buffer Bits {
    uvec4 u[16]; ivec4 s[16]; uvec4 inserted[16]; ivec4 signs[16]; vec4 significands[16];
    ivec4 exponents[16];
};
void main() {
    uint i = gl_GlobalInvocationID.x;
    int offset = int(i % 8u) * 3;
    int bits = int(i % 5u) + abs(width);
    uvec4 mixed = words * 0x9e3779b9u + uvec4(i);
    u[i] = bitfieldExtract(mixed, offset, bits);
    s[i] = bitfieldExtract(ivec4(mixed), offset, bits);
    inserted[i] = bitfieldInsert(mixed, uvec4(i * 0x5bd1e995u), offset, bits);
    signs[i] = sign(signed_words - ivec4(int(i % 3u)));
    significands[i] = frexp(values * float(i * 977u + 1u), exponents[i]);
}
";

const TEST_FILE: &str = "[require]
GLSL >= 4.30

[compute shader]
#version 430
layout(local_size_x = 1) in;
void main() {}

[test]
compute 1 1 1
";

fn compute_verdict(source: &str) -> Verdict {
    let mut stage = Nir::from_glsl(&[(Stage::Compute, source)])
        .unwrap()
        .pop()
        .unwrap();
    let reflection = stage.nir.to_common_form(Stage::Compute).unwrap();
    let module = wcontainers_nir::direct::module(&mut stage.nir).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    let options = CompareOptions {
        seed: DEFAULT_SEED,
        input_sets: 1,
    };
    glsl::compare_compute(source, &stage.names, &module, &reflection, options)
}

#[test]
fn a_glsl_compute_program_matches_llvmpipe_on_route_b() {
    assert_eq!(compute_verdict(PROGRAM), Verdict::Matched);
}

#[test]
fn a_glsl_program_sampling_textures_and_storing_images_matches_llvmpipe_on_route_b() {
    assert_eq!(compute_verdict(SAMPLING), Verdict::Matched);
}

#[test]
fn a_glsl_program_of_bitfields_signs_and_exponents_matches_llvmpipe_on_route_b() {
    assert_eq!(compute_verdict(BITS), Verdict::Matched);
}

#[test]
fn a_piglit_file_gives_its_stages() {
    let sources = glsl::shader_test_sources(TEST_FILE).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].0, Stage::Compute);
    assert!(sources[0].1.starts_with("#version 430"));
    assert!(!sources[0].1.contains("compute 1 1 1"));
}
