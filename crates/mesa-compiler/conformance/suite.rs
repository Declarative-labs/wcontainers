//! The tests every implementation of wcontainers-mesa-compiler runs, written against the trait alone. The trait's
//! shaders are typed by their place in the bridge's chain, so a caller can write only the orders the bridge's callers
//! take (the doctests below show the others do not compile); the tests walk every one of those orders: programs
//! compiled from GLSL and SPIR-V and read back in every form the trait gives; a hostile caller taking every walk from
//! every program with edge arguments at every step (values past every enumeration, empty and long lists, units past
//! every limit); and a seeded fuzzer of the same, from SPIR-V whose words were changed and GLSL whose numbers were.
//!
//! The orders that do not compile:
//!
//! ```compile_fail,E0308
//! // Zink's writer on a program not in the common form.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, read: M::SpirvShader) {
//!     let _ = compiler.spirv_common_to_spirv(read);
//! }
//! ```
//!
//! ```compile_fail,E0382
//! // Zink's writer twice on one program (it is taken by the first).
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, common: M::SpirvCommon) {
//!     let _ = compiler.spirv_common_to_spirv(common);
//!     let _ = compiler.spirv_common_to_spirv(common);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // Zink's writer on the OpenGL driver's form.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, variant: M::Variant) {
//!     let _ = compiler.spirv_common_to_spirv(variant);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // The driver's steps at a creation on a program read from SPIR-V.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, read: M::SpirvShader) {
//!     let _ = compiler.received_prepare(read);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A draw's variant of a stage the driver's creation steps have not taken.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, received: &M::Received, settings: wcontainers_mesa_compiler::VariantSettings) {
//!     let _ = compiler.prepared_variant(received, settings, &[], &[]);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // Points as squares from a program the driver's steps made (a pass-through or a square).
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, made: &M::Made) {
//!     let _ = compiler.prepared_points_as_squares(made, 0, 1.0);
//! }
//! ```
//!
//! The orders the callers take compile:
//!
//! ```
//! fn candidate_a<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, words: &[u32], ranges: &[wcontainers_mesa_compiler::SlotRange; 4]) -> Option<Vec<u32>> {
//!     let read = compiler.spirv_read(words, 0, c"main", false)?;
//!     let _key = compiler.spirv_shader_key(&read)?;
//!     let common = compiler.spirv_to_common_form(read, 0, ranges).shader?;
//!     compiler.spirv_common_to_spirv(common)
//! }
//! fn draw<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, received: M::Received, settings: wcontainers_mesa_compiler::VariantSettings) -> Option<wcontainers_mesa_compiler::Flat> {
//!     let prepared = compiler.received_prepare(received).shader?;
//!     let variant = compiler.prepared_variant(&prepared, settings, &[], &[]).shader?;
//!     compiler.variant_flatten(variant, 0)
//! }
//! ```

use std::ffi::{CStr, CString};

use wcontainers_mesa_compiler::{MesaCompiler, NearestSampler, PositionTransform, SlotRange, TextureSwizzle, VariantSettings, constants};

/// The first word of every SPIR-V module.
const SPIRV_MAGIC: u32 = 0x0723_0203;
/// The bridge's stages (enum wc_stage).
const VERTEX: u32 = constants::STAGE_VERTEX;
const FRAGMENT: u32 = constants::STAGE_FRAGMENT;
const COMPUTE: u32 = constants::STAGE_COMPUTE;
const GEOMETRY: u32 = constants::STAGE_GEOMETRY;
/// The stages a GLSL program has a source for, by enum wc_stage.
const STAGES: usize = constants::STAGES as usize;
/// A program's sources by stage (none where it has no such stage).
type Sources<'a> = [Option<&'a str>; STAGES];
/// The entry point every program here names.
const MAIN: &CStr = c"main";

/// A vertex program with a uniform block, a point size, two inputs and an output.
const VERTEX_SOURCE: &str = "#version 450
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
const FRAGMENT_SOURCE: &str = "#version 450
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
const COMPUTE_SOURCE: &str = "#version 450
layout(local_size_x = 4) in;
layout(std430, binding = 0) buffer Results { uint values[]; };
layout(binding = 0, rgba8) uniform writeonly image2D written;
void main() {
    values[gl_LocalInvocationID.x] = gl_LocalInvocationID.x;
    imageStore(written, ivec2(gl_LocalInvocationID.xy), vec4(1.0));
}
";

/// An older program: the vertex built-ins and gl_FragColor (written to every draw buffer).
const OLD_VERTEX_SOURCE: &str = "#version 110
void main() { gl_Position = gl_Vertex; gl_PointSize = 4.0; }
";
const OLD_FRAGMENT_SOURCE: &str = "#version 110
void main() { gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0); }
";

/// A vertex program writing a storage block (drawn before pass-through geometry programs) and a point size.
const STORING_VERTEX_SOURCE: &str = "#version 450
layout(std430, binding = 0) buffer Seen { uint seen[]; };
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 passed;
void main() {
    seen[gl_VertexID] = 1u;
    gl_Position = at;
    gl_PointSize = 3.0;
    passed = at;
}
";

/// A fragment program sampling and gathering integer cube maps and an integer array.
const INTEGER_FRAGMENT_SOURCE: &str = "#version 450
layout(binding = 0) uniform isamplerCube cube;
layout(binding = 1) uniform isampler2DArray layers;
layout(binding = 2) uniform usamplerCubeArray cubes;
layout(location = 0) in vec3 at;
layout(location = 0) out ivec4 colour;
void main() {
    colour = texture(cube, at) + textureGather(cube, at) + texture(layers, at) + ivec4(texture(cubes, vec4(at, 1.0)));
}
";

/// A vertex program feeding the integer program's input.
const INTEGER_VERTEX_SOURCE: &str = "#version 450
layout(location = 0) in vec4 position;
layout(location = 0) out vec3 at;
void main() {
    gl_Position = position;
    at = position.xyz;
}
";

/// A vertex program drawing points of its own size, before a geometry or tessellation stage.
const POINT_VERTEX_SOURCE: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 passed;
void main() {
    gl_Position = at;
    gl_PointSize = at.w;
    passed = at;
}
";

/// A geometry program taking and making points, each with its point size and a layer it names.
const POINT_GEOMETRY_SOURCE: &str = "#version 450
layout(points) in;
layout(points, max_vertices = 2) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 shaded;
void main() {
    for (int copy = 0; copy < 2; copy++) {
        gl_Position = gl_in[0].gl_Position + vec4(float(copy));
        gl_PointSize = gl_in[0].gl_PointSize;
        gl_Layer = copy;
        shaded = passed[0];
        EmitVertex();
    }
}
";

/// A tessellation control program of three control points, passing each on.
const CONTROL_SOURCE: &str = "#version 450
layout(vertices = 3) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 handed[];
void main() {
    handed[gl_InvocationID] = passed[gl_InvocationID];
    gl_out[gl_InvocationID].gl_Position = gl_in[gl_InvocationID].gl_Position;
    gl_TessLevelOuter[0] = 2.0;
    gl_TessLevelOuter[1] = 2.0;
    gl_TessLevelOuter[2] = 2.0;
    gl_TessLevelInner[0] = 2.0;
}
";

/// A tessellation evaluation program making triangles from the control program's patch.
const EVALUATION_SOURCE: &str = "#version 450
layout(triangles, equal_spacing, ccw) in;
layout(location = 0) in vec4 handed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_TessCoord.x * gl_in[0].gl_Position + gl_TessCoord.y * gl_in[1].gl_Position + gl_TessCoord.z * gl_in[2].gl_Position;
    shaded = handed[0];
}
";

/// A fragment program after the geometry or evaluation program, reading the layer.
const SHADED_FRAGMENT_SOURCE: &str = "#version 450
layout(location = 0) in vec4 shaded;
layout(location = 0) out vec4 colour;
void main() {
    colour = shaded + vec4(float(gl_Layer));
}
";

/// The control points of the tessellation control program's patch, its evaluation program's input.
const CONTROL_POINTS: u32 = 3;

/// A source the compiler refuses.
const BROKEN_SOURCE: &CStr = c"#version 450
void main() { this is not GLSL }
";

/// The GLSL programs the walks start from, by enum wc_stage.
const PROGRAMS: [Sources<'static>; 7] = [
    [Some(VERTEX_SOURCE), Some(FRAGMENT_SOURCE), None, None, None, None],
    [None, None, Some(COMPUTE_SOURCE), None, None, None],
    [Some(OLD_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None, None, None, None],
    [Some(STORING_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None, None, None, None],
    [Some(INTEGER_VERTEX_SOURCE), Some(INTEGER_FRAGMENT_SOURCE), None, None, None, None],
    [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(POINT_GEOMETRY_SOURCE), None, None],
    [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(CONTROL_SOURCE), Some(EVALUATION_SOURCE)],
];

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

/// The numbers the calls pick from.
const NUMBERS: [u32; 9] = [0, 1, 2, 4, 31, 32, 64, 1000, u32::MAX];
const SAMPLES: [u32; 8] = [0, 1, 2, 3, 4, 8, 16, u32::MAX];
const FLOATS: [f32; 7] = [0.0, 1.0, 64.0, -1.0, f32::MAX, f32::INFINITY, f32::NAN];
const BYTES: [u8; 7] = [0, 1, 3, 5, 6, 200, u8::MAX];
const LENGTHS: [usize; 5] = [0, 1, 4, 33, 300];
/// Ranges the common form takes every program here into, and the bridge's own refusals past them.
const SLOTS: [u32; 5] = [64, 16, 1, 0, u32::MAX];

/// Arguments picked by a seed: each call takes the next.
struct Picks {
    state: u64,
}

impl Picks {
    fn new(seed: u64) -> Picks {
        Picks { state: seed }
    }

    /// The next number of the sequence (SplitMix64).
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, count: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(count.max(1)).unwrap_or(1)).unwrap_or(0)
    }

    fn of<T: Copy>(&mut self, values: &[T]) -> T {
        values[self.below(values.len())]
    }

    fn flag(&mut self) -> bool {
        self.next().is_multiple_of(2)
    }

    fn transform(&mut self) -> PositionTransform {
        // Mostly none, as most draws take.
        if self.next().is_multiple_of(3) {
            return PositionTransform { flip_y: false, half_z: false, reverse_z: false, fixed_point_size: false };
        }
        PositionTransform { flip_y: self.flag(), half_z: self.flag(), reverse_z: self.flag(), fixed_point_size: self.flag() }
    }

    fn ranges(&mut self) -> [SlotRange; 4] {
        let first = self.of(&NUMBERS);
        [0, 1, 2, 3].map(|_| SlotRange { first: if self.flag() { 0 } else { first }, slots: self.of(&SLOTS) })
    }

    fn settings(&mut self) -> VariantSettings {
        let transform = self.transform();
        VariantSettings {
            transform: transform.clone(),
            key_transform: if self.flag() { transform } else { self.transform() },
            fetch_unit: self.of(&NUMBERS),
            fetch_multisampled: self.flag(),
            single_sampled: self.flag(),
            samples: self.of(&SAMPLES),
            set: self.of(&NUMBERS),
            ranges: self.ranges(),
        }
    }

    fn swizzles(&mut self) -> Vec<TextureSwizzle> {
        let length = if self.flag() { 0 } else { self.of(&LENGTHS) };
        (0..length).map(|_| TextureSwizzle { unit: self.of(&NUMBERS), swizzle: [0, 1, 2, 3].map(|_| self.of(&BYTES)) }).collect()
    }

    fn samplers(&mut self) -> Vec<NearestSampler> {
        let length = if self.flag() { 0 } else { self.of(&LENGTHS) };
        (0..length)
            .map(|_| NearestSampler {
                unit: self.of(&NUMBERS),
                wrap: [0, 1, 2].map(|_| self.of(&BYTES)),
                mipmapped: self.flag(),
                min_lod: self.of(&FLOATS),
                max_lod: self.of(&FLOATS),
                border: [0, 1, 2, 3].map(|_| self.of(&NUMBERS)),
            })
            .collect()
    }
}

/// Counts the trait's calls a walk makes, and by method the calls and those that made what they make (the walk then
/// went on from it).
struct Calls(u64, std::collections::BTreeMap<&'static str, (u64, u64)>);

impl Calls {
    fn new() -> Calls {
        Calls(0, std::collections::BTreeMap::new())
    }

    fn one(&mut self, method: &'static str) {
        self.0 += 1;
        self.1.entry(method).or_default().0 += 1;
    }

    /// The call of `method` just counted made what it makes; returns `made`.
    fn made<T>(&mut self, method: &'static str, made: Option<T>) -> Option<T> {
        if made.is_some() {
            self.1.entry(method).or_default().1 += 1;
        }
        made
    }

    fn report(&self, label: &str) {
        for (method, (calls, made)) in &self.1 {
            println!("{label} {method} calls {calls} made {made}");
        }
    }
}

/// The SPIR-V candidates' walk (the corpus's, run.rs): a module read (alphaToOne made in a fragment program where the
/// picks say), its key, the common form, then Zink's writer or the flat form (or nothing: the program dropped).
fn spirv_walk<M: MesaCompiler>(compiler: &M, stage: u32, words: &[u32], picks: &mut Picks, calls: &mut Calls) {
    calls.one("spirv_read");
    let alpha_to_one = picks.flag();
    let Some(read) = calls.made("spirv_read", compiler.spirv_read(words, stage, MAIN, alpha_to_one)) else { return };
    if picks.flag() {
        calls.one("spirv_shader_key");
        let _ = calls.made("spirv_shader_key", compiler.spirv_shader_key(&read));
    }
    calls.one("spirv_to_common_form");
    let form = compiler.spirv_to_common_form(read, picks.of(&NUMBERS), &picks.ranges());
    let Some(common) = calls.made("spirv_to_common_form", form.shader) else { return };
    match picks.below(3) {
        0 => {
            calls.one("spirv_common_to_spirv");
            let _ = calls.made("spirv_common_to_spirv", compiler.spirv_common_to_spirv(common));
        }
        1 => {
            calls.one("spirv_common_flatten");
            let _ = calls.made("spirv_common_flatten", compiler.spirv_common_flatten(common));
        }
        _ => drop(common),
    }
}

/// A draw's walk on a prepared or made program: its variant, then the flat form (or nothing).
fn flatten_variant<M: MesaCompiler>(compiler: &M, form: wcontainers_mesa_compiler::VariantForm<M::Variant>, picks: &mut Picks, calls: &mut Calls) {
    if let Some(variant) = form.shader
        && !picks.next().is_multiple_of(4)
    {
        calls.one("variant_flatten");
        let _ = calls.made("variant_flatten", compiler.variant_flatten(variant, picks.of(&NUMBERS)));
    }
}

/// A made program's walk: its key, its named layer, and draws of it.
fn made_walk<M: MesaCompiler>(compiler: &M, made: &M::Made, picks: &mut Picks, calls: &mut Calls) {
    calls.one("made_key");
    let _ = calls.made("made_key", compiler.made_key(made));
    calls.one("made_named_layer_location");
    let _ = calls.made("made_named_layer_location", Some(compiler.made_named_layer_location(made)).filter(|at| *at != u32::MAX));
    for _ in 0..1 + picks.below(2) {
        let (settings, swizzles, samplers) = (picks.settings(), picks.swizzles(), picks.samplers());
        calls.one("made_variant");
        let form = compiler.made_variant(made, settings, &swizzles, &samplers);
        let _ = calls.made("made_variant", form.shader.as_ref());
        flatten_variant(compiler, form, picks, calls);
    }
}

/// The OpenGL driver's walk on one stage (state.rs, draw.rs): its creation steps, its pass-through programs, draws of
/// it, and its points drawn as squares.
fn stage_walk<M: MesaCompiler>(compiler: &M, received: M::Received, picks: &mut Picks, calls: &mut Calls) {
    calls.one("received_prepare");
    let form = compiler.received_prepare(received);
    let _ = calls.made("received_prepare", form.shader.as_ref());
    for made in form.pass_through.iter().flatten() {
        made_walk(compiler, made, picks, calls);
    }
    let Some(prepared) = form.shader else { return };
    for _ in 0..1 + picks.below(3) {
        let (settings, swizzles, samplers) = (picks.settings(), picks.swizzles(), picks.samplers());
        calls.one("prepared_variant");
        let variant = compiler.prepared_variant(&prepared, settings, &swizzles, &samplers);
        let _ = calls.made("prepared_variant", variant.shader.as_ref());
        flatten_variant(compiler, variant, picks, calls);
    }
    if picks.flag() {
        calls.one("prepared_points_as_squares");
        let squares = compiler.prepared_points_as_squares(&prepared, picks.of(&NUMBERS), picks.of(&FLOATS));
        let _ = calls.made("prepared_points_as_squares", squares.geometry.as_ref());
        for made in squares.before.iter().chain(&squares.geometry) {
            made_walk(compiler, made, picks, calls);
        }
    }
    if picks.flag() {
        calls.one("prepared_geometry_points_as_squares");
        let squares = compiler.prepared_geometry_points_as_squares(&prepared, picks.of(&NUMBERS), picks.of(&FLOATS), picks.of(&NUMBERS));
        let _ = calls.made("prepared_geometry_points_as_squares", squares.geometry.as_ref());
        for made in squares.before.iter().chain(&squares.geometry) {
            made_walk(compiler, made, picks, calls);
        }
    }
}

/// A GLSL program's walk: compiled, then the driver's walk on each stage.
fn glsl_walk<M: MesaCompiler>(compiler: &M, sources: Sources<'_>, picks: &mut Picks, calls: &mut Calls) {
    let owned: Vec<Option<CString>> = sources.iter().map(|source| source.and_then(|text| CString::new(text).ok())).collect();
    let texts: [Option<&CStr>; STAGES] = std::array::from_fn(|at| owned.get(at).and_then(|text| text.as_deref()));
    calls.one("glsl_to_nir");
    let Some(program) = calls.made("glsl_to_nir", compiler.glsl_to_nir(texts).ok()) else { return };
    for received in program.stages.into_iter().flatten() {
        stage_walk(compiler, received, picks, calls);
    }
}

/// Compiles a program of each stage, takes it through the driver's steps and a draw's to the flat form; reads each
/// SPIR-V module, writes it through the common form as SPIR-V and reads that back, and flattens it.
pub fn compiles_and_reads_back<M: MesaCompiler>(compiler: M) {
    let ranges = [SlotRange { first: 0, slots: 64 }, SlotRange { first: 0, slots: 16 }, SlotRange { first: 0, slots: 16 }, SlotRange { first: 0, slots: 16 }];
    let none = PositionTransform { flip_y: false, half_z: false, reverse_z: false, fixed_point_size: false };
    let settings = VariantSettings {
        transform: none.clone(),
        key_transform: none,
        fetch_unit: 0,
        fetch_multisampled: false,
        single_sampled: true,
        samples: 1,
        set: 0,
        ranges: ranges.clone(),
    };
    for sources in PROGRAMS {
        let owned: Vec<Option<CString>> = sources.iter().map(|source| source.map(|text| CString::new(text).expect("no zero byte"))).collect();
        let texts: [Option<&CStr>; STAGES] = std::array::from_fn(|at| owned[at].as_deref());
        let program = compiler.glsl_to_nir(texts).unwrap_or_else(|log| panic!("the program compiles: {}", String::from_utf8_lossy(&log)));
        for (at, stage) in program.stages.into_iter().enumerate() {
            assert_eq!(stage.is_some(), sources[at].is_some(), "a stage made for each source");
            let Some(received) = stage else { continue };
            let form = compiler.received_prepare(received);
            assert!(!form.key.is_empty(), "the prepared stage has a key");
            assert!(!form.streams_elsewhere);
            let prepared = form.shader.expect("the stage is prepared");
            let variant = compiler.prepared_variant(&prepared, settings.clone(), &[], &[]);
            assert_eq!(variant.result, constants::VARIANT_MADE, "the variant is made");
            assert!(!variant.key.is_empty(), "the variant has a key");
            let points_in = if at == constants::STAGE_TESS_CONTROL as usize || at == constants::STAGE_TESS_EVALUATION as usize { CONTROL_POINTS } else { 0 };
            let flat = compiler.variant_flatten(variant.shader.expect("made"), points_in).unwrap_or_else(|| panic!("the variant of stage {at} flattens"));
            assert!(!flat.instrs.is_empty(), "the flat form has instructions");
            if sources[at] == Some(STORING_VERTEX_SOURCE) {
                assert!(form.passes_through && form.pass_through.iter().any(Option::is_some), "a vertex program using storage passes through");
            }
        }
    }
    // A vertex program writing a point size, drawn as squares.
    let source = CString::new(VERTEX_SOURCE).expect("no zero byte");
    let program = compiler.glsl_to_nir([Some(&source), None, None, None, None, None]).expect("the program compiles");
    let [Some(received), ..] = program.stages else { panic!("a vertex stage") };
    let prepared = compiler.received_prepare(received).shader.expect("prepared");
    let squares = compiler.prepared_points_as_squares(&prepared, 1, 64.0);
    assert_eq!(squares.kind, constants::SQUARES_MADE, "points drawn as squares");
    let geometry = squares.geometry.expect("the squares' program");
    assert!(compiler.made_key(&geometry).is_some_and(|key| !key.is_empty()));
    // A geometry program making points, naming a layer, drawn as squares: the made program names the layer too.
    let owned = [POINT_VERTEX_SOURCE, SHADED_FRAGMENT_SOURCE, POINT_GEOMETRY_SOURCE].map(|text| CString::new(text).expect("no zero byte"));
    let program = compiler.glsl_to_nir([Some(&owned[0]), Some(&owned[1]), None, Some(&owned[2]), None, None]).expect("the program compiles");
    let Some(Some(received)) = program.stages.into_iter().nth(GEOMETRY as usize) else { panic!("a geometry stage") };
    let prepared = compiler.received_prepare(received).shader.expect("prepared");
    let squares = compiler.prepared_geometry_points_as_squares(&prepared, 1, 64.0, 0);
    assert_eq!(squares.kind, constants::SQUARES_MADE, "a geometry program's points drawn as squares");
    let geometry = squares.geometry.expect("the squares' program");
    assert!(compiler.made_key(&geometry).is_some_and(|key| !key.is_empty()));
    assert_ne!(compiler.made_named_layer_location(&geometry), u32::MAX, "the squares name the layer");
    // SPIR-V for Vulkan read, then written by Zink's writer and read back; and flattened.
    for (stage, words) in modules() {
        let read = compiler.spirv_read(&words, stage, MAIN, false).expect("the SPIR-V reads");
        assert!(compiler.spirv_shader_key(&read).is_some_and(|key| !key.is_empty()));
        let form = compiler.spirv_to_common_form(read, 0, &ranges);
        assert_eq!(form.result, constants::BIND_OK, "the common form binds");
        let written = compiler.spirv_common_to_spirv(form.shader.expect("in the common form")).unwrap_or_else(|| panic!("the program of stage {stage} is written as SPIR-V"));
        assert_eq!(written.first(), Some(&SPIRV_MAGIC));
        // Mesa's reader refuses the sample mask as Zink writes it (an array of arrays stored as one value): the
        // fragment program, which writes gl_SampleMask, is not read back; the others are.
        if stage != FRAGMENT {
            assert!(compiler.spirv_read(&written, stage, MAIN, false).is_some(), "the SPIR-V of stage {stage} reads back");
        }
        let read = compiler.spirv_read(&words, stage, MAIN, stage == FRAGMENT).expect("the SPIR-V reads");
        let common = compiler.spirv_to_common_form(read, 0, &ranges).shader.expect("in the common form");
        let flat = compiler.spirv_common_flatten(common).expect("the program flattens");
        assert!(!flat.instrs.is_empty(), "the flat form has instructions");
    }
}

/// What the trait refuses: a stage past the bridge's, SPIR-V that is no module, an entry point it lacks, a
/// program that does not compile, a program of no stage.
pub fn refuses_what_is_not_a_program<M: MesaCompiler>(compiler: M) {
    let words = words(VERTEX_SPIRV);
    for stage in [GEOMETRY, constants::STAGES, constants::STAGES + 1, u32::MAX] {
        assert!(compiler.spirv_read(&words, stage, MAIN, false).is_none(), "stage {stage} refused (the module has none of it)");
    }
    assert!(compiler.spirv_read(&words, VERTEX, c"absent", false).is_none(), "an entry point the module lacks");
    assert!(compiler.spirv_read(&[], VERTEX, MAIN, false).is_none(), "no words");
    assert!(compiler.spirv_read(&words[..words.len().min(5)], VERTEX, MAIN, false).is_none(), "a header alone");
    let mut changed = words.clone();
    // The first instruction's word count past the module's end.
    changed[5] = u32::MAX;
    assert!(compiler.spirv_read(&changed, VERTEX, MAIN, false).is_none(), "a module whose first instruction runs past its end");
    let log = compiler.glsl_to_nir([Some(BROKEN_SOURCE), None, None, None, None, None]).err().expect("a broken program is refused");
    assert!(!log.is_empty(), "with the compiler's message");
    let none = compiler.glsl_to_nir([None; STAGES]);
    assert!(none.is_err() || none.is_ok_and(|program| program.stages.iter().all(Option::is_none)), "a program of no stage has none");
}

/// The walks the hostile caller takes, one a case: every SPIR-V module, with and without alphaToOne's pick, and every
/// GLSL program, each with arguments picked by `seed`. Returns the calls made, or None past the last case.
fn hostile_case<M: MesaCompiler>(compiler: &M, case: usize, seed: u64) -> Option<Calls> {
    let mut calls = Calls::new();
    let mut picks = Picks::new(seed.wrapping_mul(0x2545_F491_4F6C_DD1D).wrapping_add(u64::try_from(case).unwrap_or(0)));
    let modules = modules();
    if let Some((stage, words)) = modules.get(case) {
        spirv_walk(compiler, *stage, words, &mut picks, &mut calls);
    } else {
        let sources = PROGRAMS.get(case - modules.len())?;
        glsl_walk(compiler, *sources, &mut picks, &mut calls);
    }
    Some(calls)
}

/// The cases `hostile_case` takes.
const CASES: usize = 3 + PROGRAMS.len();
/// The seeds the hostile caller picks every case's arguments by.
const ROUNDS: u64 = 4;

/// Every walk the trait allows, taken by a hostile caller from every program, with arguments picked by `ROUNDS`
/// seeds; nothing may end the process.
pub fn every_method_is_called<M: MesaCompiler>(compiler: M) {
    let rounds = ROUNDS;
    let mut calls = Calls::new();
    for round in 0..rounds {
        for case in 0..CASES {
            if let Some(case) = hostile_case(&compiler, case, round) {
                calls.0 += case.0;
                for (method, (count, made)) in case.1 {
                    let entry = calls.1.entry(method).or_default();
                    entry.0 += count;
                    entry.1 += made;
                }
            }
        }
    }
    assert!(calls.0 >= rounds * u64::try_from(CASES).unwrap_or(0), "{} calls", calls.0);
    println!("HOSTILE rounds {rounds} cases {CASES} calls {}", calls.0);
    calls.report("HOSTILE");
}

/// SPIR-V words with one to four bits changed.
fn changed_words(words: &[u32], picks: &mut Picks) -> Vec<u32> {
    let mut changed = words.to_vec();
    for _ in 0..1 + picks.below(4) {
        let at = picks.below(changed.len());
        if let Some(word) = changed.get_mut(at) {
            *word ^= 1 << (picks.next() % 32);
        }
    }
    changed
}

/// GLSL sources with one to three of their digits changed (locations, bindings, sizes, versions).
fn changed_sources(sources: Sources<'_>, picks: &mut Picks) -> [Option<String>; STAGES] {
    let mut changed = sources.map(|source| source.map(str::to_string));
    for _ in 0..1 + picks.below(3) {
        let at = picks.below(STAGES);
        if let Some(text) = changed[at].as_mut() {
            let digits: Vec<usize> = text.char_indices().filter(|(_, c)| c.is_ascii_digit()).map(|(at, _)| at).collect();
            if !digits.is_empty() {
                let place = digits[picks.below(digits.len())];
                let digit = char::from(b'0' + u8::try_from(picks.below(10)).unwrap_or(0));
                text.replace_range(place..=place, &digit.to_string());
            }
        }
    }
    changed
}

/// Walks picked by the seed until `calls` calls are made: from the modules and programs as they are, from SPIR-V
/// whose words were changed and from GLSL whose digits were, with arguments picked at every step. Returns the calls
/// made.
fn fuzz<M: MesaCompiler>(compiler: &M, seed: u64, calls: u64) -> u64 {
    let modules = modules();
    let mut picks = Picks::new(seed);
    let mut made = Calls::new();
    // Each walk named on the error stream before it is taken, where WCONTAINERS_FUZZ_TRACE is set.
    let trace = std::env::var_os("WCONTAINERS_FUZZ_TRACE").is_some();
    let mut walk = 0_u64;
    while made.0 < calls {
        let choice = picks.next();
        let at = usize::try_from(choice >> 8).unwrap_or(0);
        if trace {
            eprintln!("WALK {walk} choice {choice} calls {}", made.0);
        }
        walk += 1;
        match choice % 8 {
            0..=3 => {
                let (stage, words) = &modules[at % modules.len()];
                let changed = changed_words(words, &mut picks);
                spirv_walk(compiler, *stage, &changed, &mut picks, &mut made);
            }
            4 => {
                let (stage, words) = &modules[at % modules.len()];
                spirv_walk(compiler, *stage, words, &mut picks, &mut made);
            }
            5 => {
                let changed = changed_sources(PROGRAMS[at % PROGRAMS.len()], &mut picks);
                let sources: Sources<'_> = std::array::from_fn(|stage| changed[stage].as_deref());
                glsl_walk(compiler, sources, &mut picks, &mut made);
            }
            _ => glsl_walk(compiler, PROGRAMS[at % PROGRAMS.len()], &mut picks, &mut made),
        }
    }
    made.report("FUZZ");
    made.0
}

/// Runs `fuzz` with the seed and count WCONTAINERS_FUZZ_SEED and WCONTAINERS_FUZZ_CALLS give; nothing without them.
pub fn fuzz_from_the_environment<M: MesaCompiler>(compiler: M) {
    let number = |name: &str| std::env::var(name).ok().and_then(|value| value.parse::<u64>().ok());
    let (Some(seed), Some(calls)) = (number("WCONTAINERS_FUZZ_SEED"), number("WCONTAINERS_FUZZ_CALLS")) else { return };
    let made = fuzz(&compiler, seed, calls);
    println!("FUZZ seed {seed} calls {made}");
}

/// Runs one hostile case alone, as WCONTAINERS_PROBE names it ("case,seed"), so that a case that ends the process is
/// found by itself; prints the number of cases first ("x" prints only that). Nothing without it.
pub fn probe_from_the_environment<M: MesaCompiler>(compiler: M) {
    let Ok(probe) = std::env::var("WCONTAINERS_PROBE") else { return };
    println!("CASES {CASES}");
    let parts: Vec<u64> = probe.split(',').filter_map(|part| part.parse().ok()).collect();
    if let [case, seed] = parts[..] {
        let calls = hostile_case(&compiler, usize::try_from(case).unwrap_or(usize::MAX), seed).map(|calls| calls.0);
        println!("PROBE {probe} ended, calls {calls:?}");
    }
}

#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn compiles_and_reads_back() {
            $crate::compiles_and_reads_back($value);
        }

        #[test]
        fn refuses_what_is_not_a_program() {
            $crate::refuses_what_is_not_a_program($value);
        }

        #[test]
        fn every_method_is_called() {
            $crate::every_method_is_called($value);
        }

        #[test]
        fn probe_from_the_environment() {
            $crate::probe_from_the_environment($value);
        }

        #[test]
        fn fuzz_from_the_environment() {
            $crate::fuzz_from_the_environment($value);
        }
    };
}
