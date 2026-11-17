//! `prepare` builds a combination's pipeline before its first draw, so that draw builds nothing:
//! counted, and the first draws timed with and without it (printed; `--nocapture` shows them).
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs. `WEBGPU_PROGRAMS` sets how many programs each
//! half draws (default 16).

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use std::time::{Duration, Instant};

use wcontainers_gpu::{
    Backend, ColorAttachment, DeviceStatus, DrawCall, DrawRange, Format, LoadOp, MAX_COLOR_TARGETS,
    PassDesc, PrepareDesc, SlowPath, StoreOp, TargetLayout, TextureUsage, Topology, Viewport,
};
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

/// A program with some work in its fragment stage, distinct per `n` so no compiled program is
/// shared.
fn source(n: u32) -> String {
    format!(
        r#"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {{
    var c = vec3f(p.xy * {n}.0 / 64.0, 0.25);
    for (var i = 0; i < 24; i++) {{
        c = fract(sin(c * 12.9898 + vec3f(f32(i), {n}.0, 7.0)) * 43758.547);
        c = c * mat3x3f(0.8, 0.1, 0.1, 0.2, 0.7, 0.1, 0.3, 0.3, 0.4) + exp2(-c);
    }}
    return vec4f(normalize(c + 0.001), 1.0);
}}
"#
    )
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[test]
fn prepared_combinations_build_nothing_at_their_first_draw() {
    let mut b = device();
    let n: u32 = std::env::var("WEBGPU_PROGRAMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    let (_target, view) =
        draw::texture_2d(&mut b, Format::Rgba8Unorm, 64, 64, TextureUsage::RENDER).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let vertex_layout = b.create_vertex_layout(&draw::layout(&[], &[])).unwrap();
    let mut targets = TargetLayout {
        colors: [None; MAX_COLOR_TARGETS],
        depth_stencil: None,
        samples: 1,
    };
    targets.colors[0] = Some(Format::Rgba8Unorm);
    let first_draws = |b: &mut WebGpu, base: u32, prepare: bool| {
        let programs: Vec<_> = (base..base + n)
            .map(|i| draw::program(b, &source(i)).unwrap())
            .collect();
        let started = Instant::now();
        if prepare {
            for program in &programs {
                b.prepare(&PrepareDesc {
                    program,
                    blend: &blend,
                    depth_stencil: &depth_stencil,
                    raster: &raster,
                    vertex_layout: &vertex_layout,
                    targets,
                    topology: Topology::TriangleList,
                    layouts: Default::default(),
                });
            }
        }
        let preparing = started.elapsed();
        let (misses, built) = (
            b.slow_paths().count(SlowPath::PipelineMiss),
            b.counters().pipelines,
        );
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &view,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear([0; 4]),
            store: StoreOp::Store,
        });
        b.begin_pass(&pass);
        let times: Vec<Duration> = programs
            .iter()
            .map(|program| {
                let t = Instant::now();
                b.draw(&DrawCall {
                    program,
                    blend: &blend,
                    depth_stencil: &depth_stencil,
                    raster: &raster,
                    vertex_layout: &vertex_layout,
                    bindings: [None; 4],
                    vertex_buffers: &[],
                    index: None,
                    topology: Topology::TriangleList,
                    restart: false,
                    viewports: &[Viewport {
                        x: 0,
                        y: 0,
                        width: 64.0f32.to_bits(),
                        height: 64.0f32.to_bits(),
                        min_depth: 0,
                        max_depth: 1.0f32.to_bits(),
                    }],
                    scissors: &[],
                    blend_constant: [0; 4],
                    stencil_reference: [0; 2],
                    immediates: &[],
                    range: DrawRange::Vertices {
                        first: 0,
                        count: 3,
                        first_instance: 0,
                        instances: 1,
                    },
                });
                t.elapsed()
            })
            .collect();
        b.end_pass();
        let fence = b.submit();
        b.wait_fence(&fence);
        assert_eq!(b.status(), DeviceStatus::Ok);
        let misses = b.slow_paths().count(SlowPath::PipelineMiss) - misses;
        let built = b.counters().pipelines - built;
        eprintln!(
            "{n} first draws {}: prepare {preparing:?} in all; draw median {:?}, longest {:?}, sum {:?}; {misses} misses, {built} pipelines built while drawing",
            if prepare {
                "after prepare"
            } else {
                "unprepared"
            },
            median(times.clone()),
            times.iter().max().unwrap(),
            times.iter().sum::<Duration>(),
        );
        (misses, built)
    };
    assert_eq!(first_draws(&mut b, 0, false), (u64::from(n), u64::from(n)));
    assert_eq!(first_draws(&mut b, 1000, true), (0, 0));
}
