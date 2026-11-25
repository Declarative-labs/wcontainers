//! Program features: subgroups and the primitive index declared exactly where the adapter has them,
//! drawn as they define where declared, and refused by name on a device made without them; fine
//! derivatives (WGSL's core) declared and drawn on every device.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[allow(dead_code)]
#[path = "../examples/scenes/renames.rs"]
mod renames;

#[allow(dead_code)]
#[path = "../examples/scenes/features.rs"]
mod features;

use features::Feature;
use wcontainers_gpu::{Backend, GpuError, ShaderStage};

fn backends() -> wgpu::Backends {
    match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    }
}

fn declared_drawn_and_refused_without(feature: Feature) {
    let wanted = feature.wgpu().expect("an optional feature");
    let instance = draw::instance(backends());
    let (mut b, name) =
        pollster::block_on(draw::device_without(&instance, wgpu::Features::empty())).unwrap();
    let has = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map(|a| a.features().contains(wanted))
    .unwrap_or(false);
    eprintln!("{name}: adapter has {feature:?}: {has}");
    assert_eq!(b.caps().has(feature.declared()), has);
    let line = features::check(&mut b, feature).unwrap();
    eprintln!("{line}");
    let (mut without, _) = pollster::block_on(draw::device_without(&instance, wanted)).unwrap();
    assert!(!without.caps().has(feature.declared()));
    assert!(matches!(
        features::shader(&mut without, feature, ShaderStage::Fragment),
        Err(GpuError::Invalid(_))
    ));
}

/// Subgroup operations in a fragment program: every invocation active, sums equal to ballots.
#[test]
fn subgroup_operations_run_where_declared_and_are_refused_elsewhere() {
    declared_drawn_and_refused_without(Feature::Subgroups);
}

/// The primitive index in a fragment program: each of four triangles reads its own number.
#[test]
fn the_primitive_index_reads_each_triangles_number_where_declared() {
    declared_drawn_and_refused_without(Feature::PrimitiveIndex);
}

/// Fine derivatives in a fragment program: each pixel differenced within its own row and column of
/// its quad.
#[test]
fn fine_derivatives_are_declared_and_difference_each_pixel_on_its_own() {
    let instance = draw::instance(backends());
    let (mut b, name) =
        pollster::block_on(draw::device_without(&instance, wgpu::Features::empty())).unwrap();
    assert!(b.caps().has(Feature::FineDerivatives.declared()), "{name}");
    let line = features::check(&mut b, Feature::FineDerivatives).unwrap();
    eprintln!("{name}: {line}");
}
