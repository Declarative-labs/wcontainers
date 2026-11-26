//! `present_shows_the_frame`, reading the window, on the main thread, which macOS windows need (the
//! standard test harness runs every test on a thread of its own). On Windows tests/surface.rs runs it.

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[cfg(not(target_arch = "wasm32"))]
mod host;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let mut args = libtest_mimic::Arguments::from_args();
    // One thread: the harness then runs each test on the main thread.
    args.test_threads = Some(1);
    let trials = if cfg!(windows) {
        Vec::new()
    } else {
        vec![libtest_mimic::Trial::test(
            "present_shows_the_frame",
            || {
                host::Native::present_shows_the_frame();
                Ok(())
            },
        )]
    };
    libtest_mimic::run(&args, trials).exit();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
