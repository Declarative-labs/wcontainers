//! Formats WebGPU lacks held as `rgba8unorm` (`FormatConverted`): declared, copied exactly, loaded
//! and filtered within their bounds of Direct3D's values, and 8-bit alpha drawn with its colour
//! kept zero (examples/scenes/converted.rs; the same runs in Chrome through web/check, and against
//! real Direct3D 11 in tests/direct3d.rs).
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[path = "../examples/scenes/converted.rs"]
mod converted;

fn backends() -> wgpu::Backends {
    match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    }
}

#[test]
fn converted_formats_copy_exactly_and_load_as_direct3d_says() {
    let (mut b, adapter) = pollster::block_on(draw::device(backends())).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    let line = converted::check(&mut b).unwrap();
    eprintln!("{line}");
}
