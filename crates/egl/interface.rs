// Written by tools/api-crate-generator from Khronos's egl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
//! EGL, the Khronos interface between OpenGL ES and the platform (ANGLE's libEGL, Mesa's), as a safe trait.

pub mod constants;

/// EGL's library: each entry point this set carries as a safe method, each handle a value of the implementation's own type.
#[allow(reviewable::explicit_shared_error, reason = "the API reports its errors through get_error, as its registry declares")]
pub trait Egl: Sized {
    /// A handle of the implementation's own, made only by its methods.
    type Config: Clone + PartialEq + core::fmt::Debug;

    /// A handle of the implementation's own, made only by its methods.
    type Context: Clone + PartialEq + core::fmt::Debug;

    /// A handle of the implementation's own, made only by its methods.
    type Display: Clone + PartialEq + core::fmt::Debug;

    /// A handle of the implementation's own, made only by its methods.
    type Surface: Clone + PartialEq + core::fmt::Debug;

    /// Loads the EGL library named libEGL in the folder, or the system's when none is named; None when it cannot be loaded or cannot give every client API's entry point by name.
    fn load(folder: Option<&std::path::Path>) -> Option<Self>;

    /// Calls [`eglBindAPI`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn bind_api(&self, api: u32) -> bool;

    /// Calls [`eglChooseConfig`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn choose_config(&self, dpy: &Self::Display, attrib_list: &[i32], configs: &mut [Option<Self::Config>]) -> Option<i32>;

    /// Calls [`eglCreateContext`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn create_context(&self, dpy: &Self::Display, config: &Self::Config, share_context: Option<&Self::Context>, attrib_list: &[i32]) -> Option<Self::Context>;

    /// Calls [`eglCreatePbufferSurface`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn create_pbuffer_surface(&self, dpy: &Self::Display, config: &Self::Config, attrib_list: &[i32]) -> Option<Self::Surface>;

    /// Calls [`eglDestroyContext`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn destroy_context(&self, dpy: &Self::Display, ctx: &Self::Context) -> bool;

    /// Calls [`eglDestroySurface`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn destroy_surface(&self, dpy: &Self::Display, surface: &Self::Surface) -> bool;

    /// Calls [`eglGetConfigAttrib`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn get_config_attrib(&self, dpy: &Self::Display, config: &Self::Config, attribute: i32) -> Option<i32>;

    /// Calls [`eglGetDisplay`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn get_display(&self) -> Option<Self::Display>;

    /// Calls [`eglGetError`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn get_error(&self) -> i32;

    /// Calls [`eglInitialize`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn initialize(&self, dpy: &Self::Display) -> Option<(i32, i32)>;

    /// Calls [`eglMakeCurrent`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn make_current(&self, dpy: &Self::Display, draw: Option<&Self::Surface>, read: Option<&Self::Surface>, ctx: Option<&Self::Context>) -> bool;

    /// Calls [`eglQueryString`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn query_string(&self, dpy: Option<&Self::Display>, name: i32) -> Option<Vec<u8>>;

    /// Calls [`eglQuerySurface`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn query_surface(&self, dpy: &Self::Display, surface: &Self::Surface, attribute: i32) -> Option<i32>;

    /// Calls [`eglSwapBuffers`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn swap_buffers(&self, dpy: &Self::Display, surface: &Self::Surface) -> bool;

    /// Calls [`eglSwapInterval`](https://github.com/KhronosGroup/EGL-Registry/blob/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml).
    fn swap_interval(&self, dpy: &Self::Display, interval: i32) -> bool;
}
