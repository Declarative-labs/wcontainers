//! How 24-bit depth is stored: the depth probe of plan 4.11.
//!
//! The back end runs it when it is made; no call of the interface reports it but `DEPTH24_UNORM`.
//!
//! Needs a GPU; `WEBGPU_API` as in unit_tests/scenes.rs.

use super::draw;

use crate::probe::Depth24;
use wcontainers_gpu::{Gpu, caps::Features};
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

/// The probe's bias moved the depth up, and the device declares `DEPTH24_UNORM` exactly when it moved
/// it by one 24-bit step, Direct3D's rule. Another amount is not refused: the bit stays clear and
/// fronts bias in the program (wgpu's Vulkan on the RTX 3080 moves it by two steps).
#[test]
fn the_depth24_probe_finds_how_depth24plus_is_stored() {
    let b = device();
    let found = b.depth24();
    eprintln!("depth24plus: {found:?}");
    let (Depth24::Unorm(v) | Depth24::Float(v) | Depth24::Other(v)) = found;
    assert!(v > 0.1 && v < 0.1001, "{found:?}");
    assert_eq!(
        b.caps().features.contains(Features::DEPTH24_UNORM),
        matches!(found, Depth24::Unorm(_))
    );
}
