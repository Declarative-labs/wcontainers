//! Timestamps: declared where the device writes them in nanoseconds, written in order between
//! passes, twice the work twice the time, and the GPU clock read through base primitives moving at
//! the CPU clock's rate (examples/scenes/timestamps.rs; the same runs in Chrome through web/check,
//! and against real Direct3D 11 in tests/direct3d.rs).
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[path = "../examples/scenes/timestamps.rs"]
mod timestamps;

use std::sync::OnceLock;
use std::time::Instant;

use wcontainers_gpu::{Gpu, caps::Features, error::GpuError, resource::QueryKind};
use wcontainers_gpu_webgpu::WebGpu;

fn backends() -> wgpu::Backends {
    match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    }
}

/// Milliseconds since the first call.
fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

#[test]
fn timestamps_are_ordered_scaled_and_run_at_the_cpu_clocks_rate() {
    let (mut b, adapter) = pollster::block_on(draw::device(backends())).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    let stamps = timestamps::measure(&mut b, now).unwrap();
    eprintln!("{}", stamps.line());
    stamps.check().unwrap();
    assert_eq!(b.timestamps_disjoint(), Ok(false));
}

/// A device without timestamp queries declares no timestamps and refuses them by name.
#[test]
fn timestamps_are_absent_on_a_device_without_timestamp_queries() {
    let (mut b, _) = pollster::block_on(draw::device_without(
        &draw::instance(backends()),
        wgpu::Features::TIMESTAMP_QUERY,
    ))
    .unwrap();
    assert!(!b.caps().features.contains(Features::TIMESTAMPS));
    assert_eq!(
        b.create_query_set(QueryKind::Timestamp, 1).unwrap_err(),
        GpuError::Absent(Features::TIMESTAMPS)
    );
    assert_eq!(
        b.timestamps_disjoint(),
        Err(GpuError::Absent(Features::TIMESTAMPS))
    );
    assert!(b.create_query_set(QueryKind::OcclusionAny, 1).is_ok());
}

/// An instance without wgpu's conversion to nanoseconds, on a GPU whose clock ticks in other
/// units, declares no timestamps and refuses them; where the clock ticks in nanoseconds it is
/// unchanged.
#[test]
fn timestamps_are_absent_where_they_would_not_be_nanoseconds() {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends();
    let instance = wgpu::Instance::new(desc);
    let (mut b, _) =
        pollster::block_on(draw::device_without(&instance, wgpu::Features::empty())).unwrap();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();
    let (_, queue) =
        pollster::block_on(adapter.request_device(&WebGpu::device_descriptor(&adapter))).unwrap();
    let period = queue.get_timestamp_period();
    eprintln!("timestamp period without conversion: {period} ns");
    if period == 1.0 {
        assert!(b.caps().features.contains(Features::TIMESTAMPS));
    } else {
        assert!(!b.caps().features.contains(Features::TIMESTAMPS));
        assert_eq!(
            b.create_query_set(QueryKind::Timestamp, 1).unwrap_err(),
            GpuError::Absent(Features::TIMESTAMPS)
        );
        assert_eq!(
            b.timestamps_disjoint(),
            Err(GpuError::Absent(Features::TIMESTAMPS))
        );
    }
}
