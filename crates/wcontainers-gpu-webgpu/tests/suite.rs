//! The shared back-end suite of `wcontainers-gpu-test`, run on this back end on a real GPU.
//!
//! Needs a GPU, and for presenting a desktop to open a window on (on the rig: `gpu-job --desktop`);
//! `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use std::cell::RefCell;

use wcontainers_gpu_webgpu::WebGpu;

thread_local! {
    /// The instance the test's device was made on, which its surfaces must come from too.
    static INSTANCE: RefCell<Option<wgpu::Instance>> = const { RefCell::new(None) };
}

struct Native;

impl wcontainers_gpu_test::Host for Native {
    type Backend = WebGpu;

    fn make() -> WebGpu {
        let backends = match std::env::var("WEBGPU_API").as_deref() {
            Ok("dx12") => wgpu::Backends::DX12,
            Ok("vulkan") => wgpu::Backends::VULKAN,
            Ok("metal") => wgpu::Backends::METAL,
            _ => wgpu::Backends::PRIMARY,
        };
        let instance = draw::instance(backends);
        let (b, adapter) = pollster::block_on(draw::device_on(&instance)).expect("a GPU adapter");
        eprintln!("adapter: {adapter}");
        INSTANCE.with(|i| *i.borrow_mut() = Some(instance));
        b
    }

    fn window(b: &mut WebGpu) -> u32 {
        const WINDOW: u32 = 1;
        let surface = INSTANCE.with(|i| {
            let instance = i.borrow();
            let instance = instance.as_ref().expect("make() first");
            // SAFETY: the window is never destroyed while the test runs.
            unsafe { instance.create_surface_unsafe(window_target()) }.expect("a surface")
        });
        b.add_window(WINDOW, surface);
        WINDOW
    }

    fn shown(_: &mut WebGpu, _: u32) -> Option<Vec<u8>> {
        // A swap chain's texels cannot be read back once presented.
        None
    }
}

/// A new plain window of this desktop.
#[cfg(windows)]
fn window_target() -> wgpu::SurfaceTargetUnsafe {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    // SAFETY: a predefined window class, no parent, menu or creation data.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            64,
            64,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    let hwnd = std::num::NonZeroIsize::new(hwnd as isize).expect("CreateWindowExW");
    let mut window = wgpu::rwh::Win32WindowHandle::new(hwnd);
    // SAFETY: the module of this program; Vulkan's surfaces need it.
    let module =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    window.hinstance = std::num::NonZeroIsize::new(module as isize);
    wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
            wgpu::rwh::WindowsDisplayHandle::new(),
        )),
        raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(window),
    }
}

#[cfg(not(windows))]
fn window_target() -> wgpu::SurfaceTargetUnsafe {
    panic!("this host opens windows only on Windows");
}

wcontainers_gpu_test::suite!(Native);
