//! TODO: say in one sentence how this implementation of wcontainers-test-window works.

use wcontainers_test_window::TestWindow;

pub struct Win32TestWindow;

impl TestWindow for Win32TestWindow {
    fn name(&self) -> String {
        env!("CARGO_PKG_NAME").to_owned()
    }
}
