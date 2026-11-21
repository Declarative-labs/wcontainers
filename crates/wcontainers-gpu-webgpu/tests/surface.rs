//! Presenting a format the window lacks: a 16-bit float frame on a window without `rgba16float`
//! (wgpu's Vulkan windows on the desktop) is drawn into an 8-bit format the window takes, shown as
//! sRGB values clipped to zero to one, as where the window takes it.
//!
//! Needs a GPU and a desktop (on the rig: `gpu-job --desktop`); Windows only, where the host reads
//! the window from another thread than the main one. `WEBGPU_API` as in tests/scenes.rs.

#![cfg(windows)]

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

mod host;

use wcontainers_gpu::{
    Backend, ColorAttachment, DeviceStatus, Format, FormatUse, LoadOp, PassDesc, SlowPath, StoreOp,
};
use wcontainers_gpu_test::Host;

#[test]
fn float_frames_show_on_windows_without_float_formats() {
    let mut b = host::Native::make();
    assert!(
        b.caps()
            .formats
            .get(Format::Rgba16Float)
            .contains(FormatUse::PRESENT),
        "rgba16float is declared presentable"
    );
    let window = host::Native::window(&mut b);
    let surface = b.create_surface(window).expect("create_surface");
    b.configure_surface(&surface, Format::Rgba16Float, 4, 4, true)
        .expect("configure_surface");
    for (value, texel) in [
        ([0.2f32, 0.4, 0.6, 1.0], [51u8, 102, 153, 255]),
        ([1.5, -0.5, 0.2, 1.0], [255, 0, 51, 255]),
    ] {
        let v = b.current_texture(&surface).expect("current_texture");
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &v,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear(value.map(f32::to_bits)),
            store: StoreOp::Store,
        });
        b.begin_pass(&pass);
        b.end_pass();
        b.present(&surface).expect("present");
        assert_eq!(host::Native::shown(&mut b, window), texel.repeat(16));
    }
    eprintln!(
        "float frames converted at present: {}",
        b.slow_paths().count(SlowPath::PresentConverted)
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}
