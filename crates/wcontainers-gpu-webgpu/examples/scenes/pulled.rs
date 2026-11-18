//! What instance data fetched in the vertex program costs (the fronts' route for step rates other
//! than one, `InstanceRateInShader`) against vertex buffers, on a scene of many objects each with its
//! own buffers, at a chosen number of streams a draw: the routes draw the same picture; CPU time to
//! record and submit a frame and GPU time (timestamps around the pass) are measured, first frame and
//! the median of the rest. Every buffer of this back end already has storage usage
//! (`UnalignedBufferEdge`), so the storage usage the fronts ask for adds nothing here; what differs
//! is the fetch and a binding set per object. Sets made per draw must find the bind groups their
//! equals of the frame before made. Shared by tests/pulled.rs and the browser page.

use std::fmt::Write;

use anyhow::{Result, anyhow, ensure};
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferRange, BufferUsage, ColorAttachment,
    DeviceStatus, DrawCall, DrawRange, Extent, Format, LoadOp, Object, Origin, PassDesc, QueryKind,
    ReadSource, StageSet, StepMode, StoreOp, TextureRegion, TextureUsage, Topology, VertexBinding,
    VertexFormat, Viewport,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::draw;

type B = WebGpu;
type Buffer = <B as Backend>::Buffer;

/// The side of the square target.
pub const SIZE: u32 = 256;
/// Objects, one draw each.
pub const OBJECTS: u32 = 256;
/// Instances a draw.
pub const INSTANCES: u32 = 16;
/// Streams a draw at most, one attribute each: the vertex attributes WebGPU guarantees.
pub const MOST_STREAMS: u32 = 16;
/// Streams a draw at least: the quad, where, how large and what colour.
pub const LEAST_STREAMS: u32 = 4;

/// How a draw gets its instance data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    /// Vertex buffers stepped per instance.
    Buffers,
    /// Fetched by the program from storage, with a binding set made for each draw and dropped
    /// after, as a front that does not keep sets does.
    PulledSetPerDraw,
    /// Fetched by the program, each object's binding set made once and kept.
    PulledSetsKept,
}

impl Route {
    pub const ALL: [Route; 3] = [
        Route::Buffers,
        Route::PulledSetPerDraw,
        Route::PulledSetsKept,
    ];

    fn name(self) -> &'static str {
        match self {
            Route::Buffers => "vertex buffers",
            Route::PulledSetPerDraw => "pulled, a set per draw",
            Route::PulledSetsKept => "pulled, sets kept",
        }
    }
}

pub struct Timing {
    pub route: Route,
    pub streams: u32,
    pub cpu_ms: Vec<f64>,
    pub gpu_ms: Vec<f64>,
    pub bind_groups: u64,
    picture: Vec<u8>,
}

fn median(v: &[f64]) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

impl Timing {
    /// One line of what was measured.
    pub fn line(&self) -> String {
        format!(
            "{}: {} streams, {OBJECTS} draws of {INSTANCES} instances; first frame CPU {:.3} ms GPU {:.3} ms; median of {} later frames CPU {:.3} ms GPU {:.3} ms; {} bind groups made",
            self.route.name(),
            self.streams,
            self.cpu_ms[0],
            self.gpu_ms[0],
            self.cpu_ms.len() - 1,
            median(&self.cpu_ms[1..]),
            median(&self.gpu_ms[1..]),
            self.bind_groups
        )
    }
}

/// The most streams a draw of this scene may have on the device, on every route.
pub fn most_streams(b: &B) -> u32 {
    let l = b.caps().limits;
    MOST_STREAMS
        .min(l.vertex_streams)
        .min(l.vertex_attributes)
        .min(l.storage_buffers_per_stage + 1)
}

/// The program of a route with `streams` streams: stream 0 the quad, 1 where, 2 how large, the
/// rest colour parts summed (each part dyadic, so any order of the sum is exact).
fn source(pulled: bool, streams: u32) -> String {
    let mut s = String::from("struct V { @builtin(position) p: vec4f, @location(0) c: vec4f }\n");
    let parts: Vec<String> = (3..streams).map(|i| format!("a{i}")).collect();
    let body = format!(
        "    return V(vec4f(pos * a2.xy + a1.xy, 0.0, 1.0), {});\n}}\n",
        parts.join(" + ")
    );
    if pulled {
        // As a front's pulled variant: byte offset, stride and divisor per stream from helper
        // words, the element assembled from words.
        s += "@group(0) @binding(0) var<uniform> pull: array<vec4<u32>, 16>;\n";
        for i in 1..streams {
            let _ = writeln!(
                s,
                "@group(0) @binding({}) var<storage, read> s{i}: array<u32>;\n\
                 fn e{i}(instance: u32) -> vec4f {{\n\
                 \x20   let p = pull[{i}];\n\
                 \x20   let w = (p.x + (instance / p.z) * p.y) / 4u;\n\
                 \x20   return vec4f(bitcast<f32>(s{i}[w]), bitcast<f32>(s{i}[w + 1u]), \
                 bitcast<f32>(s{i}[w + 2u]), bitcast<f32>(s{i}[w + 3u]));\n}}",
                159 + i
            );
        }
        s += "@vertex fn vs(@location(0) pos: vec2f, @builtin(instance_index) i: u32) -> V {\n";
        for i in 1..streams {
            let _ = writeln!(s, "    let a{i} = e{i}(i);");
        }
    } else {
        s += "@vertex fn vs(@location(0) pos: vec2f";
        for i in 1..streams {
            let _ = write!(s, ", @location({i}) a{i}: vec4f");
        }
        s += ") -> V {\n";
    }
    s += &body;
    s += "@fragment fn ps(v: V) -> @location(0) vec4f { return v.c; }\n";
    s
}

fn floats(v: &[[f32; 4]]) -> Vec<u8> {
    v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect()
}

/// One object's buffers: its quad, then where, how large and the colour parts of each instance.
fn object(b: &mut B, k: u32, streams: u32) -> Result<Vec<Buffer>> {
    let quad: Vec<u8> = [
        0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0,
    ]
    .iter()
    .flat_map(|f| f.to_le_bytes())
    .collect();
    let cell = 2.0 / 16.0;
    let (x, y) = ((k % 16) as f32 * cell - 1.0, (k / 16) as f32 * cell - 1.0);
    let at: Vec<[f32; 4]> = (0..INSTANCES)
        .map(|j| {
            let sub = cell / 4.0;
            [x + (j % 4) as f32 * sub, y + (j / 4) as f32 * sub, 0.0, 0.0]
        })
        .collect();
    let scale = vec![[cell / 5.0, cell / 5.0, 0.0, 0.0]; INSTANCES as usize];
    let usage = BufferUsage::VERTEX | BufferUsage::STORAGE;
    let mut buffers = vec![
        draw::buffer(b, &quad, BufferUsage::VERTEX)?,
        draw::buffer(b, &floats(&at), usage)?,
        draw::buffer(b, &floats(&scale), usage)?,
    ];
    for part in 3..streams {
        let colour: Vec<[f32; 4]> = (0..INSTANCES)
            .map(|j| {
                if part == 3 {
                    [k as f32 / 512.0, j as f32 / 32.0, 0.5, 1.0]
                } else {
                    [1.0 / 1024.0, 1.0 / 1024.0, 0.0, 0.0]
                }
            })
            .collect();
        buffers.push(draw::buffer(b, &floats(&colour), usage)?);
    }
    Ok(buffers)
}

/// The binding set of a pulled draw of object `o`.
fn pulled_set(b: &mut B, pull: &Buffer, o: &[Buffer]) -> Result<<B as Backend>::BindingSet> {
    let mut entries = vec![BindingEntry {
        binding: 0,
        stages: StageSet::VERTEX,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer: pull,
            offset: 0,
            size: 256,
        }),
    }];
    for (i, buffer) in o.iter().enumerate().skip(1) {
        entries.push(BindingEntry {
            binding: 159 + i as u32,
            stages: StageSet::VERTEX,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer,
                    offset: 0,
                    size: u64::from(INSTANCES) * 16,
                },
                writable: false,
            },
        });
    }
    b.create_binding_set(&entries)
        .map_err(|e| anyhow!("binding set: {e:?}"))
}

fn run(b: &mut B, route: Route, streams: u32, frames: u32, now: fn() -> f64) -> Result<Timing> {
    let pulled = route != Route::Buffers;
    let objects = (0..OBJECTS)
        .map(|k| object(b, k, streams))
        .collect::<Result<Vec<_>>>()?;
    let (target, view) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        SIZE,
        SIZE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )?;
    let program = draw::program(b, &source(pulled, streams))?;
    let gpu = |e| anyhow!("{e:?}");
    let blend = b.create_blend(&draw::opaque()).map_err(gpu)?;
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).map_err(gpu)?;
    let raster = b.create_raster(&draw::solid()).map_err(gpu)?;
    let mut layout_streams = vec![(8, StepMode::Vertex)];
    let mut attributes = vec![(0, 0, VertexFormat::Float32x2)];
    if !pulled {
        for i in 1..streams {
            layout_streams.push((16, StepMode::Instance(1)));
            attributes.push((i as u8, 0, VertexFormat::Float32x4));
        }
    }
    let vertex_layout = b
        .create_vertex_layout(&draw::layout(&layout_streams, &attributes))
        .map_err(gpu)?;
    // Offset 0, stride 16, divisor 1 for every stream: the pulled variant's helper words.
    let words: Vec<u8> = (0..16)
        .flat_map(|_| [0u32, 16, 1, 0])
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let pull = draw::buffer(b, &words, BufferUsage::UNIFORM)?;
    let kept = if route == Route::PulledSetsKept {
        objects
            .iter()
            .map(|o| pulled_set(b, &pull, o))
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let stamps = b.create_query_set(QueryKind::Timestamp, 2).map_err(gpu)?;
    let results = draw::buffer(b, &[0; 16], BufferUsage::COPY_SRC | BufferUsage::READ_BACK)?;
    let groups_before = b.counters().bind_groups;
    let mut timing = Timing {
        route,
        streams,
        cpu_ms: Vec::new(),
        gpu_ms: Vec::new(),
        bind_groups: 0,
        picture: Vec::new(),
    };
    for _ in 0..frames.max(2) {
        let started = now();
        b.write_timestamp(&stamps, 0).map_err(gpu)?;
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
        for (k, o) in objects.iter().enumerate() {
            let made = if route == Route::PulledSetPerDraw {
                Some(pulled_set(b, &pull, o)?)
            } else {
                None
            };
            let set = made.as_ref().or(kept.get(k));
            let vertex_buffers: Vec<_> = o
                .iter()
                .take(if pulled { 1 } else { o.len() })
                .map(|buffer| Some(VertexBinding { buffer, offset: 0 }))
                .collect();
            b.draw(&DrawCall {
                program: &program,
                blend: &blend,
                depth_stencil: &depth_stencil,
                raster: &raster,
                vertex_layout: &vertex_layout,
                bindings: [set, None, None, None],
                vertex_buffers: &vertex_buffers,
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
                    count: 6,
                    first_instance: 0,
                    instances: INSTANCES,
                },
            });
        }
        b.end_pass();
        b.write_timestamp(&stamps, 1).map_err(gpu)?;
        b.resolve_queries(&stamps, 0, 2, &results, 0).map_err(gpu)?;
        let rb = b
            .read_back(&ReadSource::Buffer {
                buffer: &results,
                offset: 0,
                size: 16,
            })
            .map_err(gpu)?;
        b.submit();
        timing.cpu_ms.push(now() - started);
        b.wait_read(&rb);
        let mut bytes = [0; 16];
        b.finish_read(rb, &mut bytes);
        let t = |i: usize| u64::from_le_bytes(bytes[i * 8..][..8].try_into().unwrap());
        timing.gpu_ms.push(t(1).saturating_sub(t(0)) as f64 / 1e6);
    }
    timing.bind_groups = b.counters().bind_groups - groups_before;
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture: &target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: SIZE,
                height: SIZE,
                depth_or_layers: 1,
            },
        })
        .map_err(gpu)?;
    b.submit();
    b.wait_read(&rb);
    timing.picture = vec![0; (SIZE * SIZE * 4) as usize];
    b.finish_read(rb, &mut timing.picture);
    ensure!(
        b.status() == DeviceStatus::Ok,
        "device lost: {:?}",
        b.status()
    );
    for set in kept {
        b.destroy(Object::BindingSet(set));
    }
    for buffer in objects.into_iter().flatten().chain([pull, results]) {
        b.destroy(Object::Buffer(buffer));
    }
    b.destroy(Object::Program(program));
    b.destroy(Object::TextureView(view));
    b.destroy(Object::Texture(target));
    Ok(timing)
}

/// Every route at `streams` streams a draw, `frames` frames each; fails unless every route draws
/// the same picture and something is drawn.
pub fn measure(b: &mut B, streams: u32, frames: u32, now: fn() -> f64) -> Result<Vec<Timing>> {
    ensure!(
        (LEAST_STREAMS..=most_streams(b)).contains(&streams),
        "{streams} streams: this device draws {LEAST_STREAMS} to {}",
        most_streams(b)
    );
    let timings = Route::ALL
        .into_iter()
        .map(|route| run(b, route, streams, frames, now))
        .collect::<Result<Vec<_>>>()?;
    let lit = timings[0].picture.chunks(4).filter(|p| p[3] != 0).count();
    ensure!(lit > 0, "nothing drawn");
    // Sets made again each frame find the bind groups of the frame before.
    ensure!(
        timings[1].bind_groups == timings[2].bind_groups,
        "sets made per draw made {} bind groups in {frames} frames, kept sets {}",
        timings[1].bind_groups,
        timings[2].bind_groups
    );
    for t in &timings[1..] {
        ensure!(
            t.picture == timings[0].picture,
            "{} drew another picture than vertex buffers at {streams} streams",
            t.route.name()
        );
    }
    Ok(timings)
}
