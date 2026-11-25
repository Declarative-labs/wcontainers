//! TODO: say in one sentence how this implementation of wcontainers-test-window works.

use wcontainers_test_window::TestWindow;

pub struct AppkitTestWindow;

impl TestWindow for AppkitTestWindow {
    fn name(&self) -> String {
        env!("CARGO_PKG_NAME").to_owned()
    }
}
