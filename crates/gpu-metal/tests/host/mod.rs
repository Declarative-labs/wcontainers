//! The host of the shared suite on Metal, through the direct implementation of the Metal trait.
//!
//! Its window is the test-window crate's, whose shown texels the suite reads.

use std::cell::RefCell;

use wcontainers_gpu_metal::MetalGpu;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;
use wcontainers_test_window::{StandaloneTestWindow, TestWindow};
use wcontainers_test_window_appkit::AppkitTestWindow;

thread_local! {
    /// The window the test's surface was made on, whose shown texels the test reads.
    static WINDOW: RefCell<Option<AppkitTestWindow>> = const { RefCell::new(None) };
}

/// The number the suite's window goes by.
const WINDOW_NUMBER: u32 = 1;

pub struct Native;

impl wcontainers_gpu_test::Host for Native {
    type Backend = MetalGpu<DirectMetal>;

    fn make() -> MetalGpu<DirectMetal> {
        let b = MetalGpu::new(DirectMetal::new(()));
        eprintln!("adapter: {}", wcontainers_gpu::Gpu::caps(&b).adapter.name);
        b
    }

    fn window(b: &mut MetalGpu<DirectMetal>) -> u32 {
        let window = AppkitTestWindow::new(());
        b.add_window(WINDOW_NUMBER, &window)
            .expect("a layer of the window");
        WINDOW.with(|w| *w.borrow_mut() = Some(window));
        WINDOW_NUMBER
    }

    fn shown(_b: &mut MetalGpu<DirectMetal>, _window: u32) -> Vec<u8> {
        WINDOW.with(|w| w.borrow().as_ref().expect("window() first").shown_texels())
    }
}
