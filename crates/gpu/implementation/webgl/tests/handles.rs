//! A handle naming an object the back end no longer holds is refused with the reason every such
//! handle gets (plan 4.10, `GpuError::GONE`), before any other check of the call.

use wcontainers_gpu::{Gpu, Object, StandaloneGpu, error::GpuError, resource::QueryKind};

type Gl = wcontainers_gpu_webgl::Gl<wcontainers_opengl_es_direct::OpenGlEsContext>;

#[test]
fn a_destroyed_query_set_is_refused_by_write_timestamp() {
    let mut gl = Gl::new(wcontainers_gpu_webgl::GlConfig {
        egl: std::env::var_os("WEBGL_ANGLE_DIR").map(std::path::PathBuf::from),
        off: Vec::new(),
    })
    .expect("an OpenGL ES 3.0 context from the EGL library in WEBGL_ANGLE_DIR, or the system's");
    let set = gl
        .create_query_set(QueryKind::OcclusionAny, 1)
        .expect("an occlusion set, which every OpenGL ES 3.0 device makes");
    gl.destroy(Object::QuerySet(set.clone()));
    assert_eq!(
        gl.write_timestamp(&set, 0),
        Err(GpuError::Invalid(GpuError::GONE))
    );
}
