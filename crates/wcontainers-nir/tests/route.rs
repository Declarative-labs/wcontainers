//! Both candidates end to end on programs naga writes as SPIR-V: SPIR-V to NIR, NIR's serialised
//! key, the common form's step, then Zink's SPIR-V and naga's reader (A) or NIR read straight into
//! naga (B), each validated.
#![cfg(all(mesa, feature = "standalone"))]

use wcontainers_gpu::{ShaderStage, SlotKind};
use wcontainers_nir::ConvertError;
use wcontainers_nir::bind::stage_set;
use wcontainers_nir::mesa::{GlslNameKind, GlslStage, Nir, Stage};
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
    let first = SlotKind::first_binding;
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
