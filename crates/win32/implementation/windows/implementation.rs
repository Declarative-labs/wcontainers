//! TODO: say in one sentence how this implementation of wcontainers-win32 works.

use wcontainers_win32::Win32;

pub struct WindowsWin32;

impl Win32 for WindowsWin32 {
    fn name(&self) -> String {
        env!("CARGO_PKG_NAME").to_owned()
    }
}
