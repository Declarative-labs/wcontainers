//! The shared back-end suite, and the back end's own checks, on ANGLE's OpenGL ES 3.0 (the desktop's
//! test route). `WEBGL_ANGLE_DIR`
//! names the directory holding the libEGL.dll and libGLESv2.dll angle\build.cmd builds (default
//! C:\wc\tools\angle-mozangle-0.7.1); `WEBGL_OFF` lists WebGL extensions to treat as missing,
//! separated by spaces, to take the routes of browsers without them.
#![cfg(windows)]

use std::path::PathBuf;
use wcontainers_gpu_webgl::WebGl;

struct Angle;

impl wcontainers_gpu_test::Host for Angle {
    type Backend = WebGl;

    fn make() -> WebGl {
        let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
            || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
            PathBuf::from,
        );
        let off = std::env::var("WEBGL_OFF").unwrap_or_default();
        let off: Vec<&str> = off.split_whitespace().collect();
        let mut b = WebGl::on_angle(&dir, &off).expect("ANGLE's OpenGL ES 3.0 context");
        use wcontainers_gpu::Backend;
        assert_eq!(
            b.status(),
            wcontainers_gpu::DeviceStatus::Ok,
            "the back end was lost as it was made: {}",
            b.last_log()
        );
        b
    }

    fn window(_: &mut WebGl) -> u32 {
        0
    }

    /// The EGL surface is off-screen, so the back end reads what it presented.
    fn shown(b: &mut WebGl, _: u32) -> Vec<u8> {
        b.read_presented()
    }
}

wcontainers_gpu_test::suite!(Angle);

#[path = "../examples/common/checks.rs"]
mod checks;
#[path = "../examples/common/kit.rs"]
mod kit;

#[test]
fn vertex_and_instance_numbers_include_the_bases() {
    checks::vertex_and_instance_numbers_include_the_bases::<Angle>();
}

#[test]
fn discard_then_no_overwrite_writes() {
    checks::discard_then_no_overwrite_writes::<Angle>();
}

#[test]
fn depth_and_stencil_read_back() {
    checks::depth_and_stencil_read_back::<Angle>();
}

#[test]
fn stencil_sampled_and_read_back() {
    checks::stencil_sampled_and_read_back::<Angle>();
}

#[test]
fn copies_within_a_typeless_group() {
    checks::copies_within_a_typeless_group::<Angle>();
}
