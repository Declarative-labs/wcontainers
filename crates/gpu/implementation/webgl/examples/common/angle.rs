//! The desktop's test host, the module `host` the checks name: ANGLE's OpenGL ES 3.0 on Windows, and a 4 by 4 window read as the
//! compositor shows it.

use std::cell::RefCell;
use std::path::PathBuf;
use wcontainers_test_window::{StandaloneTestWindow, TestWindow};
use wcontainers_test_window_win32::Win32TestWindow;
use wgpu::rwh::HasWindowHandle;

/// The device the checks run on: the back end on the OpenGL ES crate's direct implementation.
pub type Backend = wcontainers_gpu_webgl::Gl<wcontainers_opengl_es_direct::OpenGlEsContext>;

/// A new device on ANGLE.
pub fn make() -> Backend {
    let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
        || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
        PathBuf::from,
    );
    let off = std::env::var("WEBGL_OFF").unwrap_or_default();
    let off: Vec<&str> = off.split_whitespace().collect();
    use wcontainers_gpu::{Gpu, StandaloneGpu};
    let mut b = Backend::new(wcontainers_gpu_webgl::GlConfig {
        egl: Some(dir),
        off: off.iter().map(|name| name.to_string()).collect(),
    })
    .expect("ANGLE's OpenGL ES 3.0 context");
    assert_eq!(
        b.status(),
        wcontainers_gpu::error::DeviceStatus::Ok,
        "the back end was lost as it was made: {}",
        b.last_log()
    );
    b
}

/// A 4 by 4 window to present into, as window 1. The back end cannot be given it yet: the OpenGL ES crate
/// makes no EGL window surface (NEEDED-FROM-API-CRATES.md), so `create_surface` refuses it.
pub fn window(_: &mut Backend) -> u32 {
    const WINDOW: u32 = 1;
    let window = Win32TestWindow::new(());
    let wgpu::rwh::RawWindowHandle::Win32(handle) =
        window.window_handle().expect("a window handle").as_raw()
    else {
        unreachable!("a Win32 window");
    };
    let _ = handle;
    OPEN.with(|open| *open.borrow_mut() = Some(window));
    WINDOW
}

/// The texels the compositor shows in the window, rows top first.
pub fn shown(_: &mut Backend, _: u32) -> Vec<u8> {
    OPEN.with(|open| {
        open.borrow()
            .as_ref()
            .expect("window() first")
            .shown_texels()
    })
}

thread_local! {
    /// The test's window: a 4 by 4 popup on the device thread, read as the compositor shows it.
    static OPEN: RefCell<Option<Win32TestWindow>> = const { RefCell::new(None) };
}
