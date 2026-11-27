//! The conformance suite's measurement of a draw's cost on the direct crates, a program of its own over the EGL
//! library in the folder WCONTAINERS_EGL_FOLDER names (the system's when it names none): WCONTAINERS_MEASURE_DRAWS
//! names the draws; it fails without it.

fn main() {
    let folder = std::env::var_os("WCONTAINERS_EGL_FOLDER").map(std::path::PathBuf::from);
    wcontainers_opengl_es_conformance::measure_from_the_environment::<wcontainers_egl_direct::Library, wcontainers_opengl_es_direct::OpenGlEsContext>(folder.as_deref());
}
