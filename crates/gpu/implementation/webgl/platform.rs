//! The OpenGL ES calls in the forms the back end uses them, and where its context comes from: natively an
//! OpenGL ES 3 context of an EGL library (ANGLE's, the layer Chrome itself draws WebGL 2 with, or Mesa's), made
//! through the OpenGL ES and EGL crates' traits. The browser's WebGL 2 waits for those crates' implementation on
//! web-sys (NEEDED-FROM-API-CRATES.md in the track's scratch folder).

use std::collections::BTreeSet;
use wcontainers_opengl_es::OpenGlEs;
use wcontainers_opengl_es::constants as gles;

/// The WebGL extension names, with the GL names ANGLE reports for each natively.
const EXTENSIONS: &[(&str, &str)] = &[
    ("EXT_clip_control", "GL_EXT_clip_control"),
    ("WEBGL_provoking_vertex", "GL_ANGLE_provoking_vertex"),
    ("EXT_color_buffer_float", "GL_EXT_color_buffer_float"),
    ("OES_texture_float_linear", "GL_OES_texture_float_linear"),
    ("EXT_float_blend", "GL_EXT_float_blend"),
    (
        "KHR_parallel_shader_compile",
        "GL_KHR_parallel_shader_compile",
    ),
    (
        "EXT_texture_filter_anisotropic",
        "GL_EXT_texture_filter_anisotropic",
    ),
    ("WEBGL_stencil_texturing", "GL_ANGLE_stencil_texturing"),
    (
        "EXT_disjoint_timer_query_webgl2",
        "GL_EXT_disjoint_timer_query",
    ),
    // ANGLE reports DXT1, DXT3 and DXT5 apart; WebGL's extension needs all three.
    (
        "WEBGL_compressed_texture_s3tc",
        "GL_ANGLE_texture_compression_dxt5",
    ),
    (
        "WEBGL_compressed_texture_s3tc_srgb",
        "GL_EXT_texture_compression_s3tc_srgb",
    ),
    (
        "EXT_texture_compression_rgtc",
        "GL_EXT_texture_compression_rgtc",
    ),
    (
        "EXT_texture_compression_bptc",
        "GL_EXT_texture_compression_bptc",
    ),
    ("EXT_texture_norm16", "GL_EXT_texture_norm16"),
    ("EXT_render_snorm", "GL_EXT_render_snorm"),
    ("OES_draw_buffers_indexed", "GL_OES_draw_buffers_indexed"),
    ("WEBGL_blend_func_extended", "GL_EXT_blend_func_extended"),
    ("WEBGL_polygon_mode", "GL_ANGLE_polygon_mode"),
    ("EXT_depth_clamp", "GL_EXT_depth_clamp"),
    ("EXT_polygon_offset_clamp", "GL_EXT_polygon_offset_clamp"),
    (
        "EXT_texture_mirror_clamp_to_edge",
        "GL_EXT_texture_mirror_clamp_to_edge",
    ),
    ("WEBGL_clip_cull_distance", "GL_ANGLE_clip_cull_distance"),
    (
        NOPERSPECTIVE_INTERPOLATION,
        crate::shader::NOPERSPECTIVE_EXTENSION,
    ),
    (SAMPLE_VARIABLES, "GL_OES_sample_variables"),
    (
        MULTISAMPLE_INTERPOLATION,
        crate::shader::SAMPLE_QUALIFIER_EXTENSION,
    ),
];

/// The WebGL extension giving GLSL's `noperspective` qualifier.
pub const NOPERSPECTIVE_INTERPOLATION: &str = "NV_shader_noperspective_interpolation";

/// The WebGL extension giving the sample number, position and mask to fragment programs.
pub const SAMPLE_VARIABLES: &str = "OES_sample_variables";

/// The WebGL extension giving GLSL's `sample` qualifier.
pub const MULTISAMPLE_INTERPOLATION: &str = "OES_shader_multisample_interpolation";

/// The extensions the back end uses that the context has, by WebGL name, less those switched off.
pub(crate) fn extensions(have: &BTreeSet<String>, off: &[&str]) -> BTreeSet<&'static str> {
    EXTENSIONS
        .iter()
        .filter(|(web, native)| {
            (have.contains(*web) || have.contains(*native)) && !off.contains(web)
        })
        .map(|(web, _)| *web)
        .collect()
}

/// Every extension the context names.
pub(crate) fn supported(gl: &impl OpenGlEs) -> BTreeSet<String> {
    let mut count = [0];
    gl.get_integerv(gles::NUM_EXTENSIONS, &mut count);
    (0..count[0].max(0) as u32)
        .filter_map(|i| gl.get_stringi(gles::EXTENSIONS, i))
        .map(|name| String::from_utf8_lossy(&name).into_owned())
        .collect()
}

/// A string of the context's, empty where it gives none.
pub(crate) fn string(gl: &impl OpenGlEs, name: u32) -> String {
    gl.get_string(name)
        .map(|text| String::from_utf8_lossy(&text).into_owned())
        .unwrap_or_default()
}

/// One integer of the context's state.
pub(crate) fn integer(gl: &impl OpenGlEs, name: u32) -> i32 {
    let mut value = [0];
    gl.get_integerv(name, &mut value);
    value[0]
}

/// One float of the context's state.
pub(crate) fn float(gl: &impl OpenGlEs, name: u32) -> f32 {
    let mut value = [0.0];
    gl.get_floatv(name, &mut value);
    value[0]
}

/// One new object name from a `glGen*` call; none where GL gave 0.
pub(crate) fn one(generate: impl FnOnce(&mut [u32])) -> Option<u32> {
    let mut name = [0];
    generate(&mut name);
    (name[0] != 0).then_some(name[0])
}

/// A uniform's location in a linked program; none where the program has no such uniform.
pub(crate) fn location(gl: &impl OpenGlEs, program: u32, name: &str) -> Option<i32> {
    let name = std::ffi::CString::new(name).ok()?;
    Some(gl.get_uniform_location(program, &name)).filter(|&at| at >= 0)
}

/// A uniform block's index in a linked program; none where the program has no such block.
pub(crate) fn block_index(gl: &impl OpenGlEs, program: u32, name: &str) -> Option<u32> {
    let name = std::ffi::CString::new(name).ok()?;
    Some(gl.get_uniform_block_index(program, &name)).filter(|&index| index != gles::INVALID_INDEX)
}

/// Whether a shader compiled.
pub(crate) fn compiled(gl: &impl OpenGlEs, shader: u32) -> bool {
    let mut status = [0];
    gl.get_shaderiv(shader, gles::COMPILE_STATUS, &mut status);
    status[0] != 0
}

/// Whether a program linked.
pub(crate) fn linked(gl: &impl OpenGlEs, program: u32) -> bool {
    let mut status = [0];
    gl.get_programiv(program, gles::LINK_STATUS, &mut status);
    status[0] != 0
}

/// Whether a program's compile and link have ended (KHR_parallel_shader_compile).
pub(crate) fn completed(gl: &impl OpenGlEs, program: u32) -> bool {
    let mut status = [0];
    gl.get_programiv(program, gles::COMPLETION_STATUS_KHR, &mut status);
    status[0] != 0
}

/// A log of `glGetShaderInfoLog` or `glGetProgramInfoLog`, as text.
fn log(length: i32, read: impl FnOnce(&mut [u8]) -> i32) -> String {
    let mut bytes = vec![0; length.max(1) as usize];
    let written = read(&mut bytes).clamp(0, bytes.len() as i32) as usize;
    bytes.truncate(written);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A shader's compile log.
pub(crate) fn shader_log(gl: &impl OpenGlEs, shader: u32) -> String {
    let mut length = [0];
    gl.get_shaderiv(shader, gles::INFO_LOG_LENGTH, &mut length);
    log(length[0], |bytes| gl.get_shader_info_log(shader, bytes))
}

/// A program's link log.
pub(crate) fn program_log(gl: &impl OpenGlEs, program: u32) -> String {
    let mut length = [0];
    gl.get_programiv(program, gles::INFO_LOG_LENGTH, &mut length);
    log(length[0], |bytes| gl.get_program_info_log(program, bytes))
}

/// One value of a query object.
pub(crate) fn query_value(gl: &impl OpenGlEs, query: u32, name: u32) -> u32 {
    let mut value = [0];
    gl.get_query_objectuiv(query, name, &mut value);
    value[0]
}

/// How many bits the GPU's timestamps have; zero where it writes none.
pub(crate) fn timestamp_bits(gl: &impl OpenGlEs) -> u32 {
    let mut bits = [0];
    match gl.get_queryiv_ext(gles::TIMESTAMP_EXT, gles::QUERY_COUNTER_BITS_EXT, &mut bits) {
        Some(()) => bits[0].max(0) as u32,
        None => 0,
    }
}

/// The sample counts GL takes for a renderbuffer of `internal`.
pub(crate) fn sample_counts(gl: &impl OpenGlEs, internal: u32) -> Vec<i32> {
    let mut n = [0];
    gl.get_internalformativ(
        gles::RENDERBUFFER,
        internal,
        gles::NUM_SAMPLE_COUNTS,
        &mut n,
    );
    let mut counts = vec![0; n[0].max(0) as usize];
    if !counts.is_empty() {
        gl.get_internalformativ(gles::RENDERBUFFER, internal, gles::SAMPLES, &mut counts);
    }
    counts
}

/// Copies bytes of the buffer bound at `target` into `dst`, waiting for the GPU: what WebGL's
/// `getBufferSubData` does, by the OpenGL ES crate's copying read of a mapped range. False when the read
/// failed and `dst` holds nothing read.
pub(crate) fn get_buffer_sub_data(
    gl: &impl OpenGlEs,
    target: u32,
    offset: u64,
    dst: &mut [u8],
) -> bool {
    dst.is_empty() || gl.map_buffer_range_read(target, offset as i64, dst)
}

/// Writes `line` on the host's error output: standard error natively, the console in the browser.
pub fn report(line: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{line}");
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&line.into());
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, PoisonError};
    use wcontainers_egl::Egl;
    use wcontainers_egl::constants as egl;
    use wcontainers_opengl_es::OnEgl;

    /// The EGL objects a context made by [`context`] lives in, destroyed when this is dropped: after the
    /// back end's last OpenGL ES call, since the back end holds it as its last field. The display stays
    /// initialised: every back end of the process shares it, and terminating it would end their contexts too.
    pub(crate) struct Held {
        release: Option<Box<dyn FnOnce()>>,
    }

    impl Drop for Held {
        fn drop(&mut self) {
            if let Some(release) = self.release.take() {
                release();
            }
        }
    }

    impl core::fmt::Debug for Held {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("Held")
        }
    }

    /// The EGL library in `folder` (the system's where none is named), loaded once for the process: the
    /// OpenGL ES crate unloads a library with its last handle, and Mesa's then loads its driver again for the
    /// next context (55 ms a back end on the ARM64 build machine, MEASURED in the tenth run).
    pub(crate) fn library(
        folder: Option<&Path>,
    ) -> Result<wcontainers_egl_direct::Library, String> {
        static LOADED: Mutex<Vec<(Option<PathBuf>, wcontainers_egl_direct::Library)>> =
            Mutex::new(Vec::new());
        let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, library)) = loaded.iter().find(|(named, _)| named.as_deref() == folder) {
            return Ok(library.clone());
        }
        let library = <wcontainers_egl_direct::Library as Egl>::load(folder).ok_or_else(|| {
            format!("no EGL library giving every entry point by name in {folder:?}")
        })?;
        loaded.push((folder.map(Path::to_path_buf), library.clone()));
        Ok(library)
    }

    /// Makes an OpenGL ES 3 context through the EGL `library`, with a 1 by 1 off-screen surface, since every
    /// picture is drawn into textures. The OpenGL ES crate makes the context current on the calling thread
    /// before each call, and gives no window surface yet.
    pub(crate) fn context<E: Egl + 'static, O: OnEgl<E>>(library: E) -> Result<(O, Held), String> {
        let display = library.get_display().ok_or("EGL gave no default display")?;
        library
            .initialize(&display)
            .ok_or_else(|| format!("eglInitialize: {:#x}", library.get_error()))?;
        if !library.bind_api(egl::OPENGL_ES_API as u32) {
            return Err(format!("eglBindAPI: {:#x}", library.get_error()));
        }
        // EGL sorts deeper colour buffers first, so the RGBA8 one is picked out.
        let attributes = [
            egl::SURFACE_TYPE,
            egl::PBUFFER_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES3_BIT,
            egl::NONE,
        ];
        let mut configs = vec![None; 256];
        let found = library
            .choose_config(&display, &attributes, &mut configs)
            .ok_or_else(|| format!("eglChooseConfig: {:#x}", library.get_error()))?;
        configs.truncate(found.max(0) as usize);
        let config = configs
            .into_iter()
            .flatten()
            .find(|c| {
                let get = |a| library.get_config_attrib(&display, c, a).unwrap_or(-1);
                [
                    egl::RED_SIZE,
                    egl::GREEN_SIZE,
                    egl::BLUE_SIZE,
                    egl::ALPHA_SIZE,
                ]
                .into_iter()
                .all(|a| get(a) == 8)
                    && get(egl::SAMPLES) == 0
            })
            .ok_or("the EGL library has no RGBA8 OpenGL ES 3 configuration")?;
        let context = library
            .create_context(
                &display,
                &config,
                None,
                &[egl::CONTEXT_CLIENT_VERSION, 3, egl::NONE],
            )
            .ok_or_else(|| format!("eglCreateContext: {:#x}", library.get_error()))?;
        let Some(pbuffer) = library.create_pbuffer_surface(
            &display,
            &config,
            &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE],
        ) else {
            let error = library.get_error();
            library.destroy_context(&display, &context);
            return Err(format!("eglCreatePbufferSurface: {error:#x}"));
        };
        let gl = O::new(&library, &display, &context, Some(&pbuffer), Some(&pbuffer));
        let release = move || {
            library.make_current(&display, None, None, None);
            library.destroy_surface(&display, &pbuffer);
            library.destroy_context(&display, &context);
        };
        match gl {
            Some(gl) => Ok((
                gl,
                Held {
                    release: Some(Box::new(release)),
                },
            )),
            None => {
                release();
                Err("the OpenGL ES crate did not bind the context".into())
            }
        }
    }
}
