//! Where the GL context comes from, and the extension calls glow does not have: Chrome's WebGL 2 in the
//! browser, ANGLE's OpenGL ES 3.0 natively (a test tool, the layer Chrome itself draws WebGL 2 with).

use glow::HasContext;
use std::collections::BTreeSet;

/// GL_UPPER_LEFT_EXT and GL_ZERO_TO_ONE_EXT, and their defaults, for `clipControlEXT`.
pub const UPPER_LEFT: u32 = 0x8CA2;
pub const LOWER_LEFT: u32 = 0x8CA1;
pub const ZERO_TO_ONE: u32 = 0x935F;
pub const NEGATIVE_ONE_TO_ONE: u32 = 0x935E;

/// GL_FIRST_VERTEX_CONVENTION and GL_LAST_VERTEX_CONVENTION (GL's initial one), for
/// `provokingVertexWEBGL` and `glProvokingVertexANGLE`.
pub const FIRST_VERTEX: u32 = 0x8E4D;
pub const LAST_VERTEX: u32 = 0x8E4E;

/// GL_LINE and GL_FILL, for `polygonModeWEBGL` and `glPolygonModeANGLE`.
pub const LINE: u32 = 0x1B01;
pub const FILL: u32 = 0x1B02;
/// GL_POLYGON_OFFSET_LINE of WEBGL_polygon_mode.
pub const POLYGON_OFFSET_LINE: u32 = 0x2A02;
/// GL_DEPTH_CLAMP_EXT.
pub const DEPTH_CLAMP: u32 = 0x864F;
/// GL_MIRROR_CLAMP_TO_EDGE_EXT.
pub const MIRROR_CLAMP_TO_EDGE: u32 = 0x8743;
/// GL_CLIP_DISTANCE0_EXT; distance n is enabled at this plus n.
pub const CLIP_DISTANCE0: u32 = 0x3000;
/// The second-source blend factors of WEBGL_blend_func_extended: GL_SRC1_COLOR_EXT,
/// GL_ONE_MINUS_SRC1_COLOR_EXT, GL_SRC1_ALPHA_EXT and GL_ONE_MINUS_SRC1_ALPHA_EXT.
pub const SRC1_COLOR: u32 = 0x88F9;
pub const ONE_MINUS_SRC1_COLOR: u32 = 0x88FA;
pub const SRC1_ALPHA: u32 = 0x8589;
pub const ONE_MINUS_SRC1_ALPHA: u32 = 0x88FB;

/// The WebGL extension names, with the GL names ANGLE reports for each natively.
const EXTENSIONS: [(&str, &str); 22] = [
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
];

/// GL_TIMESTAMP_EXT, GL_QUERY_COUNTER_BITS_EXT and GL_GPU_DISJOINT_EXT.
pub const TIMESTAMP: u32 = 0x8E28;
const QUERY_COUNTER_BITS: u32 = 0x8864;
pub const GPU_DISJOINT: u32 = 0x8FBB;

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

    /// EGL_DIRECT_COMPOSITION_ANGLE, a window surface attribute of EGL_ANGLE_direct_composition.
    const DIRECT_COMPOSITION: egl::Int = 0x33A5;

    type ClipControl = unsafe extern "system" fn(u32, u32);
    type ProvokingVertex = unsafe extern "system" fn(u32);
    type GetQueryiv = unsafe extern "system" fn(u32, u32, *mut i32);
    type EnableI = unsafe extern "system" fn(u32, u32);
    type EquationI = unsafe extern "system" fn(u32, u32, u32);
    type FuncI = unsafe extern "system" fn(u32, u32, u32, u32, u32);
    type MaskI = unsafe extern "system" fn(u32, u8, u8, u8, u8);
    type PolygonMode = unsafe extern "system" fn(u32, u32);
    type OffsetClamp = unsafe extern "system" fn(f32, f32, f32);

    /// OES_draw_buffers_indexed's calls.
    struct Indexed {
        enable: EnableI,
        disable: EnableI,
        equation: EquationI,
        func: FuncI,
        mask: MaskI,
    }

    /// The calls past glow, and the EGL objects the context lives in.
    pub struct Platform {
        clip_control: Option<ClipControl>,
        provoking_vertex: Option<ProvokingVertex>,
        get_queryiv: Option<GetQueryiv>,
        indexed: Option<Indexed>,
        polygon_mode: Option<PolygonMode>,
        offset_clamp: Option<OffsetClamp>,
        egl: Egl,
    }

    struct Egl {
        instance: egl::DynamicInstance<egl::EGL1_1>,
        display: egl::Display,
        config: egl::Config,
        context: egl::Context,
        /// The 1 by 1 off-screen surface, current while no window surface is.
        pbuffer: egl::Surface,
        /// The window surface frames are presented into, current while it exists.
        window: Option<egl::Surface>,
        /// The host's windows, by the numbers its graphics hook gives them.
        windows: std::collections::HashMap<u32, egl::NativeWindowType>,
    }

    impl Drop for Egl {
        fn drop(&mut self) {
            let e = &self.instance;
            let _ = e.make_current(self.display, None, None, None);
            if let Some(window) = self.window {
                let _ = e.destroy_surface(self.display, window);
            }
            let _ = e.destroy_surface(self.display, self.pbuffer);
            let _ = e.destroy_context(self.display, self.context);
            let _ = e.terminate(self.display);
        }
    }

    impl core::fmt::Debug for Platform {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("Platform")
                .field("clip_control", &self.clip_control.is_some())
                .field("provoking_vertex", &self.provoking_vertex.is_some())
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

        pub fn provoking_vertex(&self, _gl: &glow::Context, mode: u32) {
            if let Some(f) = self.provoking_vertex {
                // SAFETY: the pointer was loaded for this context's glProvokingVertexANGLE.
                unsafe { f(mode) };
            }
        }

        /// Enables or disables `cap` for colour target `i` (OES_draw_buffers_indexed).
        pub fn enable_i(&self, cap: u32, i: u32, on: bool) {
            if let Some(x) = &self.indexed {
                // SAFETY: the pointers were loaded for this context's glEnableiOES and glDisableiOES.
                unsafe {
                    if on {
                        (x.enable)(cap, i)
                    } else {
                        (x.disable)(cap, i)
                    }
                }
            }
        }

        pub fn blend_equation_i(&self, i: u32, e: [u32; 2]) {
            if let Some(x) = &self.indexed {
                // SAFETY: the pointer was loaded for this context's glBlendEquationSeparateiOES.
                unsafe { (x.equation)(i, e[0], e[1]) }
            }
        }

        pub fn blend_func_i(&self, i: u32, f: [u32; 4]) {
            if let Some(x) = &self.indexed {
                // SAFETY: the pointer was loaded for this context's glBlendFuncSeparateiOES.
                unsafe { (x.func)(i, f[0], f[1], f[2], f[3]) }
            }
        }

        pub fn color_mask_i(&self, i: u32, m: [bool; 4]) {
            if let Some(x) = &self.indexed {
                // SAFETY: the pointer was loaded for this context's glColorMaskiOES.
                unsafe { (x.mask)(i, m[0].into(), m[1].into(), m[2].into(), m[3].into()) }
            }
        }

        /// Fills polygons (`FILL`) or draws their edges (`LINE`), for both faces.
        pub fn polygon_mode(&self, mode: u32) {
            if let Some(f) = self.polygon_mode {
                // SAFETY: the pointer was loaded for this context's glPolygonModeANGLE.
                unsafe { f(glow::FRONT_AND_BACK, mode) }
            }
        }

        pub fn polygon_offset_clamp(&self, factor: f32, units: f32, clamp: f32) {
            if let Some(f) = self.offset_clamp {
                // SAFETY: the pointer was loaded for this context's glPolygonOffsetClampEXT.
                unsafe { f(factor, units, clamp) }
            }
        }

        /// How many bits the GPU's timestamps have; zero where it writes none.
        pub fn timestamp_bits(&self, _gl: &glow::Context) -> u32 {
            let mut bits = 0;
            if let Some(f) = self.get_queryiv {
                // SAFETY: the pointer was loaded for this context's glGetQueryivEXT.
                unsafe { f(TIMESTAMP, QUERY_COUNTER_BITS, &mut bits) };
            }
            bits.max(0) as u32
        }

        /// The sample counts GL takes for a renderbuffer of `internal`.
        pub fn sample_counts(&self, gl: &glow::Context, internal: u32) -> Vec<i32> {
            let mut n = [0];
            // SAFETY: plain queries; the second gets as many values as the first says there are.
            unsafe {
                gl.get_internal_format_i32_slice(
                    glow::RENDERBUFFER,
                    internal,
                    glow::NUM_SAMPLE_COUNTS,
                    &mut n,
                );
                let mut counts = vec![0; n[0].max(0) as usize];
                if !counts.is_empty() {
                    gl.get_internal_format_i32_slice(
                        glow::RENDERBUFFER,
                        internal,
                        glow::SAMPLES,
                        &mut counts,
                    );
                }
                counts
            }
        }

        /// Natively query results arrive without the thread giving way, so nothing is done.
        pub fn next_task(&self) {}

        /// Names a host window (an `HWND` on Windows) that `attach` may make a surface on.
        ///
        /// # Safety
        /// `window` is a native window that outlives every surface made on it.
        pub unsafe fn add_window(&mut self, name: u32, window: *mut core::ffi::c_void) {
            self.egl.windows.insert(name, window);
        }

        /// Makes an EGL window surface on the host's window `name` and makes it current, so the
        /// default framebuffer is that window's back buffer.
        pub fn attach(&mut self, name: u32) -> Result<(), String> {
            let e = &mut self.egl;
            let window = *e
                .windows
                .get(&name)
                .ok_or_else(|| format!("no host window {name}"))?;
            // A flip-model swap chain through DirectComposition where ANGLE offers it: presented
            // without a copy into the window's redirection surface.
            let direct = e
                .instance
                .query_string(Some(e.display), egl::EXTENSIONS)
                .is_ok_and(|x| {
                    x.to_bytes()
                        .split(|c| *c == b' ')
                        .any(|n| n == b"EGL_ANGLE_direct_composition")
                });
            let attributes = [DIRECT_COMPOSITION, egl::TRUE as egl::Int, egl::NONE];
            // SAFETY: `add_window`'s caller promised the window outlives its surfaces.
            let surface = unsafe {
                e.instance.create_window_surface(
                    e.display,
                    e.config,
                    window,
                    direct.then_some(&attributes[..]),
                )
            }
            .map_err(|err| format!("eglCreateWindowSurface: {err}"))?;
            e.instance
                .make_current(e.display, Some(surface), Some(surface), Some(e.context))
                .map_err(|err| format!("eglMakeCurrent: {err}"))?;
            e.window = Some(surface);
            Ok(())
        }

        /// Puts the off-screen surface back and destroys the window surface.
        pub fn detach(&mut self) {
            let e = &mut self.egl;
            if let Some(window) = e.window.take() {
                let _ = e.instance.make_current(
                    e.display,
                    Some(e.pbuffer),
                    Some(e.pbuffer),
                    Some(e.context),
                );
                let _ = e.instance.destroy_surface(e.display, window);
            }
        }

        /// Sets whether swaps wait for the display. The window surface takes the window's size
        /// itself, so `width` and `height` change nothing here.
        pub fn resize(
            &mut self,
            _gl: &glow::Context,
            _width: u32,
            _height: u32,
            vsync: bool,
        ) -> Result<(), String> {
            let e = &self.egl;
            e.instance
                .swap_interval(e.display, i32::from(vsync))
                .map_err(|err| format!("eglSwapInterval: {err}"))
        }

        /// The size of the default framebuffer frames are presented into.
        pub fn drawable_size(&self, _gl: &glow::Context) -> (i32, i32) {
            let e = &self.egl;
            let Some(window) = e.window else {
                return (0, 0);
            };
            let get = |a| e.instance.query_surface(e.display, window, a).unwrap_or(0);
            (get(egl::WIDTH), get(egl::HEIGHT))
        }

        /// Shows the window surface's back buffer.
        pub fn swap(&self) -> Result<(), String> {
            let e = &self.egl;
            match e.window {
                Some(window) => e
                    .instance
                    .swap_buffers(e.display, window)
                    .map_err(|err| format!("eglSwapBuffers: {err}")),
                None => Err("no window surface".into()),
            }
        }
    }

    /// Makes an OpenGL ES 3.0 context through ANGLE's `libEGL.dll` (or `libEGL.so`, `libEGL.dylib`) in
    /// `dir`, current on this thread with a 1 by 1 off-screen surface, since every picture is drawn
    /// into textures until a surface is made on a host window.
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
            unsafe { egl::DynamicInstance::<egl::EGL1_1>::load_required_from_filename(&path) }
                .map_err(|e| format!("loading {}: {e}", path.display()))?;
        // SAFETY: the default display needs no native display handle.
        let display = unsafe { instance.get_display(egl::DEFAULT_DISPLAY) }
            .ok_or("ANGLE gave no default display")?;
        instance
            .initialize(display)
            .map_err(|e| format!("eglInitialize: {e}"))?;
        // EGL sorts deeper colour buffers first, so the RGBA8 one surfaces present into is picked out.
        let attributes = [
            egl::SURFACE_TYPE,
            egl::PBUFFER_BIT | egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES3_BIT,
            egl::NONE,
        ];
        let mut configs = Vec::with_capacity(
            instance
                .matching_config_count(display, &attributes)
                .map_err(|e| format!("eglChooseConfig: {e}"))?,
        );
        instance
            .choose_config(display, &attributes, &mut configs)
            .map_err(|e| format!("eglChooseConfig: {e}"))?;
        let config = configs
            .into_iter()
            .find(|c| {
                let get = |a| instance.get_config_attrib(display, *c, a).unwrap_or(-1);
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
            .ok_or("ANGLE has no RGBA8 OpenGL ES 3 configuration")?;
        // OpenGL ES is EGL's bound API from the start; EGL 1.1 adds the swap interval.
        let context = instance
            .create_context(
                display,
                config,
                None,
                &[egl::CONTEXT_CLIENT_VERSION, 3, egl::NONE],
            )
            .map_err(|e| format!("eglCreateContext: {e}"))?;
        let pbuffer = instance
            .create_pbuffer_surface(display, config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE])
            .map_err(|e| format!("eglCreatePbufferSurface: {e}"))?;
        instance
            .make_current(display, Some(pbuffer), Some(pbuffer), Some(context))
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
        let provoking = load("glProvokingVertexANGLE");
        let provoking_vertex = (!provoking.is_null()
            && gl
                .supported_extensions()
                .contains("GL_ANGLE_provoking_vertex"))
        // SAFETY: glProvokingVertexANGLE takes one GLenum and returns nothing.
        .then(|| unsafe {
            core::mem::transmute::<*const core::ffi::c_void, ProvokingVertex>(provoking)
        });
        let queryiv = load("glGetQueryivEXT");
        let get_queryiv = (!queryiv.is_null()
            && gl
                .supported_extensions()
                .contains("GL_EXT_disjoint_timer_query"))
        // SAFETY: glGetQueryivEXT takes two GLenums and a pointer to one GLint.
        .then(|| unsafe { core::mem::transmute::<*const core::ffi::c_void, GetQueryiv>(queryiv) });
        let has = |name| gl.supported_extensions().contains(name);
        // SAFETY (each `cast`): the pointer is the named GL ES function, of the type it is cast to.
        unsafe fn cast<T: Copy>(p: *const core::ffi::c_void) -> T {
            // SAFETY: as the caller promises.
            unsafe { core::mem::transmute_copy(&p) }
        }
        let calls = |names: &[&str]| -> Option<Vec<*const core::ffi::c_void>> {
            let p: Vec<_> = names.iter().map(|n| load(n)).collect();
            p.iter().all(|f| !f.is_null()).then_some(p)
        };
        let indexed = calls(&[
            "glEnableiOES",
            "glDisableiOES",
            "glBlendEquationSeparateiOES",
            "glBlendFuncSeparateiOES",
            "glColorMaskiOES",
        ])
        .filter(|_| has("GL_OES_draw_buffers_indexed"))
        // SAFETY: see `cast`.
        .map(|p| unsafe {
            Indexed {
                enable: cast(p[0]),
                disable: cast(p[1]),
                equation: cast(p[2]),
                func: cast(p[3]),
                mask: cast(p[4]),
            }
        });
        let polygon_mode = calls(&["glPolygonModeANGLE"])
            .filter(|_| has("GL_ANGLE_polygon_mode"))
            // SAFETY: see `cast`.
            .map(|p| unsafe { cast::<PolygonMode>(p[0]) });
        let offset_clamp = calls(&["glPolygonOffsetClampEXT"])
            .filter(|_| has("GL_EXT_polygon_offset_clamp"))
            // SAFETY: see `cast`.
            .map(|p| unsafe { cast::<OffsetClamp>(p[0]) });
        let platform = Platform {
            clip_control,
            provoking_vertex,
            get_queryiv,
            indexed,
            polygon_mode,
            offset_clamp,
            egl: Egl {
                instance,
                display,
                config,
                context,
                pbuffer,
                window: None,
                windows: Default::default(),
            },
        };
        Ok((gl, platform))
    }

    /// Copies bytes of the buffer bound at `target` into `dst`, waiting for the GPU: what WebGL's
    /// `getBufferSubData` does, by mapping the range for reading, as GLES does. False when the map
    /// failed and `dst` holds nothing read.
    ///
    /// # Safety
    /// A buffer at least `offset + dst.len()` bytes long is bound at `target`.
    pub unsafe fn get_buffer_sub_data(
        gl: &glow::Context,
        target: u32,
        offset: i32,
        dst: &mut [u8],
    ) -> bool {
        if dst.is_empty() {
            return true;
        }
        // SAFETY: the caller's range is inside the bound buffer.
        unsafe {
            let p = gl.map_buffer_range(target, offset, dst.len() as i32, glow::MAP_READ_BIT);
            if p.is_null() {
                return false;
            }
            core::ptr::copy_nonoverlapping(p, dst.as_mut_ptr(), dst.len());
            gl.unmap_buffer(target);
            true
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

    /// A function of an extension object, with the object it is called on.
    type Call = Option<(js_sys::Object, js_sys::Function)>;

    fn call(c: &Call, args: &[JsValue]) {
        if let Some((ext, f)) = c {
            let _ = match *args {
                [ref a] => f.call1(ext, a),
                [ref a, ref b] => f.call2(ext, a, b),
                [ref a, ref b, ref c] => f.call3(ext, a, b, c),
                _ => f.apply(ext, &args.iter().collect()),
            };
        }
    }

    /// The calls past glow, made on the extension objects Chrome hands out.
    #[derive(Debug)]
    pub struct Platform {
        clip_control: Call,
        provoking_vertex: Call,
        /// OES_draw_buffers_indexed's enableiOES, disableiOES, blendEquationSeparateiOES,
        /// blendFuncSeparateiOES and colorMaskiOES.
        indexed: [Call; 5],
        polygon_mode: Call,
        offset_clamp: Call,
        /// The context, and its canvas (on the page or off-screen), whose drawing buffer surfaces
        /// present into.
        context: WebGl2RenderingContext,
        canvas: JsValue,
    }

    impl Platform {
        pub fn clip_control(&self, _gl: &glow::Context, origin: u32, depth: u32) {
            if let Some((ext, f)) = &self.clip_control {
                let _ = f.call2(ext, &JsValue::from(origin), &JsValue::from(depth));
            }
        }

        pub fn provoking_vertex(&self, _gl: &glow::Context, mode: u32) {
            if let Some((ext, f)) = &self.provoking_vertex {
                let _ = f.call1(ext, &JsValue::from(mode));
            }
        }

        /// Enables or disables `cap` for colour target `i` (OES_draw_buffers_indexed).
        pub fn enable_i(&self, cap: u32, i: u32, on: bool) {
            let c = &self.indexed[if on { 0 } else { 1 }];
            call(c, &[cap.into(), i.into()]);
        }

        pub fn blend_equation_i(&self, i: u32, e: [u32; 2]) {
            call(&self.indexed[2], &[i.into(), e[0].into(), e[1].into()]);
        }

        pub fn blend_func_i(&self, i: u32, f: [u32; 4]) {
            let args = [i, f[0], f[1], f[2], f[3]].map(JsValue::from);
            call(&self.indexed[3], &args);
        }

        pub fn color_mask_i(&self, i: u32, m: [bool; 4]) {
            let args = [i.into(), m[0].into(), m[1].into(), m[2].into(), m[3].into()];
            call(&self.indexed[4], &args);
        }

        /// Fills polygons (`FILL`) or draws their edges (`LINE`), for both faces.
        pub fn polygon_mode(&self, mode: u32) {
            call(
                &self.polygon_mode,
                &[glow::FRONT_AND_BACK.into(), mode.into()],
            );
        }

        pub fn polygon_offset_clamp(&self, factor: f32, units: f32, clamp: f32) {
            call(
                &self.offset_clamp,
                &[factor.into(), units.into(), clamp.into()],
            );
        }

        /// How many bits the GPU's timestamps have; zero where it writes none.
        pub fn timestamp_bits(&self, gl: &glow::Context) -> u32 {
            if !gl
                .supported_extensions()
                .contains("EXT_disjoint_timer_query_webgl2")
            {
                return 0;
            }
            let Ok(f) = js_sys::Reflect::get(&self.context, &JsValue::from_str("getQuery"))
                .and_then(|f| f.dyn_into::<js_sys::Function>())
            else {
                return 0;
            };
            f.call2(
                &self.context,
                &JsValue::from(TIMESTAMP),
                &JsValue::from(QUERY_COUNTER_BITS),
            )
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as u32
        }

        /// The sample counts WebGL takes for a renderbuffer of `internal` (WebGL 2 answers `SAMPLES`
        /// with the whole list, and has no `NUM_SAMPLE_COUNTS`).
        pub fn sample_counts(&self, _gl: &glow::Context, internal: u32) -> Vec<i32> {
            self.context
                .get_internalformat_parameter(glow::RENDERBUFFER, internal, glow::SAMPLES)
                .ok()
                .and_then(|v| v.dyn_into::<js_sys::Int32Array>().ok())
                .map_or_else(Vec::new, |a| a.to_vec())
        }

        /// Suspends the device thread until the browser's next task (plan section 4.8), through the
        /// host's JSPI import `next_task` of module `wcontainers-device`: WebGL hands out query
        /// results only between tasks.
        pub fn next_task(&self) {
            // SAFETY: the import takes and returns nothing; the host suspends this stack in it.
            unsafe { next_task() }
        }

        /// The context draws into its canvas whatever window the host names.
        pub fn attach(&mut self, _name: u32) -> Result<(), String> {
            Ok(())
        }

        pub fn detach(&mut self) {}

        /// The size of the canvas's drawing buffer, which frames are presented into.
        pub fn drawable_size(&self, _gl: &glow::Context) -> (i32, i32) {
            (
                self.context.drawing_buffer_width(),
                self.context.drawing_buffer_height(),
            )
        }

        /// The browser shows the drawing buffer when the device thread returns to its event loop.
        pub fn swap(&self) -> Result<(), String> {
            Ok(())
        }

        /// Gives the canvas's drawing buffer, which surfaces present into, this size. Err when the
        /// browser made it smaller (WebGL may, under memory pressure or past its largest size).
        /// `vsync` changes nothing: the browser shows the canvas at the display's rate.
        pub fn resize(
            &mut self,
            _gl: &glow::Context,
            width: u32,
            height: u32,
            _vsync: bool,
        ) -> Result<(), String> {
            for (name, value) in [("width", width), ("height", height)] {
                js_sys::Reflect::set(
                    &self.canvas,
                    &JsValue::from_str(name),
                    &JsValue::from(value),
                )
                .map_err(|e| format!("setting the canvas's {name}: {e:?}"))?;
            }
            let made = (
                self.context.drawing_buffer_width(),
                self.context.drawing_buffer_height(),
            );
            if made != (width as i32, height as i32) {
                return Err(format!(
                    "the browser made the drawing buffer {} by {}, not {width} by {height}",
                    made.0, made.1
                ));
            }
            Ok(())
        }
    }

    #[link(wasm_import_module = "wcontainers-device")]
    unsafe extern "C" {
        fn next_task();
    }

    /// The extension object `name` and its function `call`, enabled on the context.
    fn extension_call(
        context: &WebGl2RenderingContext,
        name: &str,
        call: &str,
    ) -> Option<(js_sys::Object, js_sys::Function)> {
        let ext = context.get_extension(name).ok().flatten()?;
        let f = js_sys::Reflect::get(&ext, &JsValue::from_str(call)).ok()?;
        Some((ext, f.dyn_into::<js_sys::Function>().ok()?))
    }

    /// Copies bytes of the buffer bound at `target` into `dst` (WebGL's `getBufferSubData`, which
    /// waits for the GPU). False when the context reports an error, so `dst` holds nothing read.
    ///
    /// # Safety
    /// A buffer at least `offset + dst.len()` bytes long is bound at `target`.
    pub unsafe fn get_buffer_sub_data(
        gl: &glow::Context,
        target: u32,
        offset: i32,
        dst: &mut [u8],
    ) -> bool {
        // SAFETY: as the caller promises. WebGL reports a failed read only through getError; an error
        // already pending is a device loss whichever call raised it.
        unsafe {
            gl.get_buffer_sub_data(target, offset, dst);
            gl.get_error() == glow::NO_ERROR
        }
    }

    /// Wraps a canvas's WebGL 2 context, enabling the extensions glow does not enable itself.
    pub fn canvas(context: WebGl2RenderingContext) -> (glow::Context, Platform) {
        let clip_control = extension_call(&context, "EXT_clip_control", "clipControlEXT");
        let provoking_vertex =
            extension_call(&context, "WEBGL_provoking_vertex", "provokingVertexWEBGL");
        let indexed = [
            "enableiOES",
            "disableiOES",
            "blendEquationSeparateiOES",
            "blendFuncSeparateiOES",
            "colorMaskiOES",
        ]
        .map(|f| extension_call(&context, "OES_draw_buffers_indexed", f));
        let polygon_mode = extension_call(&context, "WEBGL_polygon_mode", "polygonModeWEBGL");
        let offset_clamp = extension_call(
            &context,
            "EXT_polygon_offset_clamp",
            "polygonOffsetClampEXT",
        );
        // Enabled here since glow does not know them; they add only enums and shader built-ins.
        for name in [
            "WEBGL_stencil_texturing",
            "EXT_texture_norm16",
            "EXT_render_snorm",
            "EXT_depth_clamp",
            "EXT_texture_mirror_clamp_to_edge",
            "WEBGL_clip_cull_distance",
            "WEBGL_blend_func_extended",
        ] {
            let _ = context.get_extension(name);
        }
        for compressed in [
            "WEBGL_compressed_texture_s3tc",
            "WEBGL_compressed_texture_s3tc_srgb",
            "EXT_texture_compression_rgtc",
            "EXT_texture_compression_bptc",
        ] {
            let _ = context.get_extension(compressed);
        }
        let canvas = js_sys::Reflect::get(&context, &JsValue::from_str("canvas"))
            .unwrap_or(JsValue::UNDEFINED);
        (
            glow::Context::from_webgl2_context(context.clone()),
            Platform {
                clip_control,
                provoking_vertex,
                indexed,
                polygon_mode,
                offset_clamp,
                context,
                canvas,
            },
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
