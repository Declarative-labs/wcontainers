//! The shared back-end suite of `wcontainers-gpu-test`, run on this back end on a real GPU.
//!
//! Needs a GPU, and for presenting a desktop to open a window on (on the rig: `gpu-job --desktop`);
//! `WEBGPU_API` as in tests/scenes.rs. Elsewhere than Windows, present runs in tests/present.rs on
//! the main thread, which macOS windows need.

#![cfg(not(target_arch = "wasm32"))]

#[path = "../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

mod host;

#[cfg(not(windows))]
#[macro_use]
#[path = "../examples/scenes/names.rs"]
mod names;

use host::Native;

#[cfg(windows)]
wcontainers_gpu_test::suite!(Native);

#[cfg(not(windows))]
suite_but_present!(wcontainers_gpu_test::suite { @each Native; });
