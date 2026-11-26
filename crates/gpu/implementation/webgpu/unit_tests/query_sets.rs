//! Query sets past WebGPU's 4096 queries a set, and query results taken from one ring
//! (tests/scenes/queries.rs; the same runs in Chrome through web/check).
//!
//! Needs a GPU; `WEBGPU_API` as in unit_tests/scenes.rs.

use super::{draw, queries};

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

#[test]
fn query_sets_past_4096_are_paged() {
    let mut b = device();
    let line = queries::paged_sets(&mut b).unwrap();
    eprintln!("{line}");
}

#[test]
fn query_results_are_taken_from_one_ring() {
    let mut b = device();
    let line = queries::results_ring(&mut b, 20).unwrap();
    eprintln!("{line}");
}
