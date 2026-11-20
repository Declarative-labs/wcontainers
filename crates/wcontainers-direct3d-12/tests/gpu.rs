//! The rig's Direct3D 12 triangle scene through the device on the WebGPU back end, on a real GPU,
//! compared with real Direct3D 12's picture at the rig's tolerance. Needs a GPU: run it through
//! `tools/graphics-rig/gpu-job`. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal).

#[path = "support/triangle.rs"]
mod triangle;

use wcontainers_direct3d_12::Device;
use wcontainers_direct3d_12::state::{FILL_MODE_SOLID, Raster, TOPOLOGY_TYPE_TRIANGLE};
use wcontainers_gpu::{Backend, DeviceStatus, Features, SlowPath};
use wcontainers_gpu_webgpu::WebGpu;

/// The rig's shaders passing a three-component position and a colour.
const COLOR_VS: &[u8] =
    include_bytes!("../../wcontainers-graphics-test/shaders/dxil/common.vs_color.dxil");
const COLOR_PS: &[u8] =
    include_bytes!("../../wcontainers-graphics-test/shaders/dxil/common.ps_color.dxil");
/// `DXGI_FORMAT_R32G32B32_FLOAT` and `DXGI_FORMAT_R32G32B32A32_FLOAT`.
const FLOAT3: u32 = 6;
const FLOAT4: u32 = 2;

fn webgpu() -> WebGpu {
    webgpu_without(wgpu::Features::empty())
}

/// The back end on a device made without `hidden`, as a browser lacking those features gives.
fn webgpu_without(hidden: wgpu::Features) -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    desc.flags |= WebGpu::INSTANCE_FLAGS;
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
    let mut desc = WebGpu::device_descriptor(&adapter);
    desc.required_features.remove(hidden);
    let (device, queue) = pollster::block_on(adapter.request_device(&desc)).unwrap();
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

/// The picture `scene` draws on `device`.
fn picture(device: &mut Device<WebGpu>, scene: &triangle::Scene) -> Vec<u8> {
    let queue = device.create_queue().unwrap();
    let fence = device.create_fence(0);
    device.execute(queue, &scene.lists).unwrap();
    device.queue_signal(queue, fence, 1).unwrap();
    assert!(device.wait(fence, 1));
    assert_eq!(device.backend().status(), DeviceStatus::Ok);
    let mapped = device.map(scene.readback).unwrap();
    // The readback heap's bytes, finished into its memory when the fence was reached.
    let bytes = unsafe { std::slice::from_raw_parts(mapped, scene.total as usize) };
    let f = scene.footprint;
    let row = (f.width * 4) as usize;
    (0..f.height as usize)
        .flat_map(|y| &bytes[y * f.row_pitch as usize..][..row])
        .copied()
        .collect()
}

#[test]
fn the_triangle_scene_matches_direct3d_12() {
    let mut device = Device::new(webgpu());
    let scene = triangle::record(&mut device);
    let picture = picture(&mut device, &scene);
    let expected = reference("triangle.png");
    let (allowed, beyond) = tolerance("triangle.png");
    assert_eq!(picture.len(), expected.len(), "the picture's size");
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

/// Depth clamped in the vertex program where the back end lacks its own (`DepthClampInShader`,
/// with no depth target) draws what the back end's clamp draws: a triangle beyond the far plane
/// and one before the near plane, which clipping would drop.
#[test]
fn depth_clamped_in_the_programs_draws_as_the_back_ends_clamp() {
    // Position, then colour.
    let triangles: [[f32; 7]; 6] = [
        [-0.9, -0.8, 1.5, 1.0, 0.0, 0.0, 1.0],
        [-0.1, -0.8, 1.5, 1.0, 0.0, 0.0, 1.0],
        [-0.5, 0.8, 1.5, 1.0, 0.0, 0.0, 1.0],
        [0.1, -0.8, -0.5, 0.0, 1.0, 0.0, 1.0],
        [0.9, -0.8, -0.5, 0.0, 1.0, 0.0, 1.0],
        [0.5, 0.8, -0.5, 0.0, 1.0, 0.0, 1.0],
    ];
    let data: Vec<u8> = triangles
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let float3 = size_of::<[f32; 3]>() as u32;
    let stride = size_of::<[f32; 7]>() as u32;
    let pictures = [wgpu::Features::empty(), wgpu::Features::DEPTH_CLIP_CONTROL].map(|hidden| {
        let mut device = Device::new(webgpu_without(hidden));
        assert_eq!(
            device.backend().caps().has(Features::DEPTH_CLAMP),
            hidden.is_empty(),
            "the adapter's own depth clamping"
        );
        let inputs = vec![
            triangle::element("POSITION", FLOAT3, 0),
            triangle::element("COLOR", FLOAT4, float3),
        ];
        let raster = Raster {
            fill: FILL_MODE_SOLID,
            cull: 1,
            depth_clip: false,
            ..Default::default()
        };
        let pso = triangle::pipeline_with(
            &mut device,
            COLOR_VS,
            COLOR_PS,
            inputs,
            raster,
            TOPOLOGY_TYPE_TRIANGLE,
            0,
        )
        .unwrap();
        let scene = triangle::record_with(&mut device, pso, &data, stride);
        let picture = picture(&mut device, &scene);
        let in_programs = device
            .backend()
            .slow_paths()
            .count(SlowPath::DepthClampInShader);
        assert_eq!(in_programs > 0, !hidden.is_empty(), "the slow path's count");
        eprintln!("warnings: {:?}", device.take_warnings());
        picture
    });
    // Both triangles are drawn: their centres are not the clear colour.
    let size = triangle::SIZE as usize;
    let clear = triangle::CLEAR.map(|c| (c * f32::from(u8::MAX)).round() as u8);
    for x in [size / 4, size * 3 / 4] {
        let at = (size / 2 * size + x) * clear.len();
        assert_ne!(
            pictures[0][at..at + clear.len()],
            clear,
            "the triangle at {x}"
        );
    }
    let differing = pictures[0]
        .chunks(clear.len())
        .zip(pictures[1].chunks(clear.len()))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differing, 0, "pixels differing between the two clamps");
}
