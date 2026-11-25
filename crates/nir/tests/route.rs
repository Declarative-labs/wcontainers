//! Both candidates end to end on programs naga writes as SPIR-V: SPIR-V to NIR, NIR's serialised
//! key, the common form's step, then Zink's SPIR-V and naga's reader (A) or NIR read straight into
//! naga (B), each validated.
#![cfg(all(mesa, feature = "standalone"))]

use wcontainers_gpu::{SLOT_KINDS, ShaderStage, SlotKind};
use wcontainers_nir::ConvertError;
use wcontainers_nir::bind::stage_set;
use wcontainers_nir::mesa::{Assembled, GlslNameKind, GlslStage, Nir, PositionTransform, Stage};
use wcontainers_shaders::ENTRY_POINT;

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

#[test]
fn a_compute_program_goes_through_zink_and_back() {
    let nir = Nir::from_spirv(&spirv(COMPUTE), Stage::Compute, ENTRY_POINT).unwrap();
    assert!(!nir.serialized().unwrap().is_empty());
    let words = nir.into_spirv().unwrap();
    let (module, _) = wcontainers_nir::spirv::read(&words).unwrap();
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].stage, naga::ShaderStage::Compute);
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

#[test]
fn resources_are_numbered_by_kind_in_the_stage_set() {
    let mut nir = Nir::from_spirv(&spirv(SAMPLED), Stage::Fragment, ENTRY_POINT).unwrap();
    let mut bindings = nir.to_common_form(Stage::Fragment).unwrap();
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
    let (module, _) = wcontainers_nir::spirv::read(&nir.into_spirv().unwrap()).unwrap();
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
    let key = |wgsl| {
        Nir::from_spirv(&spirv(wgsl), Stage::Compute, ENTRY_POINT)
            .unwrap()
            .key()
            .unwrap()
    };
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
    let mut a = Nir::from_spirv(&spirv(SAMPLED), Stage::Fragment, ENTRY_POINT).unwrap();
    a.to_common_form(Stage::Fragment).unwrap();
    let (through_zink, _) = wcontainers_nir::spirv::read(&a.into_spirv().unwrap()).unwrap();
    let mut b = Nir::from_spirv(&spirv(SAMPLED), Stage::Fragment, ENTRY_POINT).unwrap();
    b.to_common_form(Stage::Fragment).unwrap();
    let direct = wcontainers_nir::direct::module(&mut b).unwrap();
    wcontainers_nir::spirv::validate(&direct).unwrap();
    assert_eq!(bound(&direct), bound(&through_zink));
    assert_eq!(direct.entry_points[0].stage, naga::ShaderStage::Fragment);
}

#[test]
fn candidate_b_reads_a_compute_program() {
    let mut nir = Nir::from_spirv(&spirv(COMPUTE), Stage::Compute, ENTRY_POINT).unwrap();
    nir.to_common_form(Stage::Compute).unwrap();
    let module = wcontainers_nir::direct::module(&mut nir).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    assert_eq!(module.entry_points[0].workgroup_size, [1, 1, 1]);
}

const TRANSLUCENT: &str = "
@fragment
fn main(@location(0) colour: vec4<f32>) -> @location(0) vec4<f32> {
    return colour;
}
";

const MASKED: &str = "
struct Out {
    @location(0) colour: vec4<f32>,
    @builtin(sample_mask) mask: u32,
}
@fragment
fn main(@location(0) colour: vec4<f32>, @location(1) @interpolate(flat) mask: u32) -> Out {
    return Out(colour, mask);
}
";

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

#[test]
fn alpha_to_one_gives_route_b_a_sample_mask_and_alpha_one() {
    for program in [TRANSLUCENT, MASKED] {
        let mut nir = Nir::from_spirv(&spirv(program), Stage::Fragment, ENTRY_POINT).unwrap();
        nir.alpha_to_one();
        nir.to_common_form(Stage::Fragment).unwrap();
        let module = wcontainers_nir::direct::module(&mut nir).unwrap();
        wcontainers_nir::spirv::validate(&module).unwrap();
        assert_eq!(sample_mask_and_alpha_one(&module), (true, true));
    }
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

#[test]
fn route_b_reads_a_linked_glsl_program() {
    let stages = Nir::from_glsl(&[
        (Stage::Vertex, GLSL_VERTEX),
        (Stage::Fragment, GLSL_FRAGMENT),
    ])
    .unwrap();
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
    for GlslStage { stage, mut nir, .. } in stages {
        assert!(!nir.serialized().unwrap().is_empty());
        let bindings = nir.to_common_form(stage).unwrap();
        let module = wcontainers_nir::direct::module(&mut nir).unwrap();
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
    let Err(ConvertError::Glsl(log)) = Nir::from_glsl(&[(
        Stage::Fragment,
        "#version 450\nvoid main() { undeclared = 1; }\n",
    )]) else {
        panic!("Mesa's compiler refuses the program");
    };
    assert!(log.contains("undeclared"), "{log}");
}

const PARTLY_READ: &str = "#version 450
layout(location = 0) in mat4 columns;
layout(location = 4) in vec4 corner;
void main() { gl_Position = corner + columns[1] + columns[3]; }
";

#[test]
fn an_attribute_read_in_part_is_named_a_run_of_read_slots() {
    let stages = Nir::from_glsl(&[(Stage::Vertex, PARTLY_READ)]).unwrap();
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

#[test]
fn a_vertex_program_without_a_position_gets_one_outside_every_clip_volume() {
    let mut stages = Nir::from_glsl(&[
        (Stage::Vertex, UNPOSITIONED),
        (Stage::Fragment, UNPOSITIONED_FRAGMENT),
    ])
    .unwrap();
    let vertex = &mut stages[0].nir;
    vertex.to_common_form(Stage::Vertex).unwrap();
    let module = wcontainers_nir::direct::module(vertex).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    let result = module.entry_points[0].function.result.as_ref().unwrap();
    let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
        panic!("the outputs are a structure");
    };
    assert!(members.iter().any(|m| matches!(
        m.binding,
        Some(naga::Binding::BuiltIn(naga::BuiltIn::Position { .. }))
    )));
}

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

/// Whether a vertex program's position output is invariant in its module.
fn invariant_position(vertex: &str) -> bool {
    let mut stages = Nir::from_glsl(&[
        (Stage::Vertex, vertex),
        (Stage::Fragment, UNPOSITIONED_FRAGMENT),
    ])
    .unwrap();
    let vertex = &mut stages[0].nir;
    vertex.to_common_form(Stage::Vertex).unwrap();
    let module = wcontainers_nir::direct::module(vertex).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
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

const SUBROUTINES: &str = "#version 450
subroutine vec4 Shade(vec4 c);
subroutine(Shade) vec4 keep(vec4 c) { return c; }
subroutine(Shade) vec4 invert(vec4 c) { return 1.0 - c; }
subroutine uniform Shade shades[2];
uniform vec4 base;
layout(location = 0) out vec4 colour;
void main() { colour = shades[0](base) + shades[1](base); }
";

#[test]
fn a_subroutine_uniform_is_named_by_its_api_name_with_a_word_an_element() {
    let stages = Nir::from_glsl(&[(Stage::Fragment, SUBROUTINES)]).unwrap();
    let elements = 2;
    let named: Vec<_> = stages[0]
        .names
        .iter()
        .filter(|n| n.kind == GlslNameKind::SubroutineUniform)
        .map(|n| (n.name.as_str(), n.count))
        .collect();
    assert_eq!(named, [("shades", elements)]);
}

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

#[test]
fn images_of_buffers_are_storage_words_with_their_views_in_helper_constants() {
    let mut stage = Nir::from_glsl(&[(Stage::Compute, IMAGE_BUFFERS)])
        .unwrap()
        .pop()
        .unwrap();
    let key = stage.nir.key().unwrap();
    let bindings = stage.nir.to_common_form(Stage::Compute).unwrap();
    let routed = wcontainers_nir::direct::route(&mut stage.nir).unwrap();
    let images = 3;
    assert_eq!(routed.typed.buffers.len(), images);
    let (module, reflection) = wcontainers_nir::direct::rewrite(
        routed,
        bindings,
        &wcontainers_gpu::VariantKey::default(),
        &key,
    )
    .unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    let views = reflection
        .helpers
        .iter()
        .filter(|h| {
            matches!(
                h.value,
                wcontainers_gpu::HelperValue::TexelBufferView { .. }
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
#[test]
fn signed_and_unsigned_atomics_beside_each_other_keep_their_own_kinds() {
    for (source, looped) in SIGNED_AND_UNSIGNED_ATOMICS.into_iter().zip([false, true]) {
        let mut stage = Nir::from_glsl(&[(Stage::Compute, source)])
            .unwrap()
            .pop()
            .unwrap();
        let key = stage.nir.key().unwrap();
        let bindings = stage.nir.to_common_form(Stage::Compute).unwrap();
        let routed = wcontainers_nir::direct::route(&mut stage.nir).unwrap();
        let (module, reflection) = wcontainers_nir::direct::rewrite(
            routed,
            bindings,
            &wcontainers_gpu::VariantKey::default(),
            &key,
        )
        .unwrap();
        wcontainers_nir::spirv::validate(&module.module).unwrap();
        assert_eq!(
            reflection
                .slow_paths
                .contains(&wcontainers_gpu::SlowPath::AtomicExchangeLoop),
            looped
        );
    }
}

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

#[test]
fn images_of_textures_beside_atomics_are_storage_words_laid_out_by_helper_constants() {
    let mut stage = Nir::from_glsl(&[(Stage::Compute, IMAGE_ATOMICS)])
        .unwrap()
        .pop()
        .unwrap();
    let key = stage.nir.key().unwrap();
    let bindings = stage.nir.to_common_form(Stage::Compute).unwrap();
    let routed = wcontainers_nir::direct::route(&mut stage.nir).unwrap();
    // The image without atomics too, as images bound to one texture share its texels.
    let images = 4;
    assert_eq!(routed.typed.buffers.len(), images);
    let (module, reflection) = wcontainers_nir::direct::rewrite(
        routed,
        bindings,
        &wcontainers_gpu::VariantKey::default(),
        &key,
    )
    .unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    let count = |layout: bool| {
        reflection
            .helpers
            .iter()
            .filter(|h| match h.value {
                wcontainers_gpu::HelperValue::TexelLayout { .. } => layout,
                wcontainers_gpu::HelperValue::TexelBufferView { .. } => !layout,
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

const INTEGER_CUBE_ARRAY: &str = "#version 450
layout(binding = 0) uniform isamplerCubeArray cubes;
layout(location = 0) in vec4 at;
layout(location = 0) out ivec4 colour;
void main() {
    colour = texture(cubes, at) + textureGather(cubes, at) + ivec4(textureSize(cubes, 0), 0);
}
";

#[test]
fn an_integer_cube_map_array_sampled_is_loaded_from_a_2d_array_of_its_faces() {
    use wcontainers_nir::mesa::{NearestSampler, Wrap};
    let mut stage = Nir::from_glsl(&[(Stage::Fragment, INTEGER_CUBE_ARRAY)])
        .unwrap()
        .pop()
        .unwrap();
    // Its gather, of a cube made a 2D array of faces, reads no texel of the next face: reported.
    let all_built = stage.nir.nearest_integer_textures(&[NearestSampler {
        unit: 0,
        wrap: [Wrap::Repeat; naga::VectorSize::Tri as usize],
        mipmapped: true,
        min_lod: 0f32.to_bits(),
        max_lod: f32::MAX.to_bits(),
        border: [0; naga::VectorSize::MAX],
    }]);
    assert!(!all_built);
    stage.nir.to_common_form(Stage::Fragment).unwrap();
    let module = wcontainers_nir::direct::module(&mut stage.nir).unwrap();
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

#[test]
fn inputs_read_at_an_offset_or_a_sample_are_moved_from_where_the_host_interpolates_them() {
    let stage = || {
        Nir::from_glsl(&[(Stage::Fragment, ELSEWHERE)])
            .unwrap()
            .pop()
            .unwrap()
    };
    assert!(stage().nir.reads_sample_positions());
    // Three samples have no standard positions.
    assert!(!stage().nir.interpolate_elsewhere(3));
    for samples in [1, 2, 4, 8, 16] {
        let mut nir = stage().nir;
        assert!(nir.interpolate_elsewhere(samples));
        nir.to_common_form(Stage::Fragment).unwrap();
        let module = wcontainers_nir::direct::module(&mut nir).unwrap();
        wcontainers_nir::spirv::validate(&module).unwrap();
        // Each input at the place its plain read takes it: the pixel's centre.
        let centre = Some(naga::Sampling::Center);
        assert_eq!(fragment_inputs(&module), [(0, centre), (1, centre)]);
    }
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

#[test]
fn early_tests_a_fragment_program_forces_are_in_its_pixel_extras() {
    let mut stage = Nir::from_glsl(&[(Stage::Fragment, EARLY_TESTS)])
        .unwrap()
        .pop()
        .unwrap();
    let key = stage.nir.key().unwrap();
    let bindings = stage.nir.to_common_form(Stage::Fragment).unwrap();
    let routed = wcontainers_nir::direct::route(&mut stage.nir).unwrap();
    let (module, reflection) = wcontainers_nir::direct::rewrite(
        routed,
        bindings,
        &wcontainers_gpu::VariantKey::default(),
        &key,
    )
    .unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    assert!(matches!(
        reflection.extras,
        wcontainers_gpu::StageExtras::Pixel {
            early_tests: true,
            side_effects: true,
            discards: false,
            per_sample: false,
        }
    ));
}

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

#[test]
fn a_centroid_input_also_read_another_way_is_made_from_the_other_way() {
    let stage = || {
        Nir::from_glsl(&[(Stage::Fragment, CENTROID_ELSEWHERE)])
            .unwrap()
            .pop()
            .unwrap()
    };
    assert!(stage().nir.reads_sample_positions());
    assert!(!stage().nir.interpolate_elsewhere(3));
    for samples in [1, 2, 4, 8, 16] {
        let mut nir = stage().nir;
        assert!(nir.interpolate_elsewhere(samples));
        nir.to_common_form(Stage::Fragment).unwrap();
        let module = wcontainers_nir::direct::module(&mut nir).unwrap();
        wcontainers_nir::spirv::validate(&module).unwrap();
        // A slot read only at its centroid stays there; the others are at the pixel's centre.
        let (centre, centroid) = (Some(naga::Sampling::Center), Some(naga::Sampling::Centroid));
        assert_eq!(
            fragment_inputs(&module),
            [(0, centre), (1, centroid), (2, centre)]
        );
    }
}

const AT_OWN_SAMPLE: &str = "#version 450
layout(location = 0) in vec4 only_there;
layout(location = 1) in vec4 also_here;
layout(location = 0) out vec4 colour;
void main() {
    colour = interpolateAtSample(only_there, gl_SampleID)
        + interpolateAtSample(also_here, gl_SampleID) + also_here;
}
";

#[test]
fn an_input_read_only_at_its_own_sample_is_interpolated_at_the_sample_by_the_host() {
    let mut nir = Nir::from_glsl(&[(Stage::Fragment, AT_OWN_SAMPLE)])
        .unwrap()
        .pop()
        .unwrap()
        .nir;
    assert!(nir.interpolate_elsewhere(4));
    nir.to_common_form(Stage::Fragment).unwrap();
    let module = wcontainers_nir::direct::module(&mut nir).unwrap();
    wcontainers_nir::spirv::validate(&module).unwrap();
    // The input read also at the pixel's centre is moved from there.
    assert_eq!(
        fragment_inputs(&module),
        [
            (0, Some(naga::Sampling::Sample)),
            (1, Some(naga::Sampling::Center))
        ]
    );
}

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

#[test]
fn an_image_without_a_format_is_declared_in_the_format_the_variant_names() {
    let mut stage = Nir::from_glsl(&[(Stage::Compute, FORMATLESS)])
        .unwrap()
        .pop()
        .unwrap();
    let unit = 1;
    assert_eq!(stage.nir.formatless_images(), 1 << unit);
    let key = stage.nir.key().unwrap();
    let bindings = stage.nir.to_common_form(Stage::Compute).unwrap();
    let slot = bindings
        .iter()
        .find(|b| b.kind == SlotKind::Storage && b.register == unit)
        .unwrap()
        .slot();
    let routed = || wcontainers_nir::direct::route(&mut stage.nir.try_clone().unwrap()).unwrap();
    assert_eq!(routed().rewrites.formatless, [slot]);
    let unnamed = wcontainers_gpu::VariantKey::default();
    assert!(wcontainers_nir::direct::rewrite(routed(), bindings.clone(), &unnamed, &key).is_err());
    let format = wcontainers_gpu::Format::Rgba16Float;
    let named = wcontainers_gpu::VariantKey {
        storage_formats: vec![(slot, format)],
        ..wcontainers_gpu::VariantKey::default()
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

const INTEGER_ARRAY: &str = "#version 450
layout(binding = 0) uniform isampler2D textures[2];
layout(location = 0) in vec2 at;
layout(location = 1) flat in int which;
layout(location = 0) out ivec4 colour;
void main() {
    colour = texture(textures[which], at);
}
";

#[test]
fn an_array_of_integer_textures_indexed_at_run_time_is_loaded_alike_through_every_element() {
    use wcontainers_nir::mesa::{NearestSampler, Wrap};
    let stage = || {
        Nir::from_glsl(&[(Stage::Fragment, INTEGER_ARRAY)])
            .unwrap()
            .pop()
            .unwrap()
    };
    let sampler = |unit| NearestSampler {
        unit,
        wrap: [Wrap::Repeat; naga::VectorSize::Tri as usize],
        mipmapped: false,
        min_lod: 0f32.to_bits(),
        max_lod: f32::MAX.to_bits(),
        border: [0; naga::VectorSize::MAX],
    };
    let mut alike = stage().nir;
    assert!(alike.nearest_integer_textures(&[sampler(0), sampler(1)]));
    let key = alike.key().unwrap();
    let bindings = alike.to_common_form(Stage::Fragment).unwrap();
    let routed = wcontainers_nir::direct::route(&mut alike).unwrap();
    // The array split into its elements, each loaded where the index names it.
    let unnamed = wcontainers_gpu::VariantKey::default();
    let (module, _) = wcontainers_nir::direct::rewrite(routed, bindings, &unnamed, &key).unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
    // Elements sampled two ways are left sampled, which the route refuses by name.
    let mut unlike = stage().nir;
    let clamped = NearestSampler {
        wrap: [Wrap::ClampToEdge; naga::VectorSize::Tri as usize],
        ..sampler(1)
    };
    assert!(unlike.nearest_integer_textures(&[sampler(0), clamped]));
    unlike.to_common_form(Stage::Fragment).unwrap();
    assert!(wcontainers_nir::direct::route(&mut unlike).is_err());
}

const ARRAYS_OF_ARRAYS: &str = "#version 450
layout(binding = 0) uniform sampler2D textures[2][3];
layout(location = 0) in vec2 at;
layout(location = 1) flat in int which;
layout(location = 0) out vec4 colour;
void main() {
    colour = texture(textures[1][2], at) + texture(textures[which][1], at);
}
";

#[test]
fn an_array_of_arrays_of_textures_is_one_array_of_its_elements() {
    let mut nir = Nir::from_glsl(&[(Stage::Fragment, ARRAYS_OF_ARRAYS)])
        .unwrap()
        .pop()
        .unwrap()
        .nir;
    nir.split_resource_arrays();
    let key = nir.key().unwrap();
    let bindings = nir.to_common_form(Stage::Fragment).unwrap();
    let routed = wcontainers_nir::direct::route(&mut nir).unwrap();
    let unnamed = wcontainers_gpu::VariantKey::default();
    let (module, _) = wcontainers_nir::direct::rewrite(routed, bindings, &unnamed, &key).unwrap();
    wcontainers_nir::spirv::validate(&module.module).unwrap();
}

const LAYER_READ: &str = "#version 450
layout(location = 0) in vec4 colour;
layout(location = 0) out vec4 result;
void main() {
    result = colour * float(gl_Layer);
}
";

#[test]
fn a_fragment_programs_layer_is_a_flat_input_the_driver_places() {
    let mut nir = Nir::from_glsl(&[(Stage::Fragment, LAYER_READ)])
        .unwrap()
        .pop()
        .unwrap()
        .nir;
    nir.to_common_form(Stage::Fragment).unwrap();
    let module = wcontainers_nir::direct::module(&mut nir).unwrap();
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

#[test]
fn points_are_drawn_as_squares_passing_their_coordinate_after_every_location() {
    let mut stages = Nir::from_glsl(&[(Stage::Vertex, SIZED_POINTS), (Stage::Fragment, SPRITE)])
        .unwrap()
        .into_iter()
        .map(|s| s.nir);
    let (mut vertex, mut fragment) = (stages.next().unwrap(), stages.next().unwrap());
    let at = vertex.generic_end().max(fragment.generic_end());
    let locations = 2;
    assert_eq!(at, locations);
    fragment.to_common_form(Stage::Fragment).unwrap();
    let module = wcontainers_nir::direct::module(&mut fragment).unwrap();
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
    assert!(vertex.point_size_also_at(at));
    let mut square = vertex.point_square(at, largest).unwrap();
    square.uniform_blocks_as_words();
    // A geometry program runs as the stage helper's compute work.
    let bindings = square.to_common_form(Stage::Compute).unwrap();
    // The block of the size, the step of half a pixel and the coordinate's direction.
    assert_eq!(
        bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Uniform)
            .count(),
        1
    );
    assert_eq!(square.geometry_vertices_out(), 4);
    let key = square.key().unwrap();
    let routed = wcontainers_nir::direct::route(&mut square).unwrap();
    let unnamed = wcontainers_gpu::VariantKey::default();
    let converted = wcontainers_nir::direct::geometry(routed, bindings, &unnamed, &key).unwrap();
    for form in &converted.modules {
        wcontainers_nir::spirv::validate(&form.module).unwrap();
    }
    let mut clipped = Nir::from_glsl(&[(Stage::Vertex, CLIPPED_POINTS)])
        .unwrap()
        .pop()
        .unwrap()
        .nir;
    let at = clipped.generic_end();
    assert!(clipped.point_size_also_at(at));
    assert!(clipped.point_square(at, largest).is_none());
}

const ONE_PIXEL_POINTS: &str = "#version 450
layout(location = 0) in vec4 at;
void main() {
    gl_Position = at;
    gl_PointSize = 1.0;
}
";

#[test]
fn points_written_as_one_pixel_are_known_as_such() {
    let vertex = |source| {
        let mut nir = Nir::from_glsl(&[(Stage::Vertex, source)])
            .unwrap()
            .pop()
            .unwrap()
            .nir;
        let at = nir.generic_end();
        assert!(nir.point_size_also_at(at));
        nir
    };
    let one_pixel = 1.0;
    assert!(vertex(ONE_PIXEL_POINTS).point_size_at_most(one_pixel));
    assert!(!vertex(SIZED_POINTS).point_size_at_most(one_pixel));
}

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

#[test]
fn a_vertex_program_using_storage_gets_a_pass_through_for_every_primitive_it_may_draw() {
    let from = |source| {
        Nir::from_glsl(&[(Stage::Vertex, source)])
            .unwrap()
            .pop()
            .unwrap()
            .nir
    };
    assert!(from(ONE_PIXEL_POINTS).storage_pass_through().is_none());
    let made = from(STORING_VERTICES).storage_pass_through().unwrap();
    let point = 1;
    let line = 2;
    let triangle = 3;
    let vertices = |a| match a {
        Assembled::Points => point,
        Assembled::Lines | Assembled::LinesAdjacency => line,
        Assembled::Triangles | Assembled::TrianglesAdjacency => triangle,
    };
    for (&assembled, geometry) in Assembled::ALL.iter().zip(made) {
        let mut geometry = geometry.unwrap();
        assert_eq!(geometry.geometry_vertices_out(), vertices(assembled));
        // WebGPU draws points of one size.
        geometry.transform_position(PositionTransform {
            fixed_point_size: true,
            ..PositionTransform::default()
        });
        geometry.uniform_blocks_as_words();
        // A geometry program runs as the stage helper's compute work.
        let bindings = geometry.to_common_form(Stage::Compute).unwrap();
        let key = geometry.key().unwrap();
        let routed = wcontainers_nir::direct::route(&mut geometry).unwrap();
        let unnamed = wcontainers_gpu::VariantKey::default();
        let converted =
            wcontainers_nir::direct::geometry(routed, bindings, &unnamed, &key).unwrap();
        for form in &converted.modules {
            wcontainers_nir::spirv::validate(&form.module).unwrap();
        }
    }
}
