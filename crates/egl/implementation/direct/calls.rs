// Written by tools/api-crate-generator from Khronos's egl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! Every command of the set as a safe method: the checks of its parameters' kinds, then one call of its entry point.

use core::ffi::c_void;

use wcontainers_egl::constants;

use crate::{Config, Context, Display, Library, Surface, runtime};

/// Forced into eglChooseConfig's attribute list: nothing.
const FORCED_EGL_CHOOSE_CONFIG: &[(i32, i32)] = &[];
/// Forced into eglCreateContext's attribute list: OpenGL ES leaves a shader's or a draw's read outside a buffer undefined (it may end the program) unless the context has robust buffer access (EGL 1.5 §3.7.1.5); every context is made with it, and none is made where the implementation cannot.
const FORCED_EGL_CREATE_CONTEXT: &[(i32, i32)] = &[(constants::CONTEXT_OPENGL_ROBUST_ACCESS, constants::TRUE)];
/// Forced into eglCreatePbufferSurface's attribute list: nothing.
const FORCED_EGL_CREATE_PBUFFER_SURFACE: &[(i32, i32)] = &[];

/// The library's entry points, each null when the library gives none.
pub(crate) struct Entries {
    pub bind_api: Option<unsafe extern "system" fn(u32) -> u32>,
    pub choose_config: Option<unsafe extern "system" fn(*mut c_void, *const i32, *mut *mut c_void, i32, *mut i32) -> u32>,
    pub create_context: Option<unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void>,
    pub create_pbuffer_surface: Option<unsafe extern "system" fn(*mut c_void, *mut c_void, *const i32) -> *mut c_void>,
    pub destroy_context: Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32>,
    pub destroy_surface: Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32>,
    pub get_config_attrib: Option<unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut i32) -> u32>,
    pub get_display: Option<unsafe extern "system" fn(*mut c_void) -> *mut c_void>,
    pub get_error: Option<unsafe extern "system" fn() -> i32>,
    pub initialize: Option<unsafe extern "system" fn(*mut c_void, *mut i32, *mut i32) -> u32>,
    pub make_current: Option<unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> u32>,
    pub query_string: Option<unsafe extern "system" fn(*mut c_void, i32) -> *const u8>,
    pub query_surface: Option<unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut i32) -> u32>,
    pub swap_buffers: Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32>,
    pub swap_interval: Option<unsafe extern "system" fn(*mut c_void, i32) -> u32>,
    pub get_proc_address: Option<unsafe extern "system" fn(*const u8) -> *mut c_void>,
    pub get_current_context: Option<unsafe extern "system" fn() -> *mut c_void>,
}

impl Entries {
    /// Asks the library for every entry point by name.
    pub(crate) fn load(load: impl Fn(&core::ffi::CStr) -> *const c_void) -> Entries {
        Entries {
            bind_api: {
                let address = load(c"eglBindAPI");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglBindAPI is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(u32) -> u32>>(address) }
            },
            choose_config: {
                let address = load(c"eglChooseConfig");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglChooseConfig is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *const i32, *mut *mut c_void, i32, *mut i32) -> u32>>(address) }
            },
            create_context: {
                let address = load(c"eglCreateContext");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglCreateContext is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void>>(address) }
            },
            create_pbuffer_surface: {
                let address = load(c"eglCreatePbufferSurface");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglCreatePbufferSurface is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void, *const i32) -> *mut c_void>>(address) }
            },
            destroy_context: {
                let address = load(c"eglDestroyContext");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglDestroyContext is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32>>(address) }
            },
            destroy_surface: {
                let address = load(c"eglDestroySurface");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglDestroySurface is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32>>(address) }
            },
            get_config_attrib: {
                let address = load(c"eglGetConfigAttrib");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglGetConfigAttrib is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut i32) -> u32>>(address) }
            },
            get_display: {
                let address = load(c"eglGetDisplay");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglGetDisplay is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void) -> *mut c_void>>(address) }
            },
            get_error: {
                let address = load(c"eglGetError");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglGetError is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn() -> i32>>(address) }
            },
            initialize: {
                let address = load(c"eglInitialize");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglInitialize is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut i32, *mut i32) -> u32>>(address) }
            },
            make_current: {
                let address = load(c"eglMakeCurrent");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglMakeCurrent is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> u32>>(address) }
            },
            query_string: {
                let address = load(c"eglQueryString");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglQueryString is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, i32) -> *const u8>>(address) }
            },
            query_surface: {
                let address = load(c"eglQuerySurface");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglQuerySurface is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut i32) -> u32>>(address) }
            },
            swap_buffers: {
                let address = load(c"eglSwapBuffers");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglSwapBuffers is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> u32>>(address) }
            },
            swap_interval: {
                let address = load(c"eglSwapInterval");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglSwapInterval is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*mut c_void, i32) -> u32>>(address) }
            },
            get_proc_address: {
                let address = load(c"eglGetProcAddress");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglGetProcAddress is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn(*const u8) -> *mut c_void>>(address) }
            },
            get_current_context: {
                let address = load(c"eglGetCurrentContext");
                #[expect(unsafe_code, reason = "egl.xml: the address the library gives for eglGetCurrentContext is of the C signature the registry declares, or null")]
                // SAFETY: a function pointer and a data pointer have the same size on the targets EGL runs on, and null is None.
                unsafe { core::mem::transmute::<*const c_void, Option<unsafe extern "system" fn() -> *mut c_void>>(address) }
            },
        }
    }
}

impl wcontainers_egl::Egl for Library {
    type Config = Config;
    type Context = Context;
    type Display = Display;
    type Surface = Surface;

    fn load(folder: Option<&std::path::Path>) -> Option<Self> {
        Library::open(folder)
    }

    fn bind_api(&self, api: u32) -> bool {
        let _gate = self.reading();
        let Some(entry) = self.entries().bind_api else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglBindAPI takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(api) };
        result != 0
    }

    fn choose_config(&self, dpy: &Display, attrib_list: &[i32], configs: &mut [Option<Config>]) -> Option<i32> {
        let _gate = self.reading();
        let Some(entry) = self.entries().choose_config else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let Some(attrib_list) = runtime::attribute_list(attrib_list, FORCED_EGL_CHOOSE_CONFIG) else { runtime::refuse(constants::BAD_ATTRIBUTE); return Default::default(); };
        let mut configs_room = vec![core::ptr::null_mut::<c_void>(); configs.len()];
        let Some(config_size) = runtime::counted::<i32>(configs.len(), 1) else { runtime::refuse(constants::BAD_PARAMETER); return Default::default(); };
        let mut num_config: i32 = 0;
        #[expect(unsafe_code, reason = "egl.xml: eglChooseConfig takes the C types the field declares; dpy is a Display of this library; attrib_list ends at a key EGL_NONE; configs_room holds config_size handles; config_size is configs's length")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, attrib_list.as_ptr(), configs_room.as_mut_ptr(), config_size, &raw mut num_config) };
        let made = usize::try_from(num_config).unwrap_or(0).min(configs.len());
        for (slot, (at, address)) in configs.iter_mut().zip(configs_room.iter().enumerate()) { *slot = (result != 0 && at < made && !address.is_null()).then(|| Config::new(*address, dpy, self)); }
        (result != 0).then_some(num_config)
    }

    fn create_context(&self, dpy: &Display, config: &Config, share_context: Option<&Context>, attrib_list: &[i32]) -> Option<Self::Context> {
        let _gate = self.reading();
        let Some(entry) = self.entries().create_context else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = config;
        let Some(config) = self.config_address(handle, dpy) else { runtime::refuse(constants::BAD_CONFIG); return Default::default(); };
        let share_context = match share_context { None => core::ptr::null_mut(), Some(handle) => match self.held_address(&handle.held, dpy) { Some(address) => address, None => { runtime::refuse(constants::BAD_CONTEXT); return Default::default(); } } };
        let Some(attrib_list) = runtime::attribute_list(attrib_list, FORCED_EGL_CREATE_CONTEXT) else { runtime::refuse(constants::BAD_ATTRIBUTE); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglCreateContext takes the C types the field declares; dpy is a Display of this library; config is a Config of this library; share_context is a Context of this library or none; attrib_list ends at a key EGL_NONE")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, config, share_context, attrib_list.as_ptr()) };
        (!result.is_null()).then(|| Context::new(result, dpy, self))
    }

    fn create_pbuffer_surface(&self, dpy: &Display, config: &Config, attrib_list: &[i32]) -> Option<Self::Surface> {
        let _gate = self.reading();
        let Some(entry) = self.entries().create_pbuffer_surface else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = config;
        let Some(config) = self.config_address(handle, dpy) else { runtime::refuse(constants::BAD_CONFIG); return Default::default(); };
        let Some(attrib_list) = runtime::attribute_list(attrib_list, FORCED_EGL_CREATE_PBUFFER_SURFACE) else { runtime::refuse(constants::BAD_ATTRIBUTE); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglCreatePbufferSurface takes the C types the field declares; dpy is a Display of this library; config is a Config of this library; attrib_list ends at a key EGL_NONE")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, config, attrib_list.as_ptr()) };
        (!result.is_null()).then(|| Surface::new(result, dpy, self))
    }

    fn destroy_context(&self, dpy: &Display, ctx: &Context) -> bool {
        let _gate = self.writing();
        let Some(entry) = self.entries().destroy_context else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = ctx;
        let Some(ctx) = self.held_address(&handle.held, dpy) else { runtime::refuse(constants::BAD_CONTEXT); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglDestroyContext takes the C types the field declares; dpy is a Display of this library; ctx is a Context of this library")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, ctx) };
        if result != 0 { handle.held.end(); }
        result != 0
    }

    fn destroy_surface(&self, dpy: &Display, surface: &Surface) -> bool {
        let _gate = self.writing();
        let Some(entry) = self.entries().destroy_surface else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = surface;
        let Some(surface) = self.held_address(&handle.held, dpy) else { runtime::refuse(constants::BAD_SURFACE); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglDestroySurface takes the C types the field declares; dpy is a Display of this library; surface is a Surface of this library")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, surface) };
        if result != 0 { handle.held.end(); }
        result != 0
    }

    fn get_config_attrib(&self, dpy: &Display, config: &Config, attribute: i32) -> Option<i32> {
        let _gate = self.reading();
        let Some(entry) = self.entries().get_config_attrib else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = config;
        let Some(config) = self.config_address(handle, dpy) else { runtime::refuse(constants::BAD_CONFIG); return Default::default(); };
        let mut value: i32 = Default::default();
        #[expect(unsafe_code, reason = "egl.xml: eglGetConfigAttrib takes the C types the field declares; dpy is a Display of this library; config is a Config of this library; value is one i32 of ours")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, config, attribute, &raw mut value) };
        (result != 0).then_some(value)
    }

    fn get_display(&self) -> Option<Self::Display> {
        let _gate = self.reading();
        let Some(entry) = self.entries().get_display else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglGetDisplay takes the C types the field declares; display_id is EGL_DEFAULT_DISPLAY")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(core::ptr::null_mut()) };
        (!result.is_null()).then(|| Display::new(result, self))
    }

    fn get_error(&self) -> i32 {
        let _gate = self.reading();
        if let Some(error) = runtime::take_refusal() { return error; }
        let Some(entry) = self.entries().get_error else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglGetError takes the C types the field declares; plain values only")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry() };
        result
    }

    fn initialize(&self, dpy: &Display) -> Option<(i32, i32)> {
        let _gate = self.reading();
        let Some(entry) = self.entries().initialize else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let mut major: i32 = Default::default();
        let mut minor: i32 = Default::default();
        #[expect(unsafe_code, reason = "egl.xml: eglInitialize takes the C types the field declares; dpy is a Display of this library; major is one i32 of ours; minor is one i32 of ours")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, &raw mut major, &raw mut minor) };
        (result != 0).then_some((major, minor))
    }

    fn make_current(&self, dpy: &Display, draw: Option<&Surface>, read: Option<&Surface>, ctx: Option<&Context>) -> bool {
        let _gate = self.reading();
        let Some(entry) = self.entries().make_current else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let draw = match draw { None => core::ptr::null_mut(), Some(handle) => match self.held_address(&handle.held, dpy) { Some(address) => address, None => { runtime::refuse(constants::BAD_SURFACE); return Default::default(); } } };
        let read = match read { None => core::ptr::null_mut(), Some(handle) => match self.held_address(&handle.held, dpy) { Some(address) => address, None => { runtime::refuse(constants::BAD_SURFACE); return Default::default(); } } };
        let ctx = match ctx { None => core::ptr::null_mut(), Some(handle) => match self.held_address(&handle.held, dpy) { Some(address) => address, None => { runtime::refuse(constants::BAD_CONTEXT); return Default::default(); } } };
        #[expect(unsafe_code, reason = "egl.xml: eglMakeCurrent takes the C types the field declares; dpy is a Display of this library; draw is a Surface of this library or none; read is a Surface of this library or none; ctx is a Context of this library or none")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, draw, read, ctx) };
        result != 0
    }

    fn query_string(&self, dpy: Option<&Display>, name: i32) -> Option<Vec<u8>> {
        let _gate = self.reading();
        let Some(entry) = self.entries().query_string else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let dpy = match dpy { None => core::ptr::null_mut(), Some(handle) => match self.display_address(handle) { Some(address) => address, None => { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); } } };
        #[expect(unsafe_code, reason = "egl.xml: eglQueryString takes the C types the field declares; dpy is a Display of this library or none")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, name) };
        runtime::copied_text(result)
    }

    fn query_surface(&self, dpy: &Display, surface: &Surface, attribute: i32) -> Option<i32> {
        let _gate = self.reading();
        let Some(entry) = self.entries().query_surface else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = surface;
        let Some(surface) = self.held_address(&handle.held, dpy) else { runtime::refuse(constants::BAD_SURFACE); return Default::default(); };
        let mut value: i32 = Default::default();
        #[expect(unsafe_code, reason = "egl.xml: eglQuerySurface takes the C types the field declares; dpy is a Display of this library; surface is a Surface of this library; value is one i32 of ours")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, surface, attribute, &raw mut value) };
        (result != 0).then_some(value)
    }

    fn swap_buffers(&self, dpy: &Display, surface: &Surface) -> bool {
        let _gate = self.reading();
        let Some(entry) = self.entries().swap_buffers else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        let handle = surface;
        let Some(surface) = self.held_address(&handle.held, dpy) else { runtime::refuse(constants::BAD_SURFACE); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglSwapBuffers takes the C types the field declares; dpy is a Display of this library; surface is a Surface of this library")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, surface) };
        result != 0
    }

    fn swap_interval(&self, dpy: &Display, interval: i32) -> bool {
        let _gate = self.reading();
        let Some(entry) = self.entries().swap_interval else { runtime::refuse(constants::BAD_ACCESS); return Default::default(); };
        let handle = dpy;
        let Some(dpy) = self.display_address(handle) else { runtime::refuse(constants::BAD_DISPLAY); return Default::default(); };
        #[expect(unsafe_code, reason = "egl.xml: eglSwapInterval takes the C types the field declares; dpy is a Display of this library")]
        // SAFETY: the entry point is the library's own for this command, of the type gl.xml or egl.xml declares; every pointer passed is to memory of ours or the caller's, as long as the command reads or writes, and lives for the call.
        let result = unsafe { entry(dpy, interval) };
        result != 0
    }
}
