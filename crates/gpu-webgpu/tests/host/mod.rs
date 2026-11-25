//! The native host of the shared suite: a device on the GPU `WEBGPU_API` names, and a window whose
//! shown pixels the test reads (Windows: through the compositor; macOS: through ScreenCaptureKit).
//! The crate including it declares `scene` and `draw` as tests/suite.rs does.

use std::cell::RefCell;

use wcontainers_gpu::{Backend, SlowPath};
use wcontainers_gpu_webgpu::WebGpu;

thread_local! {
    /// The instance the test's device was made on, which its surfaces must come from too.
    static INSTANCE: RefCell<Option<wgpu::Instance>> = const { RefCell::new(None) };
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
            // SAFETY: the window is never destroyed while the test runs.
            unsafe { instance.create_surface_unsafe(window::open()) }.expect("a surface")
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

/// The test's window: 4 by 4 pixels, borderless, read through ScreenCaptureKit in the surface's
/// colour space (sRGB), so the display's colour matching is undone exactly. Made on the main thread,
/// as AppKit needs (tests/present.rs); reading needs the screen recording permission of the process
/// that started the test.
#[cfg(target_os = "macos")]
mod window {
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex};

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventMask,
        NSFloatingWindowLevel, NSScreen, NSView, NSWindow, NSWindowOcclusionState,
        NSWindowStyleMask,
    };
    use objc2_core_graphics::{CGDataProvider, CGImage, kCGColorSpaceSRGB};
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSError, NSPoint, NSRect, NSSize};
    use objc2_quartz_core::CAMetalLayer;
    use objc2_screen_capture_kit::{
        SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
    };

    /// Where a capture's texels, or why there are none, arrive from ScreenCaptureKit's queue.
    type Answer = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

    /// The window's side in pixels.
    const WINDOW_SIDE: usize = 4;

    thread_local! {
        static WINDOW: RefCell<Option<(Retained<NSWindow>, Retained<CAMetalLayer>)>> =
            const { RefCell::new(None) };
    }

    pub fn open() -> wgpu::SurfaceTargetUnsafe {
        let mtm = MainThreadMarker::new().expect("windows are made on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        // Connects the process to the window server, whose events tell windows they are visible.
        app.finishLaunching();
        let screen = NSScreen::mainScreen(mtm).expect("a screen");
        let side = WINDOW_SIDE as f64 / screen.backingScaleFactor();
        // In the middle of the screen, clear of the Dock and menu bar.
        let free = screen.visibleFrame();
        let at = NSPoint::new(
            free.origin.x + free.size.width / 2.0,
            free.origin.y + free.size.height / 2.0,
        );
        let rect = NSRect::new(at, NSSize::new(side, side));
        // SAFETY: a borderless window on the main thread, kept until the process ends.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: the window is held by `WINDOW`, not released on close.
        unsafe { window.setReleasedWhenClosed(false) };
        window.orderFrontRegardless();
        // The content view's own layer is the surface's, so nothing waits on a sublayer's setup.
        let layer = CAMetalLayer::new();
        let view = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), rect.size),
        );
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        window.setContentView(Some(&view));
        // Above other windows, and seen by the window server before frames come: wgpu hands out
        // no texture while a window is occluded.
        window.setLevel(NSFloatingWindowLevel);
        let visible = || {
            window
                .occlusionState()
                .contains(NSWindowOcclusionState::Visible)
        };
        for _ in 0..200 {
            if visible() {
                break;
            }
            run(0.01);
        }
        assert!(
            visible(),
            "macOS reports the test's window as not visible (occlusion state \
             {:#x}), so it shows no frame",
            window.occlusionState().0
        );
        let target =
            wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(Retained::as_ptr(&layer) as *mut _);
        WINDOW.with(|w| *w.borrow_mut() = Some((window, layer)));
        target
    }

    /// Handles the application's events for `seconds`; windows learn they are visible from them.
    fn run(seconds: f64) {
        let app = NSApplication::sharedApplication(MainThreadMarker::new().expect("main thread"));
        let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
        // SAFETY: the default mode is a static string.
        let mode = unsafe { NSDefaultRunLoopMode };
        while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            NSEventMask::Any,
            Some(&until),
            mode,
            true,
        ) {
            app.sendEvent(&event);
        }
    }

    fn error(e: *mut NSError) -> String {
        // SAFETY: a completion handler's error, null or valid for the call.
        unsafe { e.as_ref() }.map_or("no answer and no error".into(), |e| {
            e.localizedDescription().to_string()
        })
    }

    /// The texels of a capture made in BGRA, rows top first, RGBA; a window holds no alpha, so what
    /// it shows is opaque.
    fn texels(image: &CGImage) -> Result<Vec<u8>, String> {
        let image = Some(image);
        let size = (
            CGImage::width(image),
            CGImage::height(image),
            CGImage::bits_per_pixel(image),
        );
        if size != (WINDOW_SIDE, WINDOW_SIDE, 32) {
            return Err(format!("a capture of {size:?} (width, height, bits)"));
        }
        let pitch = CGImage::bytes_per_row(image);
        let bytes = CGDataProvider::data(CGImage::data_provider(image).as_deref())
            .ok_or("a capture without bytes")?
            .to_vec();
        Ok((0..WINDOW_SIDE)
            .flat_map(|y| {
                bytes[y * pitch..][..WINDOW_SIDE * 4]
                    .as_chunks::<4>()
                    .0
                    .to_vec()
            })
            .flat_map(|p| [p[2], p[1], p[0], 255])
            .collect())
    }

    /// Captures window `id` in sRGB into `answer`. Runs on ScreenCaptureKit's queue, so only the
    /// texels cross threads.
    fn capture(content: &SCShareableContent, id: u32, answer: Answer) {
        // SAFETY: plain property reads, a filter of one window, a configuration of plain
        // properties, and a handler that keeps what it needs and is called once.
        unsafe {
            let Some(window) = content.windows().iter().find(|w| w.windowID() == id) else {
                *answer.lock().unwrap() = Some(Err("the window is not on screen".into()));
                return;
            };
            let filter = SCContentFilter::initWithDesktopIndependentWindow(
                SCContentFilter::alloc(),
                &window,
            );
            let config = SCStreamConfiguration::new();
            config.setWidth(WINDOW_SIDE);
            config.setHeight(WINDOW_SIDE);
            config.setColorSpaceName(kCGColorSpaceSRGB);
            config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
            config.setShowsCursor(false);
            let handler = RcBlock::new(move |i: *mut CGImage, e: *mut NSError| {
                let texels = i.as_ref().map_or_else(|| Err(error(e)), texels);
                *answer.lock().unwrap() = Some(texels);
            });
            SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
                &filter,
                &config,
                Some(&handler),
            );
        }
    }

    /// The window's pixels as the window server shows them, in sRGB, rows top first, RGBA.
    pub fn shown() -> Vec<u8> {
        // The presented frame reaches the window server.
        run(0.2);
        let id =
            WINDOW.with(|w| w.borrow().as_ref().expect("open() first").0.windowNumber()) as u32;
        let answer = Arc::new(Mutex::new(None));
        let slot = answer.clone();
        let handler = RcBlock::new(move |c: *mut SCShareableContent, e: *mut NSError| {
            // SAFETY: the handler's content, null or valid for the call.
            match unsafe { c.as_ref() } {
                Some(c) => capture(c, id, slot.clone()),
                None => *slot.lock().unwrap() = Some(Err(error(e))),
            }
        });
        // SAFETY: the handler keeps what it needs and is called once.
        unsafe {
            SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
                true, true, &handler,
            )
        };
        loop {
            if let Some(texels) = answer.lock().unwrap().take() {
                return texels.unwrap_or_else(|e| panic!("ScreenCaptureKit: {e}"));
            }
            run(0.01);
        }
    }
}

/// Elsewhere this host has no window to read.
#[cfg(not(any(windows, target_os = "macos")))]
mod window {
    pub fn open() -> wgpu::SurfaceTargetUnsafe {
        panic!("this host reads what a window shows only on Windows and macOS");
    }

    pub fn shown() -> Vec<u8> {
        unreachable!("no window was opened")
    }
}
