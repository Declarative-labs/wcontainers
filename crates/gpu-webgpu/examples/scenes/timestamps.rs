//! Timestamps around two stretches of GPU work, the second twice the first, and the GPU clock read
//! twice through base primitives (write, resolve, read back) a known CPU time apart, as a front
//! calibrates it against the CPU clock: shared by tests/timestamps.rs, tests/direct3d.rs (against
//! real Direct3D 11's timestamps around the same work) and the browser page.

use wcontainers_gpu::{
    BufferUsage, ColorAttachment, DeviceStatus, DrawCall, DrawRange, Features, Format, Gpu, LoadOp,
    PassDesc, QueryKind, ReadSource, StoreOp, TextureUsage, Topology, Viewport,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::draw;

/// The side of the square target the work draws into.
pub const SIZE: u32 = 512;
/// Rounds of the fragment loop.
pub const ROUNDS: u32 = 2048;
/// Full-target draws in the first stretch; the second has twice as many.
pub const DRAWS: u32 = 16;
/// How long the CPU waits between the two clock readings, in milliseconds.
pub const APART_MS: f64 = 300.0;

/// The work: a fragment loop the compiler cannot fold, the same as tests/direct3d.rs's HLSL.
pub fn source() -> String {
    format!(
        r#"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {{
    var c = p.xy / {SIZE}.0;
    for (var i = 0; i < {ROUNDS}; i++) {{
        c = fract(c * 1.7 + c.yx * 0.3 + 0.1);
    }}
    return vec4f(c, 0.0, 1.0);
}}
"#
    )
}

/// What was measured.
#[derive(Debug)]
pub struct Stamps {
    /// Before the first stretch, between the stretches, after the second, and at once after that.
    pub ns: [u64; 4],
    /// The GPU clock's advance between the two readings, in nanoseconds.
    pub gpu_apart: i128,
    /// The least and most CPU time that can lie between the two readings, in nanoseconds.
    pub cpu_apart: (f64, f64),
}

impl Stamps {
    /// The two stretches' GPU times, in nanoseconds.
    pub fn stretches(&self) -> (u64, u64) {
        (
            self.ns[1].saturating_sub(self.ns[0]),
            self.ns[2].saturating_sub(self.ns[1]),
        )
    }

    /// The largest power of two, in nanoseconds, every timestamp is a multiple of: the clock's
    /// resolution where a browser rounds timestamps.
    pub fn step(&self) -> u64 {
        let bits = self.ns.iter().fold(0, |a, n| a | n);
        1 << bits.trailing_zeros().min(63)
    }

    /// One line of what was measured.
    pub fn line(&self) -> String {
        let (a, b) = self.stretches();
        format!(
            "timestamps {:?} (each a multiple of {} ns); stretches {:.3} ms and {:.3} ms (ratio {:.2}); GPU clock {:.3} ms apart, CPU between {:.3} and {:.3} ms",
            self.ns,
            self.step(),
            a as f64 / 1e6,
            b as f64 / 1e6,
            b as f64 / a.max(1) as f64,
            self.gpu_apart as f64 / 1e6,
            self.cpu_apart.0 / 1e6,
            self.cpu_apart.1 / 1e6,
        )
    }

    /// Order, scale against the work, and rate against the CPU clock.
    pub fn check(&self) -> Result<(), String> {
        let n = self.ns;
        if !(n[0] < n[1] && n[1] < n[2] && n[2] <= n[3]) {
            return Err(format!("timestamps out of order: {n:?}"));
        }
        let (a, b) = self.stretches();
        let ratio = b as f64 / a as f64;
        if !(1.5..=2.7).contains(&ratio) {
            return Err(format!(
                "twice the work took {ratio:.2} times as long ({a} ns, then {b} ns)"
            ));
        }
        // The clocks run at one rate within 1 %, and quantised readings by up to 0.2 ms.
        let (least, most) = self.cpu_apart;
        let slack = most * 0.01 + 200_000.0;
        let gpu = self.gpu_apart as f64;
        if gpu < least - slack || gpu > most + slack {
            return Err(format!(
                "the GPU clock moved {:.3} ms while the CPU's moved {:.3} to {:.3} ms",
                gpu / 1e6,
                least / 1e6,
                most / 1e6
            ));
        }
        Ok(())
    }
}

/// Measures on `b`; `now` is the CPU clock in milliseconds.
pub fn measure(b: &mut WebGpu, now: fn() -> f64) -> Result<Stamps, String> {
    if !b.caps().features.contains(Features::TIMESTAMPS) {
        return Err("TIMESTAMPS is not declared".into());
    }
    let (_target, view) = draw::texture_2d(b, Format::Rgba8Unorm, SIZE, SIZE, TextureUsage::RENDER)
        .map_err(|e| format!("{e:?}"))?;
    let program = draw::program(b, &source()).map_err(|e| format!("{e:?}"))?;
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let vertex_layout = b.create_vertex_layout(&draw::layout(&[], &[])).unwrap();
    let set = b
        .create_query_set(QueryKind::Timestamp, 6)
        .map_err(|e| format!("{e:?}"))?;
    let results = draw::buffer(b, &[0; 48], BufferUsage::COPY_SRC | BufferUsage::READ_BACK)
        .map_err(|e| format!("{e:?}"))?;
    let stretch = |b: &mut WebGpu, draws: u32| {
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
        for _ in 0..draws {
            b.draw(&DrawCall {
                program: &program,
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
                    width: (SIZE as f32).to_bits(),
                    height: (SIZE as f32).to_bits(),
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
        }
        b.end_pass();
    };
    let read = |b: &mut WebGpu, first: u32, count: u32| -> Result<Vec<u64>, String> {
        b.resolve_queries(&set, first, count, &results, u64::from(first) * 8)
            .map_err(|e| format!("{e:?}"))?;
        let rb = b
            .read_back(&ReadSource::Buffer {
                buffer: &results,
                offset: u64::from(first) * 8,
                size: u64::from(count) * 8,
            })
            .map_err(|e| format!("{e:?}"))?;
        b.submit();
        b.wait_read(&rb);
        let mut bytes = vec![0; count as usize * 8];
        b.finish_read(rb, &mut bytes);
        Ok(bytes
            .chunks(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect())
    };
    // The pipeline is built ahead, outside the measured stretches.
    stretch(b, 1);
    let fence = b.submit();
    b.wait_fence(&fence);
    for (index, draws) in [(0, DRAWS), (1, DRAWS * 2), (2, 0), (3, 0)] {
        b.write_timestamp(&set, index)
            .map_err(|e| format!("{e:?}"))?;
        if draws > 0 {
            stretch(b, draws);
        }
    }
    let ns: [u64; 4] = read(b, 0, 4)?.try_into().unwrap();
    // The GPU clock at a point the CPU clock brackets: the submit and the answer.
    let reading = |b: &mut WebGpu, index: u32| -> Result<(u64, f64, f64), String> {
        b.write_timestamp(&set, index)
            .map_err(|e| format!("{e:?}"))?;
        b.resolve_queries(&set, index, 1, &results, u64::from(index) * 8)
            .map_err(|e| format!("{e:?}"))?;
        let rb = b
            .read_back(&ReadSource::Buffer {
                buffer: &results,
                offset: u64::from(index) * 8,
                size: 8,
            })
            .map_err(|e| format!("{e:?}"))?;
        let before = now();
        b.submit();
        b.wait_read(&rb);
        let after = now();
        let mut bytes = [0; 8];
        b.finish_read(rb, &mut bytes);
        Ok((u64::from_le_bytes(bytes), before, after))
    };
    let (g0, before0, after0) = reading(b, 4)?;
    let start = now();
    while now() - start < APART_MS {}
    let (g1, before1, after1) = reading(b, 5)?;
    if b.status() != DeviceStatus::Ok {
        return Err(format!("device lost: {:?}", b.status()));
    }
    Ok(Stamps {
        ns,
        gpu_apart: i128::from(g1) - i128::from(g0),
        cpu_apart: ((before1 - after0) * 1e6, (after1 - before0) * 1e6),
    })
}
