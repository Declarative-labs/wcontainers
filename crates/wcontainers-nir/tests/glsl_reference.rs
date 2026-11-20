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

const TEST_FILE: &str = "[require]
GLSL >= 4.30

[compute shader]
#version 430
layout(local_size_x = 1) in;
void main() {}

[test]
compute 1 1 1
";

#[test]
fn a_glsl_compute_program_matches_llvmpipe_on_route_b() {
    let mut stage = Nir::from_glsl(&[(Stage::Compute, PROGRAM)])
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
    let verdict = glsl::compare_compute(PROGRAM, &stage.names, &module, &reflection, options);
    assert_eq!(verdict, Verdict::Matched);
}

#[test]
fn a_piglit_file_gives_its_stages() {
    let sources = glsl::shader_test_sources(TEST_FILE).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].0, Stage::Compute);
    assert!(sources[0].1.starts_with("#version 430"));
    assert!(!sources[0].1.contains("compute 1 1 1"));
}
