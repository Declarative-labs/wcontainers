//! The desktop's test host, the module `host` the checks name: ANGLE's OpenGL ES 3.0 on Windows, and a 4 by 4 window read as the
//! compositor shows it.

use std::cell::RefCell;
use std::path::PathBuf;
use wcontainers_gpu_webgl::Gl;
use wcontainers_test_window::{StandaloneTestWindow, TestWindow};
use wcontainers_test_window_win32::Win32TestWindow;
use wgpu::rwh::HasWindowHandle;

/// The device the checks run on.
pub type Backend = Gl;

/// A new device on ANGLE.
pub fn make() -> Gl {
    let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
        || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
        PathBuf::from,
    );
    let off = std::env::var("WEBGL_OFF").unwrap_or_default();
    let off: Vec<&str> = off.split_whitespace().collect();
    let mut b = Gl::on_angle(&dir, &off).expect("ANGLE's OpenGL ES 3.0 context");
    use wcontainers_gpu::Gpu;
    assert_eq!(
        b.status(),
        wcontainers_gpu::error::DeviceStatus::Ok,
        "the back end was lost as it was made: {}",
        b.last_log()
    );
    b
}

/// A 4 by 4 window to present into, given to `b` as window 1.
pub fn window(b: &mut Gl) -> u32 {
    const WINDOW: u32 = 1;
    let window = Win32TestWindow::new(());
    let wgpu::rwh::RawWindowHandle::Win32(handle) =
        window.window_handle().expect("a window handle").as_raw()
    else {
        unreachable!("a Win32 window");
    };
    OPEN.with(|open| *open.borrow_mut() = Some(window));
    // SAFETY: the window is never destroyed while the test runs.
    unsafe { b.add_window(WINDOW, handle.hwnd.get() as *mut core::ffi::c_void) };
    WINDOW
}

/// The texels the compositor shows in the window, rows top first.
pub fn shown(_: &mut Gl, _: u32) -> Vec<u8> {
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
