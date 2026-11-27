// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! The conformance suite's fuzzing on the direct crates, a program of its own over the EGL library in the folder
//! WCONTAINERS_EGL_FOLDER names (the system's when it names none): WCONTAINERS_FUZZ_SEED and WCONTAINERS_FUZZ_CALLS
//! name the seed and the calls; it fails without them.

fn main() {
    let folder = std::env::var_os("WCONTAINERS_EGL_FOLDER").map(std::path::PathBuf::from);
    let counts = wcontainers_opengl_es_conformance::Counts { gl: wcontainers_opengl_es_direct::OpenGlEsContext::refusals, egl: wcontainers_egl_direct::Library::refusals };
    wcontainers_opengl_es_conformance::fuzz_from_the_environment::<wcontainers_egl_direct::Library, wcontainers_opengl_es_direct::OpenGlEsContext>(folder.as_deref(), &counts);
}
