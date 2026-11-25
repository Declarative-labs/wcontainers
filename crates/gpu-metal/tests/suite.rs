//! The shared suite of `wcontainers-gpu-test`, run on Metal, but for `present_shows_the_frame`.
//!
//! That one runs in tests/present.rs, on the main thread, which macOS windows need.

#![cfg(target_os = "macos")]

mod host;

#[macro_use]
#[path = "../../gpu-webgpu/examples/scenes/names.rs"]
mod names;

use host::Native;

suite_but_present!(wcontainers_gpu_test::suite { @each Native; });
