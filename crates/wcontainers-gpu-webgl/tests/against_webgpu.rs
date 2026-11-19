//! The drawings of examples/common/against.rs through this back end on ANGLE, on each route a
//! comparison names (both window-origin routes, with and without EXT_clip_control, for most), and
//! through the WebGPU back end on wgpu's Direct3D 12 on the same GPU: the texels read back must be
//! equal. Windows only; ANGLE's directory as in tests/suite.rs.
#![cfg(windows)]

#[path = "../examples/common/against.rs"]
mod against;
#[path = "../examples/common/kit.rs"]
#[allow(dead_code)]
mod kit;

use against::{COMPARISONS, Reference};
use std::path::PathBuf;
use wcontainers_gpu_webgl::Gl;
use wcontainers_gpu_webgpu::WebGpu;

fn webgl(off: &[&str]) -> Gl {
    let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
        || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
        PathBuf::from,
    );
    let b = Gl::on_angle(&dir, off).expect("ANGLE's OpenGL ES 3.0 context");
    assert!(b.last_log().is_empty(), "{}", b.last_log());
    b
}

/// The WebGPU back end on the device `reference` names.
fn webgpu(reference: Reference) -> WebGpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a Direct3D 12 adapter");
    let mut device_desc = WebGpu::device_descriptor(&adapter);
    if reference == Reference::WithoutFloatFiltering {
        device_desc.required_features.remove(
            wgpu::Features::FLOAT32_FILTERABLE
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        );
    }
    let (device, queue) =
        pollster::block_on(adapter.request_device(&device_desc)).expect("a device");
    WebGpu::new(&adapter, device, queue)
}

/// Runs the comparison `name` on every route; fails naming each route that differs.
fn compare(name: &str) {
    let c = COMPARISONS
        .iter()
        .find(|c| c.name == name)
        .expect("a comparison of that name");
    let verdicts = against::compare(c, &mut webgpu(c.reference), webgl);
    let failed: Vec<String> = verdicts
        .into_iter()
        .filter_map(|(route, v)| v.err().map(|e| format!("WebGL {route}: {e}")))
        .collect();
    assert!(failed.is_empty(), "{name}:\n{}", failed.join("\n"));
}

macro_rules! comparisons {
    ($($name:ident),* $(,)?) => {
        $(#[test]
        fn $name() {
            compare(stringify!($name));
        })*

        /// Every comparison of against.rs has a test here.
        #[test]
        fn every_comparison_is_a_test() {
            let tests = [$(stringify!($name)),*];
            for c in &COMPARISONS {
                assert!(tests.contains(&c.name), "{} has no test", c.name);
            }
        }
    };
}

comparisons!(
    both_window_origins_equal_webgpu,
    a_fractional_viewport_in_the_program_equals_webgpu_native,
    cube_array_and_3d_textures_equal_webgpu,
    cube_gathers_across_face_edges_equal_webgpu,
    generate_mips_of_r32float_without_float_filtering_equals_webgpu,
    generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu,
    generate_mips_of_an_integer_twin_equals_webgpu,
    generate_mips_of_an_integer_twin_at_exact_ties_equals_webgpu,
);
