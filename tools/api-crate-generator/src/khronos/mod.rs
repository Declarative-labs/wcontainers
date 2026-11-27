//! The generator's front end for Khronos's registries (gl.xml, egl.xml, and ANGLE's gl_angle_ext.xml for ANGLE's own
//! extensions): the OpenGL ES and EGL crates the WebGL route calls natively.
//!
//!   api-crate-generator khronos <gl.xml> <egl.xml> <gl_angle_ext.xml> <output folder>

mod classify;
mod registry;
mod sets;
mod write;

pub use classify::scalar as classify_scalar;

const TEMPLATES: write::Templates = write::Templates {
    egl_direct: include_str!("../../templates/khronos/egl-direct.rs"),
    egl_manifest: include_str!("../../templates/khronos/egl-direct.toml"),
    gl_direct: include_str!("../../templates/khronos/opengl-es-direct.rs"),
    gl_manifest: include_str!("../../templates/khronos/opengl-es-direct.toml"),
    gl_direct_test: include_str!("../../templates/khronos/opengl-es-direct-test.rs"),
    gl_suite: include_str!("../../templates/khronos/opengl-es-suite.rs"),
    gl_suite_manifest: include_str!("../../templates/khronos/opengl-es-suite.toml"),
};

fn read(path: &str, api: &str) -> Result<registry::Registry, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    registry::read(&text, api).map_err(|error| format!("{path}: {error}"))
}

/// Writes the OpenGL ES and EGL crates into `out/crates`, and returns the report.
pub fn generate(gl: &str, egl: &str, angle: &str, out: &std::path::Path) -> Result<String, String> {
    let mut gl_registry = read(gl, "gles2")?;
    gl_registry.merge(read(angle, "gles2")?)?;
    let egl_registry = read(egl, "egl")?;
    let mut report = String::new();
    let mut failures = Vec::new();
    let mut written = Vec::new();
    for (set, registry) in [(&sets::EGL, &egl_registry), (&sets::OPENGL_ES, &gl_registry)] {
        match classify::classify(registry, set) {
            Ok(classified) => written.push((set, registry, classified)),
            Err(failure) => failures.push(failure),
        }
    }
    if !failures.is_empty() {
        return Err(format!("CLASSIFY FAILED:\n{}", failures.join("\n")));
    }
    for (set, registry, classified) in &written {
        report.push_str(&write::write(registry, &[set], std::slice::from_ref(classified), out, &TEMPLATES)?);
    }
    Ok(report)
}
