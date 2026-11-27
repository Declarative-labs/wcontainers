// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! The conformance suite on the direct crates, over the EGL library in the folder WCONTAINERS_EGL_FOLDER names, or the
//! system's when it names none.

/// The folder the EGL library is loaded from.
fn folder() -> Option<std::path::PathBuf> {
    std::env::var_os("WCONTAINERS_EGL_FOLDER").map(std::path::PathBuf::from)
}

#[cfg(not(target_arch = "wasm32"))]
mod direct {
    wcontainers_opengl_es_conformance::tests!(
        wcontainers_egl_direct::Library,
        wcontainers_opengl_es_direct::OpenGlEsContext,
        super::folder(),
        wcontainers_opengl_es_direct::OpenGlEsContext::refusals,
        wcontainers_egl_direct::Library::refusals
    );
}
