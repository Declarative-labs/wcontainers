//! A call stream recorded through the recording `WebGl` (feature `record`), replayed on a new WebGL 2
//! back end on ANGLE and on the WebGPU back end on wgpu's Direct3D 12, reads back the same texels as
//! the recorded run. Windows only; ANGLE's directory as in tests/suite.rs.
#![cfg(all(windows, feature = "record"))]

#[path = "../examples/common/kit.rs"]
#[allow(dead_code)]
mod kit;

use std::path::PathBuf;
use wcontainers_gpu::*;
use wcontainers_gpu_webgl::trace::Replay;
use wcontainers_gpu_webgl::{Gl, WebGl};
use wcontainers_gpu_webgpu::WebGpu;

/// A gradient in a uniform's colour, so positions, the binding and the write all reach the texels.
const SCENE: &str = "
struct D { colour: vec4<f32> }
@group(0) @binding(0) var<uniform> d: D;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32(v & 1u), f32(v >> 1u)) * 1.5 - 0.75;
    return vec4<f32>(p, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return d.colour * vec4<f32>(p.x / 32.0, p.y / 32.0, 1.0, 1.0);
}
";

fn angle() -> PathBuf {
    std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
        || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
        PathBuf::from,
    )
}

/// Draws the scene into a 32 by 32 texture and reads it back.
fn scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let kit = kit::Kit::new(b);
    let program = kit::program(b, SCENE, "vs", "fs");
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(b, TextureDimension::D2, f, [32, 32, 1], 1, usage, false);
    let target = kit::target(b, &t, f, 0, 0);
    let buffer = b
        .create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    let colour: Vec<u8> = [1.0f32, 0.5, 0.25, 1.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    b.write_buffer(&buffer, 0, &colour, WriteMode::Ordered);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::VERTEX | StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &buffer,
                offset: 0,
                size: 16,
            }),
        }])
        .expect("create_binding_set");
    let viewport = [kit::viewport(0.0, 0.0, 32.0, 32.0)];
    b.begin_pass(&kit::pass(&target, [0; 4], None));
    let call = kit::call(
        &kit,
        &program,
        Some(&set),
        &viewport,
        Topology::TriangleStrip,
        4,
    );
    b.draw(&call);
    b.end_pass();
    kit::read(b, &t, 0, 0, [32, 32, 1], 4, false)
}

/// Replays the whole stream on `b`; every call must succeed. Answers the last read back.
fn replay<B: Gpu>(mut b: B, stream: Vec<u8>) -> Vec<u8> {
    let mut r = Replay::new(stream);
    while let Some(frame) = r.frame(&mut b, None, &mut |_, _| {}) {
        let frame = frame.expect("the stream decodes");
        assert_eq!(frame.failed, 0, "{:?}", frame.first_failure);
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    r.read().to_vec()
}

fn webgpu() -> WebGpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a Direct3D 12 adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&WebGpu::device_descriptor(&adapter)))
            .expect("a device");
    WebGpu::new(&adapter, device, queue)
}

#[test]
fn a_recorded_stream_replays_to_the_same_texels_on_both_back_ends() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("webgl-trace-test.bin");
    let recorded = {
        let gl = Gl::on_angle(&angle(), &[]).expect("ANGLE's OpenGL ES 3.0 context");
        let mut b = WebGl::into_file(gl, &path).expect("the stream file");
        let texels = scene(&mut b);
        assert_eq!(b.status(), DeviceStatus::Ok);
        texels
    };
    assert!(
        recorded.chunks(4).any(|t| t != [0; 4]),
        "the scene drew nothing"
    );
    let stream = std::fs::read(&path).expect("the stream file");
    let gl = Gl::on_angle(&angle(), &[]).expect("ANGLE's OpenGL ES 3.0 context");
    assert_eq!(replay(gl, stream.clone()), recorded, "replayed on WebGL 2");
    assert_eq!(replay(webgpu(), stream), recorded, "replayed on WebGPU");
}
