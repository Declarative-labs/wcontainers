//! The native host of the conformance suite and of the window tests: a device on the GPU `WEBGPU_API` names, and a window whose
//! shown pixels the test reads (the test-window crate's, on Windows and macOS).
//! The crate including it declares `scene` and `draw` as conformance_run.rs does.

use std::cell::RefCell;
use std::sync::Arc;

use wcontainers_gpu::{
    Gpu, draw::ColorAttachment, draw::LoadOp, draw::PassDesc, draw::StoreOp, error::DeviceStatus,
    format::Format, slow::SlowPath,
};
use wcontainers_gpu_webgpu::WebGpu;

thread_local! {
    /// The instance the test's device was made on, which its surfaces must come from too.
    static INSTANCE: RefCell<Option<wgpu::Instance>> = const { RefCell::new(None) };
    /// The window the test's surface was made on, whose shown pixels the test reads.
    static WINDOW: RefCell<Option<Arc<window::Window>>> = const { RefCell::new(None) };
}

pub struct Native;

#[allow(dead_code)]
impl Native {
    /// A device on the GPU `WEBGPU_API` names.
    pub fn make() -> WebGpu {
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

    /// Opens the test window and registers it as window 1 of `b`.
    pub fn window(b: &mut WebGpu) -> u32 {
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

    /// What the test window shows.
    pub fn shown(b: &mut WebGpu, _: u32) -> Vec<u8> {
        let converted = b.slow_paths().counts[SlowPath::PresentConverted as usize];
        eprintln!("frames converted at present (PresentConverted): {converted}");
        // A Metal layer takes no `rgba8unorm`, so there every frame is drawn into the window's format.
        if cfg!(target_os = "macos") {
            assert!(converted > 0);
        }
        WINDOW.with(|w| window::shown(w.borrow().as_ref().expect("window() first")))
    }

    /// A device with the test window registered as window 1 where a test thread may open one (Windows);
    /// elsewhere a device alone, whose surfaces the conformance suite then cannot make.
    pub fn make_presenting() -> WebGpu {
        let mut b = Self::make();
        if cfg!(windows) {
            Self::window(&mut b);
        }
        b
    }

    /// What is drawn into the surface's current texture is what the window shows, frame after frame.
    pub fn present_shows_the_frame() {
        let mut b = Self::make();
        let window = Self::window(&mut b);
        let surface = b.create_surface(window).expect("create_surface");
        b.configure_surface(&surface, Format::Rgba8Unorm, 4, 4, true)
            .expect("configure_surface");
        for (value, texel) in [
            ([0.2f32, 0.4, 0.6, 1.0], [51u8, 102, 153, 255]),
            ([1.0, 0.0, 0.2, 1.0], [255, 0, 51, 255]),
        ] {
            let v = b.current_texture(&surface).expect("current_texture");
            let mut pass = PassDesc {
                colors: Default::default(),
                depth: None,
                empty: None,
                occlusion: None,
            };
            pass.colors[0] = Some(ColorAttachment {
                view: &v,
                depth_slice: 0,
                resolve: None,
                load: LoadOp::Clear(value.map(f32::to_bits)),
                store: StoreOp::Store,
            });
            b.begin_pass(&pass);
            b.end_pass();
            b.present(&surface).expect("present");
            assert_eq!(Self::shown(&mut b, window), texel.repeat(16));
        }
        assert_eq!(b.status(), DeviceStatus::Ok);
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
