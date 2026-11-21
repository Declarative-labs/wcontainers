//! Formats WebGPU lacks held as `rgba8unorm` (`FormatConverted`): declared, copied exactly, loaded
//! and filtered within their bounds of Direct3D's values, and 8-bit alpha drawn with its colour
//! kept zero (examples/scenes/converted.rs; the same runs in Chrome through web/check, and against
//! real Direct3D 11 in tests/direct3d.rs). The three-channel 32-bit formats, held in their
//! four-channel twins, keep every bit with alpha one.
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

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn converted_formats_copy_exactly_and_load_as_direct3d_says() {
    let (mut b, adapter) = pollster::block_on(draw::device(backends())).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    let seed = env("WEBGPU_SEED", 1);
    let cases = env("WEBGPU_CASES", 4) as u32;
    let line = converted::check(&mut b, seed, cases).unwrap();
    eprintln!("{line}");
}

#[test]
fn three_channel_formats_keep_every_bit_with_alpha_one() {
    let (mut b, adapter) = pollster::block_on(draw::device(backends())).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    let seed = env("WEBGPU_SEED", 1);
    let cases = env("WEBGPU_CASES", 4) as u32;
    let line = converted::three_channels(&mut b, seed, cases).unwrap();
    eprintln!("{line}");
}

/// The three-channel formats at the device's widest 2D texture, copied from a buffer, between
/// textures and into a buffer, every bit kept: the raw textures the copies go through are three
/// times as wide, so they are made in runs (before, past a third of the widest the device was
/// lost on wgpu's validation).
#[test]
fn three_channel_copies_at_the_widest_texture_keep_every_bit() {
    let (mut b, adapter) = pollster::block_on(draw::device(backends())).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    let seed = env("WEBGPU_SEED", 1);
    let line = converted::three_channels_at_widest(&mut b, seed).unwrap();
    eprintln!("{line}");
}
