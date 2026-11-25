//! What this implementation does beyond the shared suite: the refusals that keep Metal from stopping the process, and
//! the features it declares that WebGPU on Metal lacks.

#![cfg(target_os = "macos")]

use wcontainers_gpu::{Format, Gpu, GpuError, TextureDesc, TextureDimension, TextureUsage};
use wcontainers_gpu_metal::MetalGpu;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;

/// A side longer than any Apple GPU's longest texture side, which is 32768.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const BEYOND_ANY_SIDE: u32 = 40_000;

/// Returns the interface's device on the system's GPU.
fn device() -> MetalGpu<DirectMetal> {
    MetalGpu::new(DirectMetal::new(()))
}

#[test]
fn a_texture_wider_than_metal_makes_is_refused_not_fatal() {
    let mut gpu = device();
    let texture = gpu.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: BEYOND_ANY_SIDE,
        height: 1,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: TextureUsage::SAMPLE,
        cube: false,
        reinterpret: false,
    });
    assert!(matches!(texture, Err(GpuError::Invalid(_))));
}
