//! The changes our naga carries in its writers, which the back ends rely on.

use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn write_glsl(wgsl: &str, stage: naga::ShaderStage, flags: glsl::WriterFlags) -> String {
    let module = naga::front::wgsl::parse_str(wgsl).expect("parse");
    let info = Validator::new(ValidationFlags::all(), Capabilities::MULTISAMPLED_SHADING)
        .validate(&module)
        .expect("validate");
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: flags,
        ..Default::default()
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage: stage,
        entry_point: "main".into(),
        multiview: None,
    };
    let mut out = String::new();
    glsl::Writer::new(
        &mut out,
        &module,
        &info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .expect("writer")
    .write()
    .expect("write");
    out
}

const NUMBERS: &str = "
@vertex fn main(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> @builtin(position) vec4f {
    return vec4f(f32(v), f32(i), 0.0, 1.0);
}";

#[test]
fn vertex_index_adds_the_base_vertex_uniform() {
    let out = write_glsl(
        NUMBERS,
        naga::ShaderStage::Vertex,
        glsl::WriterFlags::empty(),
    );
    assert!(out.contains("uniform uint naga_vs_base_vertex;"), "{out}");
    assert!(
        out.contains("(uint(gl_VertexID) + naga_vs_base_vertex)"),
        "{out}"
    );
    assert!(
        out.contains("uniform uint naga_vs_first_instance;"),
        "{out}"
    );
    assert!(
        out.contains("(uint(gl_InstanceID) + naga_vs_first_instance)"),
        "{out}"
    );
}

#[test]
fn draw_parameters_keep_gl_vertex_id() {
    let out = write_glsl(
        NUMBERS,
        naga::ShaderStage::Vertex,
        glsl::WriterFlags::DRAW_PARAMETERS,
    );
    assert!(!out.contains("naga_vs_base_vertex"), "{out}");
    assert!(out.contains("uint(gl_VertexID)"), "{out}");
}

#[test]
fn no_uniform_without_vertex_index() {
    let wgsl = "@vertex fn main() -> @builtin(position) vec4f { return vec4f(0.0); }";
    let out = write_glsl(wgsl, naga::ShaderStage::Vertex, glsl::WriterFlags::empty());
    assert!(!out.contains("naga_vs_base_vertex"), "{out}");
}

#[test]
fn sample_index_is_converted_to_unsigned() {
    let wgsl = "
@fragment fn main(@builtin(sample_index) s: u32) -> @location(0) vec4u {
    return vec4u(s);
}";
    let out = write_glsl(
        wgsl,
        naga::ShaderStage::Fragment,
        glsl::WriterFlags::empty(),
    );
    assert!(out.contains("uint(gl_SampleID)"), "{out}");
}

#[test]
fn sample_mask_is_an_int_array_with_its_extension() {
    let wgsl = "
struct Out { @location(0) c: vec4f, @builtin(sample_mask) m: u32 }
@fragment fn main(@builtin(sample_mask) m: u32) -> Out {
    return Out(vec4f(1.0), m & 5u);
}";
    let out = write_glsl(
        wgsl,
        naga::ShaderStage::Fragment,
        glsl::WriterFlags::empty(),
    );
    assert!(
        out.contains("#extension GL_OES_sample_variables : require"),
        "{out}"
    );
    assert!(out.contains("uint(gl_SampleMaskIn[0])"), "{out}");
    assert!(out.contains("gl_SampleMask[0] = int("), "{out}");
}

#[test]
fn wgsl_literals_near_the_largest_f32_stay_in_range() {
    let wgsl = "
@fragment fn main(@location(0) x: f32) -> @location(0) vec4f {
    return vec4f(select(0.0, 1.0, abs(x) <= 3.4028234663852886e38f), 0.1, -3.4028234663852886e38f, 1.0);
}";
    let module = naga::front::wgsl::parse_str(wgsl).expect("parse");
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .expect("validate");
    let out =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .expect("write");
    // f32::MAX's own digits, which no WGSL compiler reads as out of range; short ones elsewhere.
    assert!(
        out.contains("340282346638528860000000000000000000000f"),
        "{out}"
    );
    assert!(
        !out.contains("340282350000000000000000000000000000000f"),
        "{out}"
    );
    assert!(out.contains("0.1f"), "{out}");
}
