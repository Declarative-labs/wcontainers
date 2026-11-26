//! The host of the present tests on Metal, through the direct implementation of the Metal trait.
//!
//! Its window is the test-window crate's, whose shown texels the tests read.

use std::cell::RefCell;

use wcontainers_gpu::{Gpu, StandaloneGpu};
use wcontainers_gpu_metal::{MetalGpu, MetalGpuConfig};
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;
use wcontainers_test_window::{StandaloneTestWindow, TestWindow};
use wcontainers_test_window_appkit::AppkitTestWindow;

thread_local! {
    /// The window the test's surface was made on, whose shown texels the test reads.
    static WINDOW: RefCell<Option<AppkitTestWindow>> = const { RefCell::new(None) };
}

/// The number the window goes by, as the shared suite's present test names it.
const WINDOW_NUMBER: u32 = 1;

/// Returns the interface's device on the system's GPU.
pub fn make() -> MetalGpu<DirectMetal> {
    let b = <MetalGpu<DirectMetal> as StandaloneGpu>::new(MetalGpuConfig {
        metal: DirectMetal::new(()),
    })
    .expect("Metal opens");
    eprintln!("adapter: {}", b.caps().adapter.name);
    b
}

/// Adds a window to the device and returns the number it goes by.
pub fn window(b: &mut MetalGpu<DirectMetal>) -> u32 {
    let window = AppkitTestWindow::new(());
    b.add_window(WINDOW_NUMBER, &window)
        .expect("a layer of the window");
    WINDOW.with(|w| *w.borrow_mut() = Some(window));
    WINDOW_NUMBER
}

/// Returns the texels the window shows, rows top first, tightly packed in the surface's format.
pub fn shown() -> Vec<u8> {
    WINDOW.with(|w| w.borrow().as_ref().expect("window() first").shown_texels())
}
