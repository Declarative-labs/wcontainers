//! The tests every implementation of wcontainers-mesa-compiler runs, written against the trait alone. The trait's
//! shaders are typed by their place in the bridge's chain and the OpenGL driver's by their stage, so a caller can write
//! only the orders the bridge's callers take, on the stages they take them on (the doctests below show the others do
//! not compile); the tests walk every one of those orders: programs
//! compiled from GLSL and SPIR-V and read back in every form the trait gives; a hostile caller taking every walk from
//! every program with edge arguments at every step (values past every enumeration, empty and long lists, units past
//! every limit); and a seeded fuzzer of the same, from SPIR-V whose words were changed and GLSL whose numbers were.
//!
//! The orders that do not compile:
//!
//! ```compile_fail,E0308
//! // The flat form of a program not in the common form.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, read: M::SpirvShader) {
//!     let _ = compiler.spirv_common_flatten(read);
//! }
//! ```
//!
//! ```compile_fail,E0382
//! // The flat form twice of one program (it is taken by the first).
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, common: M::SpirvCommon) {
//!     let _ = compiler.spirv_common_flatten(common);
//!     let _ = compiler.spirv_common_flatten(common);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // The SPIR-V route's flat form of the OpenGL driver's variant.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, variant: M::Variant) {
//!     let _ = compiler.spirv_common_flatten(variant);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // The driver's steps at a creation on a program read from SPIR-V.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, read: M::SpirvShader) {
//!     let _ = compiler.received_vertex_prepare(read);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A draw's variant of a stage the driver's creation steps have not taken.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, received: &M::ReceivedVertex, settings: wcontainers_mesa_compiler::VariantSettings) {
//!     let _ = compiler.prepared_vertex_variant(received, settings, &[], &[]);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // Points as squares from a program the driver's steps made (a pass-through or a square).
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, made: &M::Made) {
//!     let _ = compiler.prepared_vertex_points_as_squares(made, 0, 1.0);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A geometry stage taken through the creation steps of a vertex stage.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, received: M::ReceivedGeometry) {
//!     let _ = compiler.received_vertex_prepare(received);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A fragment program's points drawn as squares (only a vertex or evaluation program draws them so).
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, prepared: &M::PreparedFragment) {
//!     let _ = compiler.prepared_vertex_points_as_squares(prepared, 0, 1.0);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A vertex program rewritten as a geometry program drawing squares.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, prepared: &M::PreparedVertex) {
//!     let _ = compiler.prepared_geometry_points_as_squares(prepared, 0, 1.0, 0);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A draw's variant of a tessellation control stage through the vertex stage's method.
//! fn order<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, prepared: &M::PreparedTessControl, settings: wcontainers_mesa_compiler::VariantSettings) {
//!     let _ = compiler.prepared_vertex_variant(prepared, settings, &[], &[]);
//! }
//! ```
//!
//! The orders the callers take compile:
//!
//! ```
//! fn candidate_b<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, words: &[u32], ranges: &[wcontainers_mesa_compiler::SlotRange; 4]) -> Option<wcontainers_mesa_compiler::Flat> {
//!     let read = compiler.spirv_read(words, 0, c"main", false)?;
//!     let _key = compiler.spirv_shader_key(&read)?;
//!     let common = compiler.spirv_to_common_form(read, 0, ranges).shader?;
//!     compiler.spirv_common_flatten(common)
//! }
//! fn draw<M: wcontainers_mesa_compiler::MesaCompiler>(compiler: &M, received: M::ReceivedVertex, settings: wcontainers_mesa_compiler::VariantSettings) -> Option<wcontainers_mesa_compiler::Flat> {
//!     let prepared = compiler.received_vertex_prepare(received).shader?;
//!     let _squares = compiler.prepared_vertex_points_as_squares(&prepared, 0, 64.0);
//!     let variant = compiler.prepared_vertex_variant(&prepared, settings, &[], &[]).shader?;
//!     compiler.variant_flatten(variant, 0)
//! }
//! ```

use std::ffi::{CStr, CString};

use wcontainers_mesa_compiler::{MesaCompiler, NearestSampler, PositionTransform, PreparedForm, SlotRange, Squares, TextureSwizzle, VariantForm, VariantSettings, constants};

/// The bridge's stages (enum wc_stage).
const VERTEX: u32 = constants::STAGE_VERTEX;
const FRAGMENT: u32 = constants::STAGE_FRAGMENT;
const COMPUTE: u32 = constants::STAGE_COMPUTE;
const GEOMETRY: u32 = constants::STAGE_GEOMETRY;
const TESS_CONTROL: u32 = constants::STAGE_TESS_CONTROL;
const TESS_EVALUATION: u32 = constants::STAGE_TESS_EVALUATION;
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

/// A tessellation evaluation program making triangles from the control program's patch, with a point size.
const EVALUATION_SOURCE: &str = "#version 450
layout(triangles, equal_spacing, ccw) in;
layout(location = 0) in vec4 handed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_TessCoord.x * gl_in[0].gl_Position + gl_TessCoord.y * gl_in[1].gl_Position + gl_TessCoord.z * gl_in[2].gl_Position;
    gl_PointSize = 4.0;
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

/// The SPIR-V route's walk (the corpus's, run.rs): a module read (alphaToOne made in a fragment program where the picks
/// say), its key, the common form, then the flat form (or nothing: the program dropped).
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
    if picks.below(3) == 0 {
        drop(common);
        return;
    }
    calls.one("spirv_common_flatten");
    let _ = calls.made("spirv_common_flatten", compiler.spirv_common_flatten(common));
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

/// One stage's methods (its creation steps, a draw's variant, its points drawn as squares where the stage draws them),
/// so that the walks are written once for every stage.
struct Stage<M: MesaCompiler, R, P> {
    /// The stage's number (enum wc_stage).
    number: u32,
    prepare: (&'static str, fn(&M, R) -> PreparedForm<P, M::Made>),
    variant: (&'static str, fn(&M, &P, VariantSettings, &[TextureSwizzle], &[NearestSampler]) -> VariantForm<M::Variant>),
    squares: Option<(&'static str, fn(&M, &P, &mut Picks) -> Squares<M::Made>)>,
}

fn vertex<M: MesaCompiler>() -> Stage<M, M::ReceivedVertex, M::PreparedVertex> {
    Stage {
        number: VERTEX,
        prepare: ("received_vertex_prepare", M::received_vertex_prepare),
        variant: ("prepared_vertex_variant", M::prepared_vertex_variant),
        squares: Some(("prepared_vertex_points_as_squares", |compiler, prepared, picks| compiler.prepared_vertex_points_as_squares(prepared, picks.of(&NUMBERS), picks.of(&FLOATS)))),
    }
}

fn fragment<M: MesaCompiler>() -> Stage<M, M::ReceivedFragment, M::PreparedFragment> {
    Stage { number: FRAGMENT, prepare: ("received_fragment_prepare", M::received_fragment_prepare), variant: ("prepared_fragment_variant", M::prepared_fragment_variant), squares: None }
}

fn compute<M: MesaCompiler>() -> Stage<M, M::ReceivedCompute, M::PreparedCompute> {
    Stage { number: COMPUTE, prepare: ("received_compute_prepare", M::received_compute_prepare), variant: ("prepared_compute_variant", M::prepared_compute_variant), squares: None }
}

fn geometry<M: MesaCompiler>() -> Stage<M, M::ReceivedGeometry, M::PreparedGeometry> {
    Stage {
        number: GEOMETRY,
        prepare: ("received_geometry_prepare", M::received_geometry_prepare),
        variant: ("prepared_geometry_variant", M::prepared_geometry_variant),
        squares: Some(("prepared_geometry_points_as_squares", |compiler, prepared, picks| {
            compiler.prepared_geometry_points_as_squares(prepared, picks.of(&NUMBERS), picks.of(&FLOATS), picks.of(&NUMBERS))
        })),
    }
}

fn tess_control<M: MesaCompiler>() -> Stage<M, M::ReceivedTessControl, M::PreparedTessControl> {
    Stage {
        number: TESS_CONTROL,
        prepare: ("received_tess_control_prepare", M::received_tess_control_prepare),
        variant: ("prepared_tess_control_variant", M::prepared_tess_control_variant),
        squares: None,
    }
}

fn tess_evaluation<M: MesaCompiler>() -> Stage<M, M::ReceivedTessEvaluation, M::PreparedTessEvaluation> {
    Stage {
        number: TESS_EVALUATION,
        prepare: ("received_tess_evaluation_prepare", M::received_tess_evaluation_prepare),
        variant: ("prepared_tess_evaluation_variant", M::prepared_tess_evaluation_variant),
        squares: Some(("prepared_tess_evaluation_points_as_squares", |compiler, prepared, picks| {
            compiler.prepared_tess_evaluation_points_as_squares(prepared, picks.of(&NUMBERS), picks.of(&FLOATS))
        })),
    }
}

/// Every method of the trait, each of which every walk of `every_method_is_called` must see make what it makes.
const METHODS: [&str; 24] = [
    "spirv_read",
    "spirv_shader_key",
    "spirv_to_common_form",
    "spirv_common_flatten",
    "glsl_to_nir",
    "received_vertex_prepare",
    "received_fragment_prepare",
    "received_compute_prepare",
    "received_geometry_prepare",
    "received_tess_control_prepare",
    "received_tess_evaluation_prepare",
    "prepared_vertex_variant",
    "prepared_fragment_variant",
    "prepared_compute_variant",
    "prepared_geometry_variant",
    "prepared_tess_control_variant",
    "prepared_tess_evaluation_variant",
    "prepared_vertex_points_as_squares",
    "prepared_geometry_points_as_squares",
    "prepared_tess_evaluation_points_as_squares",
    "made_key",
    "made_named_layer_location",
    "made_variant",
    "variant_flatten",
];

/// The OpenGL driver's walk on one stage (state.rs, draw.rs): its creation steps, its pass-through programs, draws of
/// it, and its points drawn as squares where the stage draws them.
fn stage_walk<M: MesaCompiler, R, P>(compiler: &M, stage: &Stage<M, R, P>, received: R, picks: &mut Picks, calls: &mut Calls) {
    calls.one(stage.prepare.0);
    let form = (stage.prepare.1)(compiler, received);
    let _ = calls.made(stage.prepare.0, form.shader.as_ref());
    for made in form.pass_through.iter().flatten() {
        made_walk(compiler, made, picks, calls);
    }
    let Some(prepared) = form.shader else { return };
    for _ in 0..1 + picks.below(3) {
        let (settings, swizzles, samplers) = (picks.settings(), picks.swizzles(), picks.samplers());
        calls.one(stage.variant.0);
        let variant = (stage.variant.1)(compiler, &prepared, settings, &swizzles, &samplers);
        let _ = calls.made(stage.variant.0, variant.shader.as_ref());
        flatten_variant(compiler, variant, picks, calls);
    }
    if let Some((method, squares)) = stage.squares
        && picks.flag()
    {
        calls.one(method);
        let squares = squares(compiler, &prepared, picks);
        let _ = calls.made(method, squares.geometry.as_ref());
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
    if let Some(received) = program.vertex {
        stage_walk(compiler, &vertex(), received, picks, calls);
    }
    if let Some(received) = program.tess_control {
        stage_walk(compiler, &tess_control(), received, picks, calls);
    }
    if let Some(received) = program.tess_evaluation {
        stage_walk(compiler, &tess_evaluation(), received, picks, calls);
    }
    if let Some(received) = program.geometry {
        stage_walk(compiler, &geometry(), received, picks, calls);
    }
    if let Some(received) = program.fragment {
        stage_walk(compiler, &fragment(), received, picks, calls);
    }
    if let Some(received) = program.compute {
        stage_walk(compiler, &compute(), received, picks, calls);
    }
}

/// One stage of a compiled program through the driver's creation steps and a draw's to the flat form; the stage is
/// there exactly where the program has its source.
fn stage_reads_back<M: MesaCompiler, R, P>(compiler: &M, stage: &Stage<M, R, P>, received: Option<R>, sources: Sources<'_>, settings: &VariantSettings) {
    let at = usize::try_from(stage.number).unwrap_or(usize::MAX);
    assert_eq!(received.is_some(), sources[at].is_some(), "a stage made for each source");
    let Some(received) = received else { return };
    let form = (stage.prepare.1)(compiler, received);
    assert!(!form.key.is_empty(), "the prepared stage has a key");
    assert!(!form.streams_elsewhere);
    if sources[at] == Some(STORING_VERTEX_SOURCE) {
        assert!(form.passes_through && form.pass_through.iter().any(Option::is_some), "a vertex program using storage passes through");
    }
    let prepared = form.shader.expect("the stage is prepared");
    let variant = (stage.variant.1)(compiler, &prepared, settings.clone(), &[], &[]);
    assert_eq!(variant.result, constants::VARIANT_MADE, "the variant is made");
    assert!(!variant.key.is_empty(), "the variant has a key");
    let points_in = if stage.number == TESS_CONTROL || stage.number == TESS_EVALUATION { CONTROL_POINTS } else { 0 };
    let flat = compiler.variant_flatten(variant.shader.expect("made"), points_in).unwrap_or_else(|| panic!("the variant of stage {at} flattens"));
    assert!(!flat.instrs.is_empty(), "the flat form has instructions");
}

/// Compiles a program of each stage, takes it through the driver's steps and a draw's to the flat form, and draws the
/// points of each stage that draws them as squares; reads each SPIR-V module and flattens it through the common form.
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
        stage_reads_back(&compiler, &vertex(), program.vertex, sources, &settings);
        stage_reads_back(&compiler, &fragment(), program.fragment, sources, &settings);
        stage_reads_back(&compiler, &compute(), program.compute, sources, &settings);
        stage_reads_back(&compiler, &geometry(), program.geometry, sources, &settings);
        stage_reads_back(&compiler, &tess_control(), program.tess_control, sources, &settings);
        stage_reads_back(&compiler, &tess_evaluation(), program.tess_evaluation, sources, &settings);
    }
    // A vertex program writing a point size, drawn as squares.
    let source = CString::new(VERTEX_SOURCE).expect("no zero byte");
    let program = compiler.glsl_to_nir([Some(&source), None, None, None, None, None]).expect("the program compiles");
    let received = program.vertex.expect("a vertex stage");
    let prepared = compiler.received_vertex_prepare(received).shader.expect("prepared");
    let squares = compiler.prepared_vertex_points_as_squares(&prepared, 1, 64.0);
    assert_eq!(squares.kind, constants::SQUARES_MADE, "points drawn as squares");
    let geometry = squares.geometry.expect("the squares' program");
    assert!(compiler.made_key(&geometry).is_some_and(|key| !key.is_empty()));
    // A geometry program making points, naming a layer, drawn as squares: the made program names the layer too.
    let owned = [POINT_VERTEX_SOURCE, SHADED_FRAGMENT_SOURCE, POINT_GEOMETRY_SOURCE].map(|text| CString::new(text).expect("no zero byte"));
    let program = compiler.glsl_to_nir([Some(&owned[0]), Some(&owned[1]), None, Some(&owned[2]), None, None]).expect("the program compiles");
    let received = program.geometry.expect("a geometry stage");
    let prepared = compiler.received_geometry_prepare(received).shader.expect("prepared");
    let squares = compiler.prepared_geometry_points_as_squares(&prepared, 1, 64.0, 0);
    assert_eq!(squares.kind, constants::SQUARES_MADE, "a geometry program's points drawn as squares");
    let geometry = squares.geometry.expect("the squares' program");
    assert!(compiler.made_key(&geometry).is_some_and(|key| !key.is_empty()));
    assert_ne!(compiler.made_named_layer_location(&geometry), u32::MAX, "the squares name the layer");
    // An evaluation program writing a point size, its points drawn as squares.
    let owned = [POINT_VERTEX_SOURCE, SHADED_FRAGMENT_SOURCE, CONTROL_SOURCE, EVALUATION_SOURCE].map(|text| CString::new(text).expect("no zero byte"));
    let program = compiler.glsl_to_nir([Some(&owned[0]), Some(&owned[1]), None, None, Some(&owned[2]), Some(&owned[3])]).expect("the program compiles");
    let received = program.tess_evaluation.expect("an evaluation stage");
    let prepared = compiler.received_tess_evaluation_prepare(received).shader.expect("prepared");
    let squares = compiler.prepared_tess_evaluation_points_as_squares(&prepared, 1, 64.0);
    assert_eq!(squares.kind, constants::SQUARES_MADE, "an evaluation program's points drawn as squares");
    assert!(squares.geometry.is_some(), "the squares' program");
    // SPIR-V for Vulkan read, its key taken, then the common form flattened.
    for (stage, words) in modules() {
        let read = compiler.spirv_read(&words, stage, MAIN, false).expect("the SPIR-V reads");
        assert!(compiler.spirv_shader_key(&read).is_some_and(|key| !key.is_empty()));
        let form = compiler.spirv_to_common_form(read, 0, &ranges);
        assert_eq!(form.result, constants::BIND_OK, "the common form binds");
        assert!(form.shader.is_some(), "in the common form");
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
    assert!(
        none.is_err()
            || none.is_ok_and(|program| {
                program.vertex.is_none()
                    && program.fragment.is_none()
                    && program.compute.is_none()
                    && program.geometry.is_none()
                    && program.tess_control.is_none()
                    && program.tess_evaluation.is_none()
            }),
        "a program of no stage has none"
    );
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
const ROUNDS: u64 = 16;

/// Every walk the trait allows, taken by a hostile caller from every program, with arguments picked by `ROUNDS`
/// seeds; nothing may end the process, and every method makes what it makes in some walk.
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
    let never: Vec<&str> = METHODS.iter().copied().filter(|method| calls.1.get(method).is_none_or(|(_, made)| *made == 0)).collect();
    assert!(never.is_empty(), "methods that made nothing: {never:?}");
    assert_eq!(calls.1.len(), METHODS.len(), "every method called is one of METHODS");
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
/// made, by method.
fn fuzz<M: MesaCompiler>(compiler: &M, seed: u64, calls: u64) -> Calls {
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
    made
}

/// The number the environment names, else `default`: a run asked for by name takes its input from the environment,
/// and the suite's own run tests with the default (never nothing).
fn given(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().map_or(default, |value| value.parse::<u64>().unwrap_or_else(|_| panic!("{name} names a number, not {value}")))
}

/// The seed and calls the suite's own run fuzzes with.
const FUZZ_SEED: u64 = 1;
const FUZZ_CALLS: u64 = 10_000;

/// Runs `fuzz` with the seed and count WCONTAINERS_FUZZ_SEED and WCONTAINERS_FUZZ_CALLS give (`FUZZ_SEED` and
/// `FUZZ_CALLS` where they give none); fails when a method made nothing.
pub fn fuzz_from_the_environment<M: MesaCompiler>(compiler: M) {
    let (seed, calls) = (given("WCONTAINERS_FUZZ_SEED", FUZZ_SEED), given("WCONTAINERS_FUZZ_CALLS", FUZZ_CALLS));
    let made = fuzz(&compiler, seed, calls);
    println!("FUZZ seed {seed} calls {}", made.0);
    let never: Vec<&str> = METHODS.iter().copied().filter(|method| made.1.get(method).is_none_or(|(_, made)| *made == 0)).collect();
    assert!(never.is_empty(), "methods that made nothing in seed {seed}: {never:?}");
}

/// Runs one hostile case alone, as WCONTAINERS_PROBE names it ("case,seed"), so that a case that ends the process is
/// found by itself; prints the number of cases first ("x" prints only that). Where it names none (the suite's own
/// run), every case with seed 0, one after another: each must be a case, run to its end, and make a call.
pub fn probe_from_the_environment<M: MesaCompiler>(compiler: M) {
    println!("CASES {CASES}");
    let Ok(probe) = std::env::var("WCONTAINERS_PROBE") else {
        for case in 0..CASES {
            let calls = hostile_case(&compiler, case, 0).map(|calls| calls.0);
            println!("PROBE {case},0 ended, calls {calls:?}");
            assert!(calls.is_some_and(|calls| calls > 0), "case {case} ran to its end and made a call");
        }
        assert!(hostile_case(&compiler, CASES, 0).is_none(), "no case past the last");
        return;
    };
    let parts: Vec<u64> = probe.split(',').filter_map(|part| part.parse().ok()).collect();
    if let [case, seed] = parts[..] {
        let calls = hostile_case(&compiler, usize::try_from(case).unwrap_or(usize::MAX), seed).map(|calls| calls.0);
        println!("PROBE {probe} ended, calls {calls:?}");
        assert!(calls.is_some(), "WCONTAINERS_PROBE names a case of the {CASES}");
    } else {
        assert_eq!(probe, "x", "WCONTAINERS_PROBE names a case and a seed, or x");
    }
}

/// A second fragment program of the oldest form (green), so that two fragment sources differ alone.
const GREEN_FRAGMENT_SOURCE: &str = "#version 110
void main() { gl_FragColor = vec4(0.0, 1.0, 0.0, 1.0); }
";

/// A second compute program, of a wider workgroup.
const WIDE_COMPUTE_SOURCE: &str = "#version 450
layout(local_size_x = 8) in;
layout(std430, binding = 0) buffer Results { uint values[]; };
void main() {
    values[gl_LocalInvocationID.x] = 2u * gl_LocalInvocationID.x;
}
";

/// The point geometry program making one copy, not two.
const ONE_POINT_GEOMETRY_SOURCE: &str = "#version 450
layout(points) in;
layout(points, max_vertices = 1) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_in[0].gl_Position;
    gl_PointSize = gl_in[0].gl_PointSize;
    gl_Layer = 0;
    shaded = passed[0];
    EmitVertex();
}
";

/// The control program with other levels.
const OTHER_CONTROL_SOURCE: &str = "#version 450
layout(vertices = 3) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 handed[];
void main() {
    handed[gl_InvocationID] = passed[gl_InvocationID];
    gl_out[gl_InvocationID].gl_Position = gl_in[gl_InvocationID].gl_Position;
    gl_TessLevelOuter[0] = 3.0;
    gl_TessLevelOuter[1] = 3.0;
    gl_TessLevelOuter[2] = 3.0;
    gl_TessLevelInner[0] = 3.0;
}
";

/// The evaluation program with another point size.
const OTHER_EVALUATION_SOURCE: &str = "#version 450
layout(triangles, equal_spacing, ccw) in;
layout(location = 0) in vec4 handed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_TessCoord.x * gl_in[0].gl_Position + gl_TessCoord.y * gl_in[1].gl_Position + gl_TessCoord.z * gl_in[2].gl_Position;
    gl_PointSize = 2.0;
    shaded = handed[1];
}
";

/// A fragment program reading its target's texel (framebuffer fetch), which a draw's variant reads from a texture.
const FETCHING_FRAGMENT_SOURCE: &str = "#version 450
#extension GL_EXT_shader_framebuffer_fetch_non_coherent : require
layout(location = 0, noncoherent) inout vec4 colour;
void main() {
    colour = colour * 0.5;
}
";

/// A fragment program interpolating at a sample, whose positions a draw's variant needs.
const SAMPLE_FRAGMENT_SOURCE: &str = "#version 450
layout(location = 0) in vec2 passed;
layout(location = 0) out vec4 colour;
void main() {
    colour = vec4(interpolateAtSample(passed, 1), 0.0, 1.0);
}
";

/// The settings of a plain draw: no transform, unit 0, one sample, set 0, wide ranges.
fn plain_settings() -> VariantSettings {
    let none = PositionTransform { flip_y: false, half_z: false, reverse_z: false, fixed_point_size: false };
    VariantSettings {
        transform: none.clone(),
        key_transform: none,
        fetch_unit: 0,
        fetch_multisampled: false,
        single_sampled: false,
        samples: 4,
        set: 0,
        ranges: [0, 1, 2, 3].map(|_| SlotRange { first: 0, slots: 64 }),
    }
}

/// What a variant is, as the trait shows it: its result, its key, its bindings, and its flat form.
type VariantView = (u32, Vec<u8>, Vec<wcontainers_mesa_compiler::Binding>, Option<wcontainers_mesa_compiler::Flat>);

fn variant_view<M: MesaCompiler>(compiler: &M, form: VariantForm<M::Variant>, points_in: u32) -> VariantView {
    let flat = form.shader.and_then(|variant| compiler.variant_flatten(variant, points_in));
    (form.result, form.key, form.bindings, flat)
}

/// Compiles the sources (each must compile).
fn compiled<M: MesaCompiler>(compiler: &M, sources: Sources<'_>) -> wcontainers_mesa_compiler::GlslProgram<M::ReceivedVertex, M::ReceivedFragment, M::ReceivedCompute, M::ReceivedGeometry, M::ReceivedTessControl, M::ReceivedTessEvaluation> {
    let owned: Vec<Option<CString>> = sources.iter().map(|source| source.map(|text| CString::new(text).expect("no zero byte"))).collect();
    let texts: [Option<&CStr>; STAGES] = std::array::from_fn(|at| owned[at].as_deref());
    compiler.glsl_to_nir(texts).unwrap_or_else(|log| panic!("the program compiles: {}", String::from_utf8_lossy(&log)))
}

/// What each argument changes, as the arguments-told-apart test found it: for each (method, argument), whether the
/// two values gave two results the test tells apart.
struct Told(Vec<(&'static str, &'static str, bool, String)>);

impl Told {
    /// Records one argument: told apart where the two results differ.
    fn apart<T: PartialEq + std::fmt::Debug>(&mut self, method: &'static str, argument: &'static str, first: &T, second: &T) {
        let apart = first != second;
        let detail = if apart { String::new() } else { format!("both {:?}", first).chars().take(200).collect() };
        self.0.push((method, argument, apart, detail));
    }
}

/// The prepared form of the stage's program in the sources, or None where it has none.
fn prepared_key<M: MesaCompiler>(compiler: &M, sources: Sources<'_>, stage: u32) -> Option<Vec<u8>> {
    let owned: Vec<Option<CString>> = sources.iter().map(|source| source.map(|text| CString::new(text).expect("no zero byte"))).collect();
    let texts: [Option<&CStr>; STAGES] = std::array::from_fn(|at| owned[at].as_deref());
    let program = compiler.glsl_to_nir(texts).ok()?;
    match stage {
        VERTEX => program.vertex.map(|received| compiler.received_vertex_prepare(received).key),
        FRAGMENT => program.fragment.map(|received| compiler.received_fragment_prepare(received).key),
        COMPUTE => program.compute.map(|received| compiler.received_compute_prepare(received).key),
        GEOMETRY => program.geometry.map(|received| compiler.received_geometry_prepare(received).key),
        TESS_CONTROL => program.tess_control.map(|received| compiler.received_tess_control_prepare(received).key),
        _ => program.tess_evaluation.map(|received| compiler.received_tess_evaluation_prepare(received).key),
    }
}

/// A draw's variant of a prepared program with each settings field, swizzle and sampler changed alone, for the method
/// `method` (`variant` calls it): every field the bridge reads for the program's stage told apart.
#[expect(clippy::too_many_arguments, reason = "the method, its call, the program and which fields its stage reads")]
fn settings_told<M: MesaCompiler, P>(
    compiler: &M,
    told: &mut Told,
    method: &'static str,
    prepared: &P,
    variant: fn(&M, &P, VariantSettings, &[TextureSwizzle], &[NearestSampler]) -> VariantForm<M::Variant>,
    positioned: bool,
    resources: bool,
    points_in: u32,
) {
    let view = |settings: VariantSettings| variant_view(compiler, variant(compiler, prepared, settings, &[], &[]), points_in);
    if positioned {
        // A transform field: the position the program writes changes, so its flat form (the key is compared apart: it
        // takes the key's own transform too).
        type Field = fn(&mut PositionTransform);
        let fields: [(&'static str, &'static str, Field); 4] = [
            ("settings.transform.flip_y", "settings.key_transform.flip_y", |t| t.flip_y = true),
            ("settings.transform.half_z", "settings.key_transform.half_z", |t| t.half_z = true),
            ("settings.transform.reverse_z", "settings.key_transform.reverse_z", |t| t.reverse_z = true),
            ("settings.transform.fixed_point_size", "settings.key_transform.fixed_point_size", |t| t.fixed_point_size = true),
        ];
        for (transform_name, key_name, set) in fields {
            let mut changed = plain_settings();
            set(&mut changed.transform);
            set(&mut changed.key_transform);
            told.apart(method, transform_name, &view(plain_settings()).3, &view(changed).3);
            // The key's transform alone: the key changes, the program does not.
            let mut keyed = plain_settings();
            set(&mut keyed.key_transform);
            told.apart(method, key_name, &view(plain_settings()).1, &view(keyed).1);
        }
    }
    if resources {
        let mut other_set = plain_settings();
        other_set.set = 5;
        told.apart(method, "settings.set", &view(plain_settings()), &view(other_set));
        let mut moved = plain_settings();
        moved.ranges = [0, 1, 2, 3].map(|_| SlotRange { first: 8, slots: 64 });
        told.apart(method, "settings.ranges[].first", &view(plain_settings()), &view(moved));
        let mut narrow = plain_settings();
        narrow.ranges = [0, 1, 2, 3].map(|_| SlotRange { first: 0, slots: 0 });
        told.apart(method, "settings.ranges[].slots", &view(plain_settings()).0, &view(narrow).0);
    }
}

/// For every argument of every method, two values that give two results the test tells apart (the check of f3af6417f
/// found three wrong arguments every test passed). Arguments that are a shader of the trait's own (a handle the glue
/// cannot replace) are told apart by two shaders. Every (method, argument) is printed; the test fails on any not told
/// apart.
pub fn every_argument_tells<M: MesaCompiler>(compiler: M) {
    let compiler = &compiler;
    let mut told = Told(Vec::new());
    let [(_, vertex_words), (_, fragment_words), (_, compute_words)] = modules();
    let key_of = |words: &[u32], stage: u32, alpha: bool| compiler.spirv_read(words, stage, MAIN, alpha).and_then(|read| compiler.spirv_shader_key(&read));
    // spirv_read.
    told.apart("spirv_read", "words", &key_of(&vertex_words, VERTEX, false), &key_of(&vertex_words[..0], VERTEX, false));
    told.apart("spirv_read", "stage", &key_of(&fragment_words, FRAGMENT, false).is_some(), &key_of(&fragment_words, VERTEX, false).is_some());
    let entry = |name: &CStr| compiler.spirv_read(&vertex_words, VERTEX, name, false).is_some();
    told.apart("spirv_read", "entry_point", &entry(MAIN), &entry(c"absent"));
    told.apart("spirv_read", "alpha_to_one", &key_of(&fragment_words, FRAGMENT, false), &key_of(&fragment_words, FRAGMENT, true));
    // spirv_shader_key: its shader, two shaders.
    told.apart("spirv_shader_key", "shader", &key_of(&vertex_words, VERTEX, false), &key_of(&compute_words, COMPUTE, false));
    // spirv_to_common_form and spirv_common_flatten.
    let wide = [0, 1, 2, 3].map(|_| SlotRange { first: 0, slots: 64 });
    let common = |words: &[u32], stage: u32, set: u32, ranges: &[SlotRange; 4]| {
        let read = compiler.spirv_read(words, stage, MAIN, false).expect("the SPIR-V reads");
        let form = compiler.spirv_to_common_form(read, set, ranges);
        let flat = form.shader.and_then(|common| compiler.spirv_common_flatten(common));
        (form.result, form.bindings, flat)
    };
    told.apart("spirv_to_common_form", "taken", &common(&vertex_words, VERTEX, 0, &wide), &common(&compute_words, COMPUTE, 0, &wide));
    told.apart("spirv_to_common_form", "set", &common(&vertex_words, VERTEX, 0, &wide), &common(&vertex_words, VERTEX, 5, &wide));
    let moved = [0, 1, 2, 3].map(|_| SlotRange { first: 8, slots: 64 });
    told.apart("spirv_to_common_form", "ranges[].first", &common(&vertex_words, VERTEX, 0, &wide).1, &common(&vertex_words, VERTEX, 0, &moved).1);
    let none = [0, 1, 2, 3].map(|_| SlotRange { first: 0, slots: 0 });
    told.apart("spirv_to_common_form", "ranges[].slots", &common(&vertex_words, VERTEX, 0, &wide).0, &common(&vertex_words, VERTEX, 0, &none).0);
    told.apart("spirv_common_flatten", "taken", &common(&vertex_words, VERTEX, 0, &wide).2, &common(&fragment_words, FRAGMENT, 0, &wide).2);
    // glsl_to_nir: each stage's source, two sources of that stage alone different; the received program's key told
    // apart (and so received_*_prepare's `taken`, two programs of its stage).
    let pairs: [(&'static str, &'static str, u32, Sources<'static>, Sources<'static>); 6] = [
        ("sources[vertex]", "received_vertex_prepare", VERTEX, [Some(OLD_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None, None, None, None], [Some(STORING_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None, None, None, None]),
        ("sources[fragment]", "received_fragment_prepare", FRAGMENT, [Some(OLD_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None, None, None, None], [Some(OLD_VERTEX_SOURCE), Some(GREEN_FRAGMENT_SOURCE), None, None, None, None]),
        ("sources[compute]", "received_compute_prepare", COMPUTE, [None, None, Some(COMPUTE_SOURCE), None, None, None], [None, None, Some(WIDE_COMPUTE_SOURCE), None, None, None]),
        ("sources[geometry]", "received_geometry_prepare", GEOMETRY, [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(POINT_GEOMETRY_SOURCE), None, None], [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(ONE_POINT_GEOMETRY_SOURCE), None, None]),
        ("sources[tess_control]", "received_tess_control_prepare", TESS_CONTROL, [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(CONTROL_SOURCE), Some(EVALUATION_SOURCE)], [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(OTHER_CONTROL_SOURCE), Some(EVALUATION_SOURCE)]),
        ("sources[tess_evaluation]", "received_tess_evaluation_prepare", TESS_EVALUATION, [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(CONTROL_SOURCE), Some(EVALUATION_SOURCE)], [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(CONTROL_SOURCE), Some(OTHER_EVALUATION_SOURCE)]),
    ];
    for (argument, prepare, stage, first, second) in pairs {
        let (one, two) = (prepared_key(compiler, first, stage), prepared_key(compiler, second, stage));
        told.apart("glsl_to_nir", argument, &one, &two);
        told.apart(prepare, "taken", &one, &two);
    }
    // A draw's variant on each stage: the shader (two programs), every settings field the bridge reads for the stage,
    // the swizzles and the samplers (fragment), and the flat form's points_in (tessellation).
    let program = compiled(compiler, [Some(VERTEX_SOURCE), Some(FRAGMENT_SOURCE), None, None, None, None]);
    let vertex_prepared = compiler.received_vertex_prepare(program.vertex.expect("a vertex stage")).shader.expect("prepared");
    let fragment_prepared = compiler.received_fragment_prepare(program.fragment.expect("a fragment stage")).shader.expect("prepared");
    let old = compiled(compiler, [Some(OLD_VERTEX_SOURCE), Some(OLD_FRAGMENT_SOURCE), None, None, None, None]);
    let old_vertex = compiler.received_vertex_prepare(old.vertex.expect("a vertex stage")).shader.expect("prepared");
    let old_fragment = compiler.received_fragment_prepare(old.fragment.expect("a fragment stage")).shader.expect("prepared");
    let plain = |form: VariantForm<M::Variant>| variant_view(compiler, form, 0);
    told.apart("prepared_vertex_variant", "shader", &plain(compiler.prepared_vertex_variant(&vertex_prepared, plain_settings(), &[], &[])), &plain(compiler.prepared_vertex_variant(&old_vertex, plain_settings(), &[], &[])));
    told.apart("prepared_fragment_variant", "shader", &plain(compiler.prepared_fragment_variant(&fragment_prepared, plain_settings(), &[], &[])), &plain(compiler.prepared_fragment_variant(&old_fragment, plain_settings(), &[], &[])));
    settings_told(compiler, &mut told, "prepared_vertex_variant", &vertex_prepared, M::prepared_vertex_variant, true, true, 0);
    settings_told(compiler, &mut told, "prepared_fragment_variant", &fragment_prepared, M::prepared_fragment_variant, false, true, 0);
    // The fragment program's own settings: one sample (its sample mask written or not), the sample count (positions
    // for interpolating at a sample), the fetched target's unit and form.
    let fragment_view = |prepared: &M::PreparedFragment, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]| plain(compiler.prepared_fragment_variant(prepared, settings, swizzles, samplers));
    let mut single = plain_settings();
    single.single_sampled = true;
    told.apart("prepared_fragment_variant", "settings.single_sampled", &fragment_view(&fragment_prepared, plain_settings(), &[], &[]), &fragment_view(&fragment_prepared, single, &[], &[]));
    let sample_program = compiled(compiler, [Some(VERTEX_SOURCE), Some(SAMPLE_FRAGMENT_SOURCE), None, None, None, None]);
    let sampling = compiler.received_fragment_prepare(sample_program.fragment.expect("a fragment stage")).shader.expect("prepared");
    let mut eight = plain_settings();
    eight.samples = 8;
    told.apart("prepared_fragment_variant", "settings.samples", &fragment_view(&sampling, plain_settings(), &[], &[]), &fragment_view(&sampling, eight, &[], &[]));
    let fetch_program = compiled(compiler, [Some(OLD_VERTEX_SOURCE), Some(FETCHING_FRAGMENT_SOURCE), None, None, None, None]);
    let fetching = compiler.received_fragment_prepare(fetch_program.fragment.expect("a fragment stage")).shader.expect("prepared");
    let mut other_unit = plain_settings();
    other_unit.fetch_unit = 3;
    told.apart("prepared_fragment_variant", "settings.fetch_unit", &fragment_view(&fetching, plain_settings(), &[], &[]), &fragment_view(&fetching, other_unit, &[], &[]));
    let mut multisampled = plain_settings();
    multisampled.fetch_multisampled = true;
    told.apart("prepared_fragment_variant", "settings.fetch_multisampled", &fragment_view(&fetching, plain_settings(), &[], &[]), &fragment_view(&fetching, multisampled, &[], &[]));
    // The swizzles: the picture at unit 0, swizzled or not, at its unit or another.
    let turned = [constants::SWIZZLE_Z, constants::SWIZZLE_Y, constants::SWIZZLE_X, constants::SWIZZLE_W].map(|value| u8::try_from(value).unwrap_or(0));
    let same = [constants::SWIZZLE_X, constants::SWIZZLE_Y, constants::SWIZZLE_Z, constants::SWIZZLE_W].map(|value| u8::try_from(value).unwrap_or(0));
    let swizzled = |unit: u32, swizzle: [u8; 4]| fragment_view(&fragment_prepared, plain_settings(), &[TextureSwizzle { unit, swizzle }], &[]);
    told.apart("prepared_fragment_variant", "swizzles[].unit", &swizzled(0, turned), &swizzled(7, turned));
    told.apart("prepared_fragment_variant", "swizzles[].swizzle", &swizzled(0, same), &swizzled(0, turned));
    // The samplers: the integer texture `counts` at unit 1, sampled nearest.
    let clamp = u8::try_from(constants::WRAP_CLAMP_TO_EDGE).unwrap_or(0);
    let repeat = u8::try_from(constants::WRAP_REPEAT).unwrap_or(0);
    let border = u8::try_from(constants::WRAP_CLAMP_TO_BORDER).unwrap_or(0);
    let sampler = NearestSampler { unit: 1, wrap: [clamp; 3], mipmapped: true, min_lod: 0.0, max_lod: 4.0, border: [0; 4] };
    let sampled = |sampler: NearestSampler| fragment_view(&fragment_prepared, plain_settings(), &[], &[sampler]);
    told.apart("prepared_fragment_variant", "samplers[].unit", &sampled(sampler.clone()), &sampled(NearestSampler { unit: 6, ..sampler.clone() }));
    told.apart("prepared_fragment_variant", "samplers[].wrap", &sampled(sampler.clone()), &sampled(NearestSampler { wrap: [repeat; 3], ..sampler.clone() }));
    told.apart("prepared_fragment_variant", "samplers[].mipmapped", &sampled(sampler.clone()), &sampled(NearestSampler { mipmapped: false, ..sampler.clone() }));
    told.apart("prepared_fragment_variant", "samplers[].min_lod", &sampled(sampler.clone()), &sampled(NearestSampler { min_lod: 2.0, ..sampler.clone() }));
    told.apart("prepared_fragment_variant", "samplers[].max_lod", &sampled(sampler.clone()), &sampled(NearestSampler { max_lod: 1.0, ..sampler.clone() }));
    let bordered = NearestSampler { wrap: [border; 3], ..sampler.clone() };
    told.apart("prepared_fragment_variant", "samplers[].border", &sampled(bordered.clone()), &sampled(NearestSampler { border: [1, 2, 3, 4], ..bordered }));
    // The other stages' variants.
    let compute_program = compiled(compiler, [None, None, Some(COMPUTE_SOURCE), None, None, None]);
    let compute_prepared = compiler.received_compute_prepare(compute_program.compute.expect("a compute stage")).shader.expect("prepared");
    let wide_program = compiled(compiler, [None, None, Some(WIDE_COMPUTE_SOURCE), None, None, None]);
    let wide_prepared = compiler.received_compute_prepare(wide_program.compute.expect("a compute stage")).shader.expect("prepared");
    told.apart("prepared_compute_variant", "shader", &plain(compiler.prepared_compute_variant(&compute_prepared, plain_settings(), &[], &[])), &plain(compiler.prepared_compute_variant(&wide_prepared, plain_settings(), &[], &[])));
    settings_told(compiler, &mut told, "prepared_compute_variant", &compute_prepared, M::prepared_compute_variant, false, true, 0);
    let geometry_program = compiled(compiler, [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(POINT_GEOMETRY_SOURCE), None, None]);
    let geometry_prepared = compiler.received_geometry_prepare(geometry_program.geometry.expect("a geometry stage")).shader.expect("prepared");
    let one_program = compiled(compiler, [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, Some(ONE_POINT_GEOMETRY_SOURCE), None, None]);
    let one_prepared = compiler.received_geometry_prepare(one_program.geometry.expect("a geometry stage")).shader.expect("prepared");
    told.apart("prepared_geometry_variant", "shader", &plain(compiler.prepared_geometry_variant(&geometry_prepared, plain_settings(), &[], &[])), &plain(compiler.prepared_geometry_variant(&one_prepared, plain_settings(), &[], &[])));
    settings_told(compiler, &mut told, "prepared_geometry_variant", &geometry_prepared, M::prepared_geometry_variant, true, false, 0);
    let tess = |control: &'static str, evaluation: &'static str| compiled(compiler, [Some(POINT_VERTEX_SOURCE), Some(SHADED_FRAGMENT_SOURCE), None, None, Some(control), Some(evaluation)]);
    let (first, second) = (tess(CONTROL_SOURCE, EVALUATION_SOURCE), tess(OTHER_CONTROL_SOURCE, OTHER_EVALUATION_SOURCE));
    let control = compiler.received_tess_control_prepare(first.tess_control.expect("a control stage")).shader.expect("prepared");
    let other_control = compiler.received_tess_control_prepare(second.tess_control.expect("a control stage")).shader.expect("prepared");
    let evaluation = compiler.received_tess_evaluation_prepare(first.tess_evaluation.expect("an evaluation stage")).shader.expect("prepared");
    let other_evaluation = compiler.received_tess_evaluation_prepare(second.tess_evaluation.expect("an evaluation stage")).shader.expect("prepared");
    let tessellated = |form: VariantForm<M::Variant>| variant_view(compiler, form, CONTROL_POINTS);
    told.apart("prepared_tess_control_variant", "shader", &tessellated(compiler.prepared_tess_control_variant(&control, plain_settings(), &[], &[])), &tessellated(compiler.prepared_tess_control_variant(&other_control, plain_settings(), &[], &[])));
    told.apart("prepared_tess_evaluation_variant", "shader", &tessellated(compiler.prepared_tess_evaluation_variant(&evaluation, plain_settings(), &[], &[])), &tessellated(compiler.prepared_tess_evaluation_variant(&other_evaluation, plain_settings(), &[], &[])));
    settings_told(compiler, &mut told, "prepared_tess_evaluation_variant", &evaluation, M::prepared_tess_evaluation_variant, true, false, CONTROL_POINTS);
    // variant_flatten: its variant (two programs) and the control points a tessellation program takes.
    let flattened = |points_in: u32| compiler.prepared_tess_control_variant(&control, plain_settings(), &[], &[]).shader.and_then(|variant| compiler.variant_flatten(variant, points_in));
    told.apart("variant_flatten", "points_in", &flattened(CONTROL_POINTS), &flattened(CONTROL_POINTS + 1));
    told.apart("variant_flatten", "taken", &flattened(CONTROL_POINTS), &compiler.prepared_tess_control_variant(&other_control, plain_settings(), &[], &[]).shader.and_then(|variant| compiler.variant_flatten(variant, CONTROL_POINTS)));
    // Points as squares: the slot and the largest size, the block a geometry program's size is read from; the made
    // programs told apart by their keys.
    let square_keys = |squares: Squares<M::Made>| (squares.kind, squares.before.and_then(|made| compiler.made_key(&made)), squares.geometry.and_then(|made| compiler.made_key(&made)));
    told.apart("prepared_vertex_points_as_squares", "vs", &square_keys(compiler.prepared_vertex_points_as_squares(&vertex_prepared, 1, 64.0)), &square_keys(compiler.prepared_vertex_points_as_squares(&old_vertex, 1, 64.0)));
    told.apart("prepared_vertex_points_as_squares", "at", &square_keys(compiler.prepared_vertex_points_as_squares(&vertex_prepared, 1, 64.0)), &square_keys(compiler.prepared_vertex_points_as_squares(&vertex_prepared, 2, 64.0)));
    told.apart("prepared_vertex_points_as_squares", "largest", &square_keys(compiler.prepared_vertex_points_as_squares(&vertex_prepared, 1, 64.0)), &square_keys(compiler.prepared_vertex_points_as_squares(&vertex_prepared, 1, 8.0)));
    told.apart("prepared_tess_evaluation_points_as_squares", "vs", &square_keys(compiler.prepared_tess_evaluation_points_as_squares(&evaluation, 1, 64.0)), &square_keys(compiler.prepared_tess_evaluation_points_as_squares(&other_evaluation, 1, 64.0)));
    told.apart("prepared_tess_evaluation_points_as_squares", "at", &square_keys(compiler.prepared_tess_evaluation_points_as_squares(&evaluation, 1, 64.0)), &square_keys(compiler.prepared_tess_evaluation_points_as_squares(&evaluation, 2, 64.0)));
    told.apart("prepared_tess_evaluation_points_as_squares", "largest", &square_keys(compiler.prepared_tess_evaluation_points_as_squares(&evaluation, 1, 64.0)), &square_keys(compiler.prepared_tess_evaluation_points_as_squares(&evaluation, 1, 8.0)));
    told.apart("prepared_geometry_points_as_squares", "gs", &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 64.0, 0)), &square_keys(compiler.prepared_geometry_points_as_squares(&one_prepared, 1, 64.0, 0)));
    told.apart("prepared_geometry_points_as_squares", "at", &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 64.0, 0)), &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 2, 64.0, 0)));
    told.apart("prepared_geometry_points_as_squares", "largest", &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 64.0, 0)), &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 8.0, 0)));
    told.apart("prepared_geometry_points_as_squares", "block", &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 64.0, 0)), &square_keys(compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 64.0, 1)));
    // The made programs: the vertex program's squares and the geometry program's (which names a layer).
    let vertex_squares = compiler.prepared_vertex_points_as_squares(&vertex_prepared, 1, 64.0).geometry.expect("the vertex program's squares");
    let geometry_squares = compiler.prepared_geometry_points_as_squares(&geometry_prepared, 1, 64.0, 0).geometry.expect("the geometry program's squares");
    told.apart("made_key", "shader", &compiler.made_key(&vertex_squares), &compiler.made_key(&geometry_squares));
    told.apart("made_named_layer_location", "shader", &compiler.made_named_layer_location(&vertex_squares), &compiler.made_named_layer_location(&geometry_squares));
    told.apart("made_variant", "shader", &plain(compiler.made_variant(&vertex_squares, plain_settings(), &[], &[])), &plain(compiler.made_variant(&geometry_squares, plain_settings(), &[], &[])));
    settings_told(compiler, &mut told, "made_variant", &vertex_squares, M::made_variant, true, true, 0);
    let mut missed = 0;
    for (method, argument, apart, detail) in &told.0 {
        if *apart {
            println!("ARGUMENT {method} {argument}: told apart");
        } else {
            missed += 1;
            println!("ARGUMENT {method} {argument}: NOT TOLD APART ({detail})");
        }
    }
    println!("ARGUMENTS {} told apart of {}", told.0.len() - missed, told.0.len());
    assert_eq!(missed, 0, "arguments whose two values gave one result");
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
        fn every_argument_tells() {
            $crate::every_argument_tells($value);
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
