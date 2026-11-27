//! The tests every implementation of wcontainers-mesa-compiler runs, written against the trait alone: programs
//! compiled from GLSL and SPIR-V and read back in every form the trait gives; every method called, by a hostile
//! caller, on shaders of every stage the trait can make and with edge arguments (values past every enumeration,
//! empty and long lists, units past every limit); and a seeded fuzzer of the same, with SPIR-V words changed.

use std::ffi::CStr;

use wcontainers_mesa_compiler::{MesaCompiler, NearestSampler, PositionTransform, SlotRange, StreamOutput, TextureSwizzle};

/// The first word of every SPIR-V module.
const SPIRV_MAGIC: u32 = 0x0723_0203;
/// The bridge's stages (enum wc_stage).
const VERTEX: u32 = 0;
const FRAGMENT: u32 = 1;
const COMPUTE: u32 = 2;
/// The entry point every program here names.
const MAIN: &CStr = c"main";

/// A vertex program with a uniform block, a point size, two inputs and an output.
const VERTEX_SOURCE: &CStr = c"#version 450
layout(location = 0) in vec4 at;
layout(location = 1) in vec2 coordinate;
layout(location = 0) out vec2 passed;
layout(std140, binding = 0) uniform Block { mat4 transform; float size; };
void main() {
    gl_Position = transform * at;
    gl_PointSize = size;
    passed = coordinate;
}
";

/// A fragment program sampling a float, an integer, a 1D and a cube texture, reading sample positions,
/// interpolating at an offset and writing the sample mask.
const FRAGMENT_SOURCE: &CStr = c"#version 450
layout(location = 0) in vec2 passed;
layout(location = 0) out vec4 colour;
layout(binding = 0) uniform sampler2D picture;
layout(binding = 1) uniform isampler2D counts;
layout(binding = 2) uniform sampler1D line;
layout(binding = 3) uniform samplerCube sky;
void main() {
    vec2 moved = interpolateAtOffset(passed, vec2(0.25));
    ivec4 count = texture(counts, passed);
    colour = texture(picture, moved) + vec4(count) + texture(line, passed.x) + texture(sky, vec3(passed, 1.0));
    colour.x += gl_SamplePosition.x;
    gl_SampleMask[0] = 1;
}
";

/// A compute program writing a storage block and an image.
const COMPUTE_SOURCE: &CStr = c"#version 450
layout(local_size_x = 4) in;
layout(std430, binding = 0) buffer Results { uint values[]; };
layout(binding = 0, rgba8) uniform writeonly image2D written;
void main() {
    values[gl_LocalInvocationID.x] = gl_LocalInvocationID.x;
    imageStore(written, ivec2(gl_LocalInvocationID.xy), vec4(1.0));
}
";

/// An older program: the vertex built-ins and gl_FragColor (written to every draw buffer).
const OLD_VERTEX_SOURCE: &CStr = c"#version 110
void main() { gl_Position = gl_Vertex; gl_PointSize = 4.0; }
";
const OLD_FRAGMENT_SOURCE: &CStr = c"#version 110
void main() { gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0); }
";

/// A source the compiler refuses.
const BROKEN_SOURCE: &CStr = c"#version 450
void main() { this is not GLSL }
";

/// The three programs above as SPIR-V for Vulkan, written by glslang 16.4.0 (`glslangValidator -V --target-env
/// vulkan1.1`, the files beside each in spirv/).
const VERTEX_SPIRV: &[u8] = include_bytes!("spirv/vertex.spv");
const FRAGMENT_SPIRV: &[u8] = include_bytes!("spirv/fragment.spv");
const COMPUTE_SPIRV: &[u8] = include_bytes!("spirv/compute.spv");

/// A SPIR-V file's words (little-endian, as glslang writes them).
fn words(bytes: &[u8]) -> Vec<u32> {
    bytes.as_chunks::<4>().0.iter().map(|word| u32::from_le_bytes(*word)).collect()
}

/// The modules by stage.
fn modules() -> [(u32, Vec<u32>); 3] {
    [(VERTEX, words(VERTEX_SPIRV)), (FRAGMENT, words(FRAGMENT_SPIRV)), (COMPUTE, words(COMPUTE_SPIRV))]
}

/// The numbers the method calls pick from.
const NUMBERS: [u32; 9] = [0, 1, 2, 4, 31, 32, 64, 1000, u32::MAX];
const FLOATS: [f32; 7] = [0.0, 1.0, 64.0, -1.0, f32::MAX, f32::INFINITY, f32::NAN];
const BYTES: [u8; 7] = [0, 1, 3, 5, 6, 200, u8::MAX];
const LENGTHS: [usize; 5] = [0, 1, 4, 33, 300];

fn pick<T: Copy>(values: &[T], seed: usize) -> T {
    values[seed.wrapping_mul(0x9E37_79B9) % values.len()]
}

/// A program's stages compiled from GLSL, which must compile.
fn compiled<M: MesaCompiler>(compiler: &M, sources: [Option<&CStr>; 3]) -> [Option<M::Shader>; 3] {
    match compiler.glsl_to_nir(sources) {
        Ok(program) => program.stages,
        Err(log) => panic!("the program compiles: {}", String::from_utf8_lossy(&log)),
    }
}

/// Every shader the trait can make that the hostile calls start from: a vertex, a fragment and a compute
/// program from GLSL (new and old), each again from SPIR-V the trait wrote, the geometry program that draws a
/// vertex program's points, and the programs storage_pass_through makes.
fn pool<M: MesaCompiler>(compiler: &M) -> Vec<M::Shader> {
    let mut made = Vec::new();
    let [vertex, fragment, _] = compiled(compiler, [Some(VERTEX_SOURCE), Some(FRAGMENT_SOURCE), None]);
    let [_, _, compute] = compiled(compiler, [None, None, Some(COMPUTE_SOURCE)]);
    let [old_vertex, old_fragment, _] = compiled(compiler, [Some(OLD_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None]);
    made.extend([vertex, fragment, compute, old_vertex, old_fragment].into_iter().flatten());
    for (stage, words) in modules() {
        made.extend(compiler.spirv_to_nir(&words, stage, MAIN));
    }
    let mut shaders = Vec::new();
    for shader in made {
        if let Some(mut copy) = compiler.cloned(&shader)
            && let Some(passed) = compiler.storage_pass_through(&mut copy)
        {
            shaders.extend(passed.into_iter().flatten());
        }
        shaders.extend(compiler.point_square(&shader, 1, 64.0));
        shaders.push(shader);
    }
    shaders
}

/// Compiles a program of each stage, reads each back as a flat form, serialises, clones, writes SPIR-V and
/// reads that SPIR-V back.
pub fn compiles_and_reads_back<M: MesaCompiler>() {
    let compiler = M::new();
    let program = compiler.glsl_to_nir([Some(VERTEX_SOURCE), Some(FRAGMENT_SOURCE), None]).expect("the program compiles");
    assert!(program.stages[0].is_some() && program.stages[1].is_some() && program.stages[2].is_none());
    assert!(!program.names[0].is_empty(), "the vertex program's names");
    let [vertex, fragment, _] = program.stages;
    let [_, _, compute] = compiled(&compiler, [None, None, Some(COMPUTE_SOURCE)]);
    for shader in [vertex, fragment, compute] {
        let shader = shader.expect("the stage compiled");
        let serialised = compiler.serialize(&shader).expect("the shader serialises");
        let copy = compiler.cloned(&shader).expect("the shader clones");
        assert_eq!(compiler.serialize(&copy), Some(serialised), "a clone serialises alike");
        let mut flattened = compiler.cloned(&shader).expect("the shader clones");
        let flat = compiler.flatten(&mut flattened, 0).expect("the shader flattens");
        assert!(!flat.instrs.is_empty(), "the flat form has instructions");
    }
    // SPIR-V for Vulkan read, then written by the trait as SPIR-V and read back.
    for (stage, words) in modules() {
        let mut shader = compiler.spirv_to_nir(&words, stage, MAIN).expect("the SPIR-V reads");
        let serialised = compiler.serialize(&shader).expect("the shader serialises");
        assert!(!serialised.is_empty());
        let copy = compiler.cloned(&shader).expect("the shader clones");
        assert_eq!(compiler.serialize(&copy), Some(serialised), "a clone serialises alike");
        let mut flattened = compiler.cloned(&shader).expect("the shader clones");
        let flat = compiler.flatten(&mut flattened, 0).expect("the shader flattens");
        assert!(!flat.instrs.is_empty(), "the flat form has instructions");
        let words = compiler.to_spirv(&mut shader).expect("the shader is written as SPIR-V");
        assert_eq!(words.first(), Some(&SPIRV_MAGIC));
        let back = compiler.spirv_to_nir(&words, stage, MAIN);
        // Mesa's reader refuses the sample mask as Zink writes it (an array of arrays stored as one value):
        // the fragment program, which writes gl_SampleMask, is not read back; the others are.
        if stage != FRAGMENT {
            assert!(back.is_some(), "the SPIR-V of stage {stage} reads back");
        }
    }
}

/// What the trait refuses: a stage past the bridge's, SPIR-V that is no module, an entry point it lacks, a
/// program that does not compile, a program of no stage.
pub fn refuses_what_is_not_a_program<M: MesaCompiler>() {
    let compiler = M::new();
    let words = words(VERTEX_SPIRV);
    for stage in [3, 4, u32::MAX] {
        assert!(compiler.spirv_to_nir(&words, stage, MAIN).is_none(), "stage {stage} refused");
    }
    assert!(compiler.spirv_to_nir(&words, VERTEX, c"absent").is_none(), "an entry point the module lacks");
    assert!(compiler.spirv_to_nir(&[], VERTEX, MAIN).is_none(), "no words");
    assert!(compiler.spirv_to_nir(&words[..words.len().min(5)], VERTEX, MAIN).is_none(), "a header alone");
    let mut changed = words.clone();
    // The first instruction's word count past the module's end.
    changed[5] = u32::MAX;
    assert!(compiler.spirv_to_nir(&changed, VERTEX, MAIN).is_none(), "a module whose first instruction runs past its end");
    let log = compiler.glsl_to_nir([Some(BROKEN_SOURCE), None, None]).err().expect("a broken program is refused");
    assert!(!log.is_empty(), "with the compiler's message");
    let none = compiler.glsl_to_nir([None, None, None]);
    assert!(none.is_err() || none.is_ok_and(|program| program.stages.iter().all(Option::is_none)), "a program of no stage has none");
}

/// Every method of the trait, called by a hostile caller: each on a fresh copy of every shader of the pool, with
/// arguments picked by `rounds` seeds; nothing may end the process.
pub fn every_method_is_called<M: MesaCompiler>(rounds: usize) {
    let compiler = M::new();
    let shaders = pool(&compiler);
    assert!(shaders.len() >= 10, "the pool has {} shaders", shaders.len());
    for round in 0..rounds {
        for (at, shader) in shaders.iter().enumerate() {
            for which in 0..METHODS {
                if let Some(mut copy) = compiler.cloned(shader) {
                    call_one(&compiler, &mut copy, which, round.wrapping_mul(METHODS * 64).wrapping_add(at * METHODS + which));
                }
            }
        }
    }
}

/// The next number of a seeded sequence (SplitMix64).
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `calls` hostile calls picked by the seed: methods in sequence on the same shaders (each call changes what
/// the next sees), shaders made from SPIR-V whose words were changed, and the shaders the calls make joining the
/// pool. Returns the calls made.
pub fn fuzz<M: MesaCompiler>(seed: u64, calls: u64) -> u64 {
    let compiler = M::new();
    let mut shaders = pool(&compiler);
    let modules = modules();
    let mut state = seed;
    let mut made = 0;
    // Each call named on the error stream before it is made, where WCONTAINERS_FUZZ_TRACE is set: the calls a
    // shader went through before one that ended the process.
    let trace = std::env::var_os("WCONTAINERS_FUZZ_TRACE").is_some();
    for _ in 0..calls {
        let choice = next(&mut state);
        if choice.is_multiple_of(8) {
            // SPIR-V with one to four words changed, read as a program.
            let (stage, words) = &modules[usize::try_from(choice >> 8).unwrap_or(0) % modules.len()];
            let mut changed = words.clone();
            for _ in 0..1 + (choice >> 4) % 4 {
                let at = usize::try_from(next(&mut state)).unwrap_or(0) % changed.len().max(1);
                if let Some(word) = changed.get_mut(at) {
                    *word ^= 1 << (next(&mut state) % 32);
                }
            }
            if let Some(shader) = compiler.spirv_to_nir(&changed, *stage, MAIN)
                && shaders.len() < 64
            {
                shaders.push(shader);
            }
        } else if !shaders.is_empty() {
            let at = usize::try_from(choice >> 8).unwrap_or(0) % shaders.len();
            let which = usize::try_from(choice >> 40).unwrap_or(0) % METHODS;
            let pick_seed = usize::try_from(next(&mut state)).unwrap_or(0);
            if trace {
                let shader = &shaders[at];
                eprintln!(
                    "CALL shader {at} method {which} seed {pick_seed} (generic end {}, geometry vertices {}, makes points {})",
                    compiler.generic_end(shader),
                    compiler.geometry_vertices_out(shader),
                    compiler.makes_points(shader)
                );
            }
            if let Some(made_one) = call_one(&compiler, &mut shaders[at], which, pick_seed)
                && shaders.len() < 64
            {
                shaders.push(made_one);
            }
        }
        made += 1;
    }
    made
}

/// Runs `fuzz` with the seed and count WCONTAINERS_FUZZ_SEED and WCONTAINERS_FUZZ_CALLS give; nothing without them.
pub fn fuzz_from_the_environment<M: MesaCompiler>() {
    let number = |name: &str| std::env::var(name).ok().and_then(|value| value.parse::<u64>().ok());
    let (Some(seed), Some(calls)) = (number("WCONTAINERS_FUZZ_SEED"), number("WCONTAINERS_FUZZ_CALLS")) else { return };
    let made = fuzz::<M>(seed, calls);
    println!("FUZZ seed {seed} calls {made}");
}

/// Runs calls alone, as WCONTAINERS_PROBE names them ("shader,seed,method,...": the shader's place in the pool, the
/// seed of the arguments, then the methods' numbers in `call_one`, made in turn) on a fresh copy, so that calls that
/// end the process are found by themselves; prints the pool's size first. Nothing without it.
pub fn probe_from_the_environment<M: MesaCompiler>() {
    let Ok(probe) = std::env::var("WCONTAINERS_PROBE") else { return };
    let parts: Vec<usize> = probe.split(',').filter_map(|part| part.parse().ok()).collect();
    let compiler = M::new();
    let shaders = pool(&compiler);
    println!("POOL {}", shaders.len());
    if let [at, seed, ref methods @ ..] = parts[..]
        && let Some(mut copy) = shaders.get(at).and_then(|shader| compiler.cloned(shader))
    {
        for &which in methods {
            let _ = call_one(&compiler, &mut copy, which, seed);
        }
        println!("PROBE {probe} ended");
    }
}

/// The methods `call_one` calls (every method of the trait but `new`, `spirv_to_nir` and `glsl_to_nir`, which
/// make the shaders the calls are made on and are called by the other tests).
pub const METHODS: usize = 36;

/// Calls the trait's method numbered `which` on the shader, with arguments picked by `seed`; a shader the call
/// made is returned.
fn call_one<M: MesaCompiler>(compiler: &M, shader: &mut M::Shader, which: usize, seed: usize) -> Option<M::Shader> {
    let number = pick(&NUMBERS, seed);
    let other = pick(&NUMBERS, seed / 7 + 1);
    let float = pick(&FLOATS, seed / 3);
    let flag = seed.is_multiple_of(2);
    let length = pick(&LENGTHS, seed / 5);
    match which {
        0 => {
            let _ = compiler.flatten(shader, number);
        }
        1 => {
            let ranges = [0, 1, 2, 3].map(|kind| SlotRange { first: pick(&NUMBERS, seed + kind), slots: pick(&NUMBERS, seed / 11 + kind) });
            let _ = compiler.to_common_form(shader, number, &ranges);
        }
        2 => compiler.lower_io(shader),
        3 => compiler.alpha_to_one(shader),
        4 => {
            let _ = compiler.serialize(shader);
        }
        5 => {
            let _ = compiler.to_spirv(shader);
        }
        6 => return compiler.cloned(shader),
        7 => {
            let _ = compiler.number_vertex_elements(shader);
        }
        8 => {
            let _ = compiler.geometry_vertices_out(shader);
        }
        9 => {
            let _ = compiler.named_layer_location(shader);
        }
        10 => {
            let _ = compiler.control_points_out(shader);
        }
        11 => {
            let _ = compiler.makes_points(shader);
        }
        12 => compiler.fragcolor_to_buffers(shader, number),
        13 => compiler.split_resource_arrays(shader),
        14 => compiler.textures_1d_as_2d(shader),
        15 => compiler.drop_name(shader),
        16 => compiler.uniform_blocks_as_words(shader),
        17 => compiler.transform_position(
            shader,
            PositionTransform {
                flip_y: flag,
                half_z: seed.is_multiple_of(3),
                reverse_z: seed.is_multiple_of(5),
                fixed_point_size: seed.is_multiple_of(7),
            },
        ),
        18 => {
            let _ = compiler.reads_sample_positions(shader);
        }
        19 => {
            let _ = compiler.interpolate_elsewhere(shader, number);
        }
        20 => compiler.fit_sample_masks(shader, flag),
        21 => {
            let _ = compiler.writes_sample_mask(shader);
        }
        22 => {
            let _ = compiler.fetches_target(shader);
        }
        23 => {
            let _ = compiler.formatless_images(shader);
        }
        24 => compiler.fetch_from_texture(shader, number, flag),
        25 => {
            let swizzles: Vec<TextureSwizzle> = (0..length)
                .map(|at| TextureSwizzle { unit: pick(&NUMBERS, seed + at), swizzle: [0, 1, 2, 3].map(|c| pick(&BYTES, seed + at + c)) })
                .collect();
            compiler.swizzle_textures(shader, &swizzles);
        }
        26 => {
            let samplers: Vec<NearestSampler> = (0..length)
                .map(|at| NearestSampler {
                    unit: pick(&NUMBERS, seed + at),
                    wrap: [0, 1, 2].map(|axis| pick(&BYTES, seed + at + axis)),
                    mipmapped: (seed + at).is_multiple_of(2),
                    min_lod: pick(&FLOATS, seed + at),
                    max_lod: pick(&FLOATS, seed / 2 + at),
                    border: [0, 1, 2, 3].map(|c| pick(&NUMBERS, seed + at + c)),
                })
                .collect();
            let _ = compiler.nearest_integer_textures(shader, &samplers);
        }
        27 => {
            let _ = compiler.integer_sampled_units(shader);
        }
        28 => compiler.stream_point_size_at_a_location(shader),
        29 => {
            let _ = compiler.generic_end(shader);
        }
        30 => {
            let _ = compiler.point_size_also_at(shader, number);
        }
        31 => {
            let _ = compiler.point_size_at_most(shader, float);
        }
        32 => return compiler.point_square(shader, number, float),
        33 => return compiler.geometry_squares(shader, number, float, other),
        34 => return compiler.storage_pass_through(shader).and_then(|made| made.into_iter().flatten().next()),
        35 => {
            let mut outputs = vec![
                StreamOutput { buffer: 0, offset: 0, location: 0, position: false, first_component: 0, components: 0 };
                length
            ];
            let mut strides = vec![0_u32; pick(&LENGTHS, seed / 13)];
            let _ = compiler.stream_outputs(shader, &mut outputs, &mut strides);
        }
        _ => {}
    }
    None
}

#[macro_export]
macro_rules! tests {
    ($compiler:ty) => {
        #[test]
        fn compiles_and_reads_back() {
            $crate::compiles_and_reads_back::<$compiler>();
        }

        #[test]
        fn refuses_what_is_not_a_program() {
            $crate::refuses_what_is_not_a_program::<$compiler>();
        }

        #[test]
        fn every_method_is_called() {
            $crate::every_method_is_called::<$compiler>(4);
        }

        #[test]
        fn probe_from_the_environment() {
            $crate::probe_from_the_environment::<$compiler>();
        }

        #[test]
        fn fuzz_from_the_environment() {
            $crate::fuzz_from_the_environment::<$compiler>();
        }
    };
}
