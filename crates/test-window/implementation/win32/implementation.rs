//! A Win32 popup on the calling thread, read as the desktop compositor shows it through `PrintWindow`.
#![cfg(windows)]

use std::num::NonZeroIsize;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    Win32WindowHandle, WindowHandle,
};
use wcontainers_test_window::{StandaloneTestWindow, TEXEL_BYTES, TestWindow, WINDOW_SIDE};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
};
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetClientRect, MSG, PM_REMOVE, PW_RENDERFULLCONTENT,
    PeekMessageW, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

/// The predefined window class the popup takes, as UTF-16 with its terminating zero.
const CLASS: &str = "STATIC\0";

/// The bits of one texel of the bitmap the window is printed into: blue, green, red and an unused byte.
const BITMAP_TEXEL_BITS: u16 = 32;

/// A 4 by 4 popup window, which is never destroyed.
pub struct Win32TestWindow {
    /// The window's handle, kept as its number so that a value may go to any thread.
    window: NonZeroIsize,
    /// The program's module, which Vulkan's surfaces need.
    module: Option<NonZeroIsize>,
}

impl StandaloneTestWindow for Win32TestWindow {
    type Config = ();

    fn new(_config: ()) -> Win32TestWindow {
        let class: Vec<u16> = CLASS.encode_utf16().collect();
        let side = WINDOW_SIDE as i32;
        #[expect(unsafe_code, reason = "the calls that make the window")]
        // SAFETY: a predefined window class, no parent, menu or creation data. Awareness of each monitor's scale
        // keeps the window 4 by 4 pixels.
        let window = unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                side,
                side,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        let window = NonZeroIsize::new(window as isize).expect("CreateWindowExW");
        #[expect(unsafe_code, reason = "the call that names the program's module")]
        // SAFETY: no name, which asks for the module of this program.
        let module = unsafe { GetModuleHandleW(std::ptr::null()) };
        Win32TestWindow {
            window,
            module: NonZeroIsize::new(module as isize),
        }
    }
}

impl TestWindow for Win32TestWindow {
    fn shown_texels(&self) -> Vec<u8> {
        let hwnd = self.window.get() as HWND;
        #[expect(
            unsafe_code,
            reason = "the calls that print the window into a bitmap and read it"
        )]
        // SAFETY: plain Win32 calls on this thread's window, into structures of this function; every object made is
        // released.
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
            let printed = PrintWindow(hwnd, dc, PW_RENDERFULLCONTENT);
            SelectObject(dc, old);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: BITMAP_TEXEL_BITS,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bgra = vec![0u8; w as usize * h as usize * TEXEL_BYTES];
            let rows = GetDIBits(
                dc,
                bitmap,
                0,
                h as u32,
                bgra.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            );
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            assert_ne!(printed, 0, "PrintWindow");
            assert_eq!(rows, h, "GetDIBits");
            // A window holds no alpha: what it shows is opaque.
            bgra.as_chunks::<TEXEL_BYTES>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], u8::MAX])
                .collect()
        }
    }
}

impl HasWindowHandle for Win32TestWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let mut window = Win32WindowHandle::new(self.window);
        window.hinstance = self.module;
        #[expect(
            unsafe_code,
            reason = "the promise that the window outlives the handle"
        )]
        // SAFETY: the window is never destroyed.
        let handle = unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(window)) };
        Ok(handle)
    }
}

impl HasDisplayHandle for Win32TestWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}
