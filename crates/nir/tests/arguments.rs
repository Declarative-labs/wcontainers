//! Each argument of the route's steps (src/mesa.rs) changes what the step makes: for every argument
//! two values whose results differ, so a step that drops or replaces an argument fails here.
#![cfg(all(mesa, feature = "standalone"))]

#[path = "support/programs.rs"]
mod programs;
use programs::{CONTROL, POINT_GEOMETRY, POINT_VERTEX, SHADED_FRAGMENT};

use wcontainers_gpu::{reflect::SlotKind, shader::ShaderStage};
use wcontainers_mesa_compiler::Flat;
use wcontainers_nir::bind::stage_set;
use wcontainers_nir::mesa::{
    DrawSettings, GlslNameKind, NearestSampler, PositionTransform, Squares, Stage, Swizzle,
    TextureSwizzle, Wrap,
};

/// Mesa's compiler through our bridge.
type Bridge = wcontainers_mesa_compiler_direct::Bridge;
/// One stage of a GLSL program as the driver receives it.
type GlslStage = wcontainers_nir::mesa::GlslStage<Bridge>;
/// A stage the driver prepared.
type Prepared = wcontainers_nir::mesa::Prepared<Bridge>;
/// A program the driver's steps made.
type Made = wcontainers_nir::mesa::Made<Bridge>;
/// A program read from SPIR-V.
type SpirvProgram = wcontainers_nir::mesa::SpirvProgram<Bridge>;

/// The largest point size the tests draw squares with, and another.
const LARGEST: f32 = 64.0;
const SMALLER: f32 = 32.0;

/// The linked GLSL program of `sources`.
fn compile(sources: &[(Stage, &str)]) -> Vec<GlslStage> {
    GlslStage::compile(sources).unwrap()
}

/// The stage `stage` of the linked program of `sources`, prepared.
fn prepared_of(sources: &[(Stage, &str)], stage: Stage) -> Prepared {
    compile(sources)
        .into_iter()
        .find(|s| s.stage == stage)
        .unwrap()
        .received
        .prepare()
        .unwrap()
}

/// The program of the only source, prepared.
fn prepared(stage: Stage, source: &str) -> Prepared {
    prepared_of(&[(stage, source)], stage)
}

/// A prepared program's variant with `settings`, `swizzles` and `samplers`, flattened.
fn flat_with(
    program: &Prepared,
    settings: &DrawSettings,
    swizzles: &[TextureSwizzle],
    samplers: &[NearestSampler],
) -> Flat {
    program
        .variant(settings, swizzles, samplers)
        .unwrap()
        .flatten(0)
        .unwrap()
        .flat
}

/// A made program's variant in the set of `stage`, flattened.
fn made_flat(
    made: &Made,
    stage: Stage,
    swizzles: &[TextureSwizzle],
    samplers: &[NearestSampler],
) -> Flat {
    made.variant(stage, &DrawSettings::default(), swizzles, samplers)
        .unwrap()
        .flatten(0)
        .unwrap()
        .flat
}

/// The geometry program drawing a program's points as squares.
fn square_of(squares: Squares<Bridge>) -> Made {
    let Squares::Made { geometry, .. } = squares else {
        panic!("sized points are drawn as squares");
    };
    geometry
}

/// The vertex program copy also writing the point size, which drawing points as squares makes.
fn before_of(squares: Squares<Bridge>) -> Made {
    let Squares::Made {
        before: Some(before),
        ..
    } = squares
    else {
        panic!("a vertex program writing its point size has a copy writing it at a location");
    };
    before
}

const NAMED_VERTEX: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 passed;
uniform float vertex_scale;
void main() {
    gl_Position = at * vertex_scale;
    passed = at;
}
";

const NAMED_CONTROL: &str = "#version 450
layout(vertices = 3) out;
layout(location = 0) in vec4 passed[];
layout(location = 0) out vec4 handed[];
uniform float control_level;
void main() {
    handed[gl_InvocationID] = passed[gl_InvocationID];
    gl_out[gl_InvocationID].gl_Position = gl_in[gl_InvocationID].gl_Position;
    gl_TessLevelOuter[0] = control_level;
    gl_TessLevelOuter[1] = control_level;
    gl_TessLevelOuter[2] = control_level;
    gl_TessLevelInner[0] = control_level;
}
";

const NAMED_EVALUATION: &str = "#version 450
layout(triangles, equal_spacing, ccw) in;
layout(location = 0) in vec4 handed[];
layout(location = 0) out vec4 evaluated;
uniform float evaluation_scale;
void main() {
    gl_Position = (gl_TessCoord.x * gl_in[0].gl_Position + gl_TessCoord.y * gl_in[1].gl_Position
        + gl_TessCoord.z * gl_in[2].gl_Position) * evaluation_scale;
    evaluated = handed[0];
}
";

const NAMED_GEOMETRY: &str = "#version 450
layout(triangles) in;
layout(triangle_strip, max_vertices = 3) out;
layout(location = 0) in vec4 evaluated[];
layout(location = 0) out vec4 shaded;
uniform float geometry_scale;
void main() {
    for (int corner = 0; corner < 3; corner++) {
        gl_Position = gl_in[corner].gl_Position * geometry_scale;
        shaded = evaluated[corner];
        EmitVertex();
    }
    EndPrimitive();
}
";

const NAMED_FRAGMENT: &str = "#version 450
layout(location = 0) in vec4 shaded;
layout(location = 0) out vec4 colour;
uniform float fragment_scale;
void main() { colour = shaded * fragment_scale; }
";

#[test]
fn each_stage_of_a_linked_program_has_its_own_names() {
    let stages = compile(&[
        (Stage::Vertex, NAMED_VERTEX),
        (Stage::Fragment, NAMED_FRAGMENT),
        (Stage::Geometry, NAMED_GEOMETRY),
        (Stage::TessControl, NAMED_CONTROL),
        (Stage::TessEvaluation, NAMED_EVALUATION),
    ]);
    let own = [
        (Stage::Vertex, "vertex_scale"),
        (Stage::Fragment, "fragment_scale"),
        (Stage::Geometry, "geometry_scale"),
        (Stage::TessControl, "control_level"),
        (Stage::TessEvaluation, "evaluation_scale"),
    ];
    assert_eq!(stages.len(), own.len());
    for stage in &stages {
        let uniforms: Vec<&str> = stage
            .names
            .iter()
            .filter(|n| n.kind == GlslNameKind::Uniform)
            .map(|n| n.name.as_str())
            .collect();
        let (_, name) = own.iter().find(|(s, _)| *s == stage.stage).unwrap();
        assert_eq!(uniforms, [*name], "the {:?} stage's uniforms", stage.stage);
    }
}

/// The geometry program of [`POINT_GEOMETRY`] linked with [`POINT_VERTEX`] and [`SHADED_FRAGMENT`].
fn point_geometry() -> Prepared {
    prepared_of(
        &[
            (Stage::Vertex, POINT_VERTEX),
            (Stage::Fragment, SHADED_FRAGMENT),
            (Stage::Geometry, POINT_GEOMETRY),
        ],
        Stage::Geometry,
    )
}

#[test]
fn a_geometry_programs_squares_read_their_block_and_name_the_layer_after_the_coordinate() {
    let geometry = point_geometry();
    // `shaded` alone is generic, so the layer the prepared program names comes after location 0.
    assert_eq!(geometry.generic_end, 1);
    assert_eq!(geometry.named_layer, Some(1));
    let at = geometry.generic_end;
    let uniforms = |block| {
        let square = square_of(
            geometry
                .geometry_points_as_squares(at, LARGEST, block)
                .unwrap(),
        );
        // The point coordinate at `at`, so the layer at the location after it.
        assert_eq!(square.named_layer_location(), Some(at + 1));
        let drawn = square
            .variant(Stage::Compute, &DrawSettings::default(), &[], &[])
            .unwrap();
        drawn
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Uniform)
            .map(|b| b.register)
            .collect::<Vec<u32>>()
    };
    // The squares' block beside uniform buffer 0.
    let block = 3;
    assert!(uniforms(block).contains(&block), "{:?}", uniforms(block));
    assert!(!uniforms(0).contains(&block), "{:?}", uniforms(0));
}

#[test]
fn a_geometry_programs_squares_follow_their_location_and_largest_size() {
    let geometry = point_geometry();
    let at = geometry.generic_end;
    let flat = |at, largest| {
        let square = square_of(geometry.geometry_points_as_squares(at, largest, 0).unwrap());
        made_flat(&square, Stage::Compute, &[], &[])
    };
    assert_ne!(flat(at, LARGEST), flat(at + 2, LARGEST));
    assert_ne!(flat(at, LARGEST), flat(at, SMALLER));
}

const SIZED_POINTS: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 0) out vec4 colour;
void main() {
    gl_Position = at;
    gl_PointSize = at.z;
    colour = at.wzyx;
}
";

#[test]
fn a_vertex_programs_squares_follow_their_location_and_largest_size() {
    let vertex = prepared(Stage::Vertex, SIZED_POINTS);
    let at = vertex.generic_end;
    let flat = |at, largest| {
        made_flat(
            &square_of(vertex.points_as_squares(at, largest).unwrap()),
            Stage::Compute,
            &[],
            &[],
        )
    };
    assert_ne!(flat(at, LARGEST), flat(at + 2, LARGEST));
    assert_ne!(flat(at, LARGEST), flat(at, SMALLER));
}

/// An evaluation program drawing points, each of the size its patch's first point names.
const POINT_EVALUATION: &str = "#version 450
layout(triangles, equal_spacing, ccw, point_mode) in;
layout(location = 0) in vec4 handed[];
layout(location = 0) out vec4 shaded;
void main() {
    gl_Position = gl_TessCoord.x * gl_in[0].gl_Position + gl_TessCoord.y * gl_in[1].gl_Position
        + gl_TessCoord.z * gl_in[2].gl_Position;
    gl_PointSize = handed[0].w;
    shaded = handed[0];
}
";

#[test]
fn an_evaluation_programs_squares_follow_their_location_and_largest_size() {
    let evaluation = prepared_of(
        &[
            (Stage::Vertex, POINT_VERTEX),
            (Stage::Fragment, SHADED_FRAGMENT),
            (Stage::TessControl, CONTROL),
            (Stage::TessEvaluation, POINT_EVALUATION),
        ],
        Stage::TessEvaluation,
    );
    assert!(evaluation.makes_points);
    let at = evaluation.generic_end;
    let flat = |at, largest| {
        made_flat(
            &square_of(evaluation.points_as_squares(at, largest).unwrap()),
            Stage::Compute,
            &[],
            &[],
        )
    };
    assert_ne!(flat(at, LARGEST), flat(at + 2, LARGEST));
    assert_ne!(flat(at, LARGEST), flat(at, SMALLER));
}

/// Two fragment entry points of one module, each its own colour.
const TWO_ENTRY_POINTS: &str = "
@fragment
fn main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
@fragment
fn other() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
";

/// The SPIR-V naga writes for a WGSL program.
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
fn the_entry_point_read_is_the_one_named() {
    let words = spirv(TWO_ENTRY_POINTS);
    let key = |entry| {
        SpirvProgram::read(&words, Stage::Fragment, entry, false)
            .unwrap()
            .key()
            .unwrap()
    };
    assert_ne!(key("main"), key("other"));
}

/// A fragment program sampling two float textures, at units 0 and 1.
const TWO_TEXTURES: &str = "#version 450
layout(binding = 0) uniform sampler2D first;
layout(binding = 1) uniform sampler2D second;
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 colour;
void main() { colour = texture(first, uv) + 2.0 * texture(second, uv); }
";

/// The swizzle at `unit` taking the components in the order `swizzle`.
fn swizzled(unit: u32, swizzle: [Swizzle; 4]) -> TextureSwizzle {
    TextureSwizzle { unit, swizzle }
}

const YXZW: [Swizzle; 4] = [Swizzle::Y, Swizzle::X, Swizzle::Z, Swizzle::W];
const ZYXW: [Swizzle; 4] = [Swizzle::Z, Swizzle::Y, Swizzle::X, Swizzle::W];

#[test]
fn a_texture_swizzle_follows_its_unit_and_its_components() {
    let program = prepared(Stage::Fragment, TWO_TEXTURES);
    let flat =
        |swizzles: &[TextureSwizzle]| flat_with(&program, &DrawSettings::default(), swizzles, &[]);
    assert_ne!(flat(&[swizzled(0, YXZW)]), flat(&[]));
    assert_ne!(flat(&[swizzled(0, YXZW)]), flat(&[swizzled(1, YXZW)]));
    assert_ne!(flat(&[swizzled(0, YXZW)]), flat(&[swizzled(0, ZYXW)]));
}

/// A fragment program sampling an integer texture at unit 0.
const INTEGER_SAMPLED: &str = "#version 450
layout(binding = 0) uniform usampler2D steps;
layout(location = 0) in vec2 uv;
layout(location = 0) out uvec4 colour;
void main() { colour = texture(steps, uv); }
";

/// The nearest sampler at `unit`, repeating, selecting a level, every level.
fn repeating(unit: u32) -> NearestSampler {
    NearestSampler {
        unit,
        wrap: [Wrap::Repeat; 3],
        mipmapped: true,
        min_lod: 0f32.to_bits(),
        max_lod: f32::MAX.to_bits(),
        border: [0; 4],
    }
}

/// Samplers differing from [`repeating`] in one field each.
fn unlike_samplers(unit: u32) -> Vec<(&'static str, NearestSampler)> {
    let base = repeating(unit);
    let bordered = NearestSampler {
        wrap: [Wrap::ClampToBorder; 3],
        ..base
    };
    vec![
        ("unit", repeating(unit + 1)),
        (
            "wrap",
            NearestSampler {
                wrap: [Wrap::ClampToEdge; 3],
                ..base
            },
        ),
        (
            "mipmapped",
            NearestSampler {
                mipmapped: false,
                ..base
            },
        ),
        (
            "min_lod",
            NearestSampler {
                min_lod: 2f32.to_bits(),
                ..base
            },
        ),
        (
            "max_lod",
            NearestSampler {
                max_lod: 3f32.to_bits(),
                ..base
            },
        ),
        (
            "border",
            NearestSampler {
                border: [7; 4],
                ..bordered
            },
        ),
    ]
}

#[test]
fn a_nearest_sampler_follows_each_of_its_fields() {
    let program = prepared(Stage::Fragment, INTEGER_SAMPLED);
    let flat =
        |sampler: NearestSampler| flat_with(&program, &DrawSettings::default(), &[], &[sampler]);
    let bordered = NearestSampler {
        wrap: [Wrap::ClampToBorder; 3],
        ..repeating(0)
    };
    for (field, unlike) in unlike_samplers(0) {
        let like = if field == "border" {
            bordered
        } else {
            repeating(0)
        };
        assert_ne!(flat(like), flat(unlike), "a sampler unlike in {field}");
    }
}

/// A vertex program writing a point size it samples from a float texture at unit 0 and an integer
/// texture at unit 1.
const SAMPLED_POINT_SIZE: &str = "#version 450
layout(binding = 0) uniform sampler2D sizes;
layout(binding = 1) uniform usampler2D steps;
layout(location = 0) in vec4 at;
void main() {
    gl_Position = at;
    gl_PointSize = texture(sizes, at.xy).x + float(texture(steps, at.xy).x);
}
";

#[test]
fn a_made_programs_variant_follows_its_stage_swizzles_and_samplers() {
    let vertex = prepared(Stage::Vertex, SAMPLED_POINT_SIZE);
    let before = || {
        before_of(
            vertex
                .points_as_squares(vertex.generic_end, LARGEST)
                .unwrap(),
        )
    };
    let made = before();
    let sets = |stage| {
        made.variant(stage, &DrawSettings::default(), &[], &[])
            .unwrap()
            .bindings
            .iter()
            .map(|b| b.set)
            .collect::<Vec<_>>()
    };
    let vertex_set = stage_set(ShaderStage::Vertex);
    let fragment_set = stage_set(ShaderStage::Fragment);
    assert_ne!(vertex_set, fragment_set);
    assert!(sets(Stage::Vertex).iter().all(|&s| s == vertex_set));
    assert!(sets(Stage::Fragment).iter().all(|&s| s == fragment_set));
    assert!(!sets(Stage::Vertex).is_empty());
    let flat = |swizzles: &[TextureSwizzle], samplers: &[NearestSampler]| {
        made_flat(&made, Stage::Vertex, swizzles, samplers)
    };
    assert_ne!(flat(&[swizzled(0, YXZW)], &[]), flat(&[], &[]));
    let mipmapped = repeating(1);
    let single_level = NearestSampler {
        mipmapped: false,
        ..mipmapped
    };
    assert_ne!(flat(&[], &[mipmapped]), flat(&[], &[single_level]));
}

#[test]
fn each_part_of_the_position_transform_changes_the_program() {
    // It writes a point size, which a fixed point size removes.
    let vertex = prepared(Stage::Vertex, SIZED_POINTS);
    let flat = |transform: PositionTransform, key_transform: PositionTransform| {
        let settings = DrawSettings {
            transform,
            key_transform,
            ..DrawSettings::default()
        };
        flat_with(&vertex, &settings, &[], &[])
    };
    let none = PositionTransform::default();
    let parts = [
        PositionTransform {
            flip_y: true,
            ..none
        },
        PositionTransform {
            half_z: true,
            ..none
        },
        PositionTransform {
            reverse_z: true,
            ..none
        },
        PositionTransform {
            fixed_point_size: true,
            ..none
        },
    ];
    let plain = flat(none, none);
    for part in parts {
        assert_ne!(flat(part, part), plain, "{part:?}");
    }
    // The program follows `transform`; only its key follows `key_transform`.
    assert_ne!(flat(parts[0], none), plain);
    let key = |key_transform| {
        let settings = DrawSettings {
            key_transform,
            ..DrawSettings::default()
        };
        vertex.variant(&settings, &[], &[]).unwrap().key
    };
    assert_ne!(key(parts[0]), key(none));
    assert_eq!(flat(none, parts[0]), plain);
}

/// A fragment program writing its sample mask.
const SAMPLE_MASK: &str = "#version 450
layout(location = 0) out vec4 colour;
void main() {
    colour = vec4(1.0);
    gl_SampleMask[0] = 1;
}
";

#[test]
fn a_sample_mask_is_fitted_to_single_sampled_targets_only() {
    let program = prepared(Stage::Fragment, SAMPLE_MASK);
    assert!(program.writes_sample_mask);
    let flat = |samples| {
        let settings = DrawSettings {
            samples,
            ..DrawSettings::default()
        };
        flat_with(&program, &settings, &[], &[])
    };
    assert_ne!(flat(1), flat(4));
}

/// A fragment program sampling an array of three textures at an index known only when run.
const TEXTURE_ARRAY: &str = "#version 450
layout(binding = 0) uniform sampler2D pictures[3];
layout(location = 0) flat in int which;
layout(location = 1) in vec2 uv;
layout(location = 0) out vec4 colour;
void main() { colour = texture(pictures[which], uv); }
";

/// A compute program writing two storage blocks.
const TWO_STRIDES: &str = "#version 450
layout(local_size_x = 1) in;
layout(std430, binding = 0) buffer Words { uint words[]; };
layout(std430, binding = 1) buffer Vectors { vec4 vectors[]; };
void main() {
    words[gl_GlobalInvocationID.x] = 1u;
    vectors[gl_GlobalInvocationID.x] = vec4(2.0);
}
";

#[test]
fn a_binding_reflects_its_count_and_its_stride() {
    let pictures = prepared(Stage::Fragment, TEXTURE_ARRAY)
        .variant(&DrawSettings::default(), &[], &[])
        .unwrap()
        .bindings;
    let counts: Vec<u32> = pictures
        .iter()
        .filter(|b| b.kind == SlotKind::Texture)
        .map(|b| b.count)
        .collect();
    assert_eq!(counts, [3]);
    let storage = prepared(Stage::Compute, TWO_STRIDES)
        .variant(&DrawSettings::default(), &[], &[])
        .unwrap()
        .bindings;
    let mut strides: Vec<(u32, u32)> = storage
        .iter()
        .filter(|b| b.kind == SlotKind::Storage)
        .map(|b| (b.register, b.stride))
        .collect();
    strides.sort_unstable();
    // Storage blocks follow uniform buffer 0, each an array of words.
    let word = 4;
    assert_eq!(strides, [(1, word), (2, word)]);
}

const TYPED_INPUTS: &str = "#version 450
layout(location = 0) in vec4 at;
layout(location = 1) in float weight;
void main() { gl_Position = at * weight; }
";

#[test]
fn a_named_input_carries_its_type() {
    let stages = compile(&[(Stage::Vertex, TYPED_INPUTS)]);
    let ty = |name: &str| {
        stages[0]
            .names
            .iter()
            .find(|n| n.kind == GlslNameKind::Input && n.name == name)
            .and_then(|n| n.ty.clone())
    };
    assert!(ty("at").is_some());
    assert!(ty("weight").is_some());
    assert_ne!(ty("at"), ty("weight"));
}

/// A fragment program reading its first target's texel (framebuffer fetch).
const FETCHING: &str = "#version 320 es
#extension GL_EXT_shader_framebuffer_fetch_non_coherent : require
precision highp float;
layout(noncoherent, location = 0) inout highp vec4 colour;
void main() { colour = colour * 0.5; }
";

#[test]
fn a_target_fetched_is_read_from_the_unit_and_kind_of_texture_named() {
    let program = prepared(Stage::Fragment, FETCHING);
    assert!(program.fetches_target);
    let flat = |fetch_unit, fetch_multisampled| {
        let settings = DrawSettings {
            fetch_unit,
            fetch_multisampled,
            ..DrawSettings::default()
        };
        flat_with(&program, &settings, &[], &[])
    };
    // The common form numbers the texture by its kind, so its unit shows in its reflection.
    let units = |fetch_unit| {
        let settings = DrawSettings {
            fetch_unit,
            ..DrawSettings::default()
        };
        program
            .variant(&settings, &[], &[])
            .unwrap()
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Texture)
            .map(|b| b.register)
            .collect::<Vec<u32>>()
    };
    let (unit, other) = (3, 5);
    assert_eq!(units(unit), [unit]);
    assert_eq!(units(other), [other]);
    assert_ne!(flat(unit, false), flat(unit, true));
}
