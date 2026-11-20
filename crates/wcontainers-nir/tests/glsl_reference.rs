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
#[path = "corpus/deqp.rs"]
mod deqp;
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
#[path = "corpus/godot.rs"]
mod godot;
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/godot_rd.rs"]
mod godot_rd;
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
#[allow(
    dead_code,
    reason = "the corpus harness; this test runs one program through it"
)]
#[path = "corpus/table.rs"]
mod table;

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
    c[i] = textureLod(colours, at, 0.0) + texelFetch(colours, ivec2(i % 4u, 1), 0)
        + texelFetchOffset(colours, ivec2(i % 2u, 1), 0, ivec2(1, 2));
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

const LIBRARY_FILE: &str = r#"# a comment
group basic "Basic"
    case add
        version 300 es
        values
        {
            input float in0 = [ 0.5 | 1.0 ];
            input int in1 = [ 1 | -2 ];
            output bvec2 out0 = [ bvec2(true, false) | bvec2(false, true) ];
            output mat2 out1 = [ mat2(1.0, 0.0, 0.0, 1.0) | mat2(2.0, 0.0, 0.0, 2.0) ];
        }

        both ""
            #version 300 es
            precision mediump float;
            ${DECLARATIONS}
            void main()
            {
                ${SETUP}
                out0 = bvec2(in0 > 0.75, in1 < 0);
                out1 = mat2(in0);
                ${OUTPUT}
            }
        ""
    end

    case wrong
        expect compile_fail
        both ""
            void main() { undeclared = 1; }
        ""
    end
end

case pair
    version 100 es
    vertex ""
        ${VERTEX_DECLARATIONS}
        varying mediump float v;
        void main() { v = dEQP_Position.x; ${VERTEX_OUTPUT} }
    ""
    fragment ""
        precision mediump float;
        ${FRAGMENT_DECLARATIONS}
        varying mediump float v;
        void main() { ${FRAG_COLOR} = vec4(v); }
    ""
end
"#;

/// A shader file in the format of Godot's Compatibility renderer, and a file it includes.
const GODOT_SHADER: &str = "/* clang-format off */
#[modes]

mode_plain =
mode_tinted = #define TINT \\n#define TINT_TWICE

#[specializations]

USE_FOG = false
USE_LIGHT = true

#[vertex]

layout(location = 0) in vec2 vertex_attrib;
out vec2 uv;
#GLOBALS
void main() {
\tuv = vertex_attrib;
\tgl_Position = vec4(vertex_attrib, 0.0, 1.0);
}

#[fragment]

#include \"godot_inc.glsl\"
in vec2 uv;
uniform vec4 tint;
layout(location = 0) out vec4 frag_color;
void main() {
#CODE : FRAGMENT
\tfrag_color = shade(uv);
#ifdef TINT_TWICE
\tfrag_color *= tint * tint;
#endif
#ifdef USE_LIGHT
\tfrag_color.a = float(MAX_VIEWS);
#endif
}
";
const GODOT_INCLUDED: &str = "vec4 shade(vec2 at) { return vec4(at, ViewIndex, 1.0); }\n";

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
fn a_deqp_library_file_gives_each_case_its_program() {
    use wcontainers_nir::mesa::Stage::{Fragment, Vertex};
    let vertex = deqp::case_program(LIBRARY_FILE, "basic.add_vertex").unwrap();
    assert_eq!(
        vertex.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
        [Vertex, Fragment]
    );
    assert!(vertex[0].1.contains("in highp vec4 dEQP_Position;"));
    assert!(vertex[0].1.contains("int in1 = int(a_in1);"));
    assert!(vertex[0].1.contains("v_out0 = vec2(out0);"));
    assert!(!vertex[0].1.contains("${"));
    assert!(
        vertex[1]
            .1
            .contains("wc_output_1 = vec4(out1[1][0], out1[1][1], 0.0, 0.0);")
    );
    let fragment = deqp::case_program(LIBRARY_FILE, "basic.add_fragment").unwrap();
    assert_eq!(fragment.len(), 1);
    assert!(
        fragment[0]
            .1
            .contains("dEQP_FragColor = vec4(out1[0][0], out1[0][1], out1[1][0], out1[1][1]);")
    );
    assert!(
        fragment[0]
            .1
            .contains("wc_output_1 = uvec4(uint(out0[0]), uint(out0[1]), 0u, 0u);")
    );
    assert!(
        deqp::case_program(LIBRARY_FILE, "basic.wrong_vertex")
            .unwrap_err()
            .contains("compile_fail")
    );
    let pair = deqp::case_program(LIBRARY_FILE, "pair").unwrap();
    assert!(pair[0].1.contains("attribute highp vec4 dEQP_Position;"));
    assert!(pair[1].1.contains("gl_FragColor = vec4(v);"));
    for program in [&vertex, &fragment, &pair] {
        let given: Vec<_> = program.iter().map(|(s, t)| (*s, t.as_str())).collect();
        Nir::from_glsl(&given).unwrap_or_else(|e| panic!("{program:?}: {e}"));
    }
}

#[test]
fn a_godot_shader_file_gives_each_variant_its_program() {
    let folder = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("drivers/gles3/shaders");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("godot.glsl");
    std::fs::write(&file, GODOT_SHADER).unwrap();
    std::fs::write(folder.join("godot_inc.glsl"), GODOT_INCLUDED).unwrap();
    assert!(godot::is_shader(&file));
    let items: Vec<String> = godot::items(&file)
        .iter()
        .map(|i| i.display().to_string())
        .collect();
    let named = |v: &str| format!("{}#{v}", file.display());
    assert_eq!(
        items,
        [
            "MODE_PLAIN",
            "MODE_PLAIN:USE_FOG",
            "MODE_PLAIN:USE_LIGHT",
            "MODE_TINTED",
            "MODE_TINTED:USE_FOG",
            "MODE_TINTED:USE_LIGHT"
        ]
        .map(named)
    );
    assert!(godot::items(&folder.join("godot_inc.glsl")).is_empty());
    let program = godot::sources(std::path::Path::new(&named("MODE_TINTED:USE_FOG")))
        .unwrap()
        .unwrap();
    let fragment = &program[1].1;
    assert!(fragment.starts_with("#version 330\n#define USE_GLES_OVER_GL\n"));
    assert!(fragment.contains("#define USE_FOG\n#define USE_LIGHT\n"));
    assert!(fragment.contains("#define TINT \n#define TINT_TWICE\n"));
    assert!(fragment.contains("vec4 shade(vec2 at)"));
    assert!(!fragment.contains("#CODE"));
    assert!(!program[0].1.contains("#GLOBALS"));
    let plain = godot::sources(std::path::Path::new(&named("MODE_PLAIN:USE_LIGHT")))
        .unwrap()
        .unwrap();
    assert!(!plain[1].1.contains("#define USE_LIGHT"));
    for program in [&program, &plain] {
        let given: Vec<_> = program.iter().map(|(s, t)| (*s, t.as_str())).collect();
        Nir::from_glsl(&given).unwrap_or_else(|e| panic!("{program:?}: {e}"));
    }
    let defines = "# a comment\nscene.glsl #define A 1\\n\neffects/copy.glsl #define B\n";
    assert_eq!(
        table::general_defines(defines, std::path::Path::new("effects/copy.glsl")),
        "#define B"
    );
    assert_eq!(
        table::general_defines(defines, std::path::Path::new("sky.glsl")),
        ""
    );
}

#[test]
fn a_godot_rd_variant_is_made_as_shader_rd_makes_it() {
    let tree = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("godot-rd");
    let folder = tree.join("shaders");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("blur_inc.glsl"),
        "float weight() { return 1.0; } // a comment\n",
    )
    .unwrap();
    let file = folder.join("blur.glsl");
    std::fs::write(
        &file,
        "#[vertex]\n#version 450\n#VERSION_DEFINES\n#include \"blur_inc.glsl\"\n\
         #GLOBALS\nvoid main() {}\n#[fragment]\n#version 450\n#VERSION_DEFINES\n\
         #include \"blur_inc.glsl\"\n#CODE : FRAGMENT\nvoid main() {}\n",
    )
    .unwrap();
    let stages = godot_rd::variant_sources(&tree, &file, "#define G\n", "\n#define V\n").unwrap();
    let defines = "\n#define G\n\n#define V\n\n#define RENDER_DRIVER_VULKAN\n\
                   #define samplerExternalOES sampler2D\n#define textureExternalOES texture2D\n";
    let body = "float weight() { return 1.0; } \nvoid main() {}\n";
    let expected = format!("#version 450\n{defines}{body}");
    let extensions: Vec<_> = stages.iter().map(|(e, _)| *e).collect();
    assert_eq!(extensions, ["vert", "frag"]);
    for (_, source) in &stages {
        assert_eq!(source, &expected);
    }
}

#[test]
fn a_piglit_file_gives_its_stages() {
    let sources = glsl::shader_test_sources(TEST_FILE).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].0, Stage::Compute);
    assert!(sources[0].1.starts_with("#version 430"));
    assert!(!sources[0].1.contains("compute 1 1 1"));
}
