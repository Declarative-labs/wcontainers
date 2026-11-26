//! The tests that present, run on the main thread, which macOS windows need.
//!
//! They are `present_shows_the_frame`, which reads the window the shared suite's `presents_cleared_frames` cannot, and
//! a present inside a pass. The standard test harness runs every test on a thread of its own.

#[cfg(target_os = "macos")]
mod host;

/// Returns a pass that clears `view` to `color`, given as the bits of its four floats, and stores it.
#[cfg(target_os = "macos")]
fn clearing<'a, B: wcontainers_gpu::Gpu>(
    view: &'a B::TextureView,
    color: [u32; 4],
) -> wcontainers_gpu::draw::PassDesc<'a, B> {
    use wcontainers_gpu::draw::{ColorAttachment, LoadOp, PassDesc, StoreOp};
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(color),
        store: StoreOp::Store,
    });
    pass
}

/// Checks that what is drawn into the surface's current texture is what the window shows, frame after frame.
#[cfg(target_os = "macos")]
fn present_shows_the_frame() -> Result<(), libtest_mimic::Failed> {
    use wcontainers_gpu::{Gpu, caps::FormatUse, error::DeviceStatus, format::Format};

    // Each frame's clear color, and the texel it shows as.
    const FRAMES: [([f32; 4], [u8; 4]); 2] = [
        ([0.2, 0.4, 0.6, 1.0], [51, 102, 153, 255]),
        ([1.0, 0.0, 0.2, 1.0], [255, 0, 51, 255]),
    ];
    // The frame is as large as the window it shows in.
    let side = wcontainers_test_window::WINDOW_SIDE;
    let mut b = host::make();
    if !b.caps().formats.uses[Format::Rgba8Unorm as usize].contains(FormatUse::PRESENT) {
        return Err("rgba8unorm does not present".into());
    }
    let window = host::window(&mut b);
    let failed =
        |e: wcontainers_gpu::error::GpuError| libtest_mimic::Failed::from(format!("{e:?}"));
    let surface = b.create_surface(window).map_err(failed)?;
    let edge = u32::try_from(side).map_err(|_| "a window side past u32")?;
    b.configure_surface(&surface, Format::Rgba8Unorm, edge, edge, true)
        .map_err(failed)?;
    for (color, texel) in FRAMES {
        let view = b.current_texture(&surface).map_err(failed)?;
        b.begin_pass(&clearing::<_>(&view, color.map(f32::to_bits)));
        b.end_pass();
        b.present(&surface).map_err(failed)?;
        let shown = host::shown();
        if shown != texel.repeat(side * side) {
            return Err(
                format!("the window shows {shown:?}, not {side}x{side} of {texel:?}").into(),
            );
        }
    }
    if b.status() != DeviceStatus::Ok {
        return Err(format!("status {:?}", b.status()).into());
    }
    Ok(())
}

/// Checks that a present inside a pass, which would draw the frame in a pass of its own and commit, is refused.
///
/// The frame presents after the pass.
#[cfg(target_os = "macos")]
fn a_present_inside_a_pass_is_refused_and_the_pass_goes_on() -> Result<(), libtest_mimic::Failed> {
    use wcontainers_gpu::{Gpu, error::DeviceStatus, format::Format};

    // The frame is as large as the window it shows in.
    let side = u32::try_from(wcontainers_test_window::WINDOW_SIDE)
        .map_err(|_| "a window side past u32")?;
    let mut b = host::make();
    let window = host::window(&mut b);
    let failed =
        |e: wcontainers_gpu::error::GpuError| libtest_mimic::Failed::from(format!("{e:?}"));
    let surface = b.create_surface(window).map_err(failed)?;
    b.configure_surface(&surface, Format::Bgra8Unorm, side, side, false)
        .map_err(failed)?;
    let view = b.current_texture(&surface).map_err(failed)?;
    b.begin_pass(&clearing::<_>(
        &view,
        [0, 1.0f32.to_bits(), 0, 1.0f32.to_bits()],
    ));
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
        libtest_mimic::Trial::test("present_shows_the_frame", present_shows_the_frame),
        libtest_mimic::Trial::test(
            "a_present_inside_a_pass_is_refused_and_the_pass_goes_on",
            a_present_inside_a_pass_is_refused_and_the_pass_goes_on,
        ),
    ];
    libtest_mimic::run(&args, trials).exit();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
