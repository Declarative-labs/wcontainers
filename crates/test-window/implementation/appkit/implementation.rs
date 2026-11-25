//! A borderless AppKit window on the main thread, read through ScreenCaptureKit in sRGB, which undoes the display's
//! color matching exactly.
//!
//! Reading needs the screen recording permission of the process that started the test.
#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventMask,
    NSFloatingWindowLevel, NSScreen, NSView, NSWindow, NSWindowOcclusionState, NSWindowStyleMask,
};
use objc2_core_graphics::{CGDataProvider, CGImage, kCGColorSpaceSRGB};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSError, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAMetalLayer;
use objc2_screen_capture_kit::{
    SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
};
use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawWindowHandle, WindowHandle,
};
use wcontainers_test_window::{StandaloneTestWindow, TEXEL_BYTES, TestWindow, WINDOW_SIDE};

/// Where a capture's texels, or why there are none, arrive from ScreenCaptureKit's queue.
type Answer = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

/// The bits of one texel of a capture: blue, green, red and alpha.
const CAPTURE_TEXEL_BITS: usize = 32;

/// The pixel format a capture is made in, as its four-character code.
const CAPTURE_FORMAT: &[u8; 4] = b"BGRA";

/// The most times a new window waits `VISIBLE_WAIT_SECONDS` for the window server to call it visible.
const VISIBLE_WAITS: usize = 200;

/// How long each wait for a new window to become visible handles events, in seconds.
const VISIBLE_WAIT_SECONDS: f64 = 0.01;

/// How long events are handled before a capture, so that the presented frame reaches the window server, in seconds.
const PRESENT_WAIT_SECONDS: f64 = 0.2;

/// How long events are handled between looks for a capture's answer, in seconds.
const ANSWER_WAIT_SECONDS: f64 = 0.01;

thread_local! {
    /// Every window made on this thread and the view its surface is made on, kept until the process ends.
    static WINDOWS: RefCell<Vec<(Retained<NSWindow>, Retained<NSView>)>> = const { RefCell::new(Vec::new()) };
}

/// A borderless window of 4 by 4 pixels in the middle of the main screen, above other windows.
pub struct AppkitTestWindow {
    /// The window server's number of the window.
    number: isize,
    /// The address of the window's content view, whose own layer is a `CAMetalLayer`, kept so that a value may go to
    /// any thread.
    view: usize,
}

impl StandaloneTestWindow for AppkitTestWindow {
    type Config = ();

    fn new(_config: ()) -> AppkitTestWindow {
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
        #[expect(unsafe_code, reason = "the call that makes the window")]
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
        #[expect(unsafe_code, reason = "the call that keeps the window when closed")]
        // SAFETY: the window is held by `WINDOWS`, not released on close.
        unsafe {
            window.setReleasedWhenClosed(false)
        };
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
        // Above other windows, and seen by the window server before frames come: wgpu hands out no texture while a
        // window is occluded.
        window.setLevel(NSFloatingWindowLevel);
        let visible = || {
            window
                .occlusionState()
                .contains(NSWindowOcclusionState::Visible)
        };
        for _ in 0..VISIBLE_WAITS {
            if visible() {
                break;
            }
            run(VISIBLE_WAIT_SECONDS);
        }
        assert!(
            visible(),
            "macOS reports the test's window as not visible (occlusion state \
             {:#x}), so it shows no frame",
            window.occlusionState().0
        );
        let made = AppkitTestWindow {
            number: window.windowNumber(),
            view: Retained::as_ptr(&view).expose_provenance(),
        };
        WINDOWS.with(|windows| windows.borrow_mut().push((window, view)));
        made
    }
}

impl TestWindow for AppkitTestWindow {
    fn shown_texels(&self) -> Vec<u8> {
        // The presented frame reaches the window server.
        run(PRESENT_WAIT_SECONDS);
        let id = self.number as u32;
        let answer = Arc::new(Mutex::new(None));
        let slot = answer.clone();
        let handler = RcBlock::new(move |c: *mut SCShareableContent, e: *mut NSError| {
            #[expect(
                unsafe_code,
                reason = "the read of the content a completion handler is given"
            )]
            // SAFETY: the handler's content, null or valid for the call.
            let content = unsafe { c.as_ref() };
            match content {
                Some(c) => capture(c, id, slot.clone()),
                None => *slot.lock().unwrap() = Some(Err(error(e))),
            }
        });
        #[expect(unsafe_code, reason = "the call that lists what is on screen")]
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
            run(ANSWER_WAIT_SECONDS);
        }
    }
}

impl HasWindowHandle for AppkitTestWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let view = NonNull::new(std::ptr::with_exposed_provenance_mut::<c_void>(self.view))
            .expect("a view");
        #[expect(unsafe_code, reason = "the promise that the view outlives the handle")]
        // SAFETY: the view is held by `WINDOWS` until the process ends.
        let handle = unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(view)))
        };
        Ok(handle)
    }
}

impl HasDisplayHandle for AppkitTestWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::appkit())
    }
}

/// Handles the application's events for `seconds`; windows learn they are visible from them.
fn run(seconds: f64) {
    let app = NSApplication::sharedApplication(MainThreadMarker::new().expect("main thread"));
    let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
    #[expect(unsafe_code, reason = "the read of a static of AppKit")]
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

/// Returns the description of a completion handler's error, or says there was none.
fn error(e: *mut NSError) -> String {
    #[expect(
        unsafe_code,
        reason = "the read of the error a completion handler is given"
    )]
    // SAFETY: a completion handler's error, null or valid for the call.
    let e = unsafe { e.as_ref() };
    e.map_or("no answer and no error".into(), |e| {
        e.localizedDescription().to_string()
    })
}

/// Returns the texels of a capture made in BGRA, rows top first, RGBA; a window holds no alpha, so what it shows is
/// opaque.
fn texels(image: &CGImage) -> Result<Vec<u8>, String> {
    let image = Some(image);
    let size = (
        CGImage::width(image),
        CGImage::height(image),
        CGImage::bits_per_pixel(image),
    );
    if size != (WINDOW_SIDE, WINDOW_SIDE, CAPTURE_TEXEL_BITS) {
        return Err(format!("a capture of {size:?} (width, height, bits)"));
    }
    let pitch = CGImage::bytes_per_row(image);
    let bytes = CGDataProvider::data(CGImage::data_provider(image).as_deref())
        .ok_or("a capture without bytes")?
        .to_vec();
    Ok((0..WINDOW_SIDE)
        .flat_map(|y| {
            bytes[y * pitch..][..WINDOW_SIDE * TEXEL_BYTES]
                .as_chunks::<TEXEL_BYTES>()
                .0
                .to_vec()
        })
        .flat_map(|p| [p[2], p[1], p[0], u8::MAX])
        .collect())
}

/// Captures window `id` in sRGB into `answer`. Runs on ScreenCaptureKit's queue, so only the texels cross threads.
fn capture(content: &SCShareableContent, id: u32, answer: Answer) {
    #[expect(unsafe_code, reason = "the calls that capture one window")]
    // SAFETY: plain property reads, a filter of one window, a configuration of plain properties, and a handler that
    // keeps what it needs and is called once.
    unsafe {
        let Some(window) = content.windows().iter().find(|w| w.windowID() == id) else {
            *answer.lock().unwrap() = Some(Err("the window is not on screen".into()));
            return;
        };
        let filter =
            SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window);
        let config = SCStreamConfiguration::new();
        config.setWidth(WINDOW_SIDE);
        config.setHeight(WINDOW_SIDE);
        config.setColorSpaceName(kCGColorSpaceSRGB);
        config.setPixelFormat(u32::from_be_bytes(*CAPTURE_FORMAT));
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
    };
}
