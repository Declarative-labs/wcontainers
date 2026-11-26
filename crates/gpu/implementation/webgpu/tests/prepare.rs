//! `prepare` builds a combination's pipeline before its first draw, so that draw builds nothing:
//! counted, and the first draws timed with and without it (printed; `--nocapture` shows them). The
//! same draws run in Chrome through the scenes example (web/check).
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs. `WEBGPU_PROGRAMS` sets how many programs each
//! half draws (default 16).

use wcontainers_graphics_test::scene;

#[allow(dead_code)]
#[path = "scenes/draw.rs"]
mod draw;

#[path = "scenes/prepare.rs"]
mod prepare;

use std::sync::OnceLock;
use std::time::Instant;

use wcontainers_gpu_webgpu::WebGpu;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let (b, adapter) = pollster::block_on(draw::device(backends)).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    b
}

/// Milliseconds since the first call.
fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

#[test]
fn prepared_combinations_build_nothing_at_their_first_draw() {
    let mut b = device();
    let n: u32 = std::env::var("WEBGPU_PROGRAMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    let unprepared = prepare::first_draws(&mut b, 0, n, false, now);
    eprintln!("{}", unprepared.line(false));
    let prepared = prepare::first_draws(&mut b, 1000, n, true, now);
    eprintln!("{}", prepared.line(true));
    assert_eq!(
        (unprepared.misses, unprepared.built),
        (u64::from(n), u64::from(n))
    );
    assert_eq!((prepared.misses, prepared.built), (0, 0));
}

/// A prepared strip is built for both index formats, so its indexed draws build nothing.
#[test]
fn prepared_strips_build_nothing_at_their_indexed_draws() {
    let mut b = device();
    let line = prepare::prepared_strips(&mut b).unwrap();
    eprintln!("{line}");
}
