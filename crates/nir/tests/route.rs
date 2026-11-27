//! Both candidates end to end: programs naga writes as SPIR-V read for Vulkan, NIR's serialised key,
//! the common form's step, then Zink's SPIR-V and naga's reader (A) or NIR read straight into naga
//! (B), each validated; and GLSL programs as the OpenGL driver takes them (received, prepared, a
//! draw's variant) through route B.
#![cfg(all(mesa, feature = "standalone"))]

use wcontainers_gpu::{reflect::SLOT_KINDS, reflect::SlotKind, shader::ShaderStage};
use wcontainers_mesa_compiler::Flat;
use wcontainers_nir::ConvertError;
use wcontainers_nir::bind::stage_set;
use wcontainers_nir::mesa::{
    Assembled, DrawSettings, Flattened, GlslNameKind, NearestSampler, PositionTransform, Squares,
    Stage, Wrap,
};
use wcontainers_shaders::ENTRY_POINT;

/// Mesa's compiler through our bridge.
type Bridge = wcontainers_mesa_compiler_direct::Bridge;
/// A program read from SPIR-V, owned by the compiler.
type SpirvProgram = wcontainers_nir::mesa::SpirvProgram<Bridge>;
/// One stage of a GLSL program as the driver receives it, owned the same way.
type GlslStage = wcontainers_nir::mesa::GlslStage<Bridge>;
/// A stage the driver prepared.
type Prepared = wcontainers_nir::mesa::Prepared<Bridge>;
/// A program the driver's steps made.
type Made = wcontainers_nir::mesa::Made<Bridge>;

const COMPUTE: &str = "
@group(0) @binding(0) var<storage, read_write> data: array<u32>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] * 3u + 1u;
}
";

fn spirv(wgsl: &str) -> Vec<u32> {
    let module = naga::front::wgsl::parse_str(wgsl).expect("the test program parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the test program validates");
    naga::back::spv::write_vec(&module, &info, &naga::back::spv::Options::default(), None)
        .expect("naga writes SPIR-V")
}

const SAMPLED: &str = "
@group(2) @binding(0) var<uniform> tint: vec4<f32>;
@group(0) @binding(3) var picture: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;
@fragment
fn main(@location(0) at: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(picture, nearest, at) * tint;
}
";

const TRANSLUCENT: &str = "
@fragment
fn main(@location(0) colour: vec4<f32>) -> @location(0) vec4<f32> {
    return colour;
}
";

/// A fragment program passing its colour on and writing a sample mask (spirv/masked.frag): from glslang, since
/// naga writes the mask as a scalar, which Vulkan's SPIR-V (and so the bridge's validator) refuses.
const MASKED: &[u8] = include_bytes!("spirv/masked.spv");

/// SPIR-V's words from its bytes, little-endian as glslang writes them.
fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

/// The sample mask among a fragment entry point's results, and whether colour 0's alpha is the
/// constant one.
fn sample_mask_and_alpha_one(module: &naga::Module) -> (bool, bool) {
    let entry = &module.entry_points[0].function;
    let result = entry.result.as_ref().expect("a fragment program returns");
    let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
        panic!("route B returns a struct");
    };
    let mask = members
        .iter()
        .filter(|m| m.binding == Some(naga::Binding::BuiltIn(naga::BuiltIn::SampleMask)))
        .count();
    let one = entry
        .expressions
        .iter()
        .any(|(_, e)| matches!(e, naga::Expression::Literal(naga::Literal::F32(v)) if *v == 1.0));
    (mask == 1, one)
}

const GLSL_VERTEX: &str = "#version 450
layout(location = 0) in vec4 corner;
layout(location = 1) in vec2 at;
uniform mat4 transform;
uniform float scale[3];
layout(std140, binding = 2) uniform Light { vec4 colour; vec3 direction; } light;
layout(std430, binding = 1) readonly buffer Offsets { vec4 offsets[]; };
out vec2 texel;
out vec4 shade;
void main() {
    gl_Position = transform * corner * scale[gl_VertexID % 3] + offsets[gl_InstanceID];
    texel = at;
    shade = light.colour * max(dot(light.direction, corner.xyz), 0.0);
}
";

const GLSL_FRAGMENT: &str = "#version 450
in vec2 texel;
in vec4 shade;
uniform sampler2D picture;
layout(std430, binding = 0) buffer Count { uint hits; float last[]; };
layout(location = 0) out vec4 colour;
void main() {
    colour = texture(picture, texel) * shade;
    atomicAdd(hits, 1u);
    last[int(gl_FragCoord.x)] = colour.a;
}
";

const PARTLY_READ: &str = "#version 450
layout(location = 0) in mat4 columns;
layout(location = 4) in vec4 corner;
void main() { gl_Position = corner + columns[1] + columns[3]; }
";

const UNPOSITIONED: &str = "#version 450
layout(location = 0) in vec4 particle;
layout(location = 0) out vec4 moved;
void main() { moved = particle * 2.0; }
";
const UNPOSITIONED_FRAGMENT: &str = "#version 450
layout(location = 0) in vec4 moved;
layout(location = 0) out vec4 colour;
void main() { colour = moved; }
";

const POSITIONED: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 moved;
uniform mat4 transform;
void main() {
    gl_Position = transform * at + vec4(at.w * 0.5);
    moved = at;
}
";

const INVARIANT_POSITIONED: &str = "#version 450
invariant gl_Position;
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 moved;
uniform mat4 transform;
void main() {
    gl_Position = transform * at + vec4(at.w * 0.5);
    moved = at;
}
";

const SUBROUTINES: &str = "#version 450
subroutine vec4 Shade(vec4 c);
subroutine(Shade) vec4 keep(vec4 c) { return c; }
subroutine(Shade) vec4 invert(vec4 c) { return 1.0 - c; }
subroutine uniform Shade shades[2];
uniform vec4 base;
layout(location = 0) out vec4 colour;
void main() { colour = shades[0](base) + shades[1](base); }
";

const IMAGE_BUFFERS: &str = "#version 450
layout(local_size_x = 4) in;
layout(r32ui, binding = 0) uniform uimageBuffer counts;
layout(rgba8, binding = 1) uniform writeonly imageBuffer colours;
layout(r32i, binding = 2) uniform iimageBuffer lowest;
void main() {
    int at = int(gl_GlobalInvocationID.x);
    uint before = imageAtomicAdd(counts, at, 1u);
    imageAtomicCompSwap(counts, at + 1, before, 7u);
    imageAtomicMin(lowest, at, -at);
    if (at < imageSize(counts)) {
        imageStore(colours, at, vec4(imageLoad(counts, at)) / 255.0);
    }
}
";

const SIGNED_AND_UNSIGNED_ATOMICS: [&str; 2] = [
    "#version 310 es
layout(local_size_x = 4) in;
layout(std430, binding = 0) buffer Unsigned { uint words[4]; };
layout(std430, binding = 1) buffer Signed { int data[4]; } signed_words;
uniform uint given[2];
void main() {
    uint at = gl_GlobalInvocationID.x;
    atomicMin(words[at], given[0]);
    atomicMax(words[at], given[1]);
    atomicMin(signed_words.data[at], 1);
    atomicMax(signed_words.data[at], 2);
}
",
    "#version 310 es
layout(local_size_x = 4) in;
layout(std430, binding = 0) buffer Output { uint words[4]; int signed_words[4]; };
shared uint shared_words[4];
shared int shared_signed[4];
uniform uint given[2];
void main() {
    uint at = gl_LocalInvocationIndex;
    atomicExchange(shared_words[at], given[0]);
    atomicMin(shared_words[at], given[1]);
    atomicExchange(shared_signed[at], 3);
    atomicMin(shared_signed[at], 1);
    atomicMax(shared_signed[at], 2);
    words[at] = shared_words[at];
    signed_words[at] = shared_signed[at];
}
",
];

/// Blocks of one layout keep their own words' kinds; workgroup memory, one array of words, takes
/// the other signedness's minimum and maximum by a compare-exchange loop.
const IMAGE_ATOMICS: &str = "#version 450
layout(local_size_x = 4) in;
layout(r32ui, binding = 0) uniform uimage2D counts;
layout(r32i, binding = 1) uniform iimage2DArray lowest;
layout(r32f, binding = 2) uniform image3D swapped;
layout(rgba8, binding = 3) uniform writeonly image2D colours;
void main() {
    ivec2 at = ivec2(gl_GlobalInvocationID.xy);
    uint before = imageAtomicAdd(counts, at, 1u);
    imageAtomicCompSwap(counts, at + 1, before, 7u);
    imageAtomicMin(lowest, ivec3(at, 2), -at.x);
    imageAtomicExchange(swapped, ivec3(at, 1), 0.5);
    ivec3 size = imageSize(lowest);
    if (at.x < size.x && at.y < imageSize(swapped).z) {
        imageStore(colours, at, vec4(imageLoad(counts, at)) / 255.0);
    }
}
";

const INTEGER_CUBE_ARRAY: &str = "#version 450
layout(binding = 0) uniform isamplerCubeArray cubes;
layout(location = 0) in vec4 at;
layout(location = 0) out ivec4 colour;
void main() {
    colour = texture(cubes, at) + textureGather(cubes, at) + ivec4(textureSize(cubes, 0), 0);
}
";

/// The program above without its gather.
const INTEGER_CUBE_ARRAY_SAMPLED: &str = "#version 450
layout(binding = 0) uniform isamplerCubeArray cubes;
layout(location = 0) in vec4 at;
layout(location = 0) out ivec4 colour;
void main() {
    colour = texture(cubes, at) + ivec4(textureSize(cubes, 0), 0);
}
";

const ELSEWHERE: &str = "#version 450
layout(location = 0) in vec4 smooth_value;
layout(location = 1) noperspective in vec2 linear_value;
layout(location = 0) out vec4 colour;
void main() {
    colour = interpolateAtOffset(smooth_value, vec2(0.25, -0.125))
        + interpolateAtSample(smooth_value, gl_SampleID).yxwz
        + vec4(interpolateAtOffset(linear_value, gl_SamplePosition - 0.5), smooth_value.xy);
}
";

/// A fragment program's inputs at locations and where in the pixel the host interpolates each.
fn fragment_inputs(module: &naga::Module) -> Vec<(u32, Option<naga::Sampling>)> {
    let entry = module.entry_points.first().unwrap();
    let bindings = entry
        .function
        .arguments
        .iter()
        .flat_map(|a| match a.binding {
            Some(ref binding) => vec![binding.clone()],
            None => match &module.types[a.ty].inner {
                naga::TypeInner::Struct { members, .. } => {
                    members.iter().filter_map(|m| m.binding.clone()).collect()
                }
                _ => Vec::new(),
            },
        });
    let mut inputs: Vec<_> = bindings
        .filter_map(|b| match b {
            naga::Binding::Location {
                location, sampling, ..
            } => Some((location, sampling)),
            naga::Binding::BuiltIn(_) => None,
        })
        .collect();
    inputs.sort_by_key(|&(location, _)| location);
    inputs
}

const EARLY_TESTS: &str = "#version 450
layout(early_fragment_tests) in;
layout(r32ui, binding = 0) uniform uimage2D counts;
layout(location = 0) out vec4 colour;
void main() {
    imageAtomicAdd(counts, ivec2(gl_FragCoord.xy), 1u);
    colour = vec4(1.0);
}
";

const CENTROID_ELSEWHERE: &str = "#version 450
layout(location = 0) centroid in vec2 moved;
layout(location = 1) centroid in vec2 only_centroid;
layout(location = 2) in vec2 asked_at_centroid;
layout(location = 0) out vec4 colour;
void main() {
    colour = vec4(floor(moved) + interpolateAtOffset(moved, only_centroid), only_centroid)
        + interpolateAtCentroid(asked_at_centroid).xyxy + asked_at_centroid.yxyx;
}
";

const AT_OWN_SAMPLE: &str = "#version 450
layout(location = 0) in vec4 only_there;
layout(location = 1) in vec4 also_here;
layout(location = 0) out vec4 colour;
void main() {
    colour = interpolateAtSample(only_there, gl_SampleID)
        + interpolateAtSample(also_here, gl_SampleID) + also_here;
}
";

const FORMATLESS: &str = "#version 450
layout(local_size_x = 4) in;
layout(rgba8, binding = 0) uniform writeonly image2D formatted;
layout(binding = 1) uniform writeonly image2D unformatted;
void main() {
    ivec2 at = ivec2(gl_GlobalInvocationID.xy);
    imageStore(formatted, at, vec4(0.5));
    imageStore(unformatted, at, vec4(0.25));
}
";

const INTEGER_ARRAY: &str = "#version 450
layout(binding = 0) uniform isampler2D textures[2];
layout(location = 0) in vec2 at;
layout(location = 1) flat in int which;
layout(location = 0) out ivec4 colour;
void main() {
    colour = texture(textures[which], at);
}
";

const ARRAYS_OF_ARRAYS: &str = "#version 450
layout(binding = 0) uniform sampler2D textures[2][3];
layout(location = 0) in vec2 at;
layout(location = 1) flat in int which;
layout(location = 0) out vec4 colour;
void main() {
    colour = texture(textures[1][2], at) + texture(textures[which][1], at);
}
";

const LAYER_READ: &str = "#version 450
layout(location = 0) in vec4 colour;
layout(location = 0) out vec4 result;
void main() {
    result = colour * float(gl_Layer);
}
";

const SIZED_POINTS: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 colour;
layout(location = 1) flat out ivec2 cell;
void main() {
    gl_Position = at;
    gl_PointSize = at.z;
    colour = at.wzyx;
    cell = ivec2(at.xy);
}
";

const SPRITE: &str = "#version 450
layout(location = 0) in vec4 colour;
layout(location = 1) flat in ivec2 cell;
layout(location = 0) out vec4 result;
void main() {
    result = colour * vec4(gl_PointCoord, float(cell.x), 1.0);
}
";

const CLIPPED_POINTS: &str = "#version 450
layout(location = 0) in vec4 at;
void main() {
    gl_Position = at;
    gl_PointSize = at.z;
    gl_ClipDistance[0] = at.w;
}
";

const ONE_PIXEL_POINTS: &str = "#version 450
layout(location = 0) in vec4 at;
void main() {
    gl_Position = at;
    gl_PointSize = 1.0;
}
";

const STORING_VERTICES: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 colour;
layout(std430, binding = 0) buffer Seen { uint seen[]; };
void main() {
    atomicAdd(seen[gl_VertexID], 1u);
    gl_Position = at;
    colour = at;
}
";

/// One entry point of `words` read for Vulkan.
fn read(words: &[u32], stage: Stage) -> SpirvProgram {
    SpirvProgram::read(words, stage, ENTRY_POINT, false).unwrap()
}

/// One entry point of `words` read, in the common form of `stage`, flattened: candidate B's input.
fn flat_of(words: &[u32], stage: Stage) -> Flat {
    let (common, _) = read(words, stage).to_common_form(stage).unwrap();
    common.flatten().unwrap()
}

/// The linked GLSL program of `sources`.
fn compile(sources: &[(Stage, &str)]) -> Vec<GlslStage> {
    GlslStage::compile(sources).unwrap()
}

/// The program of the only source, prepared as the driver prepares it at its creation.
fn prepared(stage: Stage, source: &str) -> Prepared {
    compile(&[(stage, source)])
        .pop()
        .unwrap()
        .received
        .prepare()
        .unwrap()
}

/// A prepared program's variant for single-sampled targets with nothing else asked, in the set of
/// `stage`, flattened.
fn drawn(program: &Prepared, stage: Stage) -> Flattened {
    program
        .variant(stage, &DrawSettings::default(), &[], &[])
        .unwrap()
        .flatten(0)
        .unwrap()
}

#[test]
fn a_compute_program_goes_through_zink_and_back() {
    let program = read(&spirv(COMPUTE), Stage::Compute);
    assert!(!program.serialized().unwrap().is_empty());
    let (common, _) = program.to_common_form(Stage::Compute).unwrap();
    let words = common.into_spirv().unwrap();
    let (module, _) = wcontainers_nir::spirv::read(&words).unwrap();
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].stage, naga::ShaderStage::Compute);
}

#[test]
fn resources_are_numbered_by_kind_in_the_stage_set() {
    let (common, mut bindings) = read(&spirv(SAMPLED), Stage::Fragment)
        .to_common_form(Stage::Fragment)
        .unwrap();
    bindings.sort_by_key(|b| b.binding);
    let set = stage_set(ShaderStage::Fragment);
    let found: Vec<_> = bindings
        .iter()
        .map(|b| (b.kind, b.space, b.register, b.set, b.binding))
        .collect();
    let first = |kind: SlotKind| SLOT_KINDS[kind as usize].first_binding;
    assert_eq!(
        found,
        [
            (SlotKind::Uniform, 2, 0, set, first(SlotKind::Uniform)),
            (SlotKind::Sampler, 0, 1, set, first(SlotKind::Sampler)),
            (SlotKind::Texture, 0, 3, set, first(SlotKind::Texture)),
        ]
    );
    let (module, _) = wcontainers_nir::spirv::read(&common.into_spirv().unwrap()).unwrap();
    let mut bound: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(_, g)| g.binding)
        .map(|b| (b.group, b.binding))
        .collect();
    bound.sort();
    let expected: Vec<_> = bindings.iter().map(|b| (b.set, b.binding)).collect();
    assert_eq!(bound, expected);
}

#[test]
fn the_key_follows_the_nir() {
    let key = |wgsl| read(&spirv(wgsl), Stage::Compute).key().unwrap();
    assert_eq!(key(COMPUTE), key(COMPUTE));
    assert_ne!(key(COMPUTE), key(&COMPUTE.replace("3u", "5u")));
}

#[test]
fn both_routes_bind_a_sampling_program_alike() {
    let bound = |module: &naga::Module| {
        let mut b: Vec<_> = module
            .global_variables
            .iter()
            .filter_map(|(_, g)| g.binding)
            .map(|b| (b.group, b.binding))
            .collect();
        b.sort();
        b
    };
    let (a, _) = read(&spirv(SAMPLED), Stage::Fragment)
        .to_common_form(Stage::Fragment)
        .unwrap();
    let (through_zink, _) = wcontainers_nir::spirv::read(&a.into_spirv().unwrap()).unwrap();
    let direct =
        wcontainers_nir::direct::module(flat_of(&spirv(SAMPLED), Stage::Fragment)).unwrap();
    wcontainers_nir::spirv::validate(&direct).unwrap();
    assert_eq!(bound(&direct), bound(&through_zink));
    assert_eq!(direct.entry_points[0].stage, naga::ShaderStage::Fragment);
}

#[test]
fn candidate_b_reads_a_compute_program() {
    let module = wcontainers_nir::direct::module(flat_of(&spirv(COMPUTE), Stage::Compute)).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    assert_eq!(module.entry_points[0].workgroup_size, [1, 1, 1]);
}

#[test]
fn alpha_to_one_gives_route_b_a_sample_mask_and_alpha_one() {
    for program in [spirv(TRANSLUCENT), words(MASKED)] {
        let alpha_to_one = true;
        let (common, _) = SpirvProgram::read(&program, Stage::Fragment, ENTRY_POINT, alpha_to_one)
            .unwrap()
            .to_common_form(Stage::Fragment)
            .unwrap();
        let module = wcontainers_nir::direct::module(common.flatten().unwrap()).unwrap();
        wcontainers_nir::spirv::validate(&module).unwrap();
        assert_eq!(sample_mask_and_alpha_one(&module), (true, true));
    }
}

#[test]
fn route_b_reads_a_linked_glsl_program() {
    let stages = compile(&[
        (Stage::Vertex, GLSL_VERTEX),
        (Stage::Fragment, GLSL_FRAGMENT),
    ]);
    assert_eq!(
        stages.iter().map(|s| s.stage).collect::<Vec<_>>(),
        [Stage::Vertex, Stage::Fragment]
    );
    let named = |stage: usize, kind: GlslNameKind, name: &str| {
        stages[stage]
            .names
            .iter()
            .find(|n| n.kind == kind && n.name == name)
            .map(|n| (n.at, n.count))
    };
    let matrix_words = 16;
    let scales = 3;
    assert_eq!(
        named(0, GlslNameKind::Uniform, "transform").map(|n| n.1),
        Some(matrix_words)
    );
    assert_eq!(
        named(0, GlslNameKind::Uniform, "scale").map(|n| n.1),
        Some(scales)
    );
    // Uniform buffer 0 is the default block; storage blocks follow the uniform blocks.
    assert_eq!(named(0, GlslNameKind::UniformBlock, "Light"), Some((1, 1)));
    assert_eq!(
        named(0, GlslNameKind::StorageBlock, "Offsets"),
        Some((2, 1))
    );
    assert_eq!(named(0, GlslNameKind::Input, "corner"), Some((0, 1)));
    assert_eq!(named(0, GlslNameKind::Input, "at"), Some((1, 1)));
    assert_eq!(named(1, GlslNameKind::Sampler, "picture"), Some((0, 1)));
    assert_eq!(named(1, GlslNameKind::StorageBlock, "Count"), Some((1, 1)));
    assert_eq!(named(1, GlslNameKind::Output, "colour"), Some((0, 1)));
    assert!(
        stages[1]
            .names
            .iter()
            .any(|n| n.kind == GlslNameKind::FragCoordYTransform)
    );
    for GlslStage {
        stage, received, ..
    } in stages
    {
        let Flattened { flat, bindings, .. } = drawn(&received.prepare().unwrap(), stage);
        let module = wcontainers_nir::direct::module(flat).unwrap();
        wcontainers_nir::spirv::validate(&module).unwrap();
        let kinds = |kind| bindings.iter().filter(|b| b.kind == kind).count();
        match stage {
            // The default block, the uniform block, the storage block.
            Stage::Vertex => {
                assert_eq!((kinds(SlotKind::Uniform), kinds(SlotKind::Storage)), (2, 1))
            }
            // The default block (the coordinates' transform), the storage block, the texture
            // and its sampler.
            _ => assert_eq!(
                (
                    kinds(SlotKind::Uniform),
                    kinds(SlotKind::Storage),
                    kinds(SlotKind::Texture),
                    kinds(SlotKind::Sampler)
                ),
                (1, 1, 1, 1)
            ),
        }
    }
}

#[test]
fn a_glsl_program_that_does_not_compile_is_refused_with_its_log() {
    let Err(ConvertError::Glsl(log)) = GlslStage::compile(&[(
        Stage::Fragment,
        "#version 450\nvoid main() { undeclared = 1; }\n",
    )]) else {
        panic!("Mesa's compiler refuses the program");
    };
    assert!(log.contains("undeclared"), "{log}");
}

#[test]
fn an_attribute_read_in_part_is_named_a_run_of_read_slots() {
    let stages = compile(&[(Stage::Vertex, PARTLY_READ)]);
    let placed = |name: &str| {
        stages[0]
            .names
            .iter()
            .filter(|n| n.kind == GlslNameKind::Input && n.name == name)
            .map(|n| (n.element, n.at, n.count))
            .collect::<Vec<_>>()
    };
    // Vertex elements are the slots read, in order: columns 1 and 3, then the corner.
    assert_eq!(placed("columns"), [(1, 0, 1), (3, 1, 1)]);
    assert_eq!(placed("corner"), [(0, 2, 1)]);
}

/// The module of the vertex program of `vertex` linked with a fragment program, drawn as the driver
/// draws it.
fn vertex_module(vertex: &str) -> naga::Module {
    let stage = compile(&[
        (Stage::Vertex, vertex),
        (Stage::Fragment, UNPOSITIONED_FRAGMENT),
    ])
    .swap_remove(0);
    let flat = drawn(&stage.received.prepare().unwrap(), Stage::Vertex).flat;
    let module = wcontainers_nir::direct::module(flat).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    module
}

#[test]
fn a_vertex_program_without_a_position_gets_one_outside_every_clip_volume() {
    let module = vertex_module(UNPOSITIONED);
    let result = module.entry_points[0].function.result.as_ref().unwrap();
    let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
        panic!("the outputs are a structure");
    };
    assert!(members.iter().any(|m| matches!(
        m.binding,
        Some(naga::Binding::BuiltIn(naga::BuiltIn::Position { .. }))
    )));
}

/// Whether a vertex program's position output is invariant in its module.
fn invariant_position(vertex: &str) -> bool {
    let module = vertex_module(vertex);
    let result = module.entry_points[0].function.result.as_ref().unwrap();
    let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
        panic!("the outputs are a structure");
    };
    members
        .iter()
        .find_map(|m| match m.binding {
            Some(naga::Binding::BuiltIn(naga::BuiltIn::Position { invariant })) => Some(invariant),
            _ => None,
        })
        .expect("a position output")
}

#[test]
fn an_invariant_position_is_invariant_in_the_module_and_no_other_is() {
    assert!(invariant_position(INVARIANT_POSITIONED));
    assert!(!invariant_position(POSITIONED));
}

#[test]
fn a_subroutine_uniform_is_named_by_its_api_name_with_a_word_an_element() {
    let stages = compile(&[(Stage::Fragment, SUBROUTINES)]);
    let elements = 2;
    let named: Vec<_> = stages[0]
        .names
        .iter()
        .filter(|n| n.kind == GlslNameKind::SubroutineUniform)
        .map(|n| (n.name.as_str(), n.count))
        .collect();
    assert_eq!(named, [("shades", elements)]);
}

/// The program of the only source drawn as the driver draws it, through route B and the shaders
/// crate's rewrites with no variant asked.
fn rewritten(
    stage: Stage,
    source: &str,
    routed_check: impl FnOnce(&wcontainers_nir::direct::Routed),
) -> (
    wcontainers_gpu::shader::ShaderModule,
    wcontainers_gpu::reflect::Reflection,
) {
    let Flattened {
        flat,
        key,
        bindings,
    } = drawn(&prepared(stage, source), stage);
    let routed = wcontainers_nir::direct::route(flat).unwrap();
    routed_check(&routed);
    let (module, reflection) = wcontainers_nir::direct::rewrite(
        routed,
        bindings,
        &wcontainers_gpu::reflect::VariantKey::default(),
        &key,
    )
    .unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    (module, reflection)
}

#[test]
fn images_of_buffers_are_storage_words_with_their_views_in_helper_constants() {
    let images = 3;
    let (module, reflection) = rewritten(Stage::Compute, IMAGE_BUFFERS, |routed| {
        assert_eq!(routed.typed.buffers.len(), images)
    });
    let views = reflection
        .helpers
        .iter()
        .filter(|h| {
            matches!(
                h.value,
                wcontainers_gpu::reflect::HelperValue::TexelBufferView { .. }
            )
        })
        .count();
    assert_eq!(views, images);
    let storage = module
        .module
        .global_variables
        .iter()
        .filter(|(_, g)| matches!(g.space, naga::AddressSpace::Storage { .. }))
        .count();
    assert_eq!(storage, images);
}

/// Blocks of one layout keep their own words' kinds; workgroup memory, one array of words, takes
/// the other signedness's minimum and maximum by a compare-exchange loop.
#[test]
fn signed_and_unsigned_atomics_beside_each_other_keep_their_own_kinds() {
    for (source, looped) in SIGNED_AND_UNSIGNED_ATOMICS.into_iter().zip([false, true]) {
        let (_, reflection) = rewritten(Stage::Compute, source, |_| {});
        assert_eq!(
            reflection
                .slow_paths
                .contains(&wcontainers_gpu::slow::SlowPath::AtomicExchangeLoop),
            looped
        );
    }
}

#[test]
fn images_of_textures_beside_atomics_are_storage_words_laid_out_by_helper_constants() {
    // The image without atomics too, as images bound to one texture share its texels.
    let images = 4;
    let (module, reflection) = rewritten(Stage::Compute, IMAGE_ATOMICS, |routed| {
        assert_eq!(routed.typed.buffers.len(), images)
    });
    let count = |layout: bool| {
        reflection
            .helpers
            .iter()
            .filter(|h| match h.value {
                wcontainers_gpu::reflect::HelperValue::TexelLayout { .. } => layout,
                wcontainers_gpu::reflect::HelperValue::TexelBufferView { .. } => !layout,
                _ => false,
            })
            .count()
    };
    assert_eq!(count(true), images);
    assert_eq!(count(false), images);
    let globals = |storage: bool| {
        module
            .module
            .global_variables
            .iter()
            .filter(|(_, g)| match module.module.types[g.ty].inner {
                naga::TypeInner::Image { .. } => !storage,
                _ => storage && matches!(g.space, naga::AddressSpace::Storage { .. }),
            })
            .count()
    };
    assert_eq!(globals(true), images);
    assert_eq!(globals(false), 0);
}

/// The nearest sampler at `unit`, repeating, every level.
fn repeating(unit: u32, mipmapped: bool) -> NearestSampler {
    NearestSampler {
        unit,
        wrap: [Wrap::Repeat; naga::VectorSize::Tri as usize],
        mipmapped,
        min_lod: 0f32.to_bits(),
        max_lod: f32::MAX.to_bits(),
        border: [0; naga::VectorSize::MAX],
    }
}

#[test]
fn an_integer_cube_map_array_sampled_is_loaded_from_a_2d_array_of_its_faces() {
    let program = prepared(Stage::Fragment, INTEGER_CUBE_ARRAY_SAMPLED);
    let flat = program
        .variant(
            Stage::Fragment,
            &DrawSettings::default(),
            &[],
            &[repeating(0, true)],
        )
        .unwrap()
        .flatten(0)
        .unwrap()
        .flat;
    let module = wcontainers_nir::direct::module(flat).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    let textures: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(_, g)| match module.types[g.ty].inner {
            naga::TypeInner::Image { dim, arrayed, .. } => Some((dim, arrayed)),
            _ => None,
        })
        .collect();
    assert_eq!(textures, [(naga::ImageDimension::D2, true)]);
}

#[test]
fn an_integer_cube_map_array_gathered_is_refused_by_name() {
    // Its gather, of a cube made a 2D array of faces, would read no texel of the next face.
    let program = prepared(Stage::Fragment, INTEGER_CUBE_ARRAY);
    let refused = program.variant(
        Stage::Fragment,
        &DrawSettings::default(),
        &[],
        &[repeating(0, true)],
    );
    assert!(
        matches!(refused, Err(ConvertError::Unsupported(ref why)) if why.contains("integer cube")),
        "{:?}",
        refused.err()
    );
}

/// The program of `source` drawn into targets of `samples` samples, as route B's module.
fn at_samples(program: &Prepared, samples: u32) -> Result<naga::Module, ConvertError> {
    let settings = DrawSettings {
        samples,
        ..DrawSettings::default()
    };
    let flat = program
        .variant(Stage::Fragment, &settings, &[], &[])?
        .flatten(0)?
        .flat;
    let module = wcontainers_nir::direct::module(flat)?;
    wcontainers_nir::spirv::validate(&module).unwrap();
    Ok(module)
}

#[test]
fn inputs_read_at_an_offset_or_a_sample_are_moved_from_where_the_host_interpolates_them() {
    let program = prepared(Stage::Fragment, ELSEWHERE);
    assert!(program.reads_sample_positions);
    // Three samples have no standard positions.
    assert!(matches!(
        at_samples(&program, 3),
        Err(ConvertError::Unsupported(_))
    ));
    for samples in [1, 2, 4, 8, 16] {
        let module = at_samples(&program, samples).unwrap();
        // Each input at the place its plain read takes it: the pixel's centre.
        let centre = Some(naga::Sampling::Center);
        assert_eq!(fragment_inputs(&module), [(0, centre), (1, centre)]);
    }
}

#[test]
fn early_tests_a_fragment_program_forces_are_in_its_pixel_extras() {
    let (_, reflection) = rewritten(Stage::Fragment, EARLY_TESTS, |_| {});
    assert!(matches!(
        reflection.extras,
        wcontainers_gpu::reflect::StageExtras::Pixel {
            early_tests: true,
            side_effects: true,
            discards: false,
            per_sample: false,
        }
    ));
}

#[test]
fn a_centroid_input_also_read_another_way_is_made_from_the_other_way() {
    let program = prepared(Stage::Fragment, CENTROID_ELSEWHERE);
    assert!(program.reads_sample_positions);
    assert!(at_samples(&program, 3).is_err());
    for samples in [1, 2, 4, 8, 16] {
        let module = at_samples(&program, samples).unwrap();
        // A slot read only at its centroid stays there; the others are at the pixel's centre.
        let (centre, centroid) = (Some(naga::Sampling::Center), Some(naga::Sampling::Centroid));
        assert_eq!(
            fragment_inputs(&module),
            [(0, centre), (1, centroid), (2, centre)]
        );
    }
}

#[test]
fn an_input_read_only_at_its_own_sample_is_interpolated_at_the_sample_by_the_host() {
    let module = at_samples(&prepared(Stage::Fragment, AT_OWN_SAMPLE), 4).unwrap();
    // The input read also at the pixel's centre is moved from there.
    assert_eq!(
        fragment_inputs(&module),
        [
            (0, Some(naga::Sampling::Sample)),
            (1, Some(naga::Sampling::Center))
        ]
    );
}

#[test]
fn an_image_without_a_format_is_declared_in_the_format_the_variant_names() {
    let program = prepared(Stage::Compute, FORMATLESS);
    let unit = 1;
    assert_eq!(program.formatless_images, 1 << unit);
    let Flattened { key, bindings, .. } = drawn(&program, Stage::Compute);
    let found = bindings
        .iter()
        .find(|b| b.kind == SlotKind::Storage && b.register == unit)
        .unwrap();
    let slot = found.binding - SLOT_KINDS[found.kind as usize].first_binding;
    let routed = || wcontainers_nir::direct::route(drawn(&program, Stage::Compute).flat).unwrap();
    assert_eq!(routed().rewrites.formatless, [slot]);
    let unnamed = wcontainers_gpu::reflect::VariantKey::default();
    assert!(wcontainers_nir::direct::rewrite(routed(), bindings.clone(), &unnamed, &key).is_err());
    let format = wcontainers_gpu::format::Format::Rgba16Float;
    let named = wcontainers_gpu::reflect::VariantKey {
        storage_formats: vec![(slot, format)],
        ..wcontainers_gpu::reflect::VariantKey::default()
    };
    let (module, _) = wcontainers_nir::direct::rewrite(routed(), bindings, &named, &key).unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    let mut formats: Vec<_> = module
        .module
        .global_variables
        .iter()
        .filter_map(|(_, g)| match module.module.types[g.ty].inner {
            naga::TypeInner::Image {
                class: naga::ImageClass::Storage { format, .. },
                ..
            } => Some(format),
            _ => None,
        })
        .collect();
    formats.sort_by_key(|f| format!("{f:?}"));
    assert_eq!(
        formats,
        [
            naga::StorageFormat::Rgba16Float,
            naga::StorageFormat::Rgba8Unorm
        ]
    );
}

#[test]
fn an_array_of_integer_textures_indexed_at_run_time_is_loaded_alike_through_every_element() {
    let program = prepared(Stage::Fragment, INTEGER_ARRAY);
    let with = |samplers: &[NearestSampler]| {
        program
            .variant(Stage::Fragment, &DrawSettings::default(), &[], samplers)
            .unwrap()
            .flatten(0)
            .unwrap()
    };
    let Flattened {
        flat,
        key,
        bindings,
    } = with(&[repeating(0, false), repeating(1, false)]);
    let routed = wcontainers_nir::direct::route(flat).unwrap();
    // The array split into its elements, each loaded where the index names it.
    let unnamed = wcontainers_gpu::reflect::VariantKey::default();
    let (module, _) = wcontainers_nir::direct::rewrite(routed, bindings, &unnamed, &key).unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    // Elements sampled two ways are left sampled, which the route refuses by name.
    let clamped = NearestSampler {
        wrap: [Wrap::ClampToEdge; naga::VectorSize::Tri as usize],
        ..repeating(1, false)
    };
    let unlike = with(&[repeating(0, false), clamped]);
    assert!(wcontainers_nir::direct::route(unlike.flat).is_err());
}

#[test]
fn an_array_of_arrays_of_textures_is_one_array_of_its_elements() {
    rewritten(Stage::Fragment, ARRAYS_OF_ARRAYS, |_| {});
}

#[test]
fn a_fragment_programs_layer_is_a_flat_input_the_driver_places() {
    let flat = drawn(&prepared(Stage::Fragment, LAYER_READ), Stage::Fragment).flat;
    let module = wcontainers_nir::direct::module(flat).unwrap();
    let layer = module.entry_points[0]
        .function
        .arguments
        .iter()
        .find_map(|a| match a.binding {
            Some(naga::Binding::Location {
                location: wcontainers_nir::direct::NAMED_LAYER_INPUT,
                interpolation,
                ..
            }) => Some(interpolation),
            _ => None,
        });
    assert_eq!(layer, Some(Some(naga::Interpolation::Flat)));
}

/// A made program's variant in the set of the stage helper's compute work (a geometry program runs
/// there), with `settings`, flattened.
fn made_drawn(made: &Made, settings: &DrawSettings) -> Flattened {
    made.variant(Stage::Compute, settings, &[], &[])
        .unwrap()
        .flatten(0)
        .unwrap()
}

/// A made geometry program through route B and the stage helper, each module validated.
fn through_the_stage_helper(made: Flattened) {
    let Flattened {
        flat,
        key,
        bindings,
    } = made;
    let routed = wcontainers_nir::direct::route(flat).unwrap();
    let unnamed = wcontainers_gpu::reflect::VariantKey::default();
    let converted = wcontainers_nir::direct::geometry(routed, bindings, &unnamed, &key).unwrap();
    for form in &converted.modules {
        wcontainers_nir::spirv::validate(&form.module).unwrap();
    }
}

#[test]
fn points_are_drawn_as_squares_passing_their_coordinate_after_every_location() {
    let mut stages = compile(&[(Stage::Vertex, SIZED_POINTS), (Stage::Fragment, SPRITE)])
        .into_iter()
        .map(|s| s.received.prepare().unwrap());
    let (vertex, fragment) = (stages.next().unwrap(), stages.next().unwrap());
    let at = vertex.generic_end.max(fragment.generic_end);
    let locations = 2;
    assert_eq!(at, locations);
    let module = wcontainers_nir::direct::module(drawn(&fragment, Stage::Fragment).flat).unwrap();
    let coord = module.entry_points[0]
        .function
        .arguments
        .iter()
        .find_map(|a| match a.binding {
            Some(naga::Binding::Location {
                location: wcontainers_nir::direct::POINT_COORD_INPUT,
                interpolation,
                ..
            }) => Some(interpolation),
            _ => None,
        });
    assert_eq!(coord, Some(Some(naga::Interpolation::Linear)));
    let largest = 64.0;
    let Squares::Made { before, geometry } = vertex.points_as_squares(at, largest).unwrap() else {
        panic!("sized points are drawn as squares");
    };
    // The vertex program writes a point size, so a copy also writes it at `at`.
    assert!(before.is_some());
    let square = made_drawn(&geometry, &DrawSettings::default());
    // The block of the size, the step of half a pixel and the coordinate's direction.
    assert_eq!(
        square
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Uniform)
            .count(),
        1
    );
    let corners = 4;
    assert_eq!(square.flat.geometry.max_vertices, corners);
    through_the_stage_helper(square);
    let clipped = prepared(Stage::Vertex, CLIPPED_POINTS);
    assert!(matches!(
        clipped.points_as_squares(clipped.generic_end, largest),
        Ok(Squares::Refused)
    ));
}

#[test]
fn points_written_as_one_pixel_are_known_as_such() {
    let squares = |source| {
        let program = prepared(Stage::Vertex, source);
        program
            .points_as_squares(program.generic_end, 64.0)
            .unwrap()
    };
    assert!(matches!(squares(ONE_PIXEL_POINTS), Squares::OnePixel));
    assert!(matches!(squares(SIZED_POINTS), Squares::Made { .. }));
}

#[test]
fn a_vertex_program_using_storage_gets_a_pass_through_for_every_primitive_it_may_draw() {
    assert!(
        prepared(Stage::Vertex, ONE_PIXEL_POINTS)
            .pass_through
            .is_none()
    );
    let made = prepared(Stage::Vertex, STORING_VERTICES)
        .pass_through
        .unwrap();
    let point = 1;
    let line = 2;
    let triangle = 3;
    let vertices = |a| match a {
        Assembled::Points => point,
        Assembled::Lines | Assembled::LinesAdjacency => line,
        Assembled::Triangles | Assembled::TrianglesAdjacency => triangle,
    };
    // WebGPU draws points of one size.
    let fixed = PositionTransform {
        fixed_point_size: true,
        ..PositionTransform::default()
    };
    let settings = DrawSettings {
        transform: fixed,
        key_transform: fixed,
        ..DrawSettings::default()
    };
    for (&assembled, geometry) in Assembled::ALL.iter().zip(made) {
        let geometry = made_drawn(&geometry.unwrap(), &settings);
        assert_eq!(geometry.flat.geometry.max_vertices, vertices(assembled));
        through_the_stage_helper(geometry);
    }
}
