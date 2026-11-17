//! Writes in their three modes keep the order plan section 4.6 promises: exact answers through
//! copies and read backs, and an ordered write inside a pass splits it without losing what it drew.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, DeviceStatus, MapIntent, ReadSource, SlowPath, WriteMode,
};
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

fn words(v: u32) -> Vec<u8> {
    (0..4u32).flat_map(|i| (v + i).to_le_bytes()).collect()
}

/// A buffer copied out between writes of each mode: every copy sees the bytes written just before it.
#[test]
fn writes_take_effect_in_command_order() {
    let mut b = device();
    let buffer = |b: &mut WebGpu| {
        b.create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK,
            intent: MapIntent::Often,
        })
        .unwrap()
    };
    let src = buffer(&mut b);
    let dst: Vec<_> = (0..3).map(|_| buffer(&mut b)).collect();
    b.write_buffer(&src, 0, &words(100), WriteMode::Ordered);
    b.copy_buffer(&src, 0, &dst[0], 0, 16);
    b.write_buffer(&src, 0, &words(200), WriteMode::Ordered);
    b.copy_buffer(&src, 0, &dst[1], 0, 16);
    b.write_buffer(&src, 0, &words(300), WriteMode::Discard);
    b.copy_buffer(&src, 0, &dst[2], 0, 16);
    let reads: Vec<_> = dst
        .iter()
        .map(|d| {
            b.read_back(&ReadSource::Buffer {
                buffer: d,
                offset: 0,
                size: 16,
            })
            .unwrap()
        })
        .collect();
    b.submit();
    for (rb, want) in reads.into_iter().zip([100, 200, 300]) {
        b.wait_read(&rb);
        let mut got = vec![0; 16];
        b.finish_read(rb, &mut got);
        assert_eq!(got, words(want));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    let c = b.counters();
    assert_eq!((c.staged_writes, c.renames), (1, 1), "{c:?}");
}

/// An ordered write of a vertex buffer the open pass already drew with ends the pass, copies, and
/// opens it again loading what was drawn: the picture is still Direct3D's triangle.
#[test]
fn ordered_write_in_a_pass_splits_it() {
    let mut b = device();
    let p = draw::prepare(&mut b, scene::Scene::Triangle, draw::Twist::None).unwrap();
    let t = draw::targets(&mut b, false).unwrap();
    draw::begin(&mut b, &t, true);
    draw::draw(&mut b, &p);
    let vertices: Vec<u8> = bytemuck::cast_slice(&scene::TRIANGLE).to_vec();
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Ordered);
    draw::draw(&mut b, &p);
    b.end_pass();
    let rb = draw::read(&mut b, &t).unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
    b.finish_read(rb, &mut rgba);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(b.slow_paths().count(SlowPath::WriteSplitPass), 1);
    let reference = std::fs::read(format!(
        "{}/../wcontainers-graphics-test/references/d3d11/triangle.png",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(reference))
        .read_info()
        .unwrap();
    let mut want = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut want).unwrap();
    assert!(rgba == want, "the split pass lost or changed what it drew");
}
