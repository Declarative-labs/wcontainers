//! What instance data fetched in the vertex program costs (the fronts' route for step rates other
//! than one, `InstanceRateInShader`) against vertex buffers, on a scene of many objects each with its
//! own buffers: both routes draw the same picture; CPU time to record and submit a frame and GPU time
//! (timestamps around the pass) are printed, first frame and the median of the rest. Every buffer of
//! this back end already has storage usage (`UnalignedBufferEdge`), so the storage usage the fronts
//! ask for adds nothing here; what differs is the fetch and a binding set per object.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs; `WEBGPU_FRAMES` frames per route (default 20).
//! Timings that matter run alone: `gpu-job --alone`.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use std::time::Instant;

use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferRange, BufferUsage, ColorAttachment,
    DeviceStatus, DrawCall, DrawRange, Extent, Features, Format, LoadOp, Origin, PassDesc,
    QueryKind, ReadSource, StageSet, StepMode, StoreOp, TextureRegion, TextureUsage, Topology,
    VertexBinding, VertexFormat, Viewport,
};
use wcontainers_gpu_webgpu::WebGpu;

type B = WebGpu;

const SIZE: u32 = 256;
const OBJECTS: u32 = 256;
const INSTANCES: u32 = 16;

const NATIVE_WGSL: &str = r#"
struct V { @builtin(position) p: vec4f, @location(0) c: vec4f }
@vertex fn vs(@location(0) pos: vec2f, @location(1) at: vec4f, @location(2) scale: vec4f,
              @location(3) c: vec4f) -> V {
    return V(vec4f(pos * scale.xy + at.xy, 0.0, 1.0), c);
}
@fragment fn ps(v: V) -> @location(0) vec4f { return v.c; }
"#;

/// The instance attributes read from storage as a front's pulled variant reads them: byte offset,
/// stride and divisor from helper words, the element assembled from words.
const PULLED_WGSL: &str = r#"
struct V { @builtin(position) p: vec4f, @location(0) c: vec4f }
@group(0) @binding(160) var<storage, read> at: array<u32>;
@group(0) @binding(161) var<storage, read> scale: array<u32>;
@group(0) @binding(162) var<storage, read> color: array<u32>;
@group(0) @binding(0) var<uniform> pull: vec4<u32>;
fn word(i: u32, which: u32) -> f32 {
    switch which {
        case 0u: { return bitcast<f32>(at[i]); }
        case 1u: { return bitcast<f32>(scale[i]); }
        default: { return bitcast<f32>(color[i]); }
    }
}
fn element(instance: u32, which: u32) -> vec4f {
    let base = (pull.x + (instance / pull.z) * pull.y) / 4u;
    return vec4f(word(base, which), word(base + 1u, which), word(base + 2u, which),
                 word(base + 3u, which));
}
@vertex fn vs(@location(0) pos: vec2f, @builtin(instance_index) i: u32) -> V {
    let a = element(i, 0u);
    let s = element(i, 1u);
    return V(vec4f(pos * s.xy + a.xy, 0.0, 1.0), element(i, 2u));
}
@fragment fn ps(v: V) -> @location(0) vec4f { return v.c; }
"#;

fn device() -> B {
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

fn floats(v: &[[f32; 4]]) -> Vec<u8> {
    v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect()
}

/// One object's buffers: its quad, then where, how large and what colour each instance is.
fn object(b: &mut B, k: u32) -> [<B as Backend>::Buffer; 4] {
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
    let color: Vec<[f32; 4]> = (0..INSTANCES)
        .map(|j| [k as f32 / 255.0, j as f32 / 15.0, 0.5, 1.0])
        .collect();
    let usage = BufferUsage::VERTEX | BufferUsage::STORAGE;
    [
        draw::buffer(b, &quad, BufferUsage::VERTEX).unwrap(),
        draw::buffer(b, &floats(&at), usage).unwrap(),
        draw::buffer(b, &floats(&scale), usage).unwrap(),
        draw::buffer(b, &floats(&color), usage).unwrap(),
    ]
}

struct Timing {
    cpu_ms: Vec<f64>,
    gpu_ms: Vec<f64>,
    picture: Vec<u8>,
    bind_groups: u64,
}

fn median(v: &[f64]) -> f64 {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn run(b: &mut B, pulled: bool, frames: u32) -> Timing {
    let objects: Vec<_> = (0..OBJECTS).map(|k| object(b, k)).collect();
    let (target, view) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        SIZE,
        SIZE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let program = draw::program(b, if pulled { PULLED_WGSL } else { NATIVE_WGSL }).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let instance = |location| (location, 0, VertexFormat::Float32x4);
    let layout = if pulled {
        draw::layout(&[(8, StepMode::Vertex)], &[(0, 0, VertexFormat::Float32x2)])
    } else {
        draw::layout(
            &[
                (8, StepMode::Vertex),
                (16, StepMode::Instance(1)),
                (16, StepMode::Instance(1)),
                (16, StepMode::Instance(1)),
            ],
            &[
                (0, 0, VertexFormat::Float32x2),
                instance(1),
                instance(2),
                instance(3),
            ],
        )
    };
    let vertex_layout = b.create_vertex_layout(&layout).unwrap();
    // Offset 0, stride 16, divisor 1: the pulled variant's helper words.
    let words: Vec<u8> = [0u32, 16, 1, 0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let pull = draw::buffer(b, &words, BufferUsage::UNIFORM).unwrap();
    let stamps = b.create_query_set(QueryKind::Timestamp, 2).unwrap();
    let results =
        draw::buffer(b, &[0; 16], BufferUsage::COPY_SRC | BufferUsage::READ_BACK).unwrap();
    let groups_before = b.counters().bind_groups;
    let mut timing = Timing {
        cpu_ms: Vec::new(),
        gpu_ms: Vec::new(),
        picture: Vec::new(),
        bind_groups: 0,
    };
    for _ in 0..frames {
        let started = Instant::now();
        b.write_timestamp(&stamps, 0).unwrap();
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
        for o in &objects {
            // As a front does: a binding set made for the draw, the back end sharing equal ones.
            let set = pulled.then(|| {
                let storage = |binding, buffer| BindingEntry {
                    binding,
                    stages: StageSet::VERTEX,
                    resource: BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer,
                            offset: 0,
                            size: u64::from(INSTANCES) * 16,
                        },
                        writable: false,
                    },
                };
                b.create_binding_set(&[
                    BindingEntry {
                        binding: 0,
                        stages: StageSet::VERTEX,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &pull,
                            offset: 0,
                            size: 16,
                        }),
                    },
                    storage(160, &o[1]),
                    storage(161, &o[2]),
                    storage(162, &o[3]),
                ])
                .unwrap()
            });
            let streams = [
                Some(VertexBinding {
                    buffer: &o[0],
                    offset: 0,
                }),
                Some(VertexBinding {
                    buffer: &o[1],
                    offset: 0,
                }),
                Some(VertexBinding {
                    buffer: &o[2],
                    offset: 0,
                }),
                Some(VertexBinding {
                    buffer: &o[3],
                    offset: 0,
                }),
            ];
            b.draw(&DrawCall {
                program: &program,
                blend: &blend,
                depth_stencil: &depth_stencil,
                raster: &raster,
                vertex_layout: &vertex_layout,
                bindings: [set.as_ref(), None, None, None],
                vertex_buffers: if pulled { &streams[..1] } else { &streams },
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
        b.write_timestamp(&stamps, 1).unwrap();
        b.resolve_queries(&stamps, 0, 2, &results, 0).unwrap();
        let rb = b
            .read_back(&ReadSource::Buffer {
                buffer: &results,
                offset: 0,
                size: 16,
            })
            .unwrap();
        b.submit();
        timing.cpu_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        b.wait_read(&rb);
        let mut bytes = [0; 16];
        b.finish_read(rb, &mut bytes);
        let t = |i: usize| u64::from_le_bytes(bytes[i * 8..][..8].try_into().unwrap());
        timing.gpu_ms.push((t(1) - t(0)) as f64 / 1e6);
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
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    timing.picture = vec![0; (SIZE * SIZE * 4) as usize];
    b.finish_read(rb, &mut timing.picture);
    assert_eq!(b.status(), DeviceStatus::Ok);
    timing
}

#[test]
fn pulled_instance_data_draws_alike_and_its_cost_is_printed() {
    let mut b = device();
    assert!(
        b.caps().has(Features::TIMESTAMPS),
        "timings need timestamps"
    );
    let frames: u32 = std::env::var("WEBGPU_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20)
        .max(2);
    let native = run(&mut b, false, frames);
    let pulled = run(&mut b, true, frames);
    for (name, t) in [("vertex buffers", &native), ("pulled", &pulled)] {
        eprintln!(
            "{name}: {OBJECTS} draws of {INSTANCES} instances, 4 streams; first frame CPU {:.3} ms GPU {:.3} ms; median of {} later frames CPU {:.3} ms GPU {:.3} ms; {} bind groups made",
            t.cpu_ms[0],
            t.gpu_ms[0],
            frames - 1,
            median(&t.cpu_ms[1..]),
            median(&t.gpu_ms[1..]),
            t.bind_groups
        );
    }
    let lit = native.picture.chunks(4).filter(|p| p[3] != 0).count();
    assert!(lit > 0, "nothing drawn");
    assert!(
        native.picture == pulled.picture,
        "the pulled route drew another picture"
    );
}
