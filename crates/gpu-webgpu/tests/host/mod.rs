//! The native host of the shared suite: a device on the GPU `WEBGPU_API` names, and a window whose
//! shown pixels the test reads (the test-window crate's, on Windows and macOS).
//! The crate including it declares `scene` and `draw` as tests/suite.rs does.

use std::cell::RefCell;
use std::sync::Arc;

use wcontainers_gpu::{Gpu, SlowPath};
use wcontainers_gpu_webgpu::WebGpu;

thread_local! {
    /// The instance the test's device was made on, which its surfaces must come from too.
    static INSTANCE: RefCell<Option<wgpu::Instance>> = const { RefCell::new(None) };
    /// The window the test's surface was made on, whose shown pixels the test reads.
    static WINDOW: RefCell<Option<Arc<window::Window>>> = const { RefCell::new(None) };
}

pub struct Native;

impl wcontainers_gpu_test::Host for Native {
    type Backend = WebGpu;

    fn make() -> WebGpu {
        let backends = match std::env::var("WEBGPU_API").as_deref() {
            Ok("dx12") => wgpu::Backends::DX12,
            Ok("vulkan") => wgpu::Backends::VULKAN,
            Ok("metal") => wgpu::Backends::METAL,
            _ => wgpu::Backends::PRIMARY,
        };
        let instance = crate::draw::instance(backends);
        let (b, adapter) =
            pollster::block_on(crate::draw::device_on(&instance)).expect("a GPU adapter");
        eprintln!("adapter: {adapter}");
        INSTANCE.with(|i| *i.borrow_mut() = Some(instance));
        b
    }

    fn window(b: &mut WebGpu) -> u32 {
        const NATIVE_WINDOW_HANDLE: u32 = 1;
        let surface = INSTANCE.with(|i| {
            let instance = i.borrow();
            let instance = instance.as_ref().expect("make() first");
            let window = Arc::new(window::open());
            WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
            instance.create_surface(window).expect("a surface")
        });
        b.add_window(NATIVE_WINDOW_HANDLE, surface);
        NATIVE_WINDOW_HANDLE
    }

    fn shown(b: &mut WebGpu, _: u32) -> Vec<u8> {
        let converted = b.slow_paths().count(SlowPath::PresentConverted);
        eprintln!("frames converted at present (PresentConverted): {converted}");
        // A Metal layer takes no `rgba8unorm`, so there every frame is drawn into the window's format.
        if cfg!(target_os = "macos") {
            assert!(converted > 0);
        }
        WINDOW.with(|w| window::shown(w.borrow().as_ref().expect("window() first")))
    }
}

/// The test's window: 4 by 4 pixels, read as the system shows it (macOS: made on the main thread, as
/// AppKit needs, tests/present.rs).
#[cfg(any(windows, target_os = "macos"))]
mod window {
    use wcontainers_test_window::{StandaloneTestWindow, TestWindow};

    #[cfg(windows)]
    pub type Window = wcontainers_test_window_win32::Win32TestWindow;
    #[cfg(target_os = "macos")]
    pub type Window = wcontainers_test_window_appkit::AppkitTestWindow;

    pub fn open() -> Window {
        Window::new(())
    }

    /// The window's pixels as the system shows them, rows top first, RGBA with opaque alpha.
    pub fn shown(window: &Window) -> Vec<u8> {
        window.shown_texels()
    }
}

/// Elsewhere this host has no window to read.
#[cfg(not(any(windows, target_os = "macos")))]
mod window {
    use wgpu::rwh::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};

    /// No window: none is made here.
    pub struct Window;

    impl HasWindowHandle for Window {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            Err(HandleError::NotSupported)
        }
    }

    impl HasDisplayHandle for Window {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            Err(HandleError::NotSupported)
        }
    }

    pub fn open() -> Window {
        panic!("this host reads what a window shows only on Windows and macOS");
    }

    pub fn shown(_: &Window) -> Vec<u8> {
        unreachable!("no window was opened")
    }
}
