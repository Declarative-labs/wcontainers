//! Where the GL context comes from, and the extension calls glow does not have: Chrome's WebGL 2 in the
//! browser, ANGLE's OpenGL ES 3.0 natively (a test tool, the layer Chrome itself draws WebGL 2 with).

use glow::HasContext;
use std::collections::BTreeSet;

/// GL_UPPER_LEFT_EXT and GL_ZERO_TO_ONE_EXT, and their defaults, for `clipControlEXT`.
pub const UPPER_LEFT: u32 = 0x8CA2;
pub const ZERO_TO_ONE: u32 = 0x935F;
pub const NEGATIVE_ONE_TO_ONE: u32 = 0x935E;

/// The WebGL extension names, with the GL names ANGLE reports for each natively.
const EXTENSIONS: [(&str, &str); 6] = [
    ("EXT_clip_control", "GL_EXT_clip_control"),
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
];

/// The extensions the back end uses that the context has, by WebGL name, less those switched off.
pub fn extensions(gl: &glow::Context, off: &[&str]) -> BTreeSet<&'static str> {
    let have = gl.supported_extensions();
    EXTENSIONS
        .iter()
        .filter(|(web, native)| {
            (have.contains(*web) || have.contains(*native)) && !off.contains(web)
        })
        .map(|(web, _)| *web)
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use khronos_egl as egl;
    use std::path::Path;

    type ClipControl = unsafe extern "system" fn(u32, u32);

    /// The calls past glow, and the EGL objects the context lives in.
    pub struct Platform {
        clip_control: Option<ClipControl>,
        _egl: Option<Egl>,
    }

    struct Egl {
        instance: egl::DynamicInstance<egl::EGL1_0>,
        display: egl::Display,
        context: egl::Context,
        surface: egl::Surface,
    }

    impl Drop for Egl {
        fn drop(&mut self) {
            let e = &self.instance;
            let _ = e.make_current(self.display, None, None, None);
            let _ = e.destroy_surface(self.display, self.surface);
            let _ = e.destroy_context(self.display, self.context);
            let _ = e.terminate(self.display);
        }
    }

    impl core::fmt::Debug for Platform {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("Platform")
                .field("clip_control", &self.clip_control.is_some())
                .finish()
        }
    }

    impl Platform {
        pub fn clip_control(&self, _gl: &glow::Context, origin: u32, depth: u32) {
            if let Some(f) = self.clip_control {
                // SAFETY: the pointer was loaded for this context's glClipControlEXT.
                unsafe { f(origin, depth) };
            }
        }
    }

    /// Makes an OpenGL ES 3.0 context through ANGLE's `libEGL.dll` (or `libEGL.so`, `libEGL.dylib`) in
    /// `dir`, current on this thread with a 1 by 1 off-screen surface, since every picture is drawn
    /// into textures.
    pub fn angle(dir: &Path) -> Result<(glow::Context, Platform), String> {
        let name = if cfg!(windows) {
            "libEGL.dll"
        } else if cfg!(target_os = "macos") {
            "libEGL.dylib"
        } else {
            "libEGL.so"
        };
        let path = dir.join(name);
        // SAFETY: ANGLE's libEGL is a plain EGL library; loading it runs no code of ours.
        let instance =
            unsafe { egl::DynamicInstance::<egl::EGL1_0>::load_required_from_filename(&path) }
                .map_err(|e| format!("loading {}: {e}", path.display()))?;
        // SAFETY: the default display needs no native display handle.
        let display = unsafe { instance.get_display(egl::DEFAULT_DISPLAY) }
            .ok_or("ANGLE gave no default display")?;
        instance
            .initialize(display)
            .map_err(|e| format!("eglInitialize: {e}"))?;
        let config = instance
            .choose_first_config(
                display,
                &[
                    egl::SURFACE_TYPE,
                    egl::PBUFFER_BIT,
                    egl::RENDERABLE_TYPE,
                    egl::OPENGL_ES3_BIT,
                    egl::RED_SIZE,
                    8,
                    egl::GREEN_SIZE,
                    8,
                    egl::BLUE_SIZE,
                    8,
                    egl::ALPHA_SIZE,
                    8,
                    egl::NONE,
                ],
            )
            .map_err(|e| format!("eglChooseConfig: {e}"))?
            .ok_or("ANGLE has no OpenGL ES 3 configuration")?;
        // OpenGL ES is EGL's bound API from the start, so only EGL 1.0's calls are needed.
        let context = instance
            .create_context(
                display,
                config,
                None,
                &[egl::CONTEXT_CLIENT_VERSION, 3, egl::NONE],
            )
            .map_err(|e| format!("eglCreateContext: {e}"))?;
        let surface = instance
            .create_pbuffer_surface(display, config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE])
            .map_err(|e| format!("eglCreatePbufferSurface: {e}"))?;
        instance
            .make_current(display, Some(surface), Some(surface), Some(context))
            .map_err(|e| format!("eglMakeCurrent: {e}"))?;
        let load = |name: &str| {
            instance
                .get_proc_address(name)
                .map_or(core::ptr::null(), |f| f as *const core::ffi::c_void)
        };
        // SAFETY: the context is current on this thread and the loader returns its functions.
        let gl = unsafe { glow::Context::from_loader_function(load) };
        let clip = load("glClipControlEXT");
        let clip_control = (!clip.is_null()
            && gl.supported_extensions().contains("GL_EXT_clip_control"))
        // SAFETY: glClipControlEXT takes two GLenums and returns nothing.
        .then(|| unsafe { core::mem::transmute::<*const core::ffi::c_void, ClipControl>(clip) });
        let platform = Platform {
            clip_control,
            _egl: Some(Egl {
                instance,
                display,
                context,
                surface,
            }),
        };
        Ok((gl, platform))
    }

    /// Copies bytes of the buffer bound at `target` into `dst`, waiting for the GPU: what WebGL's
    /// `getBufferSubData` does, by mapping the range for reading, as GLES does.
    ///
    /// # Safety
    /// A buffer at least `offset + dst.len()` bytes long is bound at `target`.
    pub unsafe fn get_buffer_sub_data(
        gl: &glow::Context,
        target: u32,
        offset: i32,
        dst: &mut [u8],
    ) {
        if dst.is_empty() {
            return;
        }
        // SAFETY: the caller's range is inside the bound buffer; a failed map is a GL error, which
        // submit reports as device loss.
        unsafe {
            let p = gl.map_buffer_range(target, offset, dst.len() as i32, glow::MAP_READ_BIT);
            if !p.is_null() {
                core::ptr::copy_nonoverlapping(p, dst.as_mut_ptr(), dst.len());
            }
            gl.unmap_buffer(target);
        }
    }

    /// The renderer string, which names the GPU and ANGLE's own back end.
    pub fn renderer(gl: &glow::Context) -> String {
        // SAFETY: a plain string query.
        unsafe { gl.get_parameter_string(glow::RENDERER) }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::WebGl2RenderingContext;

    /// The calls past glow, made on the extension objects Chrome hands out.
    #[derive(Debug)]
    pub struct Platform {
        clip_control: Option<(js_sys::Object, js_sys::Function)>,
    }

    impl Platform {
        pub fn clip_control(&self, _gl: &glow::Context, origin: u32, depth: u32) {
            if let Some((ext, f)) = &self.clip_control {
                let _ = f.call2(ext, &JsValue::from(origin), &JsValue::from(depth));
            }
        }
    }

    /// Copies bytes of the buffer bound at `target` into `dst` (WebGL's `getBufferSubData`, which
    /// waits for the GPU).
    ///
    /// # Safety
    /// A buffer at least `offset + dst.len()` bytes long is bound at `target`.
    pub unsafe fn get_buffer_sub_data(
        gl: &glow::Context,
        target: u32,
        offset: i32,
        dst: &mut [u8],
    ) {
        // SAFETY: as the caller promises.
        unsafe { gl.get_buffer_sub_data(target, offset, dst) }
    }

    /// Wraps a canvas's WebGL 2 context, enabling the extensions glow does not enable itself.
    pub fn canvas(context: WebGl2RenderingContext) -> (glow::Context, Platform) {
        let clip_control = context
            .get_extension("EXT_clip_control")
            .ok()
            .flatten()
            .and_then(|ext| {
                let f = js_sys::Reflect::get(&ext, &JsValue::from_str("clipControlEXT")).ok()?;
                Some((ext, f.dyn_into::<js_sys::Function>().ok()?))
            });
        (
            glow::Context::from_webgl2_context(context),
            Platform { clip_control },
        )
    }

    /// The renderer string Chrome reports (the unmasked one, through WEBGL_debug_renderer_info).
    pub fn renderer(gl: &glow::Context) -> String {
        // SAFETY: plain string queries.
        let unmasked = if gl
            .supported_extensions()
            .contains("WEBGL_debug_renderer_info")
        {
            unsafe { gl.get_parameter_string(0x9246) }
        } else {
            String::new()
        };
        if unmasked.is_empty() {
            // SAFETY: a plain string query.
            unsafe { gl.get_parameter_string(glow::RENDERER) }
        } else {
            unmasked
        }
    }
}
