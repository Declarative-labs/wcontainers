//! The shared back-end suite of `wcontainers-gpu-test`, run on this back end on a real GPU.
//!
//! Needs a GPU, and for presenting a desktop to open a window on (on the rig: `gpu-job --desktop`);
//! `WEBGPU_API` as in tests/scenes.rs.

#![cfg(not(target_arch = "wasm32"))]

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
            unsafe { instance.create_surface_unsafe(window::open()) }.expect("a surface")
        });
        b.add_window(WINDOW, surface);
        WINDOW
    }

    fn shown(_: &mut WebGpu, _: u32) -> Vec<u8> {
        window::shown()
    }
}

/// The test's window: a 4 by 4 popup whose pixels are read as the compositor shows them.
#[cfg(windows)]
mod window {
    use std::cell::Cell;

    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Dwm::DwmFlush;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::Storage::Xps::PrintWindow;
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetClientRect, MSG, PM_REMOVE, PW_RENDERFULLCONTENT,
        PeekMessageW, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    thread_local! {
        static HWND_OPEN: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    }

    pub fn open() -> wgpu::SurfaceTargetUnsafe {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: a predefined window class, no parent, menu or creation data. Awareness of each
        // monitor's scale keeps the window 4 by 4 pixels.
        let hwnd = unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                4,
                4,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        HWND_OPEN.with(|h| h.set(hwnd));
        let hwnd = std::num::NonZeroIsize::new(hwnd as isize).expect("CreateWindowExW");
        let mut window = wgpu::rwh::Win32WindowHandle::new(hwnd);
        // SAFETY: the module of this program; Vulkan's surfaces need it.
        let module = unsafe {
            windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null())
        };
        window.hinstance = std::num::NonZeroIsize::new(module as isize);
        wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                wgpu::rwh::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(window),
        }
    }

    /// The window's pixels as the compositor shows them, rows top first, RGBA with opaque alpha.
    pub fn shown() -> Vec<u8> {
        let hwnd = HWND_OPEN.with(Cell::get);
        // SAFETY: plain Win32 calls on this thread's window; every object made is released.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                DispatchMessageW(&msg);
            }
            // Two compositions: the one the presented frame entered, and one that shows it.
            DwmFlush();
            DwmFlush();
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bitmap);
            // Swap chain contents included, also on a desktop no one sees.
            assert_ne!(
                PrintWindow(hwnd, dc, PW_RENDERFULLCONTENT),
                0,
                "PrintWindow"
            );
            SelectObject(dc, old);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let rows = GetDIBits(
                dc,
                bitmap,
                0,
                h as u32,
                bgra.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            );
            assert_eq!(rows, h, "GetDIBits");
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            // A window holds no alpha: what it shows is opaque.
            bgra.as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], 255])
                .collect()
        }
    }
}

#[cfg(not(windows))]
mod window {
    pub fn open() -> wgpu::SurfaceTargetUnsafe {
        panic!("this host opens windows only on Windows");
    }

    pub fn shown() -> Vec<u8> {
        unreachable!("no window was opened")
    }
}

wcontainers_gpu_test::suite!(Native);
