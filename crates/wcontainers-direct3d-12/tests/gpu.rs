//! The rig's Direct3D 12 triangle scene through the device on the WebGPU back end, on a real GPU,
//! compared with real Direct3D 12's picture at the rig's tolerance. Needs a GPU: run it through
//! `tools/graphics-rig/gpu-job`. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal).

#[path = "support/triangle.rs"]
mod triangle;

use wcontainers_direct3d_12::Device;
use wcontainers_gpu::{Backend, DeviceStatus};
use wcontainers_gpu_webgpu::WebGpu;

fn webgpu() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a GPU adapter");
    let info = adapter.get_info();
    assert_ne!(
        info.device_type,
        wgpu::DeviceType::Cpu,
        "{} is not a GPU",
        info.name
    );
    eprintln!("adapter: {} ({:?})", info.name, info.backend);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&WebGpu::device_descriptor(&adapter))).unwrap();
    WebGpu::new(&adapter, device, queue)
}

/// The rig's allowance for `name` among the Direct3D 12 references: the largest channel difference
/// and how many pixels may exceed it.
fn tolerance(name: &str) -> (u8, usize) {
    let text = include_str!("../../wcontainers-graphics-test/references/d3d12/tolerances.txt");
    let line = text
        .lines()
        .find(|l| l.split_whitespace().next() == Some(name))
        .expect("a tolerance");
    let mut words = line.split_whitespace().skip(1);
    let mut next = || words.next().unwrap().parse().unwrap();
    (next() as u8, next())
}

fn reference(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/references/d3d12/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "{path}"
    );
    buf.truncate(info.buffer_size());
    buf
}

#[test]
fn the_triangle_scene_matches_direct3d_12() {
    let mut device = Device::new(webgpu());
    let scene = triangle::record(&mut device);
    let fence = device.create_fence(0);
    device.execute(&scene.lists).unwrap();
    device.queue_signal(fence, 1).unwrap();
    assert!(device.wait(fence, 1));
    assert_eq!(device.backend().status(), DeviceStatus::Ok);
    let mapped = device.map(scene.readback).unwrap();
    // The readback heap's bytes, finished into its memory when the fence was reached.
    let bytes = unsafe { std::slice::from_raw_parts(mapped, scene.total as usize) };
    let f = scene.footprint;
    let row = (f.width * 4) as usize;
    let picture: Vec<u8> = (0..f.height as usize)
        .flat_map(|y| &bytes[y * f.row_pitch as usize..][..row])
        .copied()
        .collect();
    let expected = reference("triangle.png");
    let (allowed, beyond) = tolerance("triangle.png");
    let diffs: Vec<u8> = picture
        .iter()
        .zip(&expected)
        .map(|(a, b)| a.abs_diff(*b))
        .collect();
    let largest = diffs.iter().copied().max().unwrap_or(0);
    let pixels = diffs
        .chunks(4)
        .filter(|p| p.iter().any(|d| *d > allowed))
        .count();
    eprintln!("largest channel difference {largest}, pixels beyond {allowed}: {pixels}");
    assert!(
        pixels <= beyond,
        "{pixels} pixels differ by more than {allowed}"
    );
    eprintln!("warnings: {:?}", device.take_warnings());
}
