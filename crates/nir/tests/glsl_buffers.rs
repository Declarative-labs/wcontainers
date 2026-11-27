//! OpenGL's buffers as candidate B takes them (each block an array of words, read and written at
//! byte offsets), run: a GLSL compute program reading a std140 uniform block and writing a std430
//! storage block goes through Mesa's GLSL compiler, the common form and naga's SPIR-V writer, runs
//! on lavapipe, and is held to what GLSL's layout rules and arithmetic give, computed here.
#![cfg(all(mesa, feature = "standalone"))]

#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one compute program through it"
)]
#[path = "corpus/compare.rs"]
mod compare;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one compute program through it"
)]
#[path = "corpus/interface.rs"]
mod interface;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one compute program through it"
)]
#[path = "corpus/lavapipe.rs"]
mod lavapipe;

use ash::vk;
use lavapipe::{Descriptor, Element, Job, Lavapipe};
use wcontainers_gpu::reflect::SlotKind;
use wcontainers_nir::mesa::{DrawSettings, Stage};

/// One stage of a GLSL program as the driver receives it, owned by Mesa's compiler through our bridge.
type GlslStage = wcontainers_nir::mesa::GlslStage<wcontainers_mesa_compiler_direct::Bridge>;

const PROGRAM: &str = "#version 450
layout(local_size_x = 4) in;
layout(std140, binding = 3) uniform Inputs {
    vec3 direction;
    float scale;
    float weights[4];
    mat3 turn;
    ivec2 pair;
};
layout(std430, binding = 5) buffer Results {
    uint calls;
    float sums[];
};
void main() {
    uint i = gl_LocalInvocationID.x;
    vec3 turned = turn * direction;
    sums[i * 4u + 0u] = weights[i] * scale;
    sums[i * 4u + 1u] = turned[i % 3u];
    sums[i * 4u + 2u] = float(pair[i % 2u]);
    sums[i * 4u + 3u] = float(sums.length());
    atomicAdd(calls, 1u);
}
";

/// The invocations, and the values each writes into `sums`.
const INVOCATIONS: usize = 4;
const VALUES_EACH: usize = 4;

/// Where std140 puts each member of `Inputs` (GLSL's layout rules), and the block's size.
const DIRECTION: usize = 0;
const SCALE: usize = 12;
const WEIGHTS: usize = 16;
const ARRAY_STRIDE: usize = 16;
const TURN: usize = 80;
const COLUMN_STRIDE: usize = 16;
const PAIR: usize = 128;
const INPUTS_BYTES: usize = 144;

const DIRECTION_VALUE: [f32; 3] = [1.0, -2.0, 3.0];
const SCALE_VALUE: f32 = 0.5;
const WEIGHTS_VALUE: [f32; 4] = [2.0, -4.0, 6.5, 8.0];
/// Column by column.
const TURN_VALUE: [[f32; 3]; 3] = [[1.0, 0.0, 2.0], [0.0, 3.0, -1.0], [4.0, 1.0, 0.0]];
const PAIR_VALUE: [i32; 2] = [-7, 11];

fn put(bytes: &mut [u8], at: usize, value: [u8; 4]) {
    bytes[at..at + value.len()].copy_from_slice(&value);
}

fn inputs() -> Vec<u8> {
    let mut b = vec![0; INPUTS_BYTES];
    let word = size_of::<f32>();
    for (c, v) in DIRECTION_VALUE.iter().enumerate() {
        put(&mut b, DIRECTION + c * word, v.to_le_bytes());
    }
    put(&mut b, SCALE, SCALE_VALUE.to_le_bytes());
    for (i, v) in WEIGHTS_VALUE.iter().enumerate() {
        put(&mut b, WEIGHTS + i * ARRAY_STRIDE, v.to_le_bytes());
    }
    for (column, values) in TURN_VALUE.iter().enumerate() {
        for (row, v) in values.iter().enumerate() {
            put(
                &mut b,
                TURN + column * COLUMN_STRIDE + row * word,
                v.to_le_bytes(),
            );
        }
    }
    for (c, v) in PAIR_VALUE.iter().enumerate() {
        put(&mut b, PAIR + c * word, v.to_le_bytes());
    }
    b
}

/// What GLSL computes: `calls`, then `sums`.
fn expected() -> (u32, Vec<f32>) {
    let turned: Vec<f32> = (0..DIRECTION_VALUE.len())
        .map(|row| {
            TURN_VALUE
                .iter()
                .zip(DIRECTION_VALUE)
                .map(|(column, d)| column[row] * d)
                .sum()
        })
        .collect();
    let sums = (0..INVOCATIONS)
        .flat_map(|i| {
            [
                WEIGHTS_VALUE[i] * SCALE_VALUE,
                turned[i % turned.len()],
                PAIR_VALUE[i % PAIR_VALUE.len()] as f32,
                (INVOCATIONS * VALUES_EACH) as f32,
            ]
        })
        .collect();
    (INVOCATIONS as u32, sums)
}

#[test]
fn opengl_buffers_as_words_compute_what_glsl_says() {
    let mut stages = GlslStage::compile(&[(Stage::Compute, PROGRAM)]).expect("Mesa compiles it");
    let prepared = stages
        .pop()
        .expect("one stage")
        .received
        .prepare()
        .expect("the driver prepares it");
    let drawn = prepared
        .variant(&DrawSettings::default(), &[], &[])
        .expect("it binds")
        .flatten(0)
        .expect("it flattens");
    let reflection = drawn.bindings;
    let module = wcontainers_nir::direct::module(drawn.flat).expect("route B takes it");
    wcontainers_nir::spirv::validate(&module).expect("it validates");
    let entry = module.entry_points[0].name.clone();
    let words = compare::write_spirv(
        &module,
        naga::ShaderStage::Compute,
        &entry,
        naga::back::spv::WriterFlags::empty(),
    )
    .expect("naga writes the module");

    let word = size_of::<u32>();
    let results = vec![0; word + INVOCATIONS * VALUES_EACH * word];
    // Every uniform block is given the inputs (the default block, unused here, among them).
    let descriptors: Vec<Descriptor> = reflection
        .iter()
        .map(|r| {
            let (kind, bytes) = match r.kind {
                SlotKind::Uniform => (vk::DescriptorType::UNIFORM_BUFFER, inputs()),
                SlotKind::Storage => (vk::DescriptorType::STORAGE_BUFFER, results.clone()),
                other => panic!("the program has no {other:?}"),
            };
            Descriptor {
                set: r.set,
                binding: r.binding,
                kind,
                elements: vec![Element::Buffer(bytes)],
            }
        })
        .collect();
    let lavapipe = Lavapipe::open().expect("lavapipe");
    let out = lavapipe
        .run(&Job {
            stage: lavapipe::Stage::Compute,
            words: &words,
            entry: &entry,
            descriptors: &descriptors,
            immediates: &[],
            groups: [1, 1, 1],
            vertices: 0,
            attributes: &[],
            feeder: None,
            capture_stride: 0,
            targets: &[],
            depth: false,
            clear: [0; 4],
        })
        .expect("the module runs");
    let storage = &out.storage[0];
    let calls = u32::from_le_bytes(storage[..word].try_into().expect("a word"));
    let sums: Vec<f32> = storage[word..]
        .chunks_exact(word)
        .map(|w| f32::from_le_bytes(w.try_into().expect("a word")))
        .collect();
    assert_eq!((calls, sums), expected());
}
