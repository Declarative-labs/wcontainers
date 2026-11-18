//! Vertex formats WebGPU lacks, read through shadow streams (plan 5.7, `VertexFormatConverted`):
//! every one of them, with a float beside it in the same stream, read by points at a stream offset
//! and compared with the values the format means; the shadow is kept while its source is unwritten
//! and made again after a write (examples/scenes/vertices.rs; the same runs in Chrome through
//! web/check).
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs. `WEBGPU_SEED` picks the bytes (default 188),
//! `WEBGPU_CASES` how many vertices each format draws (default 64).

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[path = "../examples/scenes/vertices.rs"]
mod vertices;

use wcontainers_gpu_webgpu::WebGpu;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    pollster::block_on(draw::device(backends))
        .expect("a GPU adapter")
        .0
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn lacking_vertex_formats_are_read_through_shadows() {
    let seed = env("WEBGPU_SEED", 188);
    let n = env("WEBGPU_CASES", 64) as u32;
    println!("seed {seed}, {n} vertices per format");
    vertices::lacking_vertex_formats(&mut device(), seed, n);
}
