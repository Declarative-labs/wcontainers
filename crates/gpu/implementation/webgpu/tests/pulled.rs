//! What instance data fetched in the vertex program costs against vertex buffers
//! (examples/scenes/pulled.rs; the same runs in Chrome through web/check): every route draws the same
//! picture, and each route's timings are printed.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs; `WEBGPU_FRAMES` frames per route (default 20);
//! `WEBGPU_STREAMS` the streams a draw, a comma-separated list (default 4, 8 and the device's most,
//! up to 16). Timings that matter run alone: `gpu-job --alone`.

#[path = "../../../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[path = "../examples/scenes/pulled.rs"]
mod pulled;

use std::sync::OnceLock;
use std::time::Instant;

use wcontainers_gpu::{Gpu, caps::Features};

/// Milliseconds since the first call.
fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

#[test]
fn pulled_instance_data_draws_alike_and_its_cost_is_printed() {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let (mut b, adapter) = pollster::block_on(draw::device(backends)).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    assert!(
        b.caps().features.contains(Features::TIMESTAMPS),
        "timings need timestamps"
    );
    let frames: u32 = std::env::var("WEBGPU_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let most = pulled::most_streams(&b);
    let mut streams: Vec<u32> = match std::env::var("WEBGPU_STREAMS") {
        Ok(list) => list
            .split(',')
            .map(|s| s.trim().parse().expect("WEBGPU_STREAMS: numbers"))
            .collect(),
        Err(_) => vec![4, 8.min(most), most],
    };
    streams.dedup();
    for n in streams {
        for t in pulled::measure(&mut b, n, frames, now).unwrap() {
            eprintln!("{}", t.line());
        }
    }
}
