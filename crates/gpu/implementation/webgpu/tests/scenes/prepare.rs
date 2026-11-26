//! First draws of new programs, with and without `prepare` ahead of them, timed by a clock the
//! caller gives (natively `Instant`, in the browser `performance.now()`): shared by tests/prepare.rs
//! and the browser page.

use wcontainers_gpu::{
    Gpu, MAX_COLOR_TARGETS, PrepareDesc, draw::ColorAttachment, draw::DrawCall, draw::DrawRange,
    draw::LoadOp, draw::PassDesc, draw::StoreOp, draw::Viewport, error::DeviceStatus,
    format::Format, resource::TextureUsage, slow::SlowPath, state::TargetLayout, state::Topology,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::draw;

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

/// What one half measured, in milliseconds.
#[derive(Debug)]
pub struct FirstDraws {
    /// The `prepare` calls themselves.
    pub prepare_calls: f64,
    /// The `prepare` calls and a submit waited for, so every compile they started has ended.
    pub prepared: f64,
    /// Each first draw's call.
    pub draws: Vec<f64>,
    /// The draws, the submit and the wait for the GPU to finish them: the frame.
    pub frame: f64,
    /// `PipelineMiss`es and pipelines built while drawing.
    pub misses: u64,
    pub built: u64,
}

impl FirstDraws {
    /// One line: the half, its prepare, the draws' median and longest call, the frame, the counts.
    pub fn line(&self, prepared: bool) -> String {
        let mut d = self.draws.clone();
        d.sort_by(f64::total_cmp);
        format!(
            "{} first draws {}: prepare calls {:.3} ms, prepared and waited {:.3} ms; draw median {:.4} ms, longest {:.4} ms; frame {:.3} ms; {} misses, {} pipelines built while drawing",
            d.len(),
            if prepared {
                "after prepare"
            } else {
                "unprepared"
            },
            self.prepare_calls,
            self.prepared,
            d[d.len() / 2],
            d[d.len() - 1],
            self.frame,
            self.misses,
            self.built
        )
    }
}

/// Draws `n` new programs (numbered from `base`) once each into a 64 by 64 target, after preparing
/// their combinations where `prepare`, timing each part with `now` (milliseconds).
pub fn first_draws(
    b: &mut WebGpu,
    base: u32,
    n: u32,
    prepare: bool,
    now: fn() -> f64,
) -> FirstDraws {
    let (_target, view) =
        draw::texture_2d(b, Format::Rgba8Unorm, 64, 64, TextureUsage::RENDER).unwrap();
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
    let programs: Vec<_> = (base..base + n)
        .map(|i| draw::program(b, &source(i)).unwrap())
        .collect();
    let started = now();
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
    let prepare_calls = now() - started;
    let fence = b.submit();
    b.wait_fence(&fence);
    let prepared = now() - started;
    let (misses, built) = (
        b.slow_paths().counts[SlowPath::PipelineMiss as usize],
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
    let frame_started = now();
    b.begin_pass(&pass);
    let draws = programs
        .iter()
        .map(|program| {
            let t = now();
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
            now() - t
        })
        .collect();
    b.end_pass();
    let fence = b.submit();
    b.wait_fence(&fence);
    let frame = now() - frame_started;
    assert_eq!(b.status(), DeviceStatus::Ok);
    FirstDraws {
        prepare_calls,
        prepared,
        draws,
        frame,
        misses: b.slow_paths().counts[SlowPath::PipelineMiss as usize] - misses,
        built: b.counters().pipelines - built,
    }
}

/// A strip combination prepared, then drawn without indices and with 16-bit and 32-bit indices:
/// nothing is built while drawing. One line, or the failed check.
pub fn prepared_strips(b: &mut WebGpu) -> Result<String, String> {
    use wcontainers_gpu::{draw::IndexBinding, resource::BufferUsage, state::IndexFormat};
    let (_target, view) =
        draw::texture_2d(b, Format::Rgba8Unorm, 64, 64, TextureUsage::RENDER).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let vertex_layout = b.create_vertex_layout(&draw::layout(&[], &[])).unwrap();
    let program = draw::program(b, &source(5000)).unwrap();
    let mut targets = TargetLayout {
        colors: [None; MAX_COLOR_TARGETS],
        depth_stencil: None,
        samples: 1,
    };
    targets.colors[0] = Some(Format::Rgba8Unorm);
    let short: Vec<u8> = [0u16, 1, 2, 0xffff, 1, 2, 0]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let long: Vec<u8> = [0u32, 1, 2, 0xffff_ffff, 1, 2, 0]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let short = draw::buffer(b, &short, BufferUsage::INDEX).unwrap();
    let long = draw::buffer(b, &long, BufferUsage::INDEX).unwrap();
    b.prepare(&PrepareDesc {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        targets,
        topology: Topology::TriangleStrip,
        layouts: Default::default(),
    });
    let fence = b.submit();
    b.wait_fence(&fence);
    let (misses, built) = (
        b.slow_paths().counts[SlowPath::PipelineMiss as usize],
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
    for (index, format) in [
        (None, IndexFormat::U16),
        (Some(&short), IndexFormat::U16),
        (Some(&long), IndexFormat::U32),
    ] {
        b.draw(&DrawCall {
            program: &program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: &raster,
            vertex_layout: &vertex_layout,
            bindings: [None; 4],
            vertex_buffers: &[],
            index: index.map(|buffer| IndexBinding {
                buffer,
                offset: 0,
                format,
            }),
            topology: Topology::TriangleStrip,
            restart: true,
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
            range: match index {
                None => DrawRange::Vertices {
                    first: 0,
                    count: 3,
                    first_instance: 0,
                    instances: 1,
                },
                Some(_) => DrawRange::Indexed {
                    first: 0,
                    count: 7,
                    base_vertex: 0,
                    first_instance: 0,
                    instances: 1,
                },
            },
        });
    }
    b.end_pass();
    let fence = b.submit();
    b.wait_fence(&fence);
    let (misses, built) = (
        b.slow_paths().counts[SlowPath::PipelineMiss as usize] - misses,
        b.counters().pipelines - built,
    );
    if b.status() != DeviceStatus::Ok {
        return Err(format!("prepared strips: device {:?}", b.status()));
    }
    let line = format!(
        "prepared strips drawn without indices and with 16 and 32-bit indices: {misses} misses, {built} pipelines built while drawing"
    );
    match (misses, built) {
        (0, 0) => Ok(line),
        _ => Err(line),
    }
}
