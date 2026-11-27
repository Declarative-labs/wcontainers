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

/// What the direct crates add to the trait: a context that owns its EGL objects (NEEDED "After GI7" item 5).
#[cfg(not(target_arch = "wasm32"))]
mod owned {
    use wcontainers_egl::Egl;
    use wcontainers_egl::constants as egl;
    use wcontainers_egl_direct::Library;
    use wcontainers_opengl_es::OpenGlEs;
    use wcontainers_opengl_es::constants as gl;
    use wcontainers_opengl_es_direct::OpenGlEsContext;

    /// A context made by `owning` destroys its EGL context and surface at its drop: the glue refuses their
    /// handles after it, as ended.
    #[test]
    fn owning_destroys_at_drop() {
        let library = Library::load(super::folder().as_deref()).expect("EGL loads from the folder named");
        let display = library.get_display().expect("the default display");
        library.initialize(&display).expect("the display initializes");
        assert!(library.bind_api(egl::OPENGL_ES_API.cast_unsigned()));
        let mut configs = vec![None; 1];
        let wanted = [egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::RENDERABLE_TYPE, egl::OPENGL_ES3_BIT, egl::NONE];
        library.choose_config(&display, &wanted, &mut configs).expect("eglChooseConfig succeeds");
        let config = configs[0].clone().expect("a configuration");
        let context = library.create_context(&display, &config, None, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE]).expect("a context");
        let surface = library.create_pbuffer_surface(&display, &config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE]).expect("a surface");
        let owner = OpenGlEsContext::owning(library.clone(), display.clone(), context.clone(), Some(surface.clone()), Some(surface.clone()))
            .expect("the context binds");
        assert!(owner.get_string(gl::VERSION).is_some_and(|version| version.starts_with(b"OpenGL ES 3")));
        drop(owner);
        let before = library.refusals();
        assert!(!library.destroy_surface(&display, &surface), "the surface was destroyed at the drop");
        assert!(!library.destroy_context(&display, &context), "the context was destroyed at the drop");
        assert_eq!(library.refusals(), before + 2, "both refused by the glue itself: their handles had ended");
    }
}
