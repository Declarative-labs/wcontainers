//! The shared suite's `present_shows_the_frame`, and a present inside a pass, run on the main thread, which macOS
//! windows need.
//!
//! The standard test harness runs every test on a thread of its own.

#[cfg(target_os = "macos")]
mod host;

/// A present inside a pass, which would draw the frame in a pass of its own and commit, is refused; the pass's draw
/// lands, and the frame presents after it.
#[cfg(target_os = "macos")]
fn a_present_inside_a_pass_is_refused_and_the_pass_goes_on() -> Result<(), libtest_mimic::Failed> {
    use wcontainers_gpu::{ColorAttachment, DeviceStatus, Format, Gpu, LoadOp, PassDesc, StoreOp};
    use wcontainers_gpu_test::Host;

    /// The side of the frame.
    const SIDE: u32 = 4;
    let mut b = host::Native::make();
    let window = host::Native::window(&mut b);
    let failed = |e: wcontainers_gpu::GpuError| libtest_mimic::Failed::from(format!("{e:?}"));
    let surface = b.create_surface(window).map_err(failed)?;
    b.configure_surface(&surface, Format::Bgra8Unorm, SIDE, SIDE, false)
        .map_err(failed)?;
    let view = b.current_texture(&surface).map_err(failed)?;
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear([0, 1.0f32.to_bits(), 0, 1.0f32.to_bits()]),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    let inside = b.present(&surface);
    b.end_pass();
    let after = b.present(&surface);
    println!("HOSTILE present inside a pass: {inside:?}, after it: {after:?}");
    if inside.is_ok() || after.is_err() || b.status() != DeviceStatus::Ok {
        return Err(format!(
            "inside {inside:?}, after {after:?}, status {:?}",
            b.status()
        )
        .into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn main() {
    let mut args = libtest_mimic::Arguments::from_args();
    // One thread: the harness then runs each test on the main thread.
    args.test_threads = Some(1);
    let trials = vec![
        libtest_mimic::Trial::test("present_shows_the_frame", || {
            wcontainers_gpu_test::suite::present_shows_the_frame::<host::Native>();
            Ok(())
        }),
        libtest_mimic::Trial::test(
            "a_present_inside_a_pass_is_refused_and_the_pass_goes_on",
            a_present_inside_a_pass_is_refused_and_the_pass_goes_on,
        ),
    ];
    libtest_mimic::run(&args, trials).exit();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
