//! A window a test presents into, and the texels the system shows in it.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// The width and the height of a test window in pixels.
pub const WINDOW_SIDE: usize = 4;

/// The bytes of one shown texel: red, green, blue and alpha.
pub const TEXEL_BYTES: usize = 4;

/// A borderless window of `WINDOW_SIDE` by `WINDOW_SIDE` pixels, which a GPU API presents into through its handles.
///
/// It lives until the process ends. Its texels are read on the thread that made it.
pub trait TestWindow: HasWindowHandle + HasDisplayHandle + Send + Sync {
    /// Returns the texels the system shows in the window once its last presented frame is on screen.
    ///
    /// Rows come top first, each texel as RGBA in `TEXEL_BYTES` bytes, alpha opaque since a window holds none.
    fn shown_texels(&self) -> Vec<u8>;
}

/// A test window made on its own, on the calling thread.
pub trait StandaloneTestWindow: TestWindow {
    type Config;

    fn new(config: Self::Config) -> Self;
}
