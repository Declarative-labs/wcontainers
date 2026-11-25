//! The shared suite's `present_shows_the_frame`, run on the main thread, which macOS windows need.
//!
//! The standard test harness runs every test on a thread of its own.

#[cfg(target_os = "macos")]
mod host;

#[cfg(target_os = "macos")]
fn main() {
    let mut args = libtest_mimic::Arguments::from_args();
    // One thread: the harness then runs each test on the main thread.
    args.test_threads = Some(1);
    let trials = vec![libtest_mimic::Trial::test(
        "present_shows_the_frame",
        || {
            wcontainers_gpu_test::suite::present_shows_the_frame::<host::Native>();
            Ok(())
        },
    )];
    libtest_mimic::run(&args, trials).exit();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
