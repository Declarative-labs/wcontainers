//! Viewports at fractional positions (`FRACTIONAL_VIEWPORT`, plan 4.11) against real Direct3D 11's
//! picture of the rig's eight-viewport scene (crates/wcontainers-graphics-test/src/d3d11/scenes/
//! raster.rs): four of the eight viewports start half a pixel down.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[path = "../examples/scenes/viewports.rs"]
mod viewports;

use wcontainers_gpu::{Backend, DeviceStatus, Features};
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
fn fractional_viewports_match_direct3d() {
    let mut b = device();
    assert!(b.caps().features.contains(Features::FRACTIONAL_VIEWPORT));
    for rounded in [false, true] {
        let (w, h) = viewports::SIZE;
        let rb = viewports::draw(&mut b, rounded);
        b.submit();
        b.wait_read(&rb);
        let mut got = vec![0; (w * h * 4) as usize];
        b.finish_read(rb, &mut got);
        assert_eq!(b.status(), DeviceStatus::Ok);
        let path = format!(
            "{}/../wcontainers-graphics-test/references/d3d11/raster-viewports.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut want = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut want).unwrap();
        want.truncate(info.buffer_size());
        // On the GPU the reference was drawn on, every pixel; the rig allows 1 for WARP.
        let wrong: Vec<_> = (0..(w * h) as usize)
            .filter(|p| got[p * 4..p * 4 + 4] != want[p * 4..p * 4 + 4])
            .map(|p| (p as u32 % w, p as u32 / w))
            .collect();
        if rounded {
            // The same viewports moved to whole pixels must not match: the half pixel is seen.
            assert!(!wrong.is_empty(), "rounded viewports matched too");
        } else {
            assert!(
                wrong.is_empty(),
                "{} pixels differ, first {:?}",
                wrong.len(),
                wrong.first()
            );
        }
    }
}
