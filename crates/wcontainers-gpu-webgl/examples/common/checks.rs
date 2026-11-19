//! Checks of the WebGL 2 back end's own routes, beside the shared suite and run by the same hosts
//! (tests/suite.rs on ANGLE, examples/browser.rs in Chrome): each panics on a wrong answer.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use wcontainers_gpu::*;
use wcontainers_gpu_test::Host;

/// What every pattern of these checks is mixed with; 0 gives the patterns the checks were written
/// with.
static SEED: AtomicU32 = AtomicU32::new(0);

/// Why routes of the checks ran nothing here.
static NOT_RUN: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Records that a check's route did not run because the back end declares `f` absent; its
/// refusal is what `absent_features_are_refused` checks.
fn not_run(f: Features) {
    not_run_because(format!("{f:?} absent"));
}

/// Records that a check's route did not run, and why.
fn not_run_because(reason: String) {
    println!("not run: {reason}");
    NOT_RUN.lock().unwrap().push(reason);
}

/// Why routes did not run since the last call, joined; empty when every route ran. Hosts report
/// these apart from passes.
pub fn take_not_run() -> String {
    std::mem::take(&mut *NOT_RUN.lock().unwrap()).join("; ")
}

/// Runs `check` `cases` times, with the seeds from `seed` up, each printed first so a failure can be
/// replayed alone.
pub fn seeded(check: impl Fn(), seed: u32, cases: u32) {
    for n in 0..cases {
        let s = seed.wrapping_add(n);
        println!("seed {s}");
        SEED.store(s, Ordering::Relaxed);
        check();
    }
}

#[path = "extensions.rs"]
mod extensions;
pub use extensions::*;

/// Writes each point's vertex and instance numbers, and the elements its per-vertex and per-instance
/// streams read, into the pixel at (vertex number mod 8, instance number mod 4) of an 8 by 4 target.
const NUMBERS: &str = "
struct Out {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) n: vec4<u32>,
}
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32,
              @location(1) av: u32, @location(2) ai: u32) -> Out {
    var o: Out;
    o.p = vec4<f32>((f32(v % 8u) + 0.5) / 4.0 - 1.0, 1.0 - (f32(i % 4u) + 0.5) / 2.0, 0.0, 1.0);
    o.n = vec4<u32>(v, i, av, ai);
    return o;
}
@fragment fn fs(i: Out) -> @location(0) vec4<u32> {
    return i.n;
}
";

fn module(entry: &str, stage: ShaderStage) -> ShaderModule {
    let module = naga::front::wgsl::parse_str(NUMBERS).expect("the WGSL parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("the WGSL validates");
    ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: [0; 32],
    }
}

fn buffer<B: Backend>(b: &mut B, usage: BufferUsage, bytes: &[u8]) -> B::Buffer {
    let buffer = b
        .create_buffer(&BufferDesc {
            size: bytes.len() as u64,
            usage: usage | BufferUsage::COPY_DST | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    b.write_buffer(&buffer, 0, bytes, WriteMode::Ordered);
    buffer
}

fn words(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// How one draw of the numbers program is made.
enum Range {
    Vertices {
        first: u32,
        count: u32,
    },
    Indexed {
        format: IndexFormat,
        indices: Vec<u32>,
        first: u32,
        count: u32,
        base_vertex: i32,
    },
}

/// The numbers each point writes, as (pixel, [vertex, instance, per-vertex element, per-instance
/// element]); per-instance elements are 100 plus their number, which the stream's step `rate` gives.
fn expected(
    range: &Range,
    first_instance: u32,
    instances: u32,
    rate: u32,
) -> Vec<(usize, [u32; 4])> {
    let vertices: Vec<u32> = match range {
        Range::Vertices { first, count } => (*first..first + count).collect(),
        Range::Indexed {
            indices,
            first,
            count,
            base_vertex,
            ..
        } => indices[*first as usize..(first + count) as usize]
            .iter()
            .map(|i| (*i as i64 + i64::from(*base_vertex)) as u32)
            .collect(),
    };
    let mut out = Vec::new();
    for i in first_instance..first_instance + instances {
        let element = match rate {
            0 => first_instance,
            n => first_instance + (i - first_instance) / n,
        };
        for v in &vertices {
            out.push((((i % 4) * 8 + v % 8) as usize, [*v, i, *v, 100 + element]));
        }
    }
    out
}

/// `vertex_index` and `instance_index` include the draw's first or base vertex and first instance,
/// and vertex streams are read from those elements on, in non-indexed and indexed (8, 16 and 32-bit)
/// instanced draws, with the per-instance stream stepping every 1, 2 or 3 instances or never
/// (`INSTANCE_STEP_RATE`). WebGL 2 draws with no bases: the streams are bound at an offset
/// (`BaseVertexRebind`) and the program adds the hidden uniforms `naga_vs_base_vertex` and
/// `naga_vs_first_instance`.
pub fn vertex_and_instance_numbers_include_the_bases<H: Host>() {
    let mut b = H::make();
    let vs = b
        .create_shader(&module("vs", ShaderStage::Vertex))
        .expect("create_shader");
    let fs = b
        .create_shader(&module("fs", ShaderStage::Fragment))
        .expect("create_shader");
    let program = b
        .create_program(&Program::Render {
            vertex: &vs,
            fragment: Some(&fs),
        })
        .expect("create_program");
    assert!(b.caps().has(Features::INSTANCE_STEP_RATE));
    let layout_with_rate = |b: &mut H::Backend, rate| {
        let mut layout = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        layout.streams[0] = Some(VertexStream {
            stride: 4,
            step: StepMode::Vertex,
        });
        layout.streams[1] = Some(VertexStream {
            stride: 4,
            step: StepMode::Instance(rate),
        });
        for (i, location) in [1, 2].into_iter().enumerate() {
            layout.attributes[i] = Some(VertexAttribute {
                location,
                stream: i as u8,
                offset: 0,
                format: VertexFormat::Uint32x1,
            });
        }
        b.create_vertex_layout(&layout)
            .expect("create_vertex_layout")
    };
    let blend = b
        .create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; MAX_COLOR_TARGETS],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .expect("create_blend");
    let depth = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .expect("create_depth_stencil");
    let raster = b
        .create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::Clockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking: ProvokingVertex::Last,
            depth_minus_one: !b.caps().has(Features::CLIP_DEPTH_ZERO_ONE),
            sample_mask: u32::MAX,
        })
        .expect("create_raster");
    // Element k of the per-vertex stream holds k; of the per-instance stream, 100 + k.
    let per_vertex = buffer(
        &mut b,
        BufferUsage::VERTEX,
        &words(&(0..32).collect::<Vec<_>>()),
    );
    let per_instance = buffer(
        &mut b,
        BufferUsage::VERTEX,
        &words(&(100..132).collect::<Vec<_>>()),
    );
    let target = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba32Uint,
            width: 8,
            height: 4,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        })
        .expect("create_texture");
    let view = b
        .create_view(
            &target,
            &TextureViewDesc {
                format: Format::Rgba32Uint,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .expect("create_view");
    let indices = vec![3, 0, 5, 1, 2];
    let cases = [
        (
            "non-indexed",
            Range::Vertices { first: 3, count: 4 },
            2,
            2,
            1,
        ),
        (
            "8-bit indices",
            Range::Indexed {
                format: IndexFormat::U8,
                indices: indices.clone(),
                first: 1,
                count: 4,
                base_vertex: 2,
            },
            1,
            3,
            2,
        ),
        (
            "16-bit indices",
            Range::Indexed {
                format: IndexFormat::U16,
                indices: indices.clone(),
                first: 0,
                count: 5,
                base_vertex: 9,
            },
            0,
            3,
            0,
        ),
        (
            "32-bit indices",
            Range::Indexed {
                format: IndexFormat::U32,
                indices: indices.clone(),
                first: 2,
                count: 3,
                base_vertex: 0,
            },
            0,
            4,
            3,
        ),
    ];
    for (name, range, first_instance, instances, rate) in cases {
        let layout = layout_with_rate(&mut b, rate);
        let index = match &range {
            Range::Indexed {
                format, indices, ..
            } => {
                let bytes: Vec<u8> = match format {
                    IndexFormat::U8 => indices.iter().map(|i| *i as u8).collect(),
                    IndexFormat::U16 => indices
                        .iter()
                        .flat_map(|i| (*i as u16).to_le_bytes())
                        .collect(),
                    IndexFormat::U32 => words(indices),
                };
                Some((buffer(&mut b, BufferUsage::INDEX, &bytes), *format))
            }
            Range::Vertices { .. } => None,
        };
        let streams = [
            Some(VertexBinding {
                buffer: &per_vertex,
                offset: 0,
            }),
            Some(VertexBinding {
                buffer: &per_instance,
                offset: 0,
            }),
        ];
        let viewport = [Viewport {
            x: 0f32.to_bits(),
            y: 0f32.to_bits(),
            width: 8f32.to_bits(),
            height: 4f32.to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }];
        b.begin_pass(&PassDesc {
            colors: core::array::from_fn(|i| {
                (i == 0).then_some(ColorAttachment {
                    view: &view,
                    depth_slice: 0,
                    load: LoadOp::Clear([u32::MAX; 4]),
                    store: StoreOp::Store,
                    resolve: None,
                })
            }),
            depth: None,
            occlusion: None,
            empty: None,
        });
        b.draw(&DrawCall {
            program: &program,
            blend: &blend,
            depth_stencil: &depth,
            raster: &raster,
            vertex_layout: &layout,
            bindings: [None; MAX_BINDING_SETS],
            vertex_buffers: &streams,
            index: index.as_ref().map(|(buffer, format)| IndexBinding {
                buffer,
                offset: 0,
                format: *format,
            }),
            topology: Topology::PointList,
            restart: false,
            viewports: &viewport,
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: match &range {
                Range::Vertices { first, count } => DrawRange::Vertices {
                    first: *first,
                    count: *count,
                    first_instance,
                    instances,
                },
                Range::Indexed {
                    first,
                    count,
                    base_vertex,
                    ..
                } => DrawRange::Indexed {
                    first: *first,
                    count: *count,
                    base_vertex: *base_vertex,
                    first_instance,
                    instances,
                },
            },
        });
        b.end_pass();
        let mut got = vec![0u8; 8 * 4 * 16];
        b.read_back_now(
            &ReadSource::Texture {
                region: TextureRegion {
                    texture: &target,
                    mip: 0,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                extent: Extent {
                    width: 8,
                    height: 4,
                    depth_or_layers: 1,
                },
            },
            &mut got,
        )
        .expect("read_back_now");
        let texel = |p: usize| -> [u32; 4] {
            core::array::from_fn(|c| {
                u32::from_le_bytes(got[p * 16 + c * 4..p * 16 + c * 4 + 4].try_into().unwrap())
            })
        };
        let want = expected(&range, first_instance, instances, rate);
        for (pixel, numbers) in &want {
            assert_eq!(texel(*pixel), *numbers, "{name}: pixel {pixel}");
        }
        for p in (0..32).filter(|p| want.iter().all(|(w, _)| w != p)) {
            assert_eq!(
                texel(p),
                [u32::MAX; 4],
                "{name}: pixel {p} should be untouched"
            );
        }
        if let Some((buffer, _)) = index {
            b.destroy(Object::Buffer(buffer));
        }
        b.destroy(Object::VertexLayout(layout));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A `Discard` write of part of a buffer gives it fresh storage: a read back recorded before it sees the
/// old bytes, and `NoOverwrite` writes above the part land beside it.
pub fn discard_then_no_overwrite_writes<H: Host>() {
    let mut b = H::make();
    let old: Vec<u8> = (0..=255).collect();
    let buffer = buffer(&mut b, BufferUsage::VERTEX, &old);
    let read = |b: &mut H::Backend| {
        b.read_back(&ReadSource::Buffer {
            buffer: &buffer,
            offset: 0,
            size: 128,
        })
        .expect("read_back")
    };
    let before = read(&mut b);
    let part = vec![7u8; 64];
    let above = vec![9u8; 64];
    b.write_buffer(&buffer, 0, &part, WriteMode::Discard);
    b.write_buffer(&buffer, 64, &above, WriteMode::NoOverwrite);
    let after = read(&mut b);
    b.submit();
    for (readback, want) in [
        (before, old[..128].to_vec()),
        (after, [part, above].concat()),
    ] {
        b.wait_read(&readback);
        assert_eq!(b.read_state(&readback), ReadState::Ready);
        let mut got = vec![0; 128];
        b.finish_read(readback, &mut got);
        assert_eq!(got, want);
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Depth and stencil read back as stored (`DepthReadByDraw` on WebGL 2): a pass clears a 4 by 4 target
/// to depth 0.25 and stencil 0x5a, then `clear_rects` sets the middle 2 by 2 to 1 and 0xc3, all read
/// back exactly. Formats the back end cannot read back (no `COPY_OUT`) are left out.
pub fn depth_and_stencil_read_back<H: Host>() {
    let mut b = H::make();
    let formats = [
        Format::Depth16Unorm,
        Format::Depth24Plus,
        Format::Depth32Float,
        Format::Depth24PlusStencil8,
        Format::Depth32FloatStencil8,
    ];
    let inside = |p: usize| (1..3).contains(&(p % 4)) && (1..3).contains(&(p / 4));
    for format in formats {
        if !b.caps().formats.get(format).contains(FormatUse::COPY_OUT) {
            continue;
        }
        let stencil = format.kind() == FormatKind::DepthStencil;
        let t = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: 4,
                height: 4,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
                cube: false,
                reinterpret: false,
            })
            .expect("create_texture");
        let v = b
            .create_view(
                &t,
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )
            .expect("create_view");
        let ops = |v| AspectOps {
            load: LoadOp::Clear(v),
            store: StoreOp::Store,
            read_only: false,
        };
        b.begin_pass(&PassDesc {
            colors: core::array::from_fn(|_| None),
            depth: Some(DepthAttachment {
                view: &v,
                depth: Some(ops(0.25f32.to_bits())),
                stencil: stencil.then_some(AspectOps {
                    load: LoadOp::Clear(0x5a),
                    store: StoreOp::Store,
                    read_only: false,
                }),
            }),
            occlusion: None,
            empty: None,
        });
        b.end_pass();
        b.clear_rects(
            &v,
            format,
            ClearValue::DepthStencil {
                depth: Some(1f32.to_bits()),
                stencil: stencil.then_some(0xc3),
            },
            &[ScissorRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            }],
        )
        .expect("clear_rects");
        let read = |b: &mut H::Backend, stencil: bool, bytes: usize| {
            let mut got = vec![0u8; 16 * bytes];
            b.read_back_now(
                &ReadSource::Texture {
                    region: TextureRegion {
                        texture: &t,
                        mip: 0,
                        layer: 0,
                        origin: Origin::default(),
                        stencil,
                    },
                    extent: Extent {
                        width: 4,
                        height: 4,
                        depth_or_layers: 1,
                    },
                },
                &mut got,
            )
            .expect("read_back_now");
            got
        };
        let (bytes, one, tolerance) = match format {
            Format::Depth16Unorm => (2, 65535.0, 0),
            Format::Depth24Plus | Format::Depth24PlusStencil8 => (4, 16_777_215.0, 0),
            _ => (4, 0.0, 0),
        };
        let depth = read(&mut b, false, bytes);
        for p in 0..16 {
            let want = if inside(p) { 1f64 } else { 0.25 };
            let raw = &depth[p * bytes..(p + 1) * bytes];
            let got = if bytes == 2 {
                u64::from(u16::from_le_bytes(raw.try_into().unwrap()))
            } else {
                u64::from(u32::from_le_bytes(raw.try_into().unwrap()))
            };
            let want = if one == 0.0 {
                u64::from((want as f32).to_bits())
            } else {
                (want * one).round() as u64
            };
            assert!(
                got.abs_diff(want) <= tolerance,
                "{format:?}: depth at {p} is {got:#x}, expected {want:#x}"
            );
        }
        if stencil {
            let got = read(&mut b, true, 1);
            let want: Vec<u8> = (0..16)
                .map(|p| if inside(p) { 0xc3 } else { 0x5a })
                .collect();
            assert_eq!(got, want, "{format:?}: stencil");
        }
        b.destroy(Object::TextureView(v));
        b.destroy(Object::Texture(t));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Fetches the stencil under each fragment through a stencil view.
const STENCIL_FETCH: &str = "
@group(0) @binding(32) var s: texture_2d<u32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    return vec4<u32>(textureLoad(s, vec2<i32>(p.xy), 0).r, 0u, 0u, 0u);
}
";

/// The stencil of a depth-stencil texture, sampled through a stencil view and read back, twice (the
/// second time after the stencil changed): every pixel holds its own value, so each bit is seen set
/// and clear. Each host runs it with and without WEBGL_stencil_texturing; the answers must not differ.
pub fn stencil_sampled_and_read_back<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, STENCIL_FETCH, "vs", "fs");
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::R32Uint,
        [4, 4, 1],
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let out_view = kit::target(&mut b, &out, Format::R32Uint, 0, 0);
    let viewports = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    for format in [Format::Depth24PlusStencil8, Format::Depth32FloatStencil8] {
        let uses = b.caps().formats.get(format);
        assert!(
            uses.contains(FormatUse::SAMPLE | FormatUse::COPY_OUT),
            "{format:?} samples and reads back on every route"
        );
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 4, 1],
            1,
            TextureUsage::RENDER | TextureUsage::SAMPLE | TextureUsage::COPY_SRC,
            false,
        );
        let target = kit::target(&mut b, &t, format, 0, 0);
        let stencil = b
            .create_view(
                &t,
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: true,
                },
            )
            .expect("create_view of the stencil");
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&stencil),
            }])
            .expect("create_binding_set");
        for round in 0..2u32 {
            let value = |p: u32| (p * 17 + round * 101 + 3) as u8;
            for p in 0..16 {
                b.clear_rects(
                    &target,
                    format,
                    ClearValue::DepthStencil {
                        depth: None,
                        stencil: Some(value(p)),
                    },
                    &[ScissorRect {
                        x: p % 4,
                        y: p / 4,
                        width: 1,
                        height: 1,
                    }],
                )
                .expect("clear_rects");
            }
            let want: Vec<u8> = (0..16).map(value).collect();
            let read = kit::read(&mut b, &t, 0, 0, [4, 4, 1], 1, true);
            assert_eq!(read, want, "{format:?}: stencil read back, round {round}");
            b.begin_pass(&kit::pass(&out_view, [0; 4], None));
            b.draw(&kit::call(
                &k,
                &program,
                Some(&set),
                &viewports,
                Topology::TriangleList,
                3,
            ));
            b.end_pass();
            let sampled: Vec<u8> = kit::read(&mut b, &out, 0, 0, [4, 4, 1], 4, false)
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_le_bytes(*w) as u8)
                .collect();
            assert_eq!(
                sampled,
                want,
                "{format:?}: stencil sampled, round {round} (device {:?})",
                b.status()
            );
        }
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(stencil));
        b.destroy(Object::TextureView(target));
        b.destroy(Object::Texture(t));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Bytes of every value, from a fixed xorshift sequence: floats among them hit NaNs, infinities and
/// denormals.
fn pattern(len: usize, seed: u32) -> Vec<u8> {
    let mut x = (seed ^ SEED.load(Ordering::Relaxed).wrapping_mul(0x9e37_79b9)) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

/// Copies between formats of one typeless group keep every byte: a region of two layers of a 2D
/// array texture written with every bit pattern is copied into a texture of the other format at
/// another place, and read back (10-10-10-2 formats, which GL does not read back, by a program
/// loading them). Signed normalised pairs hold the smallest value (-128, -32768) in every fifth
/// component, which samples as the value above it. A pair whose formats the back end lacks is
/// skipped, and says so.
pub fn copies_within_a_typeless_group<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let pairs = [
        (Format::Rgba8Unorm, Format::Rgba8Uint),
        (Format::Rgba8Uint, Format::Rgba8Unorm),
        (Format::Rgba8Uint, Format::Rgba8Sint),
        (Format::Rgba8Sint, Format::Rgba8Unorm),
        (Format::Rg16Float, Format::Rg16Uint),
        (Format::Rg16Sint, Format::Rg16Uint),
        (Format::Rg16Uint, Format::Rg16Float),
        (Format::R16Float, Format::R16Sint),
        (Format::Rgba16Float, Format::Rgba16Uint),
        (Format::Rgba16Sint, Format::Rgba16Float),
        (Format::R32Float, Format::R32Uint),
        (Format::R32Uint, Format::R32Float),
        (Format::R32Sint, Format::R32Uint),
        (Format::Rgba32Float, Format::Rgba32Sint),
        (Format::Rgba32Uint, Format::Rgba32Float),
        (Format::Rgba8Unorm, Format::Rgba8UnormSrgb),
        (Format::Rgba8UnormSrgb, Format::Rgba8Uint),
        (Format::Bgra8Unorm, Format::Bgra8UnormSrgb),
        (Format::Bgra8UnormSrgb, Format::Bgra8Unorm),
        (Format::Rgba16Unorm, Format::Rgba16Uint),
        (Format::Rgba16Uint, Format::Rgba16Unorm),
        (Format::R16Unorm, Format::R16Uint),
        (Format::Rg16Sint, Format::Rg16Unorm),
        (Format::Rg16Unorm, Format::Rg16Sint),
        (Format::Rgb32Float, Format::Rgb32Uint),
        (Format::Rgb32Sint, Format::Rgb32Float),
        (Format::Rgb10a2Unorm, Format::Rgb10a2Uint),
        (Format::Rgb10a2Uint, Format::Rgb10a2Unorm),
        (Format::R8Unorm, Format::R8Uint),
        (Format::R8Uint, Format::R8Sint),
        (Format::R8Sint, Format::R8Unorm),
        (Format::Rg8Unorm, Format::Rg8Sint),
        (Format::Rg8Sint, Format::Rg8Uint),
        (Format::Rg8Uint, Format::Rg8Unorm),
        (Format::Rg32Float, Format::Rg32Uint),
        (Format::Rg32Uint, Format::Rg32Sint),
        (Format::Rg32Sint, Format::Rg32Float),
        (Format::Rgba8Sint, Format::Rgba8Uint),
        (Format::R8Snorm, Format::R8Uint),
        (Format::R8Uint, Format::R8Snorm),
        (Format::Rg8Snorm, Format::Rg8Unorm),
        (Format::Rg8Sint, Format::Rg8Snorm),
        (Format::Rgba8Snorm, Format::Rgba8Uint),
        (Format::Rgba8Unorm, Format::Rgba8Snorm),
        (Format::R16Snorm, Format::R16Float),
        (Format::R16Float, Format::R16Snorm),
        (Format::Rg16Snorm, Format::Rg16Uint),
        (Format::Rgba16Sint, Format::Rgba16Snorm),
        (Format::Rgba16Snorm, Format::Rgba16Unorm),
        // Copies within one format, the commonest, keep every bit too.
        (Format::R16Float, Format::R16Float),
        (Format::Rg16Float, Format::Rg16Float),
        (Format::Rgba16Float, Format::Rgba16Float),
        (Format::R32Float, Format::R32Float),
        (Format::Rg32Float, Format::Rg32Float),
        (Format::Rgba32Float, Format::Rgba32Float),
        (Format::Rgb32Float, Format::Rgb32Float),
        (Format::Rgba8Unorm, Format::Rgba8Unorm),
        (Format::Rgba8UnormSrgb, Format::Rgba8UnormSrgb),
        (Format::Rgb10a2Unorm, Format::Rgb10a2Unorm),
        (Format::Rgba16Unorm, Format::Rgba16Unorm),
        (Format::Rg16Sint, Format::Rg16Sint),
        (Format::Rgba32Uint, Format::Rgba32Uint),
        (Format::Rgba8Snorm, Format::Rgba8Snorm),
        (Format::Rg16Snorm, Format::Rg16Snorm),
    ];
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    for (i, (from, to)) in pairs.into_iter().enumerate() {
        // Formats absent here (16-bit normalised without EXT_texture_norm16) are not copied.
        if !b.caps().formats.get(from).contains(FormatUse::SAMPLE)
            || !b.caps().formats.get(to).contains(FormatUse::SAMPLE)
        {
            println!("not copied, a format absent: {from:?} to {to:?}");
            continue;
        }
        let texel = from.block().2 as usize;
        let make = |b: &mut H::Backend, format| {
            kit::texture(b, TextureDimension::D2, format, [4, 4, 2], 1, usage, false)
        };
        let src = make(&mut b, from);
        let dst = make(&mut b, to);
        let mut bytes = typeless_bytes(from, 32, 0x9e37_79b9 ^ i as u32);
        if let Some(w) = smallest_width(from).or(smallest_width(to)) {
            for c in bytes.chunks_exact_mut(w).step_by(5) {
                c.fill(0);
                c[w - 1] = 0x80;
            }
        }
        let whole = Extent {
            width: 4,
            height: 4,
            depth_or_layers: 2,
        };
        b.write_texture(
            &layer_at(&src, 0, 0, 0, false),
            whole,
            &bytes,
            4 * texel as u32,
            4,
            WriteMode::Ordered,
        );
        b.write_texture(
            &layer_at(&dst, 0, 0, 0, false),
            whole,
            &vec![0; 32 * texel],
            4 * texel as u32,
            4,
            WriteMode::Ordered,
        );
        // Three by two texels from (1, 1) to (0, 2), in both layers.
        let part = Extent {
            width: 3,
            height: 2,
            depth_or_layers: 2,
        };
        b.copy_texture(
            &layer_at(&src, 0, 1, 1, false),
            &layer_at(&dst, 0, 0, 2, false),
            part,
        );
        let got = read_typeless(&mut b, &dst, to, [4, 4, 2]);
        let mut want = vec![0u8; 32 * texel];
        for layer in 0..2 {
            for y in 0..2 {
                for x in 0..3 {
                    let at = |x: usize, y: usize| (layer * 16 + y * 4 + x) * texel;
                    let (s, d) = (at(x + 1, y + 1), at(x, y + 2));
                    want[d..d + texel].copy_from_slice(&bytes[s..s + texel]);
                }
            }
        }
        assert_eq!(got, want, "{from:?} to {to:?} (device {:?})", b.status());
        b.destroy(Object::Texture(src));
        b.destroy(Object::Texture(dst));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Copies between 8-bit sRGB and plain normalised formats keep every byte: a texture holding every
/// byte value in every component is copied whole into the other format and read back. On WebGL 2 a
/// draw decodes and encodes on the GPU, so the hardware's sRGB conversion must give back each byte.
pub fn srgb_copies_keep_every_byte<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let pairs = [
        (Format::Rgba8Unorm, Format::Rgba8UnormSrgb),
        (Format::Rgba8UnormSrgb, Format::Rgba8Unorm),
        (Format::Bgra8Unorm, Format::Bgra8UnormSrgb),
        (Format::Bgra8UnormSrgb, Format::Bgra8Unorm),
    ];
    let side = (usize::from(u8::MAX) + 1).isqrt() as u32;
    let size = [side, side, 1];
    let texel = Format::Rgba8Unorm.block().2;
    // Texel i holds i, its complement, i with its bits reversed and i again.
    let bytes: Vec<u8> = (0..=u8::MAX)
        .flat_map(|i| [i, !i, i.reverse_bits(), i])
        .collect();
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let whole = Extent {
        width: side,
        height: side,
        depth_or_layers: 1,
    };
    for (from, to) in pairs {
        let src = kit::texture(&mut b, TextureDimension::D2, from, size, 1, usage, false);
        let dst = kit::texture(&mut b, TextureDimension::D2, to, size, 1, usage, false);
        b.write_texture(
            &layer_at(&src, 0, 0, 0, false),
            whole,
            &bytes,
            side * texel,
            side,
            WriteMode::Ordered,
        );
        let before = b.slow_paths().count(SlowPath::CopyThroughBuffer);
        b.copy_texture(
            &layer_at(&src, 0, 0, 0, false),
            &layer_at(&dst, 0, 0, 0, false),
            whole,
        );
        let counted = b.slow_paths().count(SlowPath::CopyThroughBuffer) - before;
        let got = read_typeless(&mut b, &dst, to, size);
        assert_eq!(b.status(), DeviceStatus::Ok, "{from:?} to {to:?}");
        let texel = texel as usize;
        let wrong = got
            .chunks_exact(texel)
            .zip(bytes.chunks_exact(texel))
            .position(|(g, w)| g != w);
        if let Some(i) = wrong {
            let at = i * texel..(i + 1) * texel;
            panic!(
                "{from:?} to {to:?}: texel {i} is {:?}, written {:?}",
                &got[at.clone()],
                &bytes[at]
            );
        }
        println!("{from:?} to {to:?}: every byte kept, {counted} slow copies counted");
        b.destroy(Object::Texture(src));
        b.destroy(Object::Texture(dst));
    }
}

/// `texels` texels of `format` from `seed`; in 16-bit and 32-bit floats every third component is
/// one of the infinities, NaNs with payloads (signalling and quiet, of either sign), denormals and
/// signed zeros, whose bits a copy must keep.
fn typeless_bytes(format: Format, texels: usize, seed: u32) -> Vec<u8> {
    let mut bytes = pattern(texels * format.block().2 as usize, seed);
    match format {
        Format::R16Float | Format::Rg16Float | Format::Rgba16Float => {
            let special: [u16; 8] = [
                0x7c01, 0xfe01, 0x0001, 0x8000, 0x7c00, 0x7dc4, 0x83ff, 0x7e00,
            ];
            for (at, h) in bytes
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .step_by(3)
                .zip(special.iter().cycle())
            {
                at.copy_from_slice(&h.to_le_bytes());
            }
        }
        Format::Rg11b10Float => {
            // Infinities, NaNs with payloads, the smallest and largest denormals, the largest value.
            let w = |r: u32, g: u32, b: u32| r | g << 11 | b << 22;
            let special = [
                w(0x7c0, 0x7c1, 0x001),
                w(0x03f, 0x7bf, 0x3e0),
                w(0x7e5, 0x000, 0x3e3),
                w(0x040, 0x001, 0x3df),
            ];
            for (at, s) in bytes
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .step_by(3)
                .zip(special.iter().cycle())
            {
                at.copy_from_slice(&s.to_le_bytes());
            }
        }
        Format::R32Float | Format::Rg32Float | Format::Rgba32Float | Format::Rgb32Float => {
            let special: [u32; 8] = [
                0x7f80_0001,
                0xffc0_0001,
                0x0000_0001,
                0x8000_0000,
                0x7f80_0000,
                0x7fa5_5aa5,
                0x807f_ffff,
                0x7fc0_0000,
            ];
            for (at, s) in bytes
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .step_by(3)
                .zip(special.iter().cycle())
            {
                at.copy_from_slice(&s.to_le_bytes());
            }
        }
        _ => {}
    }
    bytes
}

/// The integer twin of a format the back end may not read back: of 16-bit floats the unsigned one,
/// of signed normalised formats the signed one.
fn twin(format: Format) -> Option<Format> {
    use Format as F;
    Some(match format {
        F::R16Float => F::R16Uint,
        F::Rg16Float => F::Rg16Uint,
        F::Rgba16Float => F::Rgba16Uint,
        F::R8Snorm => F::R8Sint,
        F::Rg8Snorm => F::Rg8Sint,
        F::Rgba8Snorm => F::Rgba8Sint,
        F::R16Snorm => F::R16Sint,
        F::Rg16Snorm => F::Rg16Sint,
        F::Rgba16Snorm => F::Rgba16Sint,
        _ => return None,
    })
}

/// Bytes per component of a signed normalised format, whose smallest value samples as the one
/// above it.
fn smallest_width(format: Format) -> Option<usize> {
    use Format as F;
    match format {
        F::R8Snorm | F::Rg8Snorm | F::Rgba8Snorm => Some(1),
        F::R16Snorm | F::Rg16Snorm | F::Rgba16Snorm => Some(2),
        _ => None,
    }
}

/// Signed normalised texels drawn into (with EXT_render_snorm) keep their bits through copies: a
/// draw writes no smallest value, so a draw reading them back recovers them (on the GPU into a
/// format that renders, or into the host copy, `HostCopyReadBack`); the smallest value written by
/// the host after a draw is kept. Texels drawn over after such a write cannot be told from the
/// value above, so copying their bits loses the device as not built.
pub fn signed_normalised_texels_drawn_then_copied<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let f = Format::Rgba8Snorm;
    if !b.caps().formats.get(f).contains(FormatUse::RENDER) {
        return not_run_because(format!("{f:?} does not render here"));
    }
    let usage = TextureUsage::RENDER
        | TextureUsage::SAMPLE
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let v = kit::target(&mut b, &t, f, 0, 0);
    // -1, 64/127, -32/127 and 1: 0x81, 0x40, 0xe0 and 0x7f.
    let clear = [-1.0f32, 64.0 / 127.0, -32.0 / 127.0, 1.0].map(f32::to_bits);
    let drawn = [0x81u8, 0x40, 0xe0, 0x7f].repeat(16);
    let whole = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 1,
    };
    let copied = |b: &mut H::Backend, to: Format| {
        let u = kit::texture(b, TextureDimension::D2, to, [4, 4, 1], 1, usage, false);
        b.copy_texture(
            &layer_at(&t, 0, 0, 0, false),
            &layer_at(&u, 0, 0, 0, false),
            whole,
        );
        let got = kit::read(b, &u, 0, 0, [4, 4, 1], 4, false);
        b.destroy(Object::Texture(u));
        got
    };
    let count = |b: &mut H::Backend, path| b.slow_paths().count(path);
    b.begin_pass(&kit::pass(&v, clear, None));
    b.end_pass();
    let gpu = count(&mut b, SlowPath::CopyThroughBuffer);
    assert_eq!(
        copied(&mut b, Format::Rgba8Uint),
        drawn,
        "drawn, then copied on the GPU"
    );
    assert!(count(&mut b, SlowPath::CopyThroughBuffer) > gpu);
    // The smallest value written over the first texel after the draw.
    let host = count(&mut b, SlowPath::HostCopyReadBack);
    b.write_texture(
        &layer_at(&t, 0, 0, 0, false),
        Extent {
            width: 1,
            height: 1,
            depth_or_layers: 1,
        },
        &[0x80, 0x80, 0x80, 0x80],
        4,
        1,
        WriteMode::Ordered,
    );
    assert_eq!(count(&mut b, SlowPath::HostCopyReadBack), host + 1);
    let mut want = drawn.clone();
    want[..4].fill(0x80);
    assert_eq!(
        copied(&mut b, Format::Rgba8Sint),
        want,
        "the smallest value written after a draw"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
    // Drawn over again: whether the first texel still holds -128 or a draw wrote -127 is lost.
    let load = PassDesc {
        colors: core::array::from_fn(|i| {
            (i == 0).then_some(ColorAttachment {
                view: &v,
                depth_slice: 0,
                load: LoadOp::Load,
                store: StoreOp::Store,
                resolve: None,
            })
        }),
        ..kit::pass(&v, clear, None)
    };
    b.begin_pass(&load);
    b.end_pass();
    copied(&mut b, Format::Rgba8Uint);
    assert_ne!(
        b.status(),
        DeviceStatus::Ok,
        "bits drawn over the smallest value"
    );
}

/// Where a host write is judged to hold a signed normalised minimum: only whole components count
/// (bytes 00 80 across two 16-bit components do not), the padding between rows does not, and a
/// texture drawn whole or written whole again, or whose minima were written over, holds none.
/// Each case writes, draws a pass, copies into the signed integer twin and reads back.
pub fn signed_normalised_minimum_judged_by_whole_components<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    if !b
        .caps()
        .formats
        .get(Format::Rgba8Snorm)
        .contains(FormatUse::RENDER)
    {
        return not_run_because("Rgba8Snorm does not render here".into());
    }
    let usage = TextureUsage::RENDER
        | TextureUsage::SAMPLE
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    let size = |width, height| Extent {
        width,
        height,
        depth_or_layers: 1,
    };
    // Writes of (texels wide, high, row pitch, bytes), a pass loading or clearing to (1, -1, 0, 1),
    // and what each texel reads back as.
    type Case<'a> = (
        &'a str,
        Format,
        Vec<(u32, u32, u32, Vec<u8>)>,
        bool,
        [u8; 4],
    );
    let texel = |t: [u8; 4], pitch: usize, pad: u8| {
        let mut row = t.repeat(4);
        row.resize(pitch, pad);
        row.repeat(4)
    };
    let cases: [Case; 5] = [
        (
            "16-bit components 0x0012 and 0x1280, then cleared",
            Format::Rg16Snorm,
            vec![(4, 4, 16, texel([0x12, 0, 0x80, 0x12], 16, 0))],
            false,
            [0xff, 0x7f, 0x01, 0x80],
        ),
        (
            "rows padded with 0x80, then cleared",
            Format::Rgba8Snorm,
            vec![(4, 4, 20, texel([0x10; 4], 20, 0x80))],
            false,
            [0x7f, 0x81, 0, 0x7f],
        ),
        (
            "a minimum, then cleared whole",
            Format::Rgba8Snorm,
            vec![(1, 1, 4, vec![0x80; 4])],
            false,
            [0x7f, 0x81, 0, 0x7f],
        ),
        (
            "a minimum, then written whole, then loaded",
            Format::Rgba8Snorm,
            vec![(1, 1, 4, vec![0x80; 4]), (4, 4, 16, [0x10; 64].into())],
            true,
            [0x10; 4],
        ),
        (
            "a minimum written over, then loaded",
            Format::Rgba8Snorm,
            vec![
                (4, 4, 16, [0x10; 64].into()),
                (1, 1, 4, vec![0x80; 4]),
                (1, 1, 4, vec![0x10; 4]),
            ],
            true,
            [0x10; 4],
        ),
    ];
    let clear = [1.0f32, -1.0, 0.0, 1.0].map(f32::to_bits);
    for (name, f, writes, load, want) in cases {
        let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
        let v = kit::target(&mut b, &t, f, 0, 0);
        for (w, h, pitch, bytes) in &writes {
            b.write_texture(
                &layer_at(&t, 0, 0, 0, false),
                size(*w, *h),
                bytes,
                *pitch,
                *h,
                WriteMode::Ordered,
            );
        }
        let mut pass = kit::pass(&v, clear, None);
        if load {
            pass.colors[0].as_mut().unwrap().load = LoadOp::Load;
        }
        b.begin_pass(&pass);
        b.end_pass();
        let twin = twin(f).unwrap();
        let u = kit::texture(
            &mut b,
            TextureDimension::D2,
            twin,
            [4, 4, 1],
            1,
            usage,
            false,
        );
        b.copy_texture(
            &layer_at(&t, 0, 0, 0, false),
            &layer_at(&u, 0, 0, 0, false),
            size(4, 4),
        );
        let got = kit::read(&mut b, &u, 0, 0, [4, 4, 1], 4, false);
        assert_eq!(b.status(), DeviceStatus::Ok, "{name}");
        assert_eq!(got, want.repeat(16), "{name}");
        b.destroy(Object::Texture(u));
        b.destroy(Object::TextureView(v));
        b.destroy(Object::Texture(t));
    }
    // Levels are judged apart: level 1 holds the minimum while level 0 is drawn into, and each
    // reads back as it is, level 0 through its twin and level 1 as written.
    let f = Format::Rgba8Snorm;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 2, usage, false);
    fn level<B: Backend>(texture: &B::Texture, mip: u32) -> TextureRegion<'_, B> {
        TextureRegion {
            mip,
            ..layer_at(texture, 0, 0, 0, false)
        }
    }
    b.write_texture(
        &level::<H::Backend>(&t, 1),
        size(2, 2),
        &[0x80; 16],
        8,
        2,
        WriteMode::Ordered,
    );
    b.write_texture(
        &level::<H::Backend>(&t, 0),
        size(4, 4),
        &[0x10; 64],
        16,
        4,
        WriteMode::Ordered,
    );
    let v = kit::target(&mut b, &t, f, 0, 0);
    let mut pass = kit::pass(&v, clear, None);
    pass.colors[0].as_mut().unwrap().load = LoadOp::Load;
    b.begin_pass(&pass);
    b.end_pass();
    let u = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::Rgba8Sint,
        [4, 4, 1],
        1,
        usage,
        false,
    );
    b.copy_texture(
        &level::<H::Backend>(&t, 0),
        &level::<H::Backend>(&u, 0),
        size(4, 4),
    );
    let got = kit::read(&mut b, &u, 0, 0, [4, 4, 1], 4, false);
    assert_eq!(
        b.status(),
        DeviceStatus::Ok,
        "level 0 drawn beside a minimum"
    );
    assert_eq!(got, [0x10; 64], "level 0 drawn beside a minimum");
    let got = kit::read(&mut b, &t, 1, 0, [2, 2, 1], 4, false);
    assert_eq!(got, [0x80; 16], "level 1 holding the minimum");
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// 16-bit floats and R11G11B10, which GL does not read back, read back as stored (NaN payloads,
/// infinities and denormals included) through their bits drawn into the unsigned integer twin
/// (`CopyThroughBuffer`): now, later, and into a buffer, from a region of a layer of a level.
/// B5G6R5, which GL reads back as bytes, the same way (packed again on the host).
pub fn half_floats_read_back_as_stored<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    for (n, format) in [
        Format::R16Float,
        Format::Rg16Float,
        Format::Rgba16Float,
        Format::Rg11b10Float,
        Format::B5g6r5Unorm,
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            b.caps().formats.get(format).contains(FormatUse::COPY_OUT),
            "{format:?} copies out"
        );
        let texel = format.block().2 as usize;
        let (w, h, layers) = (5usize, 4usize, 2u32);
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [w as u32 * 2, h as u32 * 2, layers],
            2,
            usage,
            false,
        );
        let bytes = typeless_bytes(format, w * h * layers as usize, n as u32);
        let level = TextureRegion {
            mip: 1,
            ..layer_at(&t, 0, 0, 0, false)
        };
        let whole = Extent {
            width: w as u32,
            height: h as u32,
            depth_or_layers: layers,
        };
        b.write_texture(
            &level,
            whole,
            &bytes,
            (w * texel) as u32,
            h as u32,
            WriteMode::Ordered,
        );
        // Columns 1 to 3 and rows 2 to 3 of layer 1.
        let part = Extent {
            width: 3,
            height: 2,
            depth_or_layers: 1,
        };
        let region = TextureRegion {
            layer: 1,
            origin: Origin { x: 1, y: 2, z: 0 },
            ..level
        };
        let want: Vec<u8> = (2..4)
            .flat_map(|y| {
                let at = ((h + y) * w + 1) * texel;
                bytes[at..at + 3 * texel].to_vec()
            })
            .collect();
        let drawn = b.slow_paths().count(SlowPath::CopyThroughBuffer);
        let converted = b.slow_paths().count(SlowPath::FormatConverted);
        let source = ReadSource::Texture {
            region: TextureRegion { ..region },
            extent: part,
        };
        let mut now = vec![0; want.len()];
        b.read_back_now(&source, &mut now).expect("read_back_now");
        assert_eq!(now, want, "{format:?} read back now");
        let later = b.read_back(&source).expect("read_back");
        b.submit();
        b.wait_read(&later);
        let mut got = vec![0; want.len()];
        b.finish_read(later, &mut got);
        assert_eq!(got, want, "{format:?} read back later");
        let out = zeros(&mut b, BufferUsage::COPY_DST | BufferUsage::COPY_SRC, 512);
        b.copy_texture_to_buffer(
            &region,
            &CopyBufferTexture {
                buffer: &out,
                offset: 16,
                row_pitch: 256,
                rows_per_layer: 2,
            },
            part,
        );
        let rows = read_buffer(&mut b, &out, 512);
        let row = 3 * texel;
        assert_eq!(
            [&rows[16..16 + row], &rows[272..272 + row]].concat(),
            want,
            "{format:?} copied into a buffer"
        );
        // 16-bit and packed floats through their bits drawn into a twin; B5G6R5 packed again on
        // the host, each of the three reads.
        let (path, before) = if format == Format::B5g6r5Unorm {
            (SlowPath::FormatConverted, converted)
        } else {
            (SlowPath::CopyThroughBuffer, drawn)
        };
        assert!(
            b.slow_paths().count(path) >= before + 3,
            "{format:?}: {path:?} counted {} times in three reads",
            b.slow_paths().count(path) - before
        );
        assert_eq!(b.status(), DeviceStatus::Ok);
        b.destroy(Object::Buffer(out));
        b.destroy(Object::Texture(t));
    }
}

/// Two devices on one thread, used in turn, each keep their own objects (natively each makes its
/// own context current again), and the second keeps working after the first is dropped.
pub fn two_devices_on_one_thread_keep_their_own_objects<H: Host>() {
    use crate::kit;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let f = Format::Rgba8Unorm;
    let mut a = H::make();
    let ta = kit::texture(&mut a, TextureDimension::D2, f, [2, 2, 1], 1, usage, false);
    let mut b = H::make();
    let tb = kit::texture(&mut b, TextureDimension::D2, f, [2, 2, 1], 1, usage, false);
    let vb = kit::target(&mut b, &tb, f, 0, 0);
    let write = |d: &mut H::Backend, t: &<H::Backend as Backend>::Texture, v: u8| {
        d.write_texture(
            &layer_at(t, 0, 0, 0, false),
            Extent {
                width: 2,
                height: 2,
                depth_or_layers: 1,
            },
            &[v; 16],
            8,
            2,
            WriteMode::Ordered,
        );
    };
    write(&mut a, &ta, 11);
    write(&mut b, &tb, 22);
    let one = 1f32.to_bits();
    b.begin_pass(&kit::pass(&vb, [one, 0, 0, one], None));
    b.end_pass();
    write(&mut a, &ta, 33);
    assert_eq!(
        kit::read(&mut a, &ta, 0, 0, [2, 2, 1], 4, false),
        [33; 16],
        "first device"
    );
    assert_eq!(
        kit::read(&mut b, &tb, 0, 0, [2, 2, 1], 4, false),
        [255, 0, 0, 255].repeat(4),
        "second device"
    );
    assert_eq!(a.status(), DeviceStatus::Ok);
    a.destroy(Object::Texture(ta));
    drop(a);
    write(&mut b, &tb, 44);
    assert_eq!(
        kit::read(&mut b, &tb, 0, 0, [2, 2, 1], 4, false),
        [44; 16],
        "second device after the first is dropped"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Two colours from one module, for shaders of equal text made more than once.
const COLOURS: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn red() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }
@fragment fn green() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 1.0, 0.0, 1.0); }
";

/// Programs of shaders of equal text each draw, whichever of them was destroyed first, and draw
/// again when made after the last was destroyed.
pub fn programs_of_equal_shaders_draw_until_each_is_destroyed<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let k = kit::Kit::new(&mut b);
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let side = 4;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [side, side, 1], 1, usage, false);
    let view = kit::target(&mut b, &t, f, 0, 0);
    let viewports = [kit::viewport(0.0, 0.0, side as f32, side as f32)];
    let make = |b: &mut H::Backend, fs: &str| {
        let module = |entry, stage| ShaderModule::from_wgsl(COLOURS, entry, stage).expect("WGSL");
        let v = b
            .create_shader(&module("vs", ShaderStage::Vertex))
            .expect("create_shader");
        let f = b
            .create_shader(&module(fs, ShaderStage::Fragment))
            .expect("create_shader");
        let p = b
            .create_program(&Program::Render {
                vertex: &v,
                fragment: Some(&f),
            })
            .expect("create_program");
        (v, f, p)
    };
    let draw = |b: &mut H::Backend, p: &<H::Backend as Backend>::Program| {
        b.begin_pass(&kit::pass(&view, [0; 4], None));
        b.draw(&kit::call(&k, p, None, &viewports, Topology::TriangleStrip, 4));
        b.end_pass();
        kit::read(b, &t, 0, 0, [side, side, 1], 4, false)
    };
    let destroy = |b: &mut H::Backend, (v, f, p)| {
        b.destroy(Object::Program(p));
        b.destroy(Object::Shader(v));
        b.destroy(Object::Shader(f));
    };
    let red = [u8::MAX, 0, 0, u8::MAX].repeat((side * side) as usize);
    let green = [0, u8::MAX, 0, u8::MAX].repeat((side * side) as usize);
    let first = make(&mut b, "red");
    let second = make(&mut b, "red");
    let other = make(&mut b, "green");
    assert_eq!(draw(&mut b, &first.2), red, "the first");
    assert_eq!(draw(&mut b, &other.2), green, "the other");
    destroy(&mut b, first);
    assert_eq!(draw(&mut b, &second.2), red, "the second, the first destroyed");
    destroy(&mut b, second);
    assert_eq!(draw(&mut b, &other.2), green, "the other, both destroyed");
    let again = make(&mut b, "red");
    assert_eq!(draw(&mut b, &again.2), red, "made again after both were destroyed");
    destroy(&mut b, again);
    destroy(&mut b, other);
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Bit operations on seeded values: each pixel of an 8 by 8 target hashes its index with the seed
/// into `x` and `y`, an offset and a count up to 35 (past the width, which the operations cut).
const BITS: &str = "
@group(0) @binding(0) var<uniform> s: vec4<u32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
fn hash(i: u32) -> u32 {
    var x = (i ^ s.x) * 0x9e3779b9u;
    x ^= x >> 15u;
    x *= 0x85ebca6bu;
    x ^= x >> 13u;
    return select(x >> (i % 32u), 0u, i % 13u == 0u);
}
fn index(p: vec4<f32>) -> u32 { return u32(p.x) + u32(p.y) * 8u; }
@fragment fn fs_a(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let i = index(p);
    let x = hash(i);
    let y = hash(i + 64u);
    let o = (i * 7u + s.y) % 36u;
    let c = (i * 11u + s.z) % 36u;
    return vec4<u32>(extractBits(x, o, c), bitcast<u32>(extractBits(bitcast<i32>(x), o, c)),
        insertBits(x, y, o, c), bitcast<u32>(insertBits(bitcast<i32>(x), bitcast<i32>(y), o, c)));
}
@fragment fn fs_b(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let x = hash(index(p));
    return vec4<u32>(countOneBits(x), reverseBits(x), firstLeadingBit(x),
        bitcast<u32>(firstLeadingBit(bitcast<i32>(x))));
}
@fragment fn fs_c(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let x = hash(index(p));
    return vec4<u32>(firstTrailingBit(x), countLeadingZeros(x), countTrailingZeros(x),
        bitcast<u32>(countLeadingZeros(bitcast<i32>(x))));
}
@fragment fn fs_d(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let i = index(p);
    let x = hash(i);
    let y = hash(i + 64u);
    let o = (i * 7u + s.y) % 36u;
    let c = (i * 11u + s.z) % 36u;
    let v = vec3<i32>(bitcast<i32>(y), 0, bitcast<i32>(x));
    return vec4<u32>(extractBits(vec2<u32>(y, x), o, c).y,
        bitcast<u32>(countOneBits(v).z),
        firstTrailingBit(vec4<u32>(y, 1u, x, 0u)).z,
        bitcast<u32>(insertBits(v, v.zxy, o, c).x));
}
";

/// What WGSL defines for `BITS`' operations, on the host.
fn bits_expected(i: u32, s: [u32; 3], entry: &str) -> [u32; 4] {
    let hash = |i: u32| {
        let mut x = (i ^ s[0]).wrapping_mul(0x9e37_79b9);
        x ^= x >> 15;
        x = x.wrapping_mul(0x85eb_ca6b);
        x ^= x >> 13;
        if i.is_multiple_of(13) {
            0
        } else {
            x >> (i % 32)
        }
    };
    let (x, y) = (hash(i), hash(i + 64));
    let (offset, count) = ((i * 7 + s[1]) % 36, (i * 11 + s[2]) % 36);
    let o = offset.min(32);
    let c = count.min(32 - o);
    let ones = |c: u32| if c == 32 { u32::MAX } else { (1 << c) - 1 };
    let extract = |e: u32| if c == 0 { 0 } else { (e >> o) & ones(c) };
    let extract_signed = |e: u32| {
        if c == 0 {
            0
        } else {
            (((e << (32 - o - c)) as i32) >> (32 - c)) as u32
        }
    };
    let insert = |e: u32, n: u32| {
        if c == 0 {
            e
        } else {
            let mask = ones(c) << o;
            (e & !mask) | ((n << o) & mask)
        }
    };
    let leading = |e: u32| {
        if e == 0 {
            u32::MAX
        } else {
            31 - e.leading_zeros()
        }
    };
    let leading_signed = |e: u32| {
        let v = if (e as i32) < 0 { !e } else { e };
        leading(v)
    };
    let trailing = |e: u32| {
        if e == 0 { u32::MAX } else { e.trailing_zeros() }
    };
    match entry {
        "fs_a" => [extract(x), extract_signed(x), insert(x, y), insert(x, y)],
        "fs_b" => [
            x.count_ones(),
            x.reverse_bits(),
            leading(x),
            leading_signed(x),
        ],
        "fs_c" => [
            trailing(x),
            x.leading_zeros(),
            x.trailing_zeros(),
            if (x as i32) < 0 { 0 } else { x.leading_zeros() },
        ],
        _ => [extract(x), x.count_ones(), trailing(x), insert(y, x)],
    }
}

/// GLSL ES 3.00 lacks GLSL ES 3.10's bit functions, which naga writes for WGSL's bit operations;
/// the back end defines them in the program. Every operation, scalar and vector, signed and
/// unsigned, on seeded values and offsets and counts past the width, equals WGSL's definition.
pub fn bit_operations_equal_their_definition<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let seed = pattern(12, 0x5eed);
    let s: [u32; 3] =
        core::array::from_fn(|i| u32::from_le_bytes(seed[i * 4..i * 4 + 4].try_into().unwrap()));
    let params = buffer(
        &mut b,
        BufferUsage::UNIFORM,
        &words(&[s[0], s[1] % 36, s[2] % 36, 0]),
    );
    let s = [s[0], s[1] % 36, s[2] % 36];
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &params,
                offset: 0,
                size: 16,
            }),
        }])
        .expect("create_binding_set");
    let f = Format::Rgba32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(&mut b, TextureDimension::D2, f, [8, 8, 1], 1, usage, false);
    let view = kit::target(&mut b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 8.0, 8.0)];
    for entry in ["fs_a", "fs_b", "fs_c", "fs_d"] {
        let program = kit::program(&mut b, BITS, "vs", entry);
        b.begin_pass(&kit::pass(&view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got = kit::read(&mut b, &out, 0, 0, [8, 8, 1], 16, false);
        assert_eq!(b.status(), DeviceStatus::Ok, "{entry}");
        for (i, t) in got.as_chunks::<16>().0.iter().enumerate() {
            let got: [u32; 4] = core::array::from_fn(|c| {
                u32::from_le_bytes(t[c * 4..c * 4 + 4].try_into().unwrap())
            });
            assert_eq!(got, bits_expected(i as u32, s, entry), "{entry} pixel {i}");
        }
        b.destroy(Object::Program(program));
    }
    b.destroy(Object::BindingSet(set));
    b.destroy(Object::TextureView(view));
    b.destroy(Object::Texture(out));
    b.destroy(Object::Buffer(params));
}

/// Gathers of each kind, pixel `i` of an 8 by 8 target gathering at footprint corner
/// (`i % 8 - 2`, `i / 8 - 2`) (a quarter texel past the corner's centre), so footprints reach past
/// the edges; cube pixels gather inside face `i % 6`.
const GATHERS: &str = "
@group(0) @binding(16) var linear: sampler;
@group(0) @binding(17) var nearest: sampler;
@group(0) @binding(18) var less: sampler_comparison;
@group(0) @binding(32) var t2: texture_2d<f32>;
@group(0) @binding(33) var ta: texture_2d_array<u32>;
@group(0) @binding(34) var tc: texture_cube<f32>;
@group(0) @binding(35) var td: texture_depth_2d;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
fn index(p: vec4<f32>) -> u32 { return u32(p.x) + u32(p.y) * 8u; }
fn at(i: u32, size: vec2<f32>) -> vec2<f32> {
    return (vec2<f32>(f32(i % 8u) - 2.0, f32(i / 8u) - 2.0) + 0.75) / size;
}
fn bytes(v: vec4<f32>) -> vec4<u32> { return vec4<u32>(round(v * 255.0)); }
@fragment fn fs_2d(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    return bytes(textureGather(1, t2, linear, at(index(p), vec2<f32>(5.0, 4.0))));
}
@fragment fn fs_offset(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let uv = at(index(p), vec2<f32>(5.0, 4.0));
    return bytes(textureGather(1, t2, linear, uv, vec2<i32>(-1, 2)));
}
@fragment fn fs_array(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let i = index(p);
    return textureGather(2, ta, nearest, at(i, vec2<f32>(3.0, 3.0)), i32(i % 2u));
}
@fragment fn fs_cube(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let i = index(p);
    var ms = array(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, -1.0));
    var us = array(vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0));
    var vv = array(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0),
        vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0));
    let f = i % 6u;
    let st = (vec2<f32>(f32((i / 6u) % 3u), f32((i / 18u) % 3u)) + 0.75) / 4.0 * 2.0 - 1.0;
    let d = ms[f] + st.x * us[f] + st.y * vv[f];
    return bytes(textureGather(0, tc, linear, d));
}
@fragment fn fs_depth(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let r = textureGatherCompare(td, less, at(index(p), vec2<f32>(4.0, 4.0)), 0.4);
    return vec4<u32>(round(r));
}
";

/// What a gather at footprint corner `b` of a `w` by `h` grid returns, `texel` giving each
/// texel's value with the corner's address mode applied by the caller.
fn footprint(b: (i32, i32), texel: impl Fn(i32, i32) -> u32) -> [u32; 4] {
    let (x, y) = b;
    [
        texel(x, y + 1),
        texel(x + 1, y + 1),
        texel(x + 1, y),
        texel(x, y),
    ]
}

/// GLSL ES 3.00 lacks `textureGather`; the back end samples the four texel centres of the
/// footprint instead. Every kind (2D with linear filtering and repeat addressing, with an offset,
/// an integer 2D array with nearest filtering and clamping, a cube inside its faces, and a depth
/// comparison) equals the footprint gathers define, on seeded texels.
pub fn gathers_equal_their_footprints<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST;
    let region = |t| TextureRegion {
        texture: t,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    let extent = |w, h, l| Extent {
        width: w,
        height: h,
        depth_or_layers: l,
    };
    let rgba = pattern(5 * 4 * 4, 1);
    let t2 = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::Rgba8Unorm,
        [5, 4, 1],
        1,
        usage,
        false,
    );
    b.write_texture(
        &region(&t2),
        extent(5, 4, 1),
        &rgba,
        20,
        4,
        WriteMode::Ordered,
    );
    let uints = pattern(3 * 3 * 2 * 4, 2);
    let ta = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::Rgba8Uint,
        [3, 3, 2],
        1,
        usage,
        false,
    );
    b.write_texture(
        &region(&ta),
        extent(3, 3, 2),
        &uints,
        12,
        3,
        WriteMode::Ordered,
    );
    let faces = pattern(4 * 4 * 6 * 4, 3);
    let tc = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::Rgba8Unorm,
        [4, 4, 6],
        1,
        usage,
        true,
    );
    b.write_texture(
        &region(&tc),
        extent(4, 4, 6),
        &faces,
        16,
        4,
        WriteMode::Ordered,
    );
    let depths: Vec<u8> = (0..16u32)
        .flat_map(|i| (i as f32 / 16.0).to_le_bytes())
        .collect();
    let td = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::Depth32Float,
        [4, 4, 1],
        1,
        usage,
        false,
    );
    b.write_texture(
        &region(&td),
        extent(4, 4, 1),
        &depths,
        16,
        4,
        WriteMode::Ordered,
    );
    let v2 = kit::view(
        &mut b,
        &t2,
        Format::Rgba8Unorm,
        ViewDimension::D2,
        0,
        1,
        0,
        1,
    );
    let va = kit::view(
        &mut b,
        &ta,
        Format::Rgba8Uint,
        ViewDimension::D2Array,
        0,
        1,
        0,
        2,
    );
    let vc = kit::view(
        &mut b,
        &tc,
        Format::Rgba8Unorm,
        ViewDimension::Cube,
        0,
        1,
        0,
        6,
    );
    let vd = kit::view(
        &mut b,
        &td,
        Format::Depth32Float,
        ViewDimension::D2,
        0,
        1,
        0,
        1,
    );
    let sampler = |b: &mut H::Backend, filter, address, compare| {
        b.create_sampler(&SamplerDesc {
            min: filter,
            mag: filter,
            mip: Filter::Nearest,
            address: [address; 3],
            lod_range: [0f32.to_bits(), 0f32.to_bits()],
            anisotropy: 1,
            compare,
            border: BorderColor::TransparentBlack,
        })
        .expect("create_sampler")
    };
    let linear = sampler(&mut b, Filter::Linear, AddressMode::Repeat, None);
    let nearest = sampler(&mut b, Filter::Nearest, AddressMode::Clamp, None);
    let less = sampler(
        &mut b,
        Filter::Linear,
        AddressMode::Clamp,
        Some(CompareFn::Less),
    );
    let entries = [
        (16, BindingResource::Sampler(&linear)),
        (17, BindingResource::Sampler(&nearest)),
        (18, BindingResource::Sampler(&less)),
        (32, BindingResource::Texture(&v2)),
        (33, BindingResource::Texture(&va)),
        (34, BindingResource::Texture(&vc)),
        (35, BindingResource::Texture(&vd)),
    ]
    .map(|(binding, resource)| BindingEntry {
        binding,
        stages: StageSet::FRAGMENT,
        resource,
    });
    let set = b.create_binding_set(&entries).expect("create_binding_set");
    let f = Format::Rgba32Uint;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [8, 8, 1],
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let view = kit::target(&mut b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 8.0, 8.0)];
    let wrap = |v: i32, n: i32| v.rem_euclid(n);
    let clamp = |v: i32, n: i32| v.clamp(0, n - 1);
    for entry in ["fs_2d", "fs_offset", "fs_array", "fs_cube", "fs_depth"] {
        let program = kit::program(&mut b, GATHERS, "vs", entry);
        b.begin_pass(&kit::pass(&view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got = kit::read(&mut b, &out, 0, 0, [8, 8, 1], 16, false);
        assert_eq!(b.status(), DeviceStatus::Ok, "{entry}");
        for (i, t) in got.as_chunks::<16>().0.iter().enumerate() {
            let got: [u32; 4] = core::array::from_fn(|c| {
                u32::from_le_bytes(t[c * 4..c * 4 + 4].try_into().unwrap())
            });
            let i = i as i32;
            let corner = (i % 8 - 2, i / 8 - 2);
            let want = match entry {
                "fs_2d" => footprint(corner, |x, y| {
                    u32::from(rgba[((wrap(y, 4) * 5 + wrap(x, 5)) * 4 + 1) as usize])
                }),
                "fs_offset" => footprint((corner.0 - 1, corner.1 + 2), |x, y| {
                    u32::from(rgba[((wrap(y, 4) * 5 + wrap(x, 5)) * 4 + 1) as usize])
                }),
                "fs_array" => footprint(corner, |x, y| {
                    let layer = i % 2;
                    u32::from(
                        uints[(((layer * 3 + clamp(y, 3)) * 3 + clamp(x, 3)) * 4 + 2) as usize],
                    )
                }),
                "fs_cube" => {
                    let face = i % 6;
                    footprint(((i / 6) % 3, (i / 18) % 3), |x, y| {
                        u32::from(faces[(((face * 4 + y) * 4 + x) * 4) as usize])
                    })
                }
                _ => footprint(corner, |x, y| {
                    u32::from(0.4 < (clamp(y, 4) * 4 + clamp(x, 4)) as f32 / 16.0)
                }),
            };
            assert_eq!(got, want, "{entry} pixel {i}");
        }
        b.destroy(Object::Program(program));
    }
    for v in [view, v2, va, vc, vd] {
        b.destroy(Object::TextureView(v));
    }
    for t in [out, t2, ta, tc, td] {
        b.destroy(Object::Texture(t));
    }
    b.destroy(Object::BindingSet(set));
}

/// One depth texture compared against and read plainly by the same program, as shadow maps are:
/// pixel (x, y) of an 8 by 8 target reads texel (x / 2, y / 2) of a 4 by 4 one.
const DEPTH_READ_BOTH_WAYS: &str = "
@group(0) @binding(16) var nearest: sampler;
@group(0) @binding(17) var less: sampler_comparison;
@group(0) @binding(32) var td: texture_depth_2d;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
fn steps(d: f32) -> u32 { return u32(round(d * 16.0)); }
fn plain(uv: vec2<f32>, level: i32) -> f32 { return textureSampleLevel(td, nearest, uv, level); }
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let uv = p.xy / 8.0;
    let compared = textureSampleCompareLevel(td, less, uv, 0.4);
    let loaded = textureLoad(td, vec2<i32>(p.xy) / 2, 0);
    var g = vec4<f32>(0.0);
    if (p.x < 4.0) {
        g = textureGather(td, nearest, uv, vec2<i32>(1, 0));
    } else {
        g = textureGather(td, nearest, uv);
    }
    return vec4<u32>(u32(compared), steps(plain(uv, 0)), steps(loaded),
        steps(g.w) | (steps(g.y) << 8u));
}
";

/// GLSL declares a depth texture as a shadow sampler, which only compares; a program that also
/// samples, loads or gathers it plainly reads a float copy of its variable. Every read equals the
/// texel the interface defines, with the comparison in the same program, and the plain reads in a
/// function, in a branch and with a constant offset.
pub fn depth_textures_compared_and_read_plainly<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let side = 4;
    let depths: Vec<u8> = (0..side * side)
        .flat_map(|i| (i as f32 / 16.0).to_le_bytes())
        .collect();
    let td = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::Depth32Float,
        [side, side, 1],
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        false,
    );
    b.write_texture(
        &TextureRegion {
            texture: &td,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: side,
            height: side,
            depth_or_layers: 1,
        },
        &depths,
        side * 4,
        side,
        WriteMode::Ordered,
    );
    let vd = kit::view(
        &mut b,
        &td,
        Format::Depth32Float,
        ViewDimension::D2,
        0,
        1,
        0,
        1,
    );
    let sampler = |b: &mut H::Backend, compare| {
        b.create_sampler(&SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [AddressMode::Clamp; 3],
            lod_range: [0f32.to_bits(), 0f32.to_bits()],
            anisotropy: 1,
            compare,
            border: BorderColor::TransparentBlack,
        })
        .expect("create_sampler")
    };
    let nearest = sampler(&mut b, None);
    let less = sampler(&mut b, Some(CompareFn::Less));
    let entries = [
        (16, BindingResource::Sampler(&nearest)),
        (17, BindingResource::Sampler(&less)),
        (32, BindingResource::Texture(&vd)),
    ]
    .map(|(binding, resource)| BindingEntry {
        binding,
        stages: StageSet::FRAGMENT,
        resource,
    });
    let set = b.create_binding_set(&entries).expect("create_binding_set");
    let f = Format::Rgba32Uint;
    let target = 8;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [target, target, 1],
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let view = kit::target(&mut b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, target as f32, target as f32)];
    let program = kit::program(&mut b, DEPTH_READ_BOTH_WAYS, "vs", "fs");
    b.begin_pass(&kit::pass(&view, [0; 4], None));
    b.draw(&kit::call(
        &k,
        &program,
        Some(&set),
        &vp,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    let texel_size = f.block().2 as usize;
    let got = kit::read(&mut b, &out, 0, 0, [target, target, 1], texel_size, false);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let side = side as i32;
    let texel = |x: i32, y: i32| (y.clamp(0, side - 1) * side + x.clamp(0, side - 1)) as u32;
    for (i, t) in got.as_chunks::<16>().0.iter().enumerate() {
        let got: [u32; 4] =
            core::array::from_fn(|c| u32::from_le_bytes(t[c * 4..c * 4 + 4].try_into().unwrap()));
        let (x, y) = (i as i32 % target as i32, i as i32 / target as i32);
        let at = texel(x / 2, y / 2);
        // The footprint's corner: the texel below and left of the sample point, (x + 0.5) / 2 - 0.5.
        let shift = i32::from(x < target as i32 / 2);
        let corner = ((x - 1).div_euclid(2) + shift, (y - 1).div_euclid(2));
        let gather = footprint(corner, texel);
        let want = [
            u32::from(0.4 < at as f32 / 16.0),
            at,
            at,
            gather[3] | (gather[1] << 8),
        ];
        assert_eq!(got, want, "pixel ({x}, {y})");
    }
    b.destroy(Object::Program(program));
    b.destroy(Object::BindingSet(set));
    for v in [view, vd] {
        b.destroy(Object::TextureView(v));
    }
    for t in [out, td] {
        b.destroy(Object::Texture(t));
    }
    for s in [nearest, less] {
        b.destroy(Object::Sampler(s));
    }
}

/// Two triangles whose vertices have different w (the second crossing the far plane, so it is
/// clipped), each value being its vertex's position on screen in clip space: linear values (at the
/// centre and at the centroid) are the fragment's own position, as window-space interpolation
/// gives; the perspective-correct one differs, which shows the draw tells them apart.
const LINEAR: &str = "
const SIDE: f32 = 32.0;
var<private> SCREEN: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(-0.95, -0.9, 1.0), vec3<f32>(-0.05, -0.6, 3.0), vec3<f32>(-0.6, 0.95, 0.5),
    vec3<f32>(0.05, -0.95, 2.0), vec3<f32>(0.95, 0.2, 0.7), vec3<f32>(0.3, 0.9, 1.5),
);
var<private> DEPTH: array<f32, 6> = array<f32, 6>(0.5, 0.5, 0.5, 0.5, 0.5, 1.6);
struct V {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(linear) a: vec2<f32>,
    @location(1) @interpolate(linear, centroid) b: f32,
    @location(2) @interpolate(perspective) c: f32,
}
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let s = SCREEN[i];
    return V(vec4<f32>(s.xy * s.z, DEPTH[i] * s.z, s.z), s.xy, s.x, s.x);
}
@fragment fn fs(v: V) -> @location(0) vec4<f32> {
    let at = vec2<f32>(v.p.x / SIDE * 2.0 - 1.0, 1.0 - v.p.y / SIDE * 2.0);
    return vec4<f32>(v.a - at, v.b - at.x, v.c - at.x);
}";

/// Linear (`noperspective`) values, which GLSL ES 3.00 lacks, interpolate in window space: at the
/// centre and at the centroid, in a triangle drawn whole and one clipped at the far plane.
pub fn linear_values_interpolate_in_window_space<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let f = Format::Rgba32Float;
    if !b.caps().formats.get(f).contains(FormatUse::RENDER) {
        not_run_because(format!("{f:?} does not render here"));
        return;
    }
    let side = 32;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [side, side, 1],
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let view = kit::target(&mut b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, side as f32, side as f32)];
    let program = kit::program(&mut b, LINEAR, "vs", "fs");
    let outside = f32::MAX;
    b.begin_pass(&kit::pass(&view, [outside.to_bits(); 4], None));
    b.draw(&kit::call(
        &k,
        &program,
        None,
        &vp,
        Topology::TriangleList,
        6,
    ));
    b.end_pass();
    let got = kit::read(
        &mut b,
        &out,
        0,
        0,
        [side, side, 1],
        f.block().2 as usize,
        false,
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
    let floats: Vec<f32> = got
        .as_chunks::<{ size_of::<f32>() }>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    let texels: Vec<[f32; 4]> = floats.as_chunks::<4>().0.to_vec();
    let drawn: Vec<(usize, [f32; 4])> = texels
        .into_iter()
        .enumerate()
        .filter(|(_, t)| t[0] != outside)
        .collect();
    // The vertices as LINEAR has them: screen x, y and w.
    let screen = [
        [-0.95f64, -0.9, 1.0],
        [-0.05, -0.6, 3.0],
        [-0.6, 0.95, 0.5],
        [0.05, -0.95, 2.0],
        [0.95, 0.2, 0.7],
        [0.3, 0.9, 1.5],
    ];
    // The perspective-correct value the host expects at (x, y) in clip space: the triangle whose
    // window-space weights are least negative there.
    let perspective = |x: f64, y: f64| {
        let weights = |t: &[[f64; 3]]| {
            let [a, b, c] = [t[0], t[1], t[2]];
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            let l1 = ((x - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (y - a[1])) / area;
            let l2 = ((b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1])) / area;
            [1.0 - l1 - l2, l1, l2]
        };
        let (t, l) = screen
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| (t, weights(t)))
            .max_by(|p, q| {
                let least = |l: &[f64; 3]| l.iter().copied().fold(f64::MAX, f64::min);
                least(&p.1).total_cmp(&least(&q.1))
            })
            .expect("two triangles");
        let (mut over_w, mut sum) = (0.0, 0.0);
        for (v, l) in t.iter().zip(l) {
            over_w += l / v[2];
            sum += l * v[0] / v[2];
        }
        sum / over_w
    };
    // Window-space interpolation in 32-bit floats through w from 0.5 to 3, against how far the
    // GPU's own perspective-correct interpolation is from the host's.
    let tolerance = 1e-3;
    let (mut linear_off, mut perspective_off, mut gpu_off) = (0f64, 0f64, 0f64);
    for (i, t) in &drawn {
        let (x, y) = (i % side as usize, i / side as usize);
        for (c, what) in [
            (0, "linear x"),
            (1, "linear y"),
            (2, "linear at the centroid"),
        ] {
            assert!(
                t[c].abs() < tolerance,
                "pixel ({x}, {y}): {what} is {} from the window-space value",
                t[c]
            );
            linear_off = linear_off.max(f64::from(t[c].abs()));
        }
        let half = f64::from(side) / 2.0;
        let (cx, cy) = ((x as f64 + 0.5) / half - 1.0, 1.0 - (y as f64 + 0.5) / half);
        perspective_off = perspective_off.max(f64::from(t[3].abs()));
        gpu_off = gpu_off.max((f64::from(t[3]) + cx - perspective(cx, cy)).abs());
    }
    println!(
        "linear values at most {linear_off:e} from window space; the GPU's perspective-correct \
         ones at most {gpu_off:e} from the host's, {perspective_off} from window space"
    );
    // The whole triangle covers about a fifth of the target, the clipped one less.
    assert!(
        drawn.len() > (side * side / 8) as usize,
        "{} pixels drawn",
        drawn.len()
    );
    assert!(
        perspective_off > 0.05,
        "the perspective-correct value is at most {perspective_off} from the linear one"
    );
    b.destroy(Object::Program(program));
    b.destroy(Object::TextureView(view));
    b.destroy(Object::Texture(out));
}

/// A full-screen triangle with the window x of each fragment interpolated per sample; one program
/// writes what it sees of its sample, the other writes the sample mask.
const PER_SAMPLE: &str = "
const SIDE: f32 = 8.0;
struct V { @builtin(position) p: vec4<f32>, @location(0) @interpolate(perspective, sample) x: f32 }
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let c = vec2<f32>(f32((i & 1u) << 2u) - 1.0, f32((i & 2u) << 1u) - 1.0);
    return V(vec4<f32>(c, 0.0, 1.0), (c.x + 1.0) * SIDE / 2.0);
}
@fragment fn fs_samples(v: V, @builtin(sample_index) s: u32, @builtin(sample_mask) inside: u32)
    -> @location(0) vec4<f32> {
    return vec4<f32>(f32(s) / 3.0, abs(fract(v.x) - 0.5) * 2.0, 0.0, f32(countOneBits(inside)) / 4.0);
}
struct O { @location(0) c: vec4<f32>, @builtin(sample_mask) m: u32 }
@fragment fn fs_mask(v: V) -> O {
    return O(vec4<f32>(1.0), select(1u, 6u, v.p.x > SIDE / 2.0));
}";

/// Programs run once per sample where they read the sample number (`SAMPLE_SHADING`, with
/// OES_sample_variables and OES_shader_multisample_interpolation in Chrome): into four samples,
/// each run sees its own number, only its own bit of the coverage, and the value interpolated at
/// its own place; a program writing the sample mask writes only the samples it names.
pub fn programs_run_per_sample_and_write_the_sample_mask<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let f = Format::Rgba8Unorm;
    let samples = 4;
    if !b.caps().has(Features::SAMPLE_SHADING) {
        not_run(Features::SAMPLE_SHADING);
        return;
    }
    if !b.caps().formats.takes_samples(f, samples) {
        not_run_because(format!("{f:?} takes no {samples} samples here"));
        return;
    }
    let k = Kit::new(&mut b);
    let side = 8;
    let whole = [kit::viewport(0.0, 0.0, side as f32, side as f32)];
    let mut resolved = Vec::new();
    for entry in ["fs_samples", "fs_mask"] {
        // Sample numbers and per-sample values need naga's multisampled shading capability.
        let [vs, fs] =
            [("vs", ShaderStage::Vertex), (entry, ShaderStage::Fragment)].map(|(e, s)| {
                let m = extensions::module_all(PER_SAMPLE, e, s);
                b.create_shader(&m).expect("create_shader")
            });
        let program = b
            .create_program(&Program::Render {
                vertex: &vs,
                fragment: Some(&fs),
            })
            .expect("create_program");
        let ms = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format: f,
                width: side,
                height: side,
                depth_or_layers: 1,
                mip_levels: 1,
                samples,
                usage: TextureUsage::RENDER,
                cube: false,
                reinterpret: false,
            })
            .expect("create_texture");
        let ms_view = kit::target(&mut b, &ms, f, 0, 0);
        let single = kit::texture(
            &mut b,
            TextureDimension::D2,
            f,
            [side, side, 1],
            1,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
            false,
        );
        let single_view = kit::target(&mut b, &single, f, 0, 0);
        let mut pass = kit::pass(&ms_view, [0; 4], None);
        if let Some(c) = &mut pass.colors[0] {
            c.resolve = Some(&single_view);
        }
        b.begin_pass(&pass);
        b.draw(&kit::call(
            &k,
            &program,
            None,
            &whole,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        resolved.push(kit::read(
            &mut b,
            &single,
            0,
            0,
            [side, side, 1],
            f.block().2 as usize,
            false,
        ));
        b.destroy(Object::Program(program));
        b.destroy(Object::Shader(vs));
        b.destroy(Object::Shader(fs));
        b.destroy(Object::TextureView(single_view));
        b.destroy(Object::Texture(single));
        b.destroy(Object::TextureView(ms_view));
        b.destroy(Object::Texture(ms));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    // A resolve averages the samples: n of four samples at 255 give n * 255 / 4.
    let of_four = |n: u32| ((n * u32::from(u8::MAX)) as f64 / f64::from(samples)).round() as i32;
    let near = |got: u8, want: i32, what: &str, at: usize| {
        assert!(
            (i32::from(got) - want).abs() <= 1,
            "pixel {at}: {what} resolved to {got}, not {want}"
        );
    };
    for (at, t) in resolved[0].as_chunks::<4>().0.iter().enumerate() {
        // The numbers 0 to 3 over 3 average a half; so do the distances of the four sample
        // places from the pixel's centre on Direct3D's pattern, where the centre gives 0.
        near(t[0], of_four(2), "the sample number", at);
        near(t[1], of_four(2), "the value interpolated at the sample", at);
        near(t[3], of_four(1), "the coverage each run reads", at);
    }
    for (at, t) in resolved[1].as_chunks::<4>().0.iter().enumerate() {
        let right = at % side as usize >= side as usize / 2;
        let want = of_four(if right { 2 } else { 1 });
        for c in t {
            near(*c, want, "the mask written", at);
        }
    }
}

/// Infinities and negative zero written into an `rgba32float` target, each way a program can hold
/// them: as a constant, from a local variable of their bits (as the DXBC converter writes
/// non-finite constants), as a literal negative zero, and from a uniform buffer.
const SPECIAL: &str = "
struct U { bits: vec4<u32> }
@group(0) @binding(0) var<uniform> u: U;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    var held = vec4<u32>(0x7f800000u, 0x80000000u, 0xff800000u, 0x3fc00000u);
    switch u32(p.x) {
        case 0u: { return bitcast<vec4<f32>>(vec4<u32>(0x7f800000u, 0x80000000u, 0xff800000u, 0x3fc00000u)); }
        case 1u: { return bitcast<vec4<f32>>(held); }
        case 2u: { return vec4<f32>(bitcast<f32>(0x7f800000u), -0.0, bitcast<f32>(0xff800000u), 1.5); }
        default: { return bitcast<vec4<f32>>(u.bits); }
    }
}";

/// Special float values reach the target with their bits by every route a program holds them.
pub fn special_float_values_keep_their_bits<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let f = Format::Rgba32Float;
    if !b.caps().formats.get(f).contains(FormatUse::RENDER) {
        not_run_because(format!("{f:?} does not render here"));
        return;
    }
    let want = [0x7f80_0000u32, 0x8000_0000, 0xff80_0000, 0x3fc0_0000];
    let bits: Vec<u8> = want.iter().flat_map(|w| w.to_le_bytes()).collect();
    let ub = buffer(&mut b, BufferUsage::UNIFORM, &bits);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ub,
                offset: 0,
                size: bits.len() as u64,
            }),
        }])
        .expect("create_binding_set");
    let routes = ["a constant", "a variable", "literals", "a uniform"];
    let width = routes.len() as u32;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [width, 1, 1],
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let view = kit::target(&mut b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, width as f32, 1.0)];
    let program = kit::program(&mut b, SPECIAL, "vs", "fs");
    b.begin_pass(&kit::pass(&view, [0; 4], None));
    b.draw(&kit::call(
        &k,
        &program,
        Some(&set),
        &vp,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    let texel = f.block().2 as usize;
    let got = kit::read(&mut b, &out, 0, 0, [width, 1, 1], texel, false);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let wrong: Vec<String> = routes
        .iter()
        .zip(got.chunks_exact(texel))
        .filter_map(|(route, t)| {
            let t: Vec<u32> = t
                .as_chunks::<{ size_of::<u32>() }>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect();
            (t != want).then(|| format!("{route}: {t:08x?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "want {want:08x?}; {}", wrong.join("; "));
    b.destroy(Object::Program(program));
    b.destroy(Object::BindingSet(set));
    b.destroy(Object::Buffer(ub));
    b.destroy(Object::TextureView(view));
    b.destroy(Object::Texture(out));
}

/// Copies within one float format are blits, taking no slow path, where the probe found blits keep
/// every bit; else they take a counted route. The bits kept are checked by
/// `copies_within_a_typeless_group`, on both routes (all.cmd runs it with `float_blits` off).
pub fn copies_within_one_float_format_take_the_probed_route<
    H: Host<Backend = wcontainers_gpu_webgl::Gl>,
>() {
    use crate::kit;
    let mut b = H::make();
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    for format in [Format::Rgba16Float, Format::R32Float] {
        let a = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 4, 1],
            1,
            usage,
            false,
        );
        let c = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 4, 1],
            1,
            usage,
            false,
        );
        let before = b.slow_paths().clone();
        let at = |texture| TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let extent = Extent {
            width: 4,
            height: 4,
            depth_or_layers: 1,
        };
        b.copy_texture(&at(&a), &at(&c), extent);
        let counted = [SlowPath::CopyThroughBuffer, SlowPath::HostCopyReadBack]
            .map(|s| b.slow_paths().count(s) - before.count(s));
        println!(
            "{format:?}: exact float blits {}, counted {counted:?}",
            b.exact_float_blits()
        );
        assert_eq!(
            b.exact_float_blits(),
            counted == [0, 0],
            "{format:?}: counted {counted:?}"
        );
        b.destroy(Object::Texture(a));
        b.destroy(Object::Texture(c));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Reads back the first layers of `texture` (of `format`, mip 0): directly, or where the format
/// does not read back (16-bit floats), through a copy into its unsigned integer twin.
fn read_typeless<B: Backend>(
    b: &mut B,
    texture: &B::Texture,
    format: Format,
    size: [u32; 3],
) -> Vec<u8> {
    use crate::kit;
    let texel = format.block().2 as usize;
    if b.caps().formats.get(format).contains(FormatUse::COPY_OUT) {
        return kit::read(b, texture, 0, 0, size, texel, false);
    }
    if matches!(format, Format::Rgb10a2Unorm | Format::Rgb10a2Uint) {
        return read_packed(b, texture, format, size);
    }
    let twin = twin(format).unwrap_or_else(|| panic!("{format:?} has no twin to read through"));
    let usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let t = kit::texture(b, TextureDimension::D2, twin, size, 1, usage, false);
    let extent = Extent {
        width: size[0],
        height: size[1],
        depth_or_layers: size[2],
    };
    b.copy_texture(
        &layer_at(texture, 0, 0, 0, false),
        &layer_at(&t, 0, 0, 0, false),
        extent,
    );
    let got = kit::read(b, &t, 0, 0, size, texel, false);
    b.destroy(Object::Texture(t));
    got
}

/// Loads one layer of a 10-10-10-2 2D array texture as its fields: integers as they are,
/// normalised ones scaled back to integers.
const PACKED_LOAD: &str = "
@group(0) @binding(32) var u: texture_2d_array<u32>;
@group(0) @binding(33) var f: texture_2d_array<f32>;
@group(0) @binding(0) var<uniform> l: vec4<u32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs_uint(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    return textureLoad(u, vec2<i32>(p.xy), i32(l.x), 0);
}
@fragment fn fs_unorm(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let v = textureLoad(f, vec2<i32>(p.xy), i32(l.x), 0);
    return vec4<u32>(round(v * vec4<f32>(1023.0, 1023.0, 1023.0, 3.0)));
}
";

/// Reads a 10-10-10-2 2D array texture, which GL does not read back, by loading each layer in a
/// program and packing the fields into the format's words again.
fn read_packed<B: Backend>(
    b: &mut B,
    texture: &B::Texture,
    format: Format,
    size: [u32; 3],
) -> Vec<u8> {
    use crate::kit::{self, Kit};
    let k = Kit::new(b);
    let (fs, binding) = match format {
        Format::Rgb10a2Uint => ("fs_uint", 32),
        _ => ("fs_unorm", 33),
    };
    let program = kit::program(b, PACKED_LOAD, "vs", fs);
    let view = kit::view(b, texture, format, ViewDimension::D2Array, 0, 1, 0, size[2]);
    let params = b
        .create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            intent: MapIntent::Often,
        })
        .expect("create_buffer");
    let set = b
        .create_binding_set(&[
            BindingEntry {
                binding,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            },
            BindingEntry {
                binding: 0,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &params,
                    offset: 0,
                    size: 16,
                }),
            },
        ])
        .expect("create_binding_set");
    let f = Format::Rgba32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(
        b,
        TextureDimension::D2,
        f,
        [size[0], size[1], 1],
        1,
        usage,
        false,
    );
    let out_view = kit::target(b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, size[0] as f32, size[1] as f32)];
    let mut packed = Vec::new();
    for layer in 0..size[2] {
        b.write_buffer(&params, 0, &words(&[layer, 0, 0, 0]), WriteMode::Discard);
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got = kit::read(b, &out, 0, 0, [size[0], size[1], 1], 16, false);
        for t in got.as_chunks::<16>().0 {
            let c = |i: usize| u32::from_le_bytes(t[i * 4..i * 4 + 4].try_into().unwrap());
            let word = c(0) | c(1) << 10 | c(2) << 20 | c(3) << 30;
            packed.extend(word.to_le_bytes());
        }
    }
    b.destroy(Object::BindingSet(set));
    b.destroy(Object::TextureView(out_view));
    b.destroy(Object::Texture(out));
    b.destroy(Object::TextureView(view));
    b.destroy(Object::Buffer(params));
    packed
}

/// A face of a cube copied whole, and a region of another face, into 2D textures of another format
/// of the group keep every byte.
pub fn cube_faces_copied_between_typeless_formats<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let pairs = [
        (Format::R32Float, Format::R32Uint),
        (Format::Rgba32Uint, Format::Rgba32Float),
        (Format::Rg16Float, Format::Rg16Uint),
        (Format::Rgba16Uint, Format::Rgba16Float),
        (Format::R8Uint, Format::R8Sint),
        (Format::Rg16Sint, Format::Rg16Uint),
        (Format::Rgba8Unorm, Format::Rgba8Uint),
        (Format::Rgba8Sint, Format::Rgba8Unorm),
        (Format::Depth32Float, Format::R32Float),
        (Format::Depth16Unorm, Format::R16Uint),
        (Format::Rgba16Float, Format::Rgba16Float),
        (Format::R32Float, Format::R32Float),
    ];
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let face = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 1,
    };
    for (i, (from, to)) in pairs.into_iter().enumerate() {
        let texel = from.block().2 as usize;
        let cube = kit::texture(
            &mut b,
            TextureDimension::D2,
            from,
            [4, 4, 6],
            1,
            usage,
            true,
        );
        let bytes = if from.is_depth_stencil() {
            depth_values(from, 96, 0x2545_f491 ^ i as u32)
        } else {
            typeless_bytes(from, 96, 0x2545_f491 ^ i as u32)
        };
        b.write_texture(
            &layer_at(&cube, 0, 0, 0, false),
            Extent {
                depth_or_layers: 6,
                ..face
            },
            &bytes,
            4 * texel as u32,
            4,
            WriteMode::Ordered,
        );
        let flat = kit::texture(&mut b, TextureDimension::D2, to, [4, 4, 2], 1, usage, false);
        b.write_texture(
            &layer_at(&flat, 0, 0, 0, false),
            Extent {
                depth_or_layers: 2,
                ..face
            },
            &vec![0; 32 * texel],
            4 * texel as u32,
            4,
            WriteMode::Ordered,
        );
        // Face 3 whole into layer 0; two by three texels of face 5 from (2, 1) to (1, 0) of layer 1.
        b.copy_texture(
            &layer_at(&cube, 3, 0, 0, false),
            &layer_at(&flat, 0, 0, 0, false),
            face,
        );
        b.copy_texture(
            &layer_at(&cube, 5, 2, 1, false),
            &layer_at(&flat, 1, 1, 0, false),
            Extent {
                width: 2,
                height: 3,
                depth_or_layers: 1,
            },
        );
        let got = read_typeless(&mut b, &flat, to, [4, 4, 2]);
        let mut want = vec![0u8; 32 * texel];
        want[..16 * texel].copy_from_slice(&bytes[3 * 16 * texel..4 * 16 * texel]);
        for y in 0..3 {
            for x in 0..2 {
                let s = (5 * 16 + (y + 1) * 4 + x + 2) * texel;
                let d = (16 + y * 4 + x + 1) * texel;
                want[d..d + texel].copy_from_slice(&bytes[s..s + texel]);
            }
        }
        assert_eq!(
            got,
            want,
            "{from:?} cube to {to:?} (device {:?})",
            b.status()
        );
        b.destroy(Object::Texture(cube));
        b.destroy(Object::Texture(flat));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Copies between regions of one subresource that do not overlap (the interface allows them; GL
/// refuses them as feedback loops), on every copy route: copyTexSubImage, the pixel pack buffer,
/// the draw, the depth blit and the host copy; in a 2D texture, a layer of an array and a 3D slice.
pub fn copies_within_one_subresource<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let cases = [
        (Format::Rgba8Unorm, TextureDimension::D2, [8, 4, 1]),
        (Format::R32Uint, TextureDimension::D2, [8, 4, 3]),
        (Format::Rgba8Unorm, TextureDimension::D3, [8, 4, 2]),
        (Format::R32Float, TextureDimension::D2, [8, 4, 2]),
        (Format::R32Float, TextureDimension::D3, [8, 4, 3]),
        (Format::Rg16Float, TextureDimension::D2, [8, 4, 1]),
        (Format::Rgb32Float, TextureDimension::D2, [8, 4, 1]),
        (Format::Depth32Float, TextureDimension::D2, [8, 4, 2]),
        (Format::Depth24Plus, TextureDimension::D2, [8, 4, 1]),
        (Format::Rgba16Float, TextureDimension::D3, [8, 4, 2]),
        (Format::R16Float, TextureDimension::D2, [8, 4, 2]),
    ];
    let before = b.slow_paths().count(SlowPath::CopyWithinTexture);
    for (n, (format, dimension, size)) in cases.into_iter().enumerate() {
        let texel = format.block().2 as usize;
        let t = kit::texture(&mut b, dimension, format, size, 1, usage, false);
        let count = (size[0] * size[1] * size[2]) as usize;
        let bytes = if format.is_depth_stencil() {
            depth_values(format, count, 0x7a3d_1c55 ^ n as u32)
        } else {
            typeless_bytes(format, count, 0x7a3d_1c55 ^ n as u32)
        };
        let all = Extent {
            width: size[0],
            height: size[1],
            depth_or_layers: size[2],
        };
        let corner = TextureRegion {
            texture: &t,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        b.write_texture(
            &corner,
            all,
            &bytes,
            size[0] * texel as u32,
            size[1],
            WriteMode::Ordered,
        );
        let read = |b: &mut H::Backend| {
            if format.is_depth_stencil() {
                kit::read(b, &t, 0, 0, size, texel, false)
            } else {
                read_typeless(b, &t, format, size)
            }
        };
        assert_eq!(read(&mut b), bytes, "{format:?} {dimension:?} as written");
        // Three by three texels from (0, 1) to (4, 0) of the last layer or slice.
        let last = size[2] - 1;
        let at = |x, y| match dimension {
            TextureDimension::D3 => TextureRegion {
                origin: Origin { x, y, z: last },
                ..corner
            },
            _ => TextureRegion {
                layer: last,
                origin: Origin { x, y, z: 0 },
                ..corner
            },
        };
        b.copy_texture(
            &at(0, 1),
            &at(4, 0),
            Extent {
                width: 3,
                height: 3,
                depth_or_layers: 1,
            },
        );
        let got = read(&mut b);
        let mut want = bytes.clone();
        let plane = (size[0] * size[1]) as usize * last as usize;
        for y in 0..3 {
            for x in 0..3 {
                let s = (plane + (y + 1) * 8 + x) * texel;
                let d = (plane + y * 8 + x + 4) * texel;
                want[d..d + texel].copy_from_slice(&bytes[s..s + texel]);
            }
        }
        assert_eq!(
            got,
            want,
            "{format:?} {dimension:?} (device {:?})",
            b.status()
        );
        b.destroy(Object::Texture(t));
    }
    // Every copy but that of the texture kept on the host, which reads the region before it writes.
    assert_eq!(
        b.slow_paths().count(SlowPath::CopyWithinTexture) - before,
        cases.len() as u64 - 1,
        "each copy through a texture counted"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Stored depth values of `format` in its buffer layout (16-bit unorm; 24-bit unorm in the low bits of
/// a word; 32-bit floats from 0 to 1, as draws can write them), one per texel, 0 and 1 among them.
fn depth_values(format: Format, texels: usize, seed: u32) -> Vec<u8> {
    let raw = pattern(texels * 4, seed);
    let words = raw
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w));
    let (one, bytes) = match format {
        Format::Depth16Unorm | Format::R16Unorm => (65535u32, 2),
        Format::Depth24Plus | Format::Depth24PlusStencil8 => (16_777_215, 4),
        _ => (0, 4),
    };
    words
        .enumerate()
        .flat_map(|(i, w)| {
            let v = match (i, one) {
                (0, 0) => 0,
                (1, 0) => 1f32.to_bits(),
                (_, 0) => ((w >> 8) as f32 / 16_777_216.0).to_bits(),
                (0, _) => 0,
                (1, one) => one,
                (_, one) => w & one,
            };
            v.to_le_bytes().into_iter().take(bytes)
        })
        .collect()
}

/// Level 0 of `texture` from layer `layer` and texel (`x`, `y`), the stencil aspect with `stencil`.
fn layer_at<B: Backend>(
    texture: &B::Texture,
    layer: u32,
    x: u32,
    y: u32,
    stencil: bool,
) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip: 0,
        layer,
        origin: Origin { x, y, z: 0 },
        stencil,
    }
}

/// Depth and stencil written by the host and copied from buffers read back as written: each aspect
/// of two layers of a texture of every depth format (a 2D array, and faces 3 and 4 of a cube)
/// written whole, then a part of each layer copied in from a buffer at an offset and a pitch of its
/// own; the other aspect of a combined format stays.
pub fn depth_and_stencil_written_read_back_as_written<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let formats = [
        Format::Depth16Unorm,
        Format::Depth24Plus,
        Format::Depth32Float,
        Format::Depth24PlusStencil8,
        Format::Depth32FloatStencil8,
    ];
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let whole = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 2,
    };
    let shapes = formats
        .into_iter()
        .enumerate()
        .flat_map(|(n, f)| [(n, f, false), (n + formats.len(), f, true)]);
    let mut wrong = Vec::new();
    for (n, format, cube) in shapes {
        if !b.caps().formats.get(format).contains(FormatUse::COPY_OUT) {
            continue;
        }
        let (layers, first) = if cube { (6, 3) } else { (2, 0) };
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 4, layers],
            1,
            usage,
            cube,
        );
        let combined = format.kind() == FormatKind::DepthStencil;
        let aspects: &[bool] = if combined { &[false, true] } else { &[false] };
        let mut want = Vec::new();
        for &stencil in aspects {
            let bytes = if stencil {
                1
            } else {
                format.block().2.min(4) as usize
            };
            let seed = 0x51ed_270b ^ (n as u32 * 2 + stencil as u32);
            let all = if stencil {
                pattern(32, seed)
            } else {
                depth_values(format, 32, seed)
            };
            b.write_texture(
                &layer_at(&t, first, 0, 0, stencil),
                whole,
                &all,
                4 * bytes as u32,
                4,
                WriteMode::Ordered,
            );
            // Two by two texels at (1, 2) of each layer, from a buffer whose rows are three texels
            // apart, starting at its third texel.
            let part = if stencil {
                pattern(3 * 4 * bytes, seed ^ 0xffff)
            } else {
                depth_values(format, 3 * 4, seed ^ 0xffff)
            };
            let mut padded = vec![0u8; 2 * bytes];
            padded.extend_from_slice(&part);
            let src = buffer(&mut b, BufferUsage::empty(), &padded);
            b.copy_buffer_to_texture(
                &CopyBufferTexture {
                    buffer: &src,
                    offset: 2 * bytes as u64,
                    row_pitch: 3 * bytes as u32,
                    rows_per_layer: 2,
                },
                &layer_at(&t, first, 1, 2, stencil),
                Extent {
                    width: 2,
                    height: 2,
                    depth_or_layers: 2,
                },
            );
            let mut expected = all;
            for layer in 0..2 {
                for y in 0..2 {
                    for x in 0..2 {
                        let d = ((layer * 16 + (y + 2) * 4 + x + 1) * bytes)..;
                        let s = ((layer * 6 + y * 3 + x) * bytes)..;
                        expected[d.start..d.start + bytes]
                            .copy_from_slice(&part[s.start..s.start + bytes]);
                    }
                }
            }
            want.push((stencil, bytes, expected));
            b.destroy(Object::Buffer(src));
        }
        for (stencil, bytes, expected) in want {
            // Both layers in one read, then each alone.
            let reads = [(0, 2), (0, 1), (1, 1)];
            for (layer, count) in reads {
                let got = kit::read(&mut b, &t, 0, first + layer, [4, 4, count], bytes, stencil);
                let at = layer as usize * 16 * bytes;
                if got != expected[at..at + got.len()] {
                    wrong.push(format!(
                        "{format:?}{} {}, {count} layers from {layer}: {got:?}, expected {:?}",
                        if cube { " cube" } else { "" },
                        if stencil { "stencil" } else { "depth" },
                        &expected[at..at + got.len()]
                    ));
                }
            }
        }
        b.destroy(Object::Texture(t));
    }
    let status = b.status();
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(status, DeviceStatus::Ok);
}

/// Copies between a buffer used only as indices and one used only for other data, both ways: WebGL
/// copies between GL buffers of one type only.
pub fn copies_between_index_and_data_buffers<H: Host>() {
    let mut b = H::make();
    let indices = pattern(64, 0x6c07_8965);
    let data = pattern(64, 0x2f6b_6f05);
    let index = buffer(&mut b, BufferUsage::INDEX, &indices);
    let other = buffer(&mut b, BufferUsage::VERTEX, &data);
    b.copy_buffer(&index, 4, &other, 20, 24);
    let mut want = data.clone();
    want[20..44].copy_from_slice(&indices[4..28]);
    assert_eq!(read_buffer(&mut b, &other, 64), want, "indices into data");
    b.copy_buffer(&other, 0, &index, 32, 32);
    let mut back = indices.clone();
    back[32..64].copy_from_slice(&want[0..32]);
    assert_eq!(read_buffer(&mut b, &index, 64), back, "data into indices");
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Copies between a colour and a depth format of one typeless group (`r32float` and
/// `depth32float`, `r16unorm` and `depth16unorm`) keep every value, both ways: a part of each layer
/// of a two-layer texture is copied into the other format at another place and back.
pub fn copies_between_depth_and_colour_formats<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let whole = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 2,
    };
    let part = Extent {
        width: 3,
        height: 2,
        depth_or_layers: 2,
    };
    for (i, (colour, depth)) in [
        (Format::R32Float, Format::Depth32Float),
        (Format::R16Unorm, Format::Depth16Unorm),
    ]
    .into_iter()
    .enumerate()
    {
        let caps = &b.caps().formats;
        if !caps.get(colour).contains(FormatUse::COPY_OUT)
            || !caps.get(depth).contains(FormatUse::COPY_OUT)
        {
            continue;
        }
        let bytes = colour.block().2 as usize;
        for (from, to) in [(colour, depth), (depth, colour)] {
            let src = kit::texture(
                &mut b,
                TextureDimension::D2,
                from,
                [4, 4, 2],
                1,
                usage,
                false,
            );
            let dst = kit::texture(&mut b, TextureDimension::D2, to, [4, 4, 2], 1, usage, false);
            let values = depth_values(colour, 32, 0x2545_f491 ^ i as u32 ^ (from == depth) as u32);
            let zeros = depth_values(colour, 1, 0).repeat(32);
            b.write_texture(
                &layer_at(&src, 0, 0, 0, false),
                whole,
                &values,
                4 * bytes as u32,
                4,
                WriteMode::Ordered,
            );
            b.write_texture(
                &layer_at(&dst, 0, 0, 0, false),
                whole,
                &zeros,
                4 * bytes as u32,
                4,
                WriteMode::Ordered,
            );
            // Three by two texels from (1, 1) to (0, 2), in both layers.
            b.copy_texture(
                &layer_at(&src, 0, 1, 1, false),
                &layer_at(&dst, 0, 0, 2, false),
                part,
            );
            let got = kit::read(&mut b, &dst, 0, 0, [4, 4, 2], bytes, false);
            let status = b.status();
            let mut want = zeros;
            for layer in 0..2 {
                for y in 0..2 {
                    for x in 0..3 {
                        let at = |x: usize, y: usize| (layer * 16 + y * 4 + x) * bytes;
                        let (s, d) = (at(x + 1, y + 1), at(x, y + 2));
                        want[d..d + bytes].copy_from_slice(&values[s..s + bytes]);
                    }
                }
            }
            assert_eq!(
                got,
                want,
                "{from:?} to {to:?} (device {status:?}: {:?})",
                b.slow_paths()
            );
            b.destroy(Object::Texture(src));
            b.destroy(Object::Texture(dst));
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Loads the texel at the fragment from level `l` of the view, and asks that level's width.
const LEVEL_LOAD: &str = "
@group(0) @binding(32) var t: texture_2d<u32>;
@group(0) @binding(0) var<uniform> l: vec4<u32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let level = i32(l.x);
    return vec4<u32>(textureLoad(t, vec2<i32>(p.xy), level).x, textureDimensions(t, level).x, 0u, 0u);
}
@fragment fn fs_size() -> @location(0) vec4<u32> {
    return vec4<u32>(255u, textureDimensions(t, i32(l.x)).x, 0u, 0u);
}
";

/// Loads and sizes count levels from the view's first, as WGSL's `textureLoad` and
/// `textureDimensions` do: each level of an `rgba8uint` texture holds its own number, and views
/// starting at each level are loaded and sized at each of their levels, also by a program that only
/// sizes the texture.
pub fn loads_count_levels_from_the_view<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LEVEL_LOAD, "vs", "fs");
    let sizing = kit::program(&mut b, LEVEL_LOAD, "vs", "fs_size");
    let f = Format::Rgba8Uint;
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 3, usage, false);
    for mip in 0..3u32 {
        let n = 4 >> mip;
        b.write_texture(
            &TextureRegion {
                texture: &t,
                mip,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: n,
                height: n,
                depth_or_layers: 1,
            },
            &vec![mip as u8; (n * n * 4) as usize],
            n * 4,
            n,
            WriteMode::Ordered,
        );
    }
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(&mut b, TextureDimension::D2, f, [1, 1, 1], 1, usage, false);
    let out_view = kit::target(&mut b, &out, f, 0, 0);
    let params = b
        .create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            intent: MapIntent::Often,
        })
        .expect("create_buffer");
    let vp = [kit::viewport(0.0, 0.0, 1.0, 1.0)];
    for base in 0..3u32 {
        let view = kit::view(&mut b, &t, f, ViewDimension::D2, base, 3 - base, 0, 1);
        let set = b
            .create_binding_set(&[
                BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&view),
                },
                BindingEntry {
                    binding: 0,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &params,
                        offset: 0,
                        size: 16,
                    }),
                },
            ])
            .expect("create_binding_set");
        for level in 0..3 - base {
            let mut word = [0u8; 16];
            word[0] = level as u8;
            b.write_buffer(&params, 0, &word, WriteMode::Discard);
            let at = base + level;
            for (p, first, what) in [(&program, at as u8, "loaded"), (&sizing, 255, "sized only")] {
                b.begin_pass(&kit::pass(&out_view, [9; 4], None));
                b.draw(&kit::call(
                    &k,
                    p,
                    Some(&set),
                    &vp,
                    Topology::TriangleList,
                    3,
                ));
                b.end_pass();
                let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
                assert_eq!(
                    got,
                    vec![first, 4 >> at, 0, 0],
                    "level {level} of the view from level {base}, {what}: its number and width"
                );
            }
        }
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(view));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A triangle covering the target, writing 1.
const WRITE_ONE: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<u32> {
    return vec4<u32>(1u);
}
";

/// Vertices 1, 2 and 3 (counted mod 4) make a triangle over the whole target; vertex 0 is a point
/// at its corner.
const LARGEST: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 4>(vec2<f32>(-1.0, -1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0),
                                vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[v % 4u], 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<u32> {
    return vec4<u32>(1u);
}
";

/// In a draw that does not restart, the largest index value is an ordinary vertex, which GL ES
/// (always restarting there) would drop: lists and fans holding it draw their triangle, for 8, 16
/// and 32-bit indices, written by the host or copied on the GPU (`RestartRewrite`, counted only
/// where a write may have put the value in the draw's range). 32-bit indices holding both 0 and
/// the largest value lose the device as not built.
pub fn lists_draw_the_largest_index_as_a_vertex<H: Host>() {
    use crate::kit::{self, Kit};
    let mut formats = vec![IndexFormat::U16, IndexFormat::U32];
    let mut b = H::make();
    if b.caps().has(Features::INDEX_U8) {
        formats.insert(0, IndexFormat::U8);
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LARGEST, "vs", "fs");
    let f = Format::R32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let v = kit::target(&mut b, &t, f, 0, 0);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    let covered = |b: &mut H::Backend| {
        let got = kit::read(b, &t, 0, 0, [4, 4, 1], 4, false);
        got.as_chunks::<4>().0.iter().filter(|p| p[0] == 1).count()
    };
    let rewrites = |b: &mut H::Backend| b.slow_paths().count(SlowPath::RestartRewrite);
    for format in formats {
        let width = format.size() as usize;
        let largest = format.restart_value();
        let bytes = |values: &[u32]| {
            let mut out: Vec<u8> = values
                .iter()
                .flat_map(|v| v.to_le_bytes()[..width].to_vec())
                .collect();
            out.resize(out.len().next_multiple_of(4), 0);
            out
        };
        // A triangle through the largest value, then one without it.
        let indices = bytes(&[1, 2, largest, 1, 2, 3]);
        let host = buffer(&mut b, BufferUsage::INDEX, &indices);
        let data = buffer(&mut b, BufferUsage::VERTEX, &indices);
        let both = BufferUsage::INDEX | BufferUsage::VERTEX;
        let copied = buffer(&mut b, both, &vec![0; indices.len()]);
        b.copy_buffer(&data, 0, &copied, 0, indices.len() as u64);
        let draw = |b: &mut H::Backend, ib: &<H::Backend as Backend>::Buffer, first: u32| {
            b.begin_pass(&kit::pass(&v, [0; 4], None));
            b.draw(&DrawCall {
                index: Some(IndexBinding {
                    buffer: ib,
                    offset: 0,
                    format,
                }),
                range: DrawRange::Indexed {
                    first,
                    count: 3,
                    base_vertex: 0,
                    first_instance: 0,
                    instances: 1,
                },
                ..kit::call(&k, &program, None, &whole, Topology::TriangleList, 3)
            });
            b.end_pass();
        };
        for (ib, name) in [
            (&host, "written by the host"),
            (&copied, "copied on the GPU"),
        ] {
            let before = rewrites(&mut b);
            draw(&mut b, ib, 0);
            assert_eq!(
                covered(&mut b),
                16,
                "{format:?} list through the largest value, {name}"
            );
            assert_eq!(rewrites(&mut b), before + 1, "{format:?} {name}");
            draw(&mut b, ib, 3);
            assert_eq!(covered(&mut b), 16, "{format:?} list without it, {name}");
        }
        let before = rewrites(&mut b);
        draw(&mut b, &host, 3);
        assert_eq!(
            rewrites(&mut b),
            before,
            "{format:?}: a range known to lack the value draws as it is"
        );
        // The largest value made by a host write of its last byte only.
        let top = 3 * width - 1;
        let mut partial = indices.clone();
        partial[top] = 0;
        let ib = buffer(&mut b, BufferUsage::INDEX, &partial);
        b.write_buffer(&ib, top as u64, &[0xff], WriteMode::Ordered);
        draw(&mut b, &ib, 0);
        assert_eq!(
            covered(&mut b),
            16,
            "{format:?}: the largest value made by a write of its last byte"
        );
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        let source = IndexSource::Guest {
            indices: &indices[..3 * width],
            format,
            base_vertex: 0,
        };
        let fan = kit::call(&k, &program, None, &whole, Topology::TriangleFan, 3);
        b.draw_fan_or_loop(
            &fan,
            FanOrLoop::TriangleFan,
            source,
            ProvokingVertex::Last,
            0,
            1,
        )
        .expect("draw_fan_or_loop");
        b.end_pass();
        assert_eq!(
            covered(&mut b),
            16,
            "{format:?} fan through the largest value"
        );
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
    }
    let indices = words(&[0, 2, u32::MAX, 0]);
    let ib = buffer(&mut b, BufferUsage::INDEX, &indices);
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&DrawCall {
        index: Some(IndexBinding {
            buffer: &ib,
            offset: 0,
            format: IndexFormat::U32,
        }),
        range: DrawRange::Indexed {
            first: 0,
            count: 3,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        },
        ..kit::call(&k, &program, None, &whole, Topology::TriangleList, 3)
    });
    b.end_pass();
    assert_ne!(
        b.status(),
        DeviceStatus::Ok,
        "32-bit indices holding 0 and 2^32 - 1"
    );
}

/// Without `FRACTIONAL_VIEWPORT` a draw with a viewport at a fraction of a pixel breaks the
/// interface's rule: the device is lost rather than the viewport rounded.
pub fn fractional_viewports_are_refused<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if b.caps().has(Features::FRACTIONAL_VIEWPORT) {
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, WRITE_ONE, "vs", "fs");
    let f = Format::R32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let v = kit::target(&mut b, &t, f, 0, 0);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    let fraction = [kit::viewport(0.5, 0.0, 3.0, 4.0)];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&kit::call(
        &k,
        &program,
        None,
        &whole,
        Topology::TriangleList,
        3,
    ));
    assert_eq!(b.status(), DeviceStatus::Ok, "a whole-pixel viewport draws");
    b.draw(&kit::call(
        &k,
        &program,
        None,
        &fraction,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    assert_eq!(b.status(), DeviceStatus::Lost(LossReason::Error));
}

/// A viewport at the interface's bounds (with `m` the largest 2D texture: x at -2m, x + width at
/// 2m - 1) draws; one past them, or with depths outside 0 to 1 or reversed, breaks the rule and
/// loses the device rather than reaching GL, which would clamp it.
pub fn viewports_past_their_bounds_are_refused<H: Host>() {
    use crate::kit::{self, Kit};
    let m = H::make().caps().limits.max_texture_2d as f32;
    let depths = |min: f32, max: f32| Viewport {
        min_depth: min.to_bits(),
        max_depth: max.to_bits(),
        ..kit::viewport(0.0, 0.0, 4.0, 4.0)
    };
    let cases = [
        (kit::viewport(-2.0 * m, 0.0, m, 4.0), true),
        (kit::viewport(m - 1.0, m - 1.0, m, m), true),
        (depths(0.25, 0.25), true),
        (kit::viewport(-2.0 * m - 1.0, 0.0, 4.0, 4.0), false),
        (kit::viewport(m, 0.0, m, 4.0), false),
        (kit::viewport(0.0, 0.0, m + 1.0, 4.0), false),
        (kit::viewport(0.0, 0.0, 4.0, -1.0), false),
        (depths(0.0, 1.5), false),
        (depths(0.75, 0.25), false),
    ];
    for (vp, takes) in cases {
        let mut b = H::make();
        let k = Kit::new(&mut b);
        let program = kit::program(&mut b, WRITE_ONE, "vs", "fs");
        let f = Format::R32Uint;
        let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
        let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
        let v = kit::target(&mut b, &t, f, 0, 0);
        let vps = [vp];
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            None,
            &vps,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let want = if takes {
            DeviceStatus::Ok
        } else {
            DeviceStatus::Lost(LossReason::Error)
        };
        assert_eq!(b.status(), want, "the viewport {vp:?}");
    }
}

/// Loads texel (0, 0) of level 0 of two views and adds them.
const TWO_LOADS: &str = "
@group(0) @binding(32) var a: texture_2d<u32>;
@group(0) @binding(33) var c: texture_2d<u32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<u32> {
    return textureLoad(a, vec2<i32>(0), 0) + textureLoad(c, vec2<i32>(0), 0);
}
";

/// Without `TEXTURE_VIEWS` (WebGL 2 samples a GL texture whole, its mip range set on the texture):
/// binding a view of one layer of an array, or of one face of a cube, is `Absent(TEXTURE_VIEWS)`; a
/// draw reading one texture through two mip ranges breaks the interface's rule and loses the device;
/// through one range twice, it draws.
pub fn sampled_views_are_whole_textures_of_one_mip_range<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    assert!(!b.caps().has(Features::TEXTURE_VIEWS));
    let f = Format::Rgba8Uint;
    let usage = TextureUsage::SAMPLE | TextureUsage::RENDER | TextureUsage::COPY_DST;
    let array = kit::texture(&mut b, TextureDimension::D2, f, [2, 2, 3], 1, usage, false);
    let cube = kit::texture(&mut b, TextureDimension::D2, f, [2, 2, 6], 1, usage, true);
    let part = |dimension, layer, layers| TextureViewDesc {
        format: f,
        dimension,
        base_mip: 0,
        mip_count: 1,
        base_layer: layer,
        layer_count: layers,
        stencil: false,
    };
    for (texture, desc, what) in [
        (
            &array,
            part(ViewDimension::D2, 1, 1),
            "one layer of an array",
        ),
        (&cube, part(ViewDimension::D2, 2, 1), "one face of a cube"),
        (
            &array,
            part(ViewDimension::D2Array, 0, 2),
            "two of three layers",
        ),
        (
            &cube,
            part(ViewDimension::D2Array, 0, 6),
            "a cube as an array",
        ),
    ] {
        // Refused when the view is made, or when it is bound.
        let got = b.create_view(texture, &desc).and_then(|view| {
            b.create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .map(drop)
        });
        assert!(
            matches!(got, Err(GpuError::Absent(f)) if f == Features::TEXTURE_VIEWS),
            "{what}: {got:?}"
        );
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, TWO_LOADS, "vs", "fs");
    let t = kit::texture(&mut b, TextureDimension::D2, f, [2, 2, 1], 2, usage, false);
    for mip in 0..2u32 {
        let n = 2 >> mip;
        b.write_texture(
            &TextureRegion {
                texture: &t,
                mip,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: n,
                height: n,
                depth_or_layers: 1,
            },
            &vec![10 + mip as u8; (n * n * 4) as usize],
            n * 4,
            n,
            WriteMode::Ordered,
        );
    }
    let out_usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [1, 1, 1],
        1,
        out_usage,
        false,
    );
    let out_view = kit::target(&mut b, &out, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 1.0, 1.0)];
    let level = |b: &mut H::Backend, mip| kit::view(b, &t, f, ViewDimension::D2, mip, 1, 0, 1);
    let (level0, level0_again, level1) = (level(&mut b, 0), level(&mut b, 0), level(&mut b, 1));
    for (second, sum, lost) in [(&level0_again, 20, false), (&level1, 0, true)] {
        let set = b
            .create_binding_set(&[
                BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&level0),
                },
                BindingEntry {
                    binding: 33,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(second),
                },
            ])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        if lost {
            assert_eq!(
                b.status(),
                DeviceStatus::Lost(LossReason::Error),
                "two mip ranges"
            );
        } else {
            let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
            assert_eq!(got, vec![sum; 4], "one mip range twice");
        }
    }
}

/// A `bgra8unorm` surface presents its frame: cleared to two colours in turn, the window shows each
/// (the hosts read the window's bytes, which are RGBA).
pub fn bgra_surfaces_present<H: Host>() {
    let mut b = H::make();
    assert!(
        b.caps()
            .formats
            .get(Format::Bgra8Unorm)
            .contains(FormatUse::PRESENT)
    );
    let window = H::window(&mut b);
    let surface = b.create_surface(window).expect("create_surface");
    b.configure_surface(&surface, Format::Bgra8Unorm, 4, 4, true)
        .expect("configure_surface");
    for (value, texel) in [
        ([0.2f32, 0.4, 0.6, 1.0], [51u8, 102, 153, 255]),
        ([1.0, 0.0, 0.2, 1.0], [255, 0, 51, 255]),
    ] {
        let v = b.current_texture(&surface).expect("current_texture");
        b.begin_pass(&PassDesc {
            colors: core::array::from_fn(|i| {
                (i == 0).then_some(ColorAttachment {
                    view: &v,
                    depth_slice: 0,
                    load: LoadOp::Clear(value.map(f32::to_bits)),
                    store: StoreOp::Store,
                    resolve: None,
                })
            }),
            depth: None,
            occlusion: None,
            empty: None,
        });
        b.end_pass();
        b.present(&surface).expect("present");
        assert_eq!(H::shown(&mut b, window), texel.repeat(16), "{value:?}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Loads texel (0, 0) of a float texture into an `rgba32float` target.
const LOAD_CORNER_FLOAT: &str = "
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(0, 0), 0);
}
";

/// The signed normalised formats follow EXT_render_snorm: with it they render and blend, and a
/// clear to -1, 0, 1 and 64 steps loads back as those values. With or without it they sample and
/// filter; without it they copy out from their host copy, with it not (`readPixels` returns -128
/// as -127, and draws write texels the host copy lacks).
pub fn signed_normalised_formats_follow_their_extension<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let has = b
        .caps()
        .formats
        .get(F::Rgba8Snorm)
        .contains(FormatUse::RENDER);
    let mut formats = vec![F::R8Snorm, F::Rg8Snorm, F::Rgba8Snorm];
    if b.caps().formats.get(F::Rgba16Snorm) != FormatUse::empty() {
        formats.extend([F::R16Snorm, F::Rg16Snorm, F::Rgba16Snorm]);
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LOAD_CORNER_FLOAT, "vs", "fs");
    let out_usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let f32x4 = F::Rgba32Float;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        f32x4,
        [1, 1, 1],
        1,
        out_usage,
        false,
    );
    let out_view = kit::target(&mut b, &out, f32x4, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 1.0, 1.0)];
    let usage = TextureUsage::SAMPLE | TextureUsage::RENDER;
    for format in formats {
        let uses = b.caps().formats.get(format);
        assert!(
            uses.contains(FormatUse::SAMPLE | FormatUse::FILTER)
                && uses.contains(FormatUse::COPY_OUT) != has,
            "{format:?}: {uses:?}"
        );
        if !has {
            let held = FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT;
            assert_eq!(uses, held, "{format:?}");
            continue;
        }
        assert!(
            uses.contains(FormatUse::RENDER | FormatUse::BLEND),
            "{format:?}: {uses:?}"
        );
        let max = match format {
            F::R16Snorm | F::Rg16Snorm | F::Rgba16Snorm => 32767.0,
            _ => 127.0,
        };
        let value = [-1.0f32, 0.0, 1.0, 64.0 / max];
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [2, 2, 1],
            1,
            usage,
            false,
        );
        let v = kit::target(&mut b, &t, format, 0, 0);
        b.begin_pass(&PassDesc {
            colors: core::array::from_fn(|i| {
                (i == 0).then_some(ColorAttachment {
                    view: &v,
                    depth_slice: 0,
                    load: LoadOp::Clear(value.map(f32::to_bits)),
                    store: StoreOp::Store,
                    resolve: None,
                })
            }),
            depth: None,
            occlusion: None,
            empty: None,
        });
        b.end_pass();
        let view = kit::view(&mut b, &t, format, ViewDimension::D2, 0, 1, 0, 1);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 16, false);
        let got: Vec<f32> = got
            .chunks(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // Channels the format lacks load as 0, 0 and 1.
        let n = (format.block().2 as usize) / if max > 200.0 { 2 } else { 1 };
        let mut want = [0.0, 0.0, 0.0, 1.0];
        want[..n].copy_from_slice(&value[..n]);
        assert_eq!(got, want, "{format:?}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// The 16-bit normalised formats follow EXT_texture_norm16: with it, the unsigned ones sample,
/// filter, render, blend and read back, as stored, now and later, and the signed ones sample and
/// filter; without it, none has a use and making one is refused.
pub fn sixteen_bit_normalised_formats_follow_their_extension<H: Host>() {
    use crate::kit;
    use Format as F;
    let mut b = H::make();
    let has = b.caps().formats.get(F::Rgba16Unorm) != FormatUse::empty();
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    for format in [F::R16Unorm, F::Rg16Unorm, F::Rgba16Unorm] {
        let uses = b.caps().formats.get(format);
        if !has {
            assert_eq!(uses, FormatUse::empty(), "{format:?}");
            continue;
        }
        let want = FormatUse::SAMPLE
            | FormatUse::FILTER
            | FormatUse::RENDER
            | FormatUse::BLEND
            | FormatUse::COPY_OUT;
        assert!(uses.contains(want), "{format:?}: {uses:?}");
        let texel = format.block().2 as usize;
        let bytes = pattern(12 * texel, 0x51ed_270b);
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 3, 1],
            1,
            usage,
            false,
        );
        let region = TextureRegion {
            texture: &t,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let extent = Extent {
            width: 4,
            height: 3,
            depth_or_layers: 1,
        };
        b.write_texture(
            &region,
            extent,
            &bytes,
            4 * texel as u32,
            3,
            WriteMode::Ordered,
        );
        assert_eq!(
            kit::read(&mut b, &t, 0, 0, [4, 3, 1], texel, false),
            bytes,
            "{format:?} now"
        );
        let rb = b
            .read_back(&ReadSource::Texture { region, extent })
            .expect("read_back");
        b.submit();
        b.wait_read(&rb);
        let mut later = vec![0; bytes.len()];
        b.finish_read(rb, &mut later);
        assert_eq!(later, bytes, "{format:?} later");
    }
    for format in [F::R16Snorm, F::Rg16Snorm, F::Rgba16Snorm] {
        let uses = b.caps().formats.get(format);
        if has {
            // Rendering them too needs EXT_render_snorm.
            assert!(
                uses.contains(FormatUse::SAMPLE | FormatUse::FILTER),
                "{format:?}: {uses:?}"
            );
        } else {
            assert_eq!(uses, FormatUse::empty(), "{format:?}");
            let made = b.create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: 4,
                height: 4,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::SAMPLE,
                cube: false,
                reinterpret: false,
            });
            assert!(matches!(made, Err(GpuError::Invalid(_))), "{format:?}");
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// `generate_mips` of formats that render and filter, which this back end fills by
/// `generateMipmap`, equals the interface's draws (`generate_mips_by_draws`) on a texture with the
/// same texels: 2D textures and 2D arrays of even and odd sizes, cube and 3D textures, every level
/// and only the last two (the levels above written with texels of their own), in every such format
/// that reads back. Where they differ, the first such texel is given with the box average of the four
/// it comes from (sides of even length).
pub fn generate_mipmap_equals_the_draws<H: Host>() {
    use crate::kit;
    use Format as F;
    let mut b = H::make();
    let usage = TextureUsage::SAMPLE
        | TextureUsage::RENDER
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    let shapes = [
        (TextureDimension::D2, [16, 16, 1], false),
        (TextureDimension::D2, [13, 7, 1], false),
        (TextureDimension::D2, [13, 7, 2], false),
        (TextureDimension::D2, [16, 8, 2], false),
        (TextureDimension::D2, [8, 8, 6], true),
        (TextureDimension::D2, [7, 7, 6], true),
        (TextureDimension::D3, [8, 8, 4], false),
    ];
    let mut tested = 0;
    let mut failures = Vec::new();
    for format in [
        F::Rgba8Unorm,
        F::Rgba8UnormSrgb,
        F::Bgra8Unorm,
        F::R32Float,
        F::Rgba16Unorm,
        F::R16Unorm,
    ] {
        let need = FormatUse::RENDER | FormatUse::FILTER | FormatUse::COPY_OUT;
        if !b.caps().formats.get(format).contains(need) {
            continue;
        }
        let texel = format.block().2 as usize;
        for (seed, &(dimension, size, cube)) in shapes.iter().enumerate() {
            // Four levels, or the full chain where it is shorter.
            let largest: u32 = size[0]
                .max(size[1])
                .max(if dimension == TextureDimension::D3 {
                    size[2]
                } else {
                    1
                });
            let levels = 4.min(largest.ilog2() + 1);
            let made =
                |b: &mut H::Backend| kit::texture(b, dimension, format, size, levels, usage, cube);
            for first in [1, 2] {
                let (native, drawn) = (made(&mut b), made(&mut b));
                let layers = if dimension == TextureDimension::D3 {
                    1
                } else {
                    size[2]
                };
                let shape = |mip: u32| {
                    let n = |s: u32| (s >> mip).max(1);
                    let depth = if dimension == TextureDimension::D3 {
                        n(size[2])
                    } else {
                        size[2]
                    };
                    [n(size[0]), n(size[1]), depth]
                };
                let mut written = Vec::new();
                for mip in 0..first {
                    let s = shape(mip);
                    let count = (s[0] * s[1] * s[2]) as usize;
                    let mut bytes = pattern(count * texel, 7 + seed as u32 * 31 + mip * 5);
                    if format == F::R32Float {
                        // Values in [0, 1): no NaNs or infinities, which the filters need not agree on.
                        for v in bytes.chunks_mut(4) {
                            let x = u32::from_le_bytes([v[0], v[1], v[2], v[3]]) >> 9;
                            v.copy_from_slice(&(x as f32 / (1 << 23) as f32).to_le_bytes());
                        }
                    }
                    for t in [&native, &drawn] {
                        b.write_texture(
                            &TextureRegion {
                                texture: t,
                                mip,
                                layer: 0,
                                origin: Origin::default(),
                                stencil: false,
                            },
                            Extent {
                                width: s[0],
                                height: s[1],
                                depth_or_layers: s[2],
                            },
                            &bytes,
                            s[0] * texel as u32,
                            s[1],
                            WriteMode::Ordered,
                        );
                    }
                    written.push(bytes);
                }
                let desc = TextureDesc {
                    dimension,
                    format,
                    width: size[0],
                    height: size[1],
                    depth_or_layers: size[2],
                    mip_levels: levels,
                    samples: 1,
                    usage,
                    cube,
                    reinterpret: false,
                };
                let before = b.slow_paths().count(SlowPath::FilterInShader);
                b.generate_mips(&native, &desc, format, first..levels, 0..layers)
                    .expect("generate_mips");
                generate_mips_by_draws(&mut b, &drawn, &desc, format, first..levels, 0..layers)
                    .expect("generate_mips_by_draws");
                assert_eq!(b.slow_paths().count(SlowPath::FilterInShader), before);
                let what =
                    format!("{format:?} {dimension:?} {size:?} cube {cube}, from level {first}");
                for mip in 0..levels {
                    let s = shape(mip);
                    let want = kit::read(&mut b, &drawn, mip, 0, s, texel, false);
                    let got = kit::read(&mut b, &native, mip, 0, s, texel, false);
                    if let Some(bytes) = written.get(mip as usize) {
                        if &got != bytes {
                            failures.push(format!("{what}: generateMipmap changed level {mip}"));
                        }
                        if &want != bytes {
                            failures.push(format!("{what}: the draws changed level {mip}"));
                        }
                    }
                    // Differing texels per layer (per slice in 3D).
                    let layer = (s[0] * s[1]) as usize * texel;
                    let differ: Vec<usize> = got
                        .chunks(layer)
                        .zip(want.chunks(layer))
                        .map(|(g, w)| {
                            g.chunks(texel)
                                .zip(w.chunks(texel))
                                .filter(|(a, b)| a != b)
                                .count()
                        })
                        .collect();
                    if differ.iter().any(|&n| n > 0) {
                        // The first texel that differs, and the box average of the four it
                        // comes from where the level above was written with even sides.
                        let at = got
                            .chunks(texel)
                            .zip(want.chunks(texel))
                            .position(|(a, b)| a != b)
                            .unwrap_or(0);
                        let above = written.get(mip as usize - 1).filter(|_| {
                            let p = shape(mip - 1);
                            p[0] % 2 == 0 && p[1] % 2 == 0 && dimension == TextureDimension::D2
                        });
                        let box_average = above.map(|bytes| {
                            let (w, plane) = (s[0] as usize, (s[0] * s[1]) as usize);
                            let (l, x, y) = (at / plane, at % w, at % plane / w);
                            let p = shape(mip - 1);
                            let base = |dx: usize, dy: usize| {
                                (l * (p[0] * p[1]) as usize
                                    + (2 * y + dy) * p[0] as usize
                                    + 2 * x
                                    + dx)
                                    * texel
                            };
                            (0..texel)
                                .map(|c| {
                                    let sum: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
                                        .iter()
                                        .map(|&(dx, dy)| u32::from(bytes[base(dx, dy) + c]))
                                        .sum();
                                    sum as f32 / 4.0
                                })
                                .collect::<Vec<_>>()
                        });
                        let t = |v: &[u8]| v[at * texel..(at + 1) * texel].to_vec();
                        failures.push(format!(
                            "{what}: level {mip} differs in {differ:?} of {} texels per layer; \
                             texel {at}: generateMipmap {:?}, the draws {:?}, box {box_average:?}",
                            s[0] * s[1],
                            t(&got),
                            t(&want)
                        ));
                    }
                }
                b.destroy(Object::Texture(native));
                b.destroy(Object::Texture(drawn));
                tested += 1;
            }
        }
    }
    assert!(tested > 0, "no format renders, filters and reads back");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// With `TIMESTAMPS`, two timestamps around a draw read back in order (unless the GPU reports a
/// disjoint stretch, which reading clears), and an occlusion set still counts beside them. Without
/// it, timestamp sets, writes and the disjoint flag all answer `Absent(TIMESTAMPS)`.
pub fn timestamps_and_their_disjoint_flag<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let absent = Err(GpuError::Absent(Features::TIMESTAMPS));
    if !b.caps().has(Features::TIMESTAMPS) {
        assert_eq!(
            b.create_query_set(QueryKind::Timestamp, 2).err(),
            absent.err()
        );
        assert_eq!(b.timestamps_disjoint(), absent);
        not_run(Features::TIMESTAMPS);
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, WRITE_ONE, "vs", "fs");
    let f = Format::R32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [64, 64, 1],
        1,
        usage,
        false,
    );
    let v = kit::target(&mut b, &t, f, 0, 0);
    let whole = [kit::viewport(0.0, 0.0, 64.0, 64.0)];
    let set = b
        .create_query_set(QueryKind::Timestamp, 2)
        .expect("create_query_set");
    let _ = b.timestamps_disjoint().expect("timestamps_disjoint");
    b.write_timestamp(&set, 0).expect("write_timestamp");
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    for _ in 0..16 {
        b.draw(&kit::call(
            &k,
            &program,
            None,
            &whole,
            Topology::TriangleList,
            3,
        ));
    }
    b.end_pass();
    b.write_timestamp(&set, 1).expect("write_timestamp");
    let rb = b.read_queries(&set, 0, 2).expect("read_queries");
    b.submit();
    b.wait_read(&rb);
    let mut out = [0u8; 16];
    b.finish_read(rb, &mut out);
    let [t0, t1] = [0, 8].map(|i| u64::from_le_bytes(out[i..i + 8].try_into().unwrap()));
    let disjoint = b.timestamps_disjoint().expect("timestamps_disjoint");
    assert!(
        disjoint || (t0 > 0 && t1 >= t0),
        "timestamps {t0} then {t1}, not disjoint"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Half of a 4 by 4 target, cut by its diagonal through pixel centres, so the pixels on it are partly
/// covered.
const HALF: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0));
    return vec4<f32>(p[v], 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.25, 1.0);
}
";

/// A whole copy between two four-sample `rgba8unorm` textures (`MultisampleCopyResolved` where
/// programs cannot read samples) resolves to the same texels as its source, with some pixels on the
/// diagonal partly covered; resolved at a pass's end and by `resolve`, alike.
pub fn multisampled_copies_resolve_as_their_source<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let f = Format::Rgba8Unorm;
    if !b.caps().formats.takes_samples(f, 4) {
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, HALF, "vs", "fs");
    let samples = |b: &mut H::Backend| {
        b.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: f,
            width: 4,
            height: 4,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 4,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .expect("create_texture")
    };
    let (ms, copy) = (samples(&mut b), samples(&mut b));
    let ms_view = kit::target(&mut b, &ms, f, 0, 0);
    let single = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let [at_end, of_source, of_copy] =
        [0; 3].map(|_| kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, single, false));
    let end_view = kit::target(&mut b, &at_end, f, 0, 0);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    let mut pass = kit::pass(&ms_view, [0; 4], None);
    if let Some(c) = &mut pass.colors[0] {
        c.resolve = Some(&end_view);
    }
    b.begin_pass(&pass);
    b.draw(&kit::call(
        &k,
        &program,
        None,
        &whole,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    let region = |t| TextureRegion {
        texture: t,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    let extent = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 1,
    };
    let before = b.slow_paths().count(SlowPath::MultisampleCopyResolved);
    b.copy_texture(&region(&ms), &region(&copy), extent);
    let counted = b.slow_paths().count(SlowPath::MultisampleCopyResolved) - before;
    b.resolve(&region(&ms), &region(&of_source), f);
    b.resolve(&region(&copy), &region(&of_copy), f);
    let [end, source, copied] =
        [&at_end, &of_source, &of_copy].map(|t| kit::read(&mut b, t, 0, 0, [4, 4, 1], 4, false));
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(end, source, "resolved at the pass's end and by resolve");
    assert_eq!(copied, source, "the copy's resolve and the source's");
    let partial = source
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|t| t[0] != 0 && t[0] != 255)
        .count();
    assert!(partial > 0, "no pixel on the diagonal is partly covered");
    let reads = b
        .caps()
        .has(Features::MULTISAMPLE_READ | Features::SAMPLE_SHADING);
    assert_eq!(
        counted,
        u64::from(!reads),
        "MultisampleCopyResolved counted"
    );
}

/// Writes one into an `r32uint` target over the whole of it, at the depth `z` in window space, given
/// in clip space for the back end's convention.
fn at_depth(z: f64, minus_one: bool) -> String {
    let clip = if minus_one { 2.0 * z - 1.0 } else { z };
    format!(
        "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {{
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, {clip:.12}, 1.0);
}}
@fragment fn fs() -> @location(0) vec4<u32> {{
    return vec4<u32>(1u);
}}
"
    )
}

/// With `FRACTIONAL_DEPTH_BIAS`, a constant bias of 0.1 units on a `depth16unorm` depth 0.45 of a
/// step above step 1000 takes it past the half step, so it stores 1001 where no bias stores 1000.
/// Without it, a raster block with a fractional bias is refused as absent.
pub fn fractional_depth_bias<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if !b.caps().has(Features::FRACTIONAL_DEPTH_BIAS) {
        let desc = RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::Clockwise,
            depth_bias: 0.1f32.to_bits(),
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking: ProvokingVertex::Last,
            depth_minus_one: !b.caps().has(Features::CLIP_DEPTH_ZERO_ONE),
            sample_mask: u32::MAX,
        };
        assert_eq!(
            b.create_raster(&desc).err(),
            Some(GpuError::Absent(Features::FRACTIONAL_DEPTH_BIAS))
        );
        not_run(Features::FRACTIONAL_DEPTH_BIAS);
        return;
    }
    let k = Kit::new(&mut b);
    let minus_one = !b.caps().has(Features::CLIP_DEPTH_ZERO_ONE);
    let program = kit::program(&mut b, &at_depth(1000.45 / 65535.0, minus_one), "vs", "fs");
    let always = kit::depth_stencil(&mut b, Some(CompareFn::Always), true, None);
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let c = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::R32Uint,
        [4, 4, 1],
        1,
        usage,
        false,
    );
    let cv = kit::target(&mut b, &c, Format::R32Uint, 0, 0);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    let mut stored = Vec::new();
    for bias in [0.0, 0.1] {
        let raster = kit::raster(&mut b, CullMode::None, FrontFace::Clockwise, bias, 0.0);
        let f = Format::Depth16Unorm;
        let d = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
        let dv = kit::target(&mut b, &d, f, 0, 0);
        b.begin_pass(&kit::pass(&cv, [0; 4], Some((&dv, 0.0, false))));
        b.draw(&DrawCall {
            depth_stencil: &always,
            raster: &raster,
            ..kit::call(&k, &program, None, &whole, Topology::TriangleList, 3)
        });
        b.end_pass();
        let got = kit::read(&mut b, &d, 0, 0, [4, 4, 1], 2, false);
        stored.push(u16::from_le_bytes([got[0], got[1]]));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(
        stored,
        [1000, 1001],
        "depth stored without and with a bias of 0.1 units"
    );
}

/// The constant alpha blend factor drawn natively: white scaled by a constant alpha of 0.5 gives 128
/// in every channel of an `rgba8unorm` target. A blend using the constant colour and the constant
/// alpha together, which WebGL forbids, is refused as absent.
pub fn constant_alpha_blends<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if !b.caps().has(Features::BLEND_CONSTANT_ALPHA) {
        not_run(Features::BLEND_CONSTANT_ALPHA);
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, HALF, "vs", "fs");
    let both = |b: &mut H::Backend, colour: BlendFactor| {
        let c = BlendComponent {
            src: BlendFactor::ConstantAlpha,
            dst: colour,
            op: BlendOp::Add,
        };
        b.create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: Some((c, c)),
                writes: ColorWrites::all(),
            }; MAX_COLOR_TARGETS],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
    };
    if let Err(e) = both(&mut b, BlendFactor::Constant) {
        assert_eq!(e, GpuError::Absent(Features::BLEND_CONSTANT_ALPHA));
    }
    let blend = both(&mut b, BlendFactor::Zero).expect("create_blend");
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let v = kit::target(&mut b, &t, f, 0, 0);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    for alpha in [0.5f32, 0.25] {
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&DrawCall {
            blend: &blend,
            // The constant colour is not used; only its alpha is.
            blend_constant: [1.0f32, 1.0, 1.0, alpha].map(f32::to_bits),
            ..kit::call(&k, &program, None, &whole, Topology::TriangleList, 3)
        });
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [4, 4, 1], 4, false);
        assert_eq!(b.status(), DeviceStatus::Ok);
        // The top left texel is wholly inside HALF's triangle: (1, 0.5, 0.25, 1) scaled by alpha,
        // within one step (the GPU's blend rounds 127.5 either way).
        let want = [1.0f32, 0.5, 0.25, 1.0].map(|c| c * alpha * 255.0);
        let close = got[..4]
            .iter()
            .zip(want)
            .all(|(g, w)| (f32::from(*g) - w).abs() <= 1.0);
        assert!(
            close,
            "the top left texel at alpha {alpha}: {:?}, want {want:?}",
            &got[..4]
        );
    }
}

/// Vertex `v` writes its numbers to locations 0 and 1, and its position makes triangles over the top
/// left half of the target, drawn red.
const CAPTURED: &str = "
struct Cap {
    @builtin(position) p: vec4<f32>,
    @location(0) a: vec4<f32>,
    @location(1) @interpolate(flat) n: u32,
}
@vertex fn vs(@builtin(vertex_index) v: u32) -> Cap {
    var o: Cap;
    let corner = vec2<f32>(f32(v % 3u == 1u), f32(v % 3u == 2u));
    o.p = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    o.a = vec4<f32>(f32(v), f32(v) * 0.5, 1.0, -2.0);
    o.n = 10u + v;
    return o;
}
@fragment fn fs(i: Cap) -> @location(0) vec4<f32> {
    return vec4<f32>(i.a.z, 0.0, 0.0, 1.0);
}
struct Ix {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) n: u32,
}
@vertex fn vs_indices(@builtin(vertex_index) v: u32) -> Ix {
    var indices = array<u32, 4>(3u, 0u, 5u, 1u);
    var o: Ix;
    o.p = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    o.n = indices[v];
    return o;
}
@fragment fn fs_indices(i: Ix) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i.n), 0.0, 0.0, 1.0);
}
struct Num {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) v: u32,
    @location(1) @interpolate(flat) i: u32,
}
@vertex fn vs_numbers(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> Num {
    var o: Num;
    let corner = vec2<f32>(f32(v % 3u == 1u), f32(v % 3u == 2u));
    o.p = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    o.v = v;
    o.i = i;
    return o;
}
@fragment fn fs_numbers(i: Num) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
struct Read {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) v: u32,
    @location(1) @interpolate(flat) i: u32,
    @location(2) @interpolate(flat) e: u32,
}
@vertex fn vs_streams(@builtin(instance_index) i: u32, @location(0) per_vertex: u32,
                      @location(1) per_instance: u32) -> Read {
    var o: Read;
    o.p = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    o.v = per_vertex;
    o.i = i;
    o.e = per_instance;
    return o;
}
@fragment fn fs_streams(i: Read) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i.v), 0.0, 0.0, 1.0);
}
";

/// A buffer of `size` zero bytes with `usage`, for captures.
fn zeros<B: Backend>(b: &mut B, usage: BufferUsage, size: usize) -> B::Buffer {
    buffer(b, usage | BufferUsage::CAPTURE, &vec![0; size])
}

fn read_buffer<B: Backend>(b: &mut B, buffer: &B::Buffer, size: usize) -> Vec<u8> {
    let mut got = vec![0; size];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer,
            offset: 0,
            size: size as u64,
        },
        &mut got,
    )
    .expect("read_back_now");
    got
}

/// What CAPTURED's vertex `v` writes to location 0 and location 1.
fn captured(v: u32) -> (Vec<u8>, Vec<u8>) {
    let a = [v as f32, v as f32 * 0.5, 1.0, -2.0];
    (
        a.iter().flat_map(|c| c.to_le_bytes()).collect(),
        (10 + v).to_le_bytes().to_vec(),
    )
}

/// `capture_vertices` writes the location outputs of every vertex of a list draw: interleaved into one
/// target, or one output per target, without drawing outside a pass and drawing inside one; a draw
/// that leaves out the last part of a primitive captures whole primitives only.
pub fn vertices_captured_with_and_without_rasterising<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if !b.caps().has(Features::TRANSFORM_FEEDBACK) {
        not_run(Features::TRANSFORM_FEEDBACK);
        return;
    }
    assert!(b.caps().limits.capture_buffers >= 2);
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, CAPTURED, "vs", "fs");
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    let interleaved: Vec<u8> = (0..6)
        .flat_map(|v| {
            let (a, n) = captured(v);
            [a, n].concat()
        })
        .collect();
    // Interleaved, outside a pass; 7 vertices make 2 whole triangles.
    let one = zeros(&mut b, BufferUsage::VERTEX, 6 * 20 + 8);
    let call = kit::call(&k, &program, None, &whole, Topology::TriangleList, 7);
    let targets = [BufferRange {
        buffer: &one,
        offset: 0,
        size: 6 * 20 + 8,
    }];
    b.capture_vertices(&call, &targets, false)
        .expect("capture_vertices");
    let got = read_buffer(&mut b, &one, 6 * 20 + 8);
    assert_eq!(&got[..6 * 20], &interleaved[..], "interleaved capture");
    assert_eq!(&got[6 * 20..], &[0; 8], "bytes past the whole primitives");
    // Separate, outside a pass, at an offset into the second target.
    let (a, n) = (
        zeros(&mut b, BufferUsage::VERTEX, 6 * 16),
        zeros(&mut b, BufferUsage::VERTEX, 8 + 6 * 4),
    );
    let call = kit::call(&k, &program, None, &whole, Topology::TriangleList, 6);
    let targets = [
        BufferRange {
            buffer: &a,
            offset: 0,
            size: 6 * 16,
        },
        BufferRange {
            buffer: &n,
            offset: 8,
            size: 6 * 4,
        },
    ];
    b.capture_vertices(&call, &targets, false)
        .expect("capture_vertices");
    let (want_a, want_n): (Vec<_>, Vec<_>) = (0..6).map(captured).unzip();
    assert_eq!(
        read_buffer(&mut b, &a, 6 * 16),
        want_a.concat(),
        "location 0"
    );
    assert_eq!(
        read_buffer(&mut b, &n, 8 + 6 * 4),
        [vec![0; 8], want_n.concat()].concat(),
        "location 1 at offset 8"
    );
    // Interleaved, drawing too.
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let v = kit::target(&mut b, &t, f, 0, 0);
    let drawn = zeros(&mut b, BufferUsage::VERTEX, 6 * 20);
    let call = kit::call(&k, &program, None, &whole, Topology::TriangleList, 6);
    let targets = [BufferRange {
        buffer: &drawn,
        offset: 0,
        size: 6 * 20,
    }];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.capture_vertices(&call, &targets, true)
        .expect("capture_vertices");
    b.end_pass();
    assert_eq!(
        read_buffer(&mut b, &drawn, 6 * 20),
        interleaved,
        "capture while drawing"
    );
    let picture = kit::read(&mut b, &t, 0, 0, [4, 4, 1], 4, false);
    assert_eq!(&picture[..4], &[255, 0, 0, 255], "the top left texel drawn");
    assert_eq!(
        &picture[60..],
        &[0, 0, 0, 0],
        "the bottom right texel not drawn"
    );
    // Drawing while capturing outside a pass breaks the interface's rule.
    let call = kit::call(&k, &program, None, &whole, Topology::TriangleList, 6);
    assert!(matches!(
        b.capture_vertices(&call, &targets, true),
        Err(GpuError::Invalid(_))
    ));
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A buffer holding indices and vertex data written by `capture_vertices` reads the captured values
/// both as indices and as a vertex stream (on WebGL 2 the index twin is refreshed first,
/// `IndexTwinReadBack`).
pub fn captured_indices_drawn_both_ways<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if !b.caps().has(Features::TRANSFORM_FEEDBACK) {
        not_run(Features::TRANSFORM_FEEDBACK);
        return;
    }
    let k = Kit::new(&mut b);
    let capture = kit::program(&mut b, CAPTURED, "vs_indices", "fs_indices");
    let both = zeros(&mut b, BufferUsage::INDEX | BufferUsage::VERTEX, 16);
    // Written by the host first, so both of WebGL's GL buffers hold old values.
    b.write_buffer(&both, 0, &words(&[7, 7, 7, 7]), WriteMode::Ordered);
    let whole = [kit::viewport(0.0, 0.0, 8.0, 4.0)];
    let call = kit::call(&k, &capture, None, &whole, Topology::PointList, 4);
    let targets = [BufferRange {
        buffer: &both,
        offset: 0,
        size: 16,
    }];
    b.capture_vertices(&call, &targets, false)
        .expect("capture_vertices");
    let vs = b
        .create_shader(&module("vs", ShaderStage::Vertex))
        .expect("create_shader");
    let fs = b
        .create_shader(&module("fs", ShaderStage::Fragment))
        .expect("create_shader");
    let numbers = b
        .create_program(&Program::Render {
            vertex: &vs,
            fragment: Some(&fs),
        })
        .expect("create_program");
    let mut layout = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    for (i, step) in [StepMode::Vertex, StepMode::Instance(1)]
        .into_iter()
        .enumerate()
    {
        layout.streams[i] = Some(VertexStream { stride: 4, step });
        layout.attributes[i] = Some(VertexAttribute {
            location: 1 + i as u32,
            stream: i as u8,
            offset: 0,
            format: VertexFormat::Uint32x1,
        });
    }
    let layout = b
        .create_vertex_layout(&layout)
        .expect("create_vertex_layout");
    let per_vertex = buffer(
        &mut b,
        BufferUsage::VERTEX,
        &words(&(0..8).collect::<Vec<_>>()),
    );
    let per_instance = buffer(&mut b, BufferUsage::VERTEX, &words(&[100]));
    let f = Format::Rgba32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [8, 4, 1], 1, usage, false);
    let v = kit::target(&mut b, &t, f, 0, 0);
    let captured = [3u32, 0, 5, 1];
    for as_indices in [true, false] {
        let first = if as_indices { &per_vertex } else { &both };
        let streams = [
            Some(VertexBinding {
                buffer: first,
                offset: 0,
            }),
            Some(VertexBinding {
                buffer: &per_instance,
                offset: 0,
            }),
        ];
        let before = b.slow_paths().count(SlowPath::IndexTwinReadBack);
        b.begin_pass(&kit::pass(&v, [u32::MAX; 4], None));
        b.draw(&DrawCall {
            vertex_layout: &layout,
            vertex_buffers: &streams,
            index: as_indices.then_some(IndexBinding {
                buffer: &both,
                offset: 0,
                format: IndexFormat::U32,
            }),
            range: if as_indices {
                DrawRange::Indexed {
                    first: 0,
                    count: 4,
                    base_vertex: 0,
                    first_instance: 0,
                    instances: 1,
                }
            } else {
                DrawRange::Vertices {
                    first: 0,
                    count: 4,
                    first_instance: 0,
                    instances: 1,
                }
            },
            ..kit::call(&k, &numbers, None, &whole, Topology::PointList, 4)
        });
        b.end_pass();
        let refreshed = b.slow_paths().count(SlowPath::IndexTwinReadBack) - before;
        let got = kit::read(&mut b, &t, 0, 0, [8, 4, 1], 16, false);
        let texel = |p: usize| -> [u32; 4] {
            core::array::from_fn(|c| {
                u32::from_le_bytes(got[p * 16 + c * 4..p * 16 + c * 4 + 4].try_into().unwrap())
            })
        };
        let want: Vec<(usize, [u32; 4])> = if as_indices {
            captured
                .iter()
                .map(|i| (*i as usize, [*i, 0, *i, 100]))
                .collect()
        } else {
            (0..4)
                .map(|i| (i as usize, [i, 0, captured[i as usize], 100]))
                .collect()
        };
        for (pixel, numbers) in &want {
            assert_eq!(
                texel(*pixel),
                *numbers,
                "as indices {as_indices}: pixel {pixel}"
            );
        }
        for p in (0..32).filter(|p| want.iter().all(|(w, _)| w != p)) {
            assert_eq!(
                texel(p),
                [u32::MAX; 4],
                "as indices {as_indices}: pixel {p}"
            );
        }
        if as_indices && b.slow_paths().count(SlowPath::IndexBufferTwin) > 0 {
            assert_eq!(refreshed, 1, "the index twin refreshed once");
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Loads texel (0, 0) of level 0.
const LOAD_CORNER: &str = "
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(0, 0), 0);
}
";

/// RGB9E5 and every block-compressed format the back end serves read back exactly as written (on
/// WebGL 2 from the host copy, `HostCopyKept`): two layers of three levels of an 8 by 8 texture, the
/// last level smaller than a block, then one block of level 0 written again, read back now and
/// later. A solid BC1 block and an RGB9E5 texel load as their colour, whichever way the back end
/// stores them (natively, or decoded: `BlockDecoded`).
pub fn block_compressed_and_shared_exponent_textures_read_back_as_written<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LOAD_CORNER, "vs", "fs");
    let out_format = F::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        out_format,
        [1, 1, 1],
        1,
        usage,
        false,
    );
    let out_view = kit::target(&mut b, &out, out_format, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 1.0, 1.0)];
    let formats = [
        F::Rgb9e5Float,
        F::Bc1RgbaUnorm,
        F::Bc1RgbaUnormSrgb,
        F::Bc2RgbaUnorm,
        F::Bc2RgbaUnormSrgb,
        F::Bc3RgbaUnorm,
        F::Bc3RgbaUnormSrgb,
        F::Bc4RUnorm,
        F::Bc4RSnorm,
        F::Bc5RgUnorm,
        F::Bc5RgSnorm,
        F::Bc6hRgbUfloat,
        F::Bc6hRgbFloat,
        F::Bc7RgbaUnorm,
        F::Bc7RgbaUnormSrgb,
    ];
    let mut served = 0;
    for (n, format) in formats.into_iter().enumerate() {
        let want = FormatUse::SAMPLE | FormatUse::COPY_OUT;
        if !b.caps().formats.get(format).contains(want) {
            continue;
        }
        served += 1;
        let (bw, bh, bytes) = format.block();
        let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC;
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [8, 8, 2],
            3,
            usage,
            false,
        );
        // Each level's two layers, as rows of blocks.
        let mut kept = Vec::new();
        for mip in 0..3u32 {
            let size = (8 >> mip) as u32;
            let row = (size.div_ceil(bw) * bytes) as usize;
            let rows = size.div_ceil(bh) as usize;
            let mut level = pattern(row * rows * 2, 0x9e37 + n as u32 * 7 + mip);
            if mip == 0 {
                // Block (0, 0) of layer 0 solid: red in BC1, (1, 0, 0.25) in RGB9E5 (exponent 16), whose
                // unorm values (255, 0, 63.75) round without a tie.
                match format {
                    F::Bc1RgbaUnorm => level[..8].copy_from_slice(&[0, 0xf8, 0, 0xf8, 0, 0, 0, 0]),
                    F::Rgb9e5Float => {
                        let texel: u32 = 256 | (64 << 18) | (16 << 27);
                        level[..4].copy_from_slice(&texel.to_le_bytes());
                    }
                    _ => {}
                }
            }
            b.write_texture(
                &TextureRegion {
                    texture: &t,
                    mip,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                Extent {
                    width: size,
                    height: size,
                    depth_or_layers: 2,
                },
                &level,
                row as u32,
                rows as u32,
                WriteMode::Ordered,
            );
            kept.push((row, rows, level));
        }
        // One block of layer 1 of level 0 again, at the second column and row of blocks.
        let again = pattern(bytes as usize, 0x51ed + n as u32);
        b.write_texture(
            &TextureRegion {
                texture: &t,
                mip: 0,
                layer: 1,
                origin: Origin { x: bw, y: bh, z: 0 },
                stencil: false,
            },
            Extent {
                width: bw,
                height: bh,
                depth_or_layers: 1,
            },
            &again,
            bytes,
            1,
            WriteMode::Ordered,
        );
        let (row, rows, level) = &mut kept[0];
        let at = *row * *rows + *row + bytes as usize;
        level[at..at + bytes as usize].copy_from_slice(&again);
        for (mip, (row, rows, level)) in kept.iter().enumerate() {
            let size = (8 >> mip) as u32;
            let mut got = vec![0; row * rows * 2];
            b.read_back_now(
                &ReadSource::Texture {
                    region: TextureRegion {
                        texture: &t,
                        mip: mip as u32,
                        layer: 0,
                        origin: Origin::default(),
                        stencil: false,
                    },
                    extent: Extent {
                        width: size,
                        height: size,
                        depth_or_layers: 2,
                    },
                },
                &mut got,
            )
            .expect("read_back_now");
            assert_eq!(&got, level, "{format:?} level {mip}, read back now");
        }
        // Level 1 again through a read back waited for.
        let (row, rows, level) = &kept[1];
        let rb = b
            .read_back(&ReadSource::Texture {
                region: TextureRegion {
                    texture: &t,
                    mip: 1,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                extent: Extent {
                    width: 4,
                    height: 4,
                    depth_or_layers: 2,
                },
            })
            .expect("read_back");
        b.wait_read(&rb);
        let mut got = vec![0; row * rows * 2];
        b.finish_read(rb, &mut got);
        assert_eq!(&got, level, "{format:?} level 1, read back later");
        let expected: Option<[u8; 4]> = match format {
            F::Bc1RgbaUnorm => Some([255, 0, 0, 255]),
            F::Rgb9e5Float => Some([255, 0, 64, 255]),
            _ => None,
        };
        if let Some(expected) = expected {
            let flat = kit::texture(
                &mut b,
                TextureDimension::D2,
                format,
                [8, 8, 1],
                1,
                usage,
                false,
            );
            let (row, rows, level) = &kept[0];
            b.write_texture(
                &TextureRegion {
                    texture: &flat,
                    mip: 0,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                Extent {
                    width: 8,
                    height: 8,
                    depth_or_layers: 1,
                },
                &level[..row * rows],
                *row as u32,
                *rows as u32,
                WriteMode::Ordered,
            );
            let flat_view = kit::view(&mut b, &flat, format, ViewDimension::D2, 0, 1, 0, 1);
            let set = b
                .create_binding_set(&[BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&flat_view),
                }])
                .expect("create_binding_set");
            b.begin_pass(&kit::pass(&out_view, [0; 4], None));
            b.draw(&kit::call(
                &k,
                &program,
                Some(&set),
                &vp,
                Topology::TriangleList,
                3,
            ));
            b.end_pass();
            let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
            assert_eq!(got, expected, "{format:?}: texel (0, 0) loaded");
        }
    }
    assert!(
        served > 0
            || !b
                .caps()
                .formats
                .get(F::Rgb9e5Float)
                .contains(FormatUse::SAMPLE)
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Loads the texel under each fragment.
const LOAD_EACH: &str = "
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(p.xy), 0);
}
";

/// Every interpolated value of BC4 and BC5 blocks (unsigned and signed, both modes: six values
/// between the endpoints, and four with the two extremes) loaded into a 32-bit float target. Stored
/// decoded (`BlockDecoded`), each must be the formula's value (bcdec_rs's float decoder) to the
/// half float it is stored as. Natively compressed, the GPU decodes it as Direct3D on the same GPU
/// does; that route's largest difference from the formula is printed, as a measurement.
pub fn interpolated_bc4_and_bc5_values<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let out_format = F::Rgba32Float;
    if !b.caps().formats.get(out_format).contains(FormatUse::RENDER) {
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LOAD_EACH, "vs", "fs");
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        out_format,
        [8, 4, 1],
        1,
        usage,
        false,
    );
    let out_view = kit::target(&mut b, &out, out_format, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 8.0, 4.0)];
    // A BC4 block: two endpoints, then a 3-bit index per texel, row by row.
    let block = |e: [u8; 2], index: &dyn Fn(u64) -> u64| {
        let bits = (0..16).fold(0u64, |a, j| a | (index(j) << (3 * j)));
        let mut out = [0u8; 8];
        out[..2].copy_from_slice(&e);
        out[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
        out
    };
    let compressed = b.caps().has(Features::BLOCK_COMPRESSION);
    for format in [F::Bc4RUnorm, F::Bc4RSnorm, F::Bc5RgUnorm, F::Bc5RgSnorm] {
        if !b.caps().formats.get(format).contains(FormatUse::SAMPLE) {
            continue;
        }
        let signed = matches!(format, F::Bc4RSnorm | F::Bc5RgSnorm);
        let two = matches!(format, F::Bc5RgUnorm | F::Bc5RgSnorm);
        let (red, green) = if signed {
            ([0x83, 101], [0x91, 0x7f])
        } else {
            ([250, 3], [17, 200])
        };
        // Two blocks side by side, the second with its endpoints swapped (the other mode).
        let mut blocks = Vec::new();
        for swap in [false, true] {
            let order = |e: [u8; 2]| if swap { [e[1], e[0]] } else { e };
            blocks.extend(block(order(red), &|j| j % 8));
            if two {
                blocks.extend(block(order(green), &|j| 7 - j % 8));
            }
        }
        let bytes = blocks.len() as u32 / 2;
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [8, 4, 1],
            1,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            false,
        );
        b.write_texture(
            &TextureRegion {
                texture: &t,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: 8,
                height: 4,
                depth_or_layers: 1,
            },
            &blocks,
            2 * bytes,
            1,
            WriteMode::Ordered,
        );
        let view = kit::view(&mut b, &t, format, ViewDimension::D2, 0, 1, 0, 1);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got: Vec<f32> = kit::read(&mut b, &out, 0, 0, [8, 4, 1], 16, false)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        let channels = if two { 2 } else { 1 };
        let mut worst = 0f32;
        for (n, block) in blocks.chunks_exact(bytes as usize).enumerate() {
            let mut want = [0f32; 32];
            if two {
                bcdec_rs::bc5_float(block, &mut want, 8, signed);
            } else {
                bcdec_rs::bc4_float(block, &mut want, 4, signed);
            }
            for y in 0..4 {
                for x in 0..4 {
                    for c in 0..channels {
                        let w = want[(y * 4 + x) * channels + c];
                        let g = got[(y * 8 + n * 4 + x) * 4 + c];
                        let differ = (w - g).abs();
                        worst = worst.max(differ);
                        // Half a half float's step below 1.
                        assert!(
                            compressed || differ <= 1.0 / 4096.0,
                            "{format:?} decoded: texel ({}, {y}) channel {c}: {g}, the formula {w}",
                            n * 4 + x
                        );
                    }
                }
            }
        }
        let route = if compressed {
            "decoded by the GPU"
        } else {
            "stored decoded"
        };
        let step = if signed { 127.0 } else { 255.0 };
        println!(
            "{format:?} {route}: largest difference from the formula {worst:e} ({:.2} of an 8-bit step)",
            worst * step
        );
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(view));
        b.destroy(Object::Texture(t));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A triangle over the target at clip z 0.5, writing its fragments' window z.
const FRAGMENT_Z: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.5, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(p.z, 0.0, 0.0, 1.0);
}
";

/// The fragment's window z (WGSL's position z) and the depth stored are clip z mapped by the raster
/// block's convention: 0.5 from zero to one, 0.75 from minus one to one.
pub fn fragment_depth_follows_the_clip_convention<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let colour = Format::R32Float;
    let wanted = FormatUse::RENDER | FormatUse::COPY_OUT;
    if !b.caps().formats.get(colour).contains(wanted) {
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, FRAGMENT_Z, "vs", "fs");
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        colour,
        [2, 2, 1],
        1,
        usage,
        false,
    );
    let v = kit::target(&mut b, &t, colour, 0, 0);
    let depth = Format::Depth32Float;
    let d = kit::texture(
        &mut b,
        TextureDimension::D2,
        depth,
        [2, 2, 1],
        1,
        usage,
        false,
    );
    let dv = kit::target(&mut b, &d, depth, 0, 0);
    let ds = kit::depth_stencil(&mut b, Some(CompareFn::Always), true, None);
    let whole = [kit::viewport(0.0, 0.0, 2.0, 2.0)];
    let mut conventions = vec![(true, 0.75f32)];
    if b.caps().has(Features::CLIP_DEPTH_ZERO_ONE) {
        conventions.push((false, 0.5));
    }
    for (minus_one, want) in conventions {
        let raster = b
            .create_raster(&RasterDesc {
                fill: FillMode::Solid,
                cull: CullMode::None,
                front: FrontFace::Clockwise,
                depth_bias: 0,
                depth_bias_slope: 0,
                depth_bias_clamp: 0,
                depth_clip: true,
                scissor: false,
                line_width: 1.0f32.to_bits(),
                provoking: ProvokingVertex::Last,
                depth_minus_one: minus_one,
                sample_mask: u32::MAX,
            })
            .expect("create_raster");
        b.begin_pass(&kit::pass(&v, [0; 4], Some((&dv, 1.0, false))));
        b.draw(&DrawCall {
            raster: &raster,
            depth_stencil: &ds,
            ..kit::call(&k, &program, None, &whole, Topology::TriangleList, 3)
        });
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [2, 2, 1], 4, false);
        let stored = kit::read(&mut b, &d, 0, 0, [2, 2, 1], 4, false);
        for (what, bytes) in [("position z", &got), ("stored depth", &stored)] {
            for texel in bytes.as_chunks::<4>().0 {
                let z = f32::from_le_bytes(*texel);
                assert_eq!(z, want, "{what} with depth_minus_one {minus_one}");
            }
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Strips, fans, loops and indexed draws are captured as the lists they draw, each primitive's
/// vertices in an order that keeps the raster block's provoking vertex where it was, restarting at the
/// largest index value, instance after instance, interleaved and separate, with and without drawing;
/// per-instance streams read at their step rate.
pub fn strips_fans_loops_and_indexed_draws_captured_in_list_order<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if !b.caps().has(Features::TRANSFORM_FEEDBACK) {
        not_run(Features::TRANSFORM_FEEDBACK);
        return;
    }
    let first = b.caps().has(Features::PROVOKING_VERTEX_FIRST);
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, CAPTURED, "vs_numbers", "fs_numbers");
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    // (vertex_index, instance_index) of each vertex captured.
    let pairs = |vertices: &[u32], instances: std::ops::Range<u32>| -> Vec<(u32, u32)> {
        instances
            .flat_map(|i| vertices.iter().map(move |&v| (v, i)))
            .collect()
    };
    let interleaved = |pairs: &[(u32, u32)]| -> Vec<u8> {
        words(&pairs.iter().flat_map(|&(v, i)| [v, i]).collect::<Vec<_>>())
    };
    let index_buffer = |b: &mut H::Backend, bytes: &[u8]| buffer(b, BufferUsage::INDEX, bytes);
    let u16s = |v: &[u16]| -> Vec<u8> { v.iter().flat_map(|i| i.to_le_bytes()).collect() };
    struct Case {
        name: &'static str,
        topology: Topology,
        /// Index bytes and their format, or none for a draw without indices.
        indices: Option<(Vec<u8>, IndexFormat)>,
        range: (u32, u32, i32, u32, u32),
        want: Vec<(u32, u32)>,
    }
    let cases = [
        Case {
            name: "triangle strip from vertex 2, two instances from 1",
            topology: Topology::TriangleStrip,
            indices: None,
            range: (2, 5, 0, 1, 2),
            want: pairs(
                &if first {
                    [2, 3, 4, 3, 5, 4, 4, 5, 6]
                } else {
                    [2, 3, 4, 4, 3, 5, 4, 5, 6]
                },
                1..3,
            ),
        },
        Case {
            name: "triangle fan",
            topology: Topology::TriangleFan,
            indices: None,
            range: (0, 4, 0, 0, 1),
            want: pairs(
                &if first {
                    [1, 2, 0, 2, 3, 0]
                } else {
                    [0, 1, 2, 0, 2, 3]
                },
                0..1,
            ),
        },
        Case {
            name: "line loop",
            topology: Topology::LineLoop,
            indices: None,
            range: (0, 3, 0, 0, 1),
            want: pairs(&[0, 1, 1, 2, 2, 0], 0..1),
        },
        Case {
            name: "line strip",
            topology: Topology::LineStrip,
            indices: None,
            range: (0, 3, 0, 0, 1),
            want: pairs(&[0, 1, 1, 2], 0..1),
        },
        Case {
            name: "16-bit indexed triangle list from index 1 with base vertex 10",
            topology: Topology::TriangleList,
            indices: Some((u16s(&[9, 7, 3, 9, 3, 4, 5]), IndexFormat::U16)),
            range: (1, 6, 10, 0, 1),
            want: pairs(&[17, 13, 19, 13, 14, 15], 0..1),
        },
        Case {
            name: "8-bit indexed triangle strip with base vertex 1 and a restart",
            topology: Topology::TriangleStrip,
            indices: Some((vec![0, 1, 2, 3, 0xff, 5, 6, 7], IndexFormat::U8)),
            range: (0, 8, 1, 3, 1),
            want: pairs(
                &if first {
                    [1, 2, 3, 2, 4, 3, 6, 7, 8]
                } else {
                    [1, 2, 3, 3, 2, 4, 6, 7, 8]
                },
                3..4,
            ),
        },
        Case {
            name: "32-bit indexed points",
            topology: Topology::PointList,
            indices: Some((words(&[4, 0, 70000]), IndexFormat::U32)),
            range: (0, 3, 0, 0, 2),
            want: pairs(&[4, 0, 70000], 0..2),
        },
    ];
    for case in &cases {
        let (start, count, base_vertex, first_instance, instances) = case.range;
        let ib = case
            .indices
            .as_ref()
            .map(|(bytes, format)| (index_buffer(&mut b, bytes), *format));
        let mut call = kit::call(&k, &program, None, &whole, case.topology, count);
        call.range = match &ib {
            None => DrawRange::Vertices {
                first: start,
                count,
                first_instance,
                instances,
            },
            Some(_) => DrawRange::Indexed {
                first: start,
                count,
                base_vertex,
                first_instance,
                instances,
            },
        };
        call.index = ib.as_ref().map(|(buffer, format)| IndexBinding {
            buffer,
            offset: 0,
            format: *format,
        });
        // Only strips restart; a list never does.
        call.restart =
            ib.is_some() && matches!(case.topology, Topology::TriangleStrip | Topology::LineStrip);
        let n = case.want.len();
        // Interleaved, with room for one vertex more that must stay zero.
        let one = zeros(&mut b, BufferUsage::VERTEX, (n + 1) * 8);
        let targets = [BufferRange {
            buffer: &one,
            offset: 0,
            size: (n as u64 + 1) * 8,
        }];
        b.capture_vertices(&call, &targets, false)
            .unwrap_or_else(|e| panic!("{}: capture_vertices: {e:?}", case.name));
        let got = read_buffer(&mut b, &one, (n + 1) * 8);
        assert_eq!(
            &got[..n * 8],
            &interleaved(&case.want)[..],
            "{}: interleaved",
            case.name
        );
        assert_eq!(&got[n * 8..], &[0; 8], "{}: past the vertices", case.name);
        // Separate.
        let (v, i) = (
            zeros(&mut b, BufferUsage::VERTEX, n * 4),
            zeros(&mut b, BufferUsage::VERTEX, n * 4),
        );
        let targets = [
            BufferRange {
                buffer: &v,
                offset: 0,
                size: n as u64 * 4,
            },
            BufferRange {
                buffer: &i,
                offset: 0,
                size: n as u64 * 4,
            },
        ];
        b.capture_vertices(&call, &targets, false)
            .unwrap_or_else(|e| panic!("{}: capture_vertices: {e:?}", case.name));
        let (want_v, want_i): (Vec<u32>, Vec<u32>) = case.want.iter().copied().unzip();
        assert_eq!(
            read_buffer(&mut b, &v, n * 4),
            words(&want_v),
            "{}: vertex numbers",
            case.name
        );
        assert_eq!(
            read_buffer(&mut b, &i, n * 4),
            words(&want_i),
            "{}: instance numbers",
            case.name
        );
    }
    // A strip with a per-vertex stream and a per-instance one stepping every 1, 2 or no instances:
    // each instance's vertices read the elements the whole draw gives that instance.
    let streams = kit::program(&mut b, CAPTURED, "vs_streams", "fs_streams");
    let per_vertex = buffer(
        &mut b,
        BufferUsage::VERTEX,
        &words(&(0..8).collect::<Vec<_>>()),
    );
    let per_instance = buffer(
        &mut b,
        BufferUsage::VERTEX,
        &words(&(100..108).collect::<Vec<_>>()),
    );
    for rate in [1, 2, 0] {
        let mut layout = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        for (s, step) in [StepMode::Vertex, StepMode::Instance(rate)]
            .into_iter()
            .enumerate()
        {
            layout.streams[s] = Some(VertexStream { stride: 4, step });
            layout.attributes[s] = Some(VertexAttribute {
                location: s as u32,
                stream: s as u8,
                offset: 0,
                format: VertexFormat::Uint32x1,
            });
        }
        let layout = b
            .create_vertex_layout(&layout)
            .expect("create_vertex_layout");
        let bound = [
            Some(VertexBinding {
                buffer: &per_vertex,
                offset: 0,
            }),
            Some(VertexBinding {
                buffer: &per_instance,
                offset: 0,
            }),
        ];
        let mut call = kit::call(&k, &streams, None, &whole, Topology::TriangleStrip, 4);
        call.vertex_layout = &layout;
        call.vertex_buffers = &bound;
        call.range = DrawRange::Vertices {
            first: 1,
            count: 4,
            first_instance: 1,
            instances: 4,
        };
        let order = if first {
            [1, 2, 3, 2, 4, 3]
        } else {
            [1, 2, 3, 3, 2, 4]
        };
        let want: Vec<u32> = (1..5)
            .flat_map(|i| {
                let element = match rate {
                    0 => 1,
                    n => 1 + (i - 1) / n,
                };
                order.iter().flat_map(move |&v| [v, i, 100 + element])
            })
            .collect();
        let out = zeros(&mut b, BufferUsage::VERTEX, want.len() * 4);
        let targets = [BufferRange {
            buffer: &out,
            offset: 0,
            size: want.len() as u64 * 4,
        }];
        b.capture_vertices(&call, &targets, false)
            .unwrap_or_else(|e| panic!("streams at rate {rate}: capture_vertices: {e:?}"));
        assert_eq!(
            read_buffer(&mut b, &out, want.len() * 4),
            words(&want),
            "streams at rate {rate}"
        );
        b.destroy(Object::VertexLayout(layout));
    }
    // Drawing too: an indexed strip whose first triangle covers the top left texel.
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(&mut b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let view = kit::target(&mut b, &t, f, 0, 0);
    let ib = index_buffer(&mut b, &u16s(&[0, 1, 2, 1]));
    let mut call = kit::call(&k, &program, None, &whole, Topology::TriangleStrip, 4);
    call.range = DrawRange::Indexed {
        first: 0,
        count: 4,
        base_vertex: 0,
        first_instance: 0,
        instances: 1,
    };
    call.index = Some(IndexBinding {
        buffer: &ib,
        offset: 0,
        format: IndexFormat::U16,
    });
    let drawn = zeros(&mut b, BufferUsage::VERTEX, 6 * 8);
    let targets = [BufferRange {
        buffer: &drawn,
        offset: 0,
        size: 6 * 8,
    }];
    b.begin_pass(&kit::pass(&view, [0; 4], None));
    b.capture_vertices(&call, &targets, true)
        .expect("capture_vertices while drawing");
    b.end_pass();
    let want = pairs(
        &if first {
            [0, 1, 2, 1, 1, 2]
        } else {
            [0, 1, 2, 2, 1, 1]
        },
        0..1,
    );
    assert_eq!(
        read_buffer(&mut b, &drawn, 6 * 8),
        interleaved(&want),
        "capture while drawing"
    );
    let picture = kit::read(&mut b, &t, 0, 0, [4, 4, 1], 4, false);
    assert_eq!(&picture[..4], &[255, 0, 0, 255], "the top left texel drawn");
    assert_eq!(
        &picture[60..],
        &[0, 0, 0, 0],
        "the bottom right texel not drawn"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// What WebGL 2 lacks is left out of the caps and refused as absent where it is asked for: blend,
/// depth-stencil and raster blocks, samplers, compute programs, binding kinds and query kinds. The
/// features an extension gives are refused only where the back end was made without it. A sampler
/// past the anisotropy limit is invalid.
pub fn absent_features_are_refused<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let caps = b.caps().clone();
    // Plan 4.11's absent list for WebGL 2.
    let never = Features::COMPUTE
        | Features::VERTEX_STORAGE_READ
        | Features::VERTEX_STORAGE_WRITE
        | Features::FRAGMENT_STORAGE_WRITE
        | Features::INDIRECT_FIRST_INSTANCE
        | Features::BASE_VERTEX_INSTANCE
        | Features::TEXTURE_VIEWS
        | Features::SEPARATE_SAMPLERS
        | Features::TEXTURE_LEVEL_QUERY
        | Features::MULTISAMPLE_READ
        | Features::SUBGROUPS
        | Features::FRACTIONAL_VIEWPORT
        | Features::RESTART_SWITCH
        | Features::POLYGON_MODE_POINT
        | Features::LOGIC_OP
        | Features::WIDE_LINES
        | Features::SAMPLE_MASK
        | Features::OCCLUSION_COUNT
        | Features::INDIRECT
        | Features::RESOURCE_TABLES
        | Features::BORDER_COLOR
        | Features::DEPTH_BOUNDS
        | Features::STENCIL_PER_FACE
        | Features::TEXEL_BUFFERS
        | Features::FINE_DERIVATIVES;
    assert!(
        !caps.features.intersects(never),
        "declared what is not built: {:?}",
        caps.features & never
    );
    let absent = |r: Result<(), GpuError>, f: Features, what: &str| {
        assert_eq!(r, Err(GpuError::Absent(f)), "{what}");
    };
    // Made where the feature is declared, refused as absent where not.
    let by_caps = |r: Result<(), GpuError>, f: Features, what: &str| {
        if caps.has(f) {
            r.unwrap_or_else(|e| panic!("{what}: {e:?}"));
        } else {
            absent(r, f, what);
        }
    };
    let solid = (
        BlendComponent {
            src: BlendFactor::One,
            dst: BlendFactor::Zero,
            op: BlendOp::Add,
        },
        BlendComponent {
            src: BlendFactor::One,
            dst: BlendFactor::Zero,
            op: BlendOp::Add,
        },
    );
    let blend_desc = |blend| BlendDesc {
        targets: [RenderTargetBlend {
            blend,
            writes: ColorWrites::all(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    };
    let mut d = blend_desc(Some(solid));
    d.logic_op = Some(LogicOp::Xor);
    absent(b.create_blend(&d).map(drop), Features::LOGIC_OP, "logic op");
    let mut d = blend_desc(Some(solid));
    d.same_for_all = false;
    d.targets[1].blend = None;
    by_caps(
        b.create_blend(&d).map(drop),
        Features::INDEPENDENT_BLEND,
        "a blend per target",
    );
    let mut second = solid;
    second.0.dst = BlendFactor::OneMinusSrc1;
    by_caps(
        b.create_blend(&blend_desc(Some(second))).map(drop),
        Features::DUAL_SOURCE_BLEND,
        "a second source",
    );
    let depth_desc = DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    };
    let mut d = depth_desc;
    d.depth_bounds = Some([0f32.to_bits(), 0.5f32.to_bits()]);
    absent(
        b.create_depth_stencil(&d).map(drop),
        Features::DEPTH_BOUNDS,
        "depth bounds",
    );
    let mut d = depth_desc;
    d.stencil_write_mask = [0xff, 0x0f];
    absent(
        b.create_depth_stencil(&d).map(drop),
        Features::STENCIL_PER_FACE,
        "a stencil mask per face",
    );
    let raster_desc = RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::Clockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::Last,
        depth_minus_one: !caps.has(Features::CLIP_DEPTH_ZERO_ONE),
        sample_mask: u32::MAX,
    };
    b.create_raster(&raster_desc)
        .expect("the plain raster block");
    let mut raster = |what: &str, f: Features, change: &dyn Fn(&mut RasterDesc)| {
        let mut d = raster_desc;
        change(&mut d);
        let r = b.create_raster(&d).map(drop);
        if caps.has(f) {
            r.unwrap_or_else(|e| panic!("{what}: {e:?}"));
        } else {
            absent(r, f, what);
        }
    };
    raster("lines", Features::POLYGON_MODE, &|d| {
        d.fill = FillMode::Line
    });
    raster("no depth clip", Features::DEPTH_CLAMP, &|d| {
        d.depth_clip = false
    });
    raster("a bias clamp", Features::DEPTH_BIAS_CLAMP, &|d| {
        d.depth_bias = 2.0f32.to_bits();
        d.depth_bias_clamp = 0.5f32.to_bits();
    });
    raster("points", Features::POLYGON_MODE_POINT, &|d| {
        d.fill = FillMode::Point
    });
    raster("a wide line", Features::WIDE_LINES, &|d| {
        d.line_width = 2.0f32.to_bits()
    });
    raster("a partial sample mask", Features::SAMPLE_MASK, &|d| {
        d.sample_mask = 0b101
    });
    raster("the first vertex", Features::PROVOKING_VERTEX_FIRST, &|d| {
        d.provoking = ProvokingVertex::First
    });
    raster(
        "clip depth from zero",
        Features::CLIP_DEPTH_ZERO_ONE,
        &|d| d.depth_minus_one = false,
    );
    raster("a fractional bias", Features::FRACTIONAL_DEPTH_BIAS, &|d| {
        d.depth_bias = 1.5f32.to_bits()
    });
    let sampler_desc = SamplerDesc {
        min: Filter::Linear,
        mag: Filter::Linear,
        mip: Filter::Linear,
        address: [AddressMode::Clamp; 3],
        lod_range: [0f32.to_bits(), 1000f32.to_bits()],
        anisotropy: 1,
        compare: None,
        border: BorderColor::TransparentBlack,
    };
    for (mode, f) in [
        (AddressMode::Border, Features::BORDER_COLOR),
        (AddressMode::MirrorOnce, Features::MIRROR_ONCE),
    ] {
        let mut d = sampler_desc;
        d.address[1] = mode;
        by_caps(b.create_sampler(&d).map(drop), f, "an address mode");
    }
    let mut d = sampler_desc;
    d.anisotropy = caps.limits.max_anisotropy;
    b.create_sampler(&d).expect("anisotropy at the limit");
    d.anisotropy += 1;
    assert!(
        matches!(b.create_sampler(&d), Err(GpuError::Invalid(_))),
        "anisotropy past the limit"
    );
    let compute = ShaderModule::from_wgsl(
        "@compute @workgroup_size(1) fn main() {}",
        "main",
        ShaderStage::Compute,
    )
    .expect("the WGSL validates");
    absent(
        b.create_shader(&compute).map(drop),
        Features::COMPUTE,
        "a compute shader",
    );
    for (wgsl, entry, stage, f) in [
        (
            extensions::CLIPPED,
            "vs",
            ShaderStage::Vertex,
            Features::CLIP_DISTANCE,
        ),
        (
            extensions::TWO,
            "fs_second",
            ShaderStage::Fragment,
            Features::DUAL_SOURCE_BLEND,
        ),
    ] {
        let m = extensions::module_all(wgsl, entry, stage);
        by_caps(b.create_shader(&m).map(drop), f, entry);
    }
    // Shaders reading or writing what the caps may lack, refused when they are made.
    const OUT: &str = "-> @location(0) vec4<f32>";
    let fragment = |head: &str, args: &str, body: &str| {
        format!("{head}\n@fragment fn fs({args}) {OUT} {{ {body} }}")
    };
    let wgsl_absent = [
        (
            fragment("", "@builtin(sample_index) s: u32", "return vec4<f32>(f32(s));"),
            "sample_index",
            Features::SAMPLE_SHADING,
        ),
        (
            "struct O { @location(0) c: vec4<f32>, @builtin(sample_mask) m: u32 }\n\
             @fragment fn fs(@builtin(sample_mask) m: u32) -> O { return O(vec4<f32>(1.0), m & 1u); }"
                .into(),
            "sample_mask",
            Features::SAMPLE_SHADING,
        ),
        (
            fragment(
                "",
                "@location(0) @interpolate(perspective, sample) v: vec4<f32>",
                "return v;",
            ),
            "interpolated per sample",
            Features::SAMPLE_SHADING,
        ),
        (
            fragment(
                "@group(0) @binding(0) var t: texture_multisampled_2d<f32>;",
                "@builtin(position) p: vec4<f32>",
                "return textureLoad(t, vec2<i32>(p.xy), 0);",
            ),
            "a multisampled texture",
            Features::MULTISAMPLE_READ,
        ),
        (
            fragment(
                "@group(0) @binding(0) var t: texture_2d<f32>;",
                "",
                "return vec4<f32>(f32(textureNumLevels(t)));",
            ),
            "textureNumLevels",
            Features::TEXTURE_LEVEL_QUERY,
        ),
        (
            fragment(
                "@group(0) @binding(0) var<storage, read_write> s: array<u32>;",
                "",
                "s[0] = 1u; return vec4<f32>(1.0);",
            ),
            "a storage buffer written",
            Features::FRAGMENT_STORAGE_WRITE,
        ),
        (
            fragment(
                "@group(0) @binding(0) var<storage, read> s: array<vec4<f32>>;",
                "",
                "return s[0];",
            ),
            "a storage buffer read",
            Features::FRAGMENT_STORAGE_WRITE,
        ),
        (
            fragment(
                "@group(0) @binding(0) var t: texture_storage_2d<r32uint, write>;",
                "@builtin(position) p: vec4<f32>",
                "textureStore(t, vec2<i32>(p.xy), vec4<u32>(1u)); return vec4<f32>(1.0);",
            ),
            "a storage texture",
            Features::FRAGMENT_STORAGE_WRITE,
        ),
        (
            fragment(
                "enable f16;",
                "@builtin(position) p: vec4<f32>",
                "return vec4<f32>(f32(f16(p.x)));",
            ),
            "16-bit floats",
            Features::SHADER_F16,
        ),
        (
            fragment(
                "enable primitive_index;",
                "@builtin(primitive_index) i: u32",
                "return vec4<f32>(f32(i));",
            ),
            "primitive_index",
            Features::PRIMITIVE_INDEX,
        ),
        (
            fragment(
                "",
                "@builtin(position) p: vec4<f32>",
                "return vec4<f32>(dpdxFine(p.x), dpdyFine(p.y), fwidthFine(p.x), 1.0);",
            ),
            "fine derivatives",
            Features::FINE_DERIVATIVES,
        ),
    ];
    for (wgsl, what, f) in wgsl_absent {
        let m = extensions::module_all(&wgsl, "fs", ShaderStage::Fragment);
        by_caps(b.create_shader(&m).map(drop), f, what);
    }
    // Coarse derivatives are GLSL's plain ones, which may be coarse: no bit names them.
    let coarse = fragment(
        "",
        "@builtin(position) p: vec4<f32>",
        "return vec4<f32>(dpdxCoarse(p.x), dpdyCoarse(p.y), fwidthCoarse(p.x), 1.0);",
    );
    let m = extensions::module_all(&coarse, "fs", ShaderStage::Fragment);
    let made = b.create_shader(&m).expect("coarse derivatives");
    b.destroy(Object::Shader(made));
    for (wgsl, what, f) in [
        (
            "@group(0) @binding(0) var<storage, read> s: array<vec4<f32>>;\n\
             @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> { return s[i]; }",
            "a storage buffer read by a vertex program",
            Features::VERTEX_STORAGE_READ,
        ),
        (
            "@group(0) @binding(0) var<storage, read_write> s: array<vec4<f32>>;\n\
             @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> { s[i] = vec4<f32>(1.0); return s[0]; }",
            "a storage buffer written by a vertex program",
            Features::VERTEX_STORAGE_WRITE,
        ),
    ] {
        let m = extensions::module_all(wgsl, "vs", ShaderStage::Vertex);
        by_caps(b.create_shader(&m).map(drop), f, what);
    }
    let buffer = buffer(
        &mut b,
        BufferUsage::UNIFORM | BufferUsage::VERTEX,
        &[0; 256],
    );
    let range = BufferRange {
        buffer: &buffer,
        offset: 0,
        size: 256,
    };
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        Format::R32Uint,
        [4, 4, 1],
        1,
        TextureUsage::SAMPLE,
        false,
    );
    let view = kit::target(&mut b, &t, Format::R32Uint, 0, 0);
    let entry = |resource| BindingEntry {
        binding: 0,
        stages: StageSet::FRAGMENT,
        resource,
    };
    for (what, resource, f) in [
        (
            "a storage buffer",
            BindingResource::StorageBuffer {
                range,
                writable: false,
            },
            Features::FRAGMENT_STORAGE_WRITE | Features::VERTEX_STORAGE_READ,
        ),
        (
            "a storage texture",
            BindingResource::StorageTexture {
                view: &view,
                writable: true,
            },
            Features::FRAGMENT_STORAGE_WRITE | Features::VERTEX_STORAGE_READ,
        ),
        (
            "a texel buffer",
            BindingResource::TexelBuffer {
                range,
                format: Format::R32Uint,
                writable: false,
            },
            Features::TEXEL_BUFFERS,
        ),
    ] {
        absent(b.create_binding_set(&[entry(resource)]).map(drop), f, what);
    }
    absent(
        b.create_resource_table(16).map(drop),
        Features::RESOURCE_TABLES,
        "a resource table",
    );
    absent(
        b.create_query_set(QueryKind::Occlusion, 1).map(drop),
        Features::OCCLUSION_COUNT,
        "exact occlusion counts",
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
    // Draws asking for what is absent break the interface's rules: each loses its device.
    for what in ["an indirect draw", "immediates past the limit"] {
        let mut b = H::make();
        let k = kit::Kit::new(&mut b);
        let program = kit::program(&mut b, WRITE_ONE, "vs", "fs");
        let f = Format::R32Uint;
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            f,
            [4, 4, 1],
            1,
            TextureUsage::RENDER,
            false,
        );
        let v = kit::target(&mut b, &t, f, 0, 0);
        let args = self::buffer(&mut b, BufferUsage::INDIRECT, &words(&[3, 1, 0, 0]));
        let vp = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
        let immediates = vec![0u8; b.caps().limits.immediate_bytes as usize + 4];
        let mut call = kit::call(&k, &program, None, &vp, Topology::TriangleList, 3);
        if what == "an indirect draw" {
            call.range = DrawRange::Indirect {
                args: &args,
                offset: 0,
            };
        } else {
            call.immediates = &immediates;
        }
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&call);
        b.end_pass();
        assert_eq!(b.status(), DeviceStatus::Lost(LossReason::Error), "{what}");
    }
}

/// Copies into and out of textures kept on the host (RGB9E5 and block-compressed ones): between two
/// of them, into an uncompressed texture whose texels are the blocks and back
/// (`HostCopyReadBack`), from a buffer and into one, each read back as the blocks moved. A solid
/// BC1 block copied in from an `rg32uint` texture loads as its colour, so the GPU's form is written
/// too.
pub fn copies_into_and_out_of_host_kept_textures<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LOAD_CORNER, "vs", "fs");
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC;
    macro_rules! region {
        ($texture:expr, $layer:expr, $x:expr, $y:expr) => {
            TextureRegion {
                texture: $texture,
                mip: 0,
                layer: $layer,
                origin: Origin { x: $x, y: $y, z: 0 },
                stencil: false,
            }
        };
    }
    let extent = |width, height, depth_or_layers| Extent {
        width,
        height,
        depth_or_layers,
    };
    for (n, (format, plain)) in [
        (F::Bc1RgbaUnorm, Some(F::Rg32Uint)),
        (F::Bc3RgbaUnorm, Some(F::Rgba32Uint)),
        (F::Bc7RgbaUnormSrgb, Some(F::Rgba32Uint)),
        (F::Bc4RSnorm, Some(F::Rg32Uint)),
        (F::Rgb9e5Float, None),
    ]
    .into_iter()
    .enumerate()
    {
        let want = FormatUse::SAMPLE | FormatUse::COPY_OUT;
        if !b.caps().formats.get(format).contains(want) {
            continue;
        }
        let (bw, bh, bytes) = format.block();
        let (cols, rows) = (8 / bw, 8 / bh);
        let block = bytes as usize;
        let row = (cols * bytes) as usize;
        let layer_bytes = row * rows as usize;
        // What texture `into` should hold, both layers, as rows of blocks.
        let at = |layer: u32, bx: u32, by: u32| {
            layer as usize * layer_bytes + by as usize * row + bx as usize * block
        };
        let new = |b: &mut H::Backend, bytes: &[u8]| {
            let t = kit::texture(b, TextureDimension::D2, format, [8, 8, 2], 1, usage, false);
            b.write_texture(
                &region!(&t, 0, 0, 0),
                extent(8, 8, 2),
                bytes,
                row as u32,
                rows,
                WriteMode::Ordered,
            );
            t
        };
        let read = |b: &mut H::Backend, t: &<H::Backend as Backend>::Texture| {
            let mut got = vec![0; layer_bytes * 2];
            b.read_back_now(
                &ReadSource::Texture {
                    region: region!(t, 0, 0, 0),
                    extent: extent(8, 8, 2),
                },
                &mut got,
            )
            .expect("read_back_now");
            got
        };
        let from_bytes = pattern(layer_bytes * 2, 0x3c6e + n as u32);
        let from = new(&mut b, &from_bytes);
        let mut model = pattern(layer_bytes * 2, 0x7f4a + n as u32);
        let into = new(&mut b, &model);
        // Between two kept textures: the right half of the top of layer 1 into the bottom left of
        // layer 0.
        b.copy_texture(
            &region!(&from, 1, 4, 0),
            &region!(&into, 0, 0, 4),
            extent(4, 4, 1),
        );
        for by in 0..rows / 2 {
            for bx in 0..cols / 2 {
                let (s, d) = (at(1, cols / 2 + bx, by), at(0, bx, rows / 2 + by));
                model[d..d + block].copy_from_slice(&from_bytes[s..s + block]);
            }
        }
        assert_eq!(
            read(&mut b, &into),
            model,
            "{format:?}: copied between kept textures"
        );
        // From a buffer at a pitch of 256: the top half of layer 1.
        let pitch = 256usize;
        let rows_half = rows as usize / 2;
        let mut staged = vec![0; pitch * rows_half];
        let fresh = pattern(row * rows_half, 0x2b99 + n as u32);
        for r in 0..rows_half {
            staged[r * pitch..r * pitch + row].copy_from_slice(&fresh[r * row..(r + 1) * row]);
            let d = at(1, 0, r as u32);
            model[d..d + row].copy_from_slice(&fresh[r * row..(r + 1) * row]);
        }
        let staging = buffer(&mut b, BufferUsage::VERTEX, &staged);
        let readbacks = b.slow_paths().count(SlowPath::HostCopyReadBack);
        b.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &staging,
                offset: 0,
                row_pitch: pitch as u32,
                rows_per_layer: rows_half as u32,
            },
            &region!(&into, 1, 0, 0),
            extent(8, 4, 1),
        );
        assert_eq!(
            b.slow_paths().count(SlowPath::HostCopyReadBack),
            readbacks + 1,
            "{format:?}: the buffer read back"
        );
        assert_eq!(
            read(&mut b, &into),
            model,
            "{format:?}: copied from a buffer"
        );
        // Into a buffer, rows apart by one block more than they hold and layers by a row more.
        let (pitch, per_layer) = (row + block, rows as usize + 1);
        let out = buffer(
            &mut b,
            BufferUsage::VERTEX,
            &vec![0xee; pitch * per_layer * 2],
        );
        b.copy_texture_to_buffer(
            &region!(&into, 0, 0, 0),
            &CopyBufferTexture {
                buffer: &out,
                offset: 0,
                row_pitch: pitch as u32,
                rows_per_layer: per_layer as u32,
            },
            extent(8, 8, 2),
        );
        let got = read_buffer(&mut b, &out, pitch * per_layer * 2);
        let mut want = vec![0xee; pitch * per_layer * 2];
        for layer in 0..2 {
            for r in 0..rows as usize {
                let s = layer * layer_bytes + r * row;
                let d = (layer * per_layer + r) * pitch;
                want[d..d + row].copy_from_slice(&model[s..s + row]);
            }
        }
        assert_eq!(got, want, "{format:?}: copied into a buffer");
        let Some(plain) = plain else { continue };
        // Out to an uncompressed texture whose texels are the blocks, then back in.
        let texel_usage = usage | TextureUsage::RENDER;
        let p = kit::texture(
            &mut b,
            TextureDimension::D2,
            plain,
            [cols, rows, 1],
            1,
            texel_usage,
            false,
        );
        b.copy_texture(
            &region!(&into, 1, 0, 0),
            &region!(&p, 0, 0, 0),
            extent(8, 8, 1),
        );
        let texels = kit::read(&mut b, &p, 0, 0, [cols, rows, 1], block, false);
        assert_eq!(
            texels,
            &model[layer_bytes..],
            "{format:?}: copied into {plain:?}"
        );
        let written = pattern(block * (cols * rows) as usize, 0x6d2b + n as u32);
        let mut solid = written.clone();
        if format == F::Bc1RgbaUnorm {
            // Texel (0, 0) a solid red block.
            solid[..8].copy_from_slice(&[0, 0xf8, 0, 0xf8, 0, 0, 0, 0]);
        }
        b.write_texture(
            &region!(&p, 0, 0, 0),
            extent(cols, rows, 1),
            &solid,
            cols * bytes,
            rows,
            WriteMode::Ordered,
        );
        // Texel (1, 1) into the block at the top left of layer 0, and texel (0, 0) into a
        // texture of its own.
        b.copy_texture(
            &region!(&p, 0, 1, 1),
            &region!(&into, 0, 0, 0),
            extent(1, 1, 1),
        );
        let s = (cols as usize + 1) * block;
        let d = at(0, 0, 0);
        model[d..d + block].copy_from_slice(&solid[s..s + block]);
        assert_eq!(
            b.slow_paths().count(SlowPath::HostCopyReadBack),
            readbacks + 2,
            "{format:?}: {plain:?} read back"
        );
        assert_eq!(
            read(&mut b, &into),
            model,
            "{format:?}: copied from {plain:?}"
        );
        if format == F::Bc1RgbaUnorm {
            let single = kit::texture(
                &mut b,
                TextureDimension::D2,
                format,
                [4, 4, 1],
                1,
                usage,
                false,
            );
            b.copy_texture(
                &region!(&p, 0, 0, 0),
                &region!(&single, 0, 0, 0),
                extent(1, 1, 1),
            );
            let view = kit::view(&mut b, &single, format, ViewDimension::D2, 0, 1, 0, 1);
            let set = b
                .create_binding_set(&[BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&view),
                }])
                .expect("create_binding_set");
            let target = F::Rgba8Unorm;
            let out = kit::texture(
                &mut b,
                TextureDimension::D2,
                target,
                [1, 1, 1],
                1,
                TextureUsage::RENDER | TextureUsage::COPY_SRC,
                false,
            );
            let out_view = kit::target(&mut b, &out, target, 0, 0);
            b.begin_pass(&kit::pass(&out_view, [0; 4], None));
            b.draw(&kit::call(
                &k,
                &program,
                Some(&set),
                &[kit::viewport(0.0, 0.0, 1.0, 1.0)],
                Topology::TriangleList,
                3,
            ));
            b.end_pass();
            let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
            assert_eq!(got, [255, 0, 0, 255], "the copied solid block loaded");
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// An sRGB view of a BC1 texture decodes its colours from sRGB, and follows writes made after the
/// view: a grey block loads about 132 through the texture's own view and about 59 through the sRGB
/// one (one step either way, for decoders expanding 5 bits differently), then a white block 255.
pub fn srgb_views_of_block_compressed_textures<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let want = FormatUse::SAMPLE;
    if !b.caps().formats.get(F::Bc1RgbaUnorm).contains(want) {
        return;
    }
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LOAD_CORNER, "vs", "fs");
    let target = F::Rgba8Unorm;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        target,
        [1, 1, 1],
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let out_view = kit::target(&mut b, &out, target, 0, 0);
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        F::Bc1RgbaUnorm,
        [4, 4, 1],
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        false,
    );
    let write = |b: &mut H::Backend, colour: u16| {
        let c = colour.to_le_bytes();
        b.write_texture(
            &TextureRegion {
                texture: &t,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: 4,
                height: 4,
                depth_or_layers: 1,
            },
            &[c[0], c[1], c[0], c[1], 0, 0, 0, 0],
            8,
            1,
            WriteMode::Ordered,
        );
    };
    let load = |b: &mut H::Backend, format| {
        let view = kit::view(b, &t, format, ViewDimension::D2, 0, 1, 0, 1);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &[kit::viewport(0.0, 0.0, 1.0, 1.0)],
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        kit::read(b, &out, 0, 0, [1, 1, 1], 4, false)[0]
    };
    // Grey: red 16 of 31 in 5:6:5.
    write(&mut b, (16 << 11) | (32 << 5) | 16);
    let linear = load(&mut b, F::Bc1RgbaUnorm);
    assert!((131..=133).contains(&linear), "linear view loads {linear}");
    let srgb = load(&mut b, F::Bc1RgbaUnormSrgb);
    assert!((58..=60).contains(&srgb), "sRGB view loads {srgb}");
    write(&mut b, 0xffff);
    assert_eq!(
        load(&mut b, F::Bc1RgbaUnormSrgb),
        255,
        "after a later write"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// BGRA textures keep their byte order through every route, stored as RGBA with red and blue
/// swapped on the host (`FormatConverted`): written, read back now and later, loaded (red where the
/// bytes put red), cleared as a target, and copied into a vertex buffer and an index-only one and
/// back (`HostCopyReadBack`). An `r8uint` texture, which `readPixels` reads wider than stored,
/// copies into a buffer the same way.
pub fn bgra_textures_keep_their_byte_order<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LOAD_CORNER, "vs", "fs");
    macro_rules! region {
        ($texture:expr) => {
            TextureRegion {
                texture: $texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            }
        };
    }
    let extent = |width, height| Extent {
        width,
        height,
        depth_or_layers: 1,
    };
    // Texel i of a 4 by 2 texture: blue 10 + i, green 100 + i, red 200 + i, alpha 250 - i.
    let bgra: Vec<u8> = (0..8u8)
        .flat_map(|i| [10 + i, 100 + i, 200 + i, 250 - i])
        .collect();
    let rgba8 = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        F::Rgba8Unorm,
        [1, 1, 1],
        1,
        rgba8,
        false,
    );
    let out_view = kit::target(&mut b, &out, F::Rgba8Unorm, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 1.0, 1.0)];
    let usage = TextureUsage::SAMPLE
        | TextureUsage::RENDER
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    for format in [F::Bgra8Unorm, F::Bgra8UnormSrgb] {
        let want = FormatUse::SAMPLE | FormatUse::RENDER | FormatUse::COPY_OUT;
        assert!(
            b.caps().formats.get(format).contains(want),
            "{format:?}: {want:?}"
        );
        let before = b.slow_paths().count(SlowPath::FormatConverted);
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 2, 1],
            1,
            usage,
            false,
        );
        b.write_texture(&region!(&t), extent(4, 2), &bgra, 16, 2, WriteMode::Ordered);
        let written = b.slow_paths().count(SlowPath::FormatConverted);
        assert!(written > before);
        assert_eq!(
            kit::read(&mut b, &t, 0, 0, [4, 2, 1], 4, false),
            bgra,
            "{format:?} now"
        );
        let read = b.slow_paths().count(SlowPath::FormatConverted);
        assert!(
            read > written,
            "{format:?}: a read swapping on the host is not counted"
        );
        let rb = b
            .read_back(&ReadSource::Texture {
                region: region!(&t),
                extent: extent(4, 2),
            })
            .expect("read_back");
        b.submit();
        b.wait_read(&rb);
        let mut later = vec![0; 32];
        b.finish_read(rb, &mut later);
        assert_eq!(later, bgra, "{format:?} later");
        assert!(b.slow_paths().count(SlowPath::FormatConverted) > read);
        // Loaded through a view of its own format, texel (0, 0) is red 200 (sRGB decoded for the
        // sRGB format).
        let view = kit::view(&mut b, &t, format, ViewDimension::D2, 0, 1, 0, 1);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
        let expected = if format == F::Bgra8Unorm {
            [200, 100, 10, 250]
        } else {
            [147, 32, 1, 250]
        };
        for (c, (g, e)) in got.iter().zip(expected).enumerate() {
            assert!(
                g.abs_diff(e) <= 1,
                "{format:?} loaded: {got:?}, expected {expected:?} ({c})"
            );
        }
        // Copied into a vertex buffer and an index-only buffer, and from each into a second texture.
        for usage in [BufferUsage::VERTEX, BufferUsage::INDEX] {
            let buf = buffer(&mut b, usage, &[0; 64]);
            let into = CopyBufferTexture {
                buffer: &buf,
                offset: 0,
                row_pitch: 32,
                rows_per_layer: 2,
            };
            b.copy_texture_to_buffer(&region!(&t), &into, extent(4, 2));
            let got = read_buffer(&mut b, &buf, 64);
            assert_eq!(
                &got[..16],
                &bgra[..16],
                "{format:?} row 0 in a {usage:?} buffer"
            );
            assert_eq!(
                &got[32..48],
                &bgra[16..],
                "{format:?} row 1 in a {usage:?} buffer"
            );
            let plain = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
            let t2 = kit::texture(
                &mut b,
                TextureDimension::D2,
                format,
                [4, 2, 1],
                1,
                plain,
                false,
            );
            b.copy_buffer_to_texture(&into, &region!(&t2), extent(4, 2));
            let back = kit::read(&mut b, &t2, 0, 0, [4, 2, 1], 4, false);
            assert_eq!(back, bgra, "{format:?} from a {usage:?} buffer");
            b.destroy(Object::Texture(t2));
            b.destroy(Object::Buffer(buf));
        }
        // Cleared as a target to red, read back blue first.
        let target = kit::target(&mut b, &t, format, 0, 0);
        b.begin_pass(&kit::pass(
            &target,
            [1f32.to_bits(), 0, 0, 1f32.to_bits()],
            None,
        ));
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [1, 1, 1], 4, false);
        assert_eq!(got, [0, 0, 255, 255], "{format:?} cleared red");
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(view));
        b.destroy(Object::TextureView(target));
        b.destroy(Object::Texture(t));
    }
    // An r8uint texture's bytes, read back as RGBA integers, land in a buffer as stored.
    let bytes: Vec<u8> = (0..8u8).map(|i| 7 * i + 3).collect();
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        F::R8Uint,
        [4, 2, 1],
        1,
        usage,
        false,
    );
    b.write_texture(&region!(&t), extent(4, 2), &bytes, 4, 2, WriteMode::Ordered);
    let buf = buffer(&mut b, BufferUsage::VERTEX, &[0; 8]);
    let into = CopyBufferTexture {
        buffer: &buf,
        offset: 0,
        row_pitch: 4,
        rows_per_layer: 2,
    };
    let before = b.slow_paths().count(SlowPath::HostCopyReadBack);
    b.copy_texture_to_buffer(&region!(&t), &into, extent(4, 2));
    assert!(b.slow_paths().count(SlowPath::HostCopyReadBack) > before);
    assert_eq!(read_buffer(&mut b, &buf, 8), bytes, "r8uint in a buffer");
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A full-screen triangle writing one colour of alpha 64 / 255 to targets 0 and 1.
const QUARTER_ALPHA: &str = "
const C = vec4<f32>(102.0, 51.0, 204.0, 64.0) / 255.0;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return C;
}
struct Targets {
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
}
@fragment fn fs_two() -> Targets {
    return Targets(C, C);
}
";

/// BGRX textures (Direct3D's `B8G8R8X8`) read their missing alpha as one through every route, the
/// X byte read back as 255 as on WebGPU: written with other X bytes and read back now, later and
/// through a buffer; loaded; drawn into with alpha 64 while blending by the destination's alpha
/// (one); cleared with alpha 0; copied; and drawn in one pass with an RGBA target, which keeps the
/// drawn alpha (per-target masks, OES_draw_buffers_indexed).
pub fn bgrx_textures_hold_alpha_one<H: Host>() {
    use crate::kit::{self, Kit};
    use Format as F;
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let load = kit::program(&mut b, LOAD_CORNER, "vs", "fs");
    let quarter = kit::program(&mut b, QUARTER_ALPHA, "vs", "fs");
    macro_rules! region {
        ($texture:expr) => {
            TextureRegion {
                texture: $texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            }
        };
    }
    let extent = Extent {
        width: 4,
        height: 2,
        depth_or_layers: 1,
    };
    // Texel i: blue 10 + i, green 100 + i, red 200 + i, X 7 i.
    let bgrx: Vec<u8> = (0..8u8)
        .flat_map(|i| [10 + i, 100 + i, 200 + i, 7 * i])
        .collect();
    let opaque: Vec<u8> = bgrx
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|t| [t[0], t[1], t[2], 255])
        .collect();
    let rgba8 = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let out = kit::texture(
        &mut b,
        TextureDimension::D2,
        F::Rgba8Unorm,
        [4, 2, 1],
        1,
        rgba8,
        false,
    );
    let out_view = kit::target(&mut b, &out, F::Rgba8Unorm, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 4.0, 2.0)];
    let usage = TextureUsage::SAMPLE
        | TextureUsage::RENDER
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    let by_dst_alpha = kit::blend(
        &mut b,
        Some((
            BlendComponent {
                src: BlendFactor::DstAlpha,
                dst: BlendFactor::Zero,
                op: BlendOp::Add,
            },
            BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::Zero,
                op: BlendOp::Add,
            },
        )),
    );
    for format in [F::Bgrx8Unorm, F::Bgrx8UnormSrgb] {
        let want = FormatUse::SAMPLE
            | FormatUse::FILTER
            | FormatUse::RENDER
            | FormatUse::BLEND
            | FormatUse::COPY_OUT;
        assert!(
            b.caps().formats.get(format).contains(want),
            "{format:?}: {want:?}"
        );
        let t = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 2, 1],
            1,
            usage,
            false,
        );
        b.write_texture(&region!(&t), extent, &bgrx, 16, 2, WriteMode::Ordered);
        assert_eq!(kit::read(&mut b, &t, 0, 0, [4, 2, 1], 4, false), opaque);
        let rb = b
            .read_back(&ReadSource::Texture {
                region: region!(&t),
                extent,
            })
            .expect("read_back");
        b.submit();
        b.wait_read(&rb);
        let mut later = vec![0; 32];
        b.finish_read(rb, &mut later);
        assert_eq!(later, opaque, "{format:?} later");
        // Loaded: alpha one.
        let view = kit::view(&mut b, &t, format, ViewDimension::D2, 0, 1, 0, 1);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&out_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &load,
            Some(&set),
            &vp,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let got = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
        assert_eq!(got[3], 255, "{format:?} loaded: {got:?}");
        // Drawn into with alpha 64, scaled by the destination's alpha: colour as drawn, alpha one.
        let target = kit::target(&mut b, &t, format, 0, 0);
        let mut pass = kit::pass(&target, [0; 4], None);
        if let Some(c) = &mut pass.colors[0] {
            c.load = LoadOp::Load;
        }
        b.begin_pass(&pass);
        b.draw(&DrawCall {
            blend: &by_dst_alpha,
            ..kit::call(&k, &quarter, None, &vp, Topology::TriangleList, 3)
        });
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [4, 2, 1], 4, false);
        let drawn: &[u8] = if format == F::Bgrx8Unorm {
            &[204, 51, 102, 255]
        } else {
            // 102, 51 and 204 / 255 sRGB encoded.
            &[231, 124, 170, 255]
        };
        for t in got.as_chunks::<4>().0 {
            for (c, (g, w)) in t.iter().zip(drawn).enumerate() {
                assert!(g.abs_diff(*w) <= 1, "{format:?} drawn: {t:?} ({c})");
            }
        }
        // Copied into a second texture of the format and through a buffer.
        let t2 = kit::texture(
            &mut b,
            TextureDimension::D2,
            format,
            [4, 2, 1],
            1,
            usage,
            false,
        );
        b.copy_texture(&region!(&t), &region!(&t2), extent);
        assert_eq!(kit::read(&mut b, &t2, 0, 0, [4, 2, 1], 4, false), got);
        let buf = buffer(&mut b, BufferUsage::VERTEX, &[0; 32]);
        let into = CopyBufferTexture {
            buffer: &buf,
            offset: 0,
            row_pitch: 16,
            rows_per_layer: 2,
        };
        b.write_buffer(&buf, 0, &bgrx, WriteMode::Ordered);
        b.copy_buffer_to_texture(&into, &region!(&t2), extent);
        assert_eq!(kit::read(&mut b, &t2, 0, 0, [4, 2, 1], 4, false), opaque);
        b.copy_texture_to_buffer(&region!(&t), &into, extent);
        assert_eq!(read_buffer(&mut b, &buf, 32), got, "{format:?} in a buffer");
        // Cleared with alpha 0.
        b.begin_pass(&kit::pass(&target, [0, 0, 1f32.to_bits(), 0], None));
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [1, 1, 1], 4, false);
        assert_eq!(got, [255, 0, 0, 255], "{format:?} cleared blue");
        // With an RGBA target in one pass: its alpha drawn, the BGRX one left one.
        if b.caps().has(Features::INDEPENDENT_BLEND) {
            let two = kit::program(&mut b, QUARTER_ALPHA, "vs", "fs_two");
            let colors = [&out_view, &target];
            b.begin_pass(&PassDesc {
                colors: core::array::from_fn(|i| {
                    colors.get(i).map(|&view| ColorAttachment {
                        view,
                        depth_slice: 0,
                        load: LoadOp::Clear([0; 4]),
                        store: StoreOp::Store,
                        resolve: None,
                    })
                }),
                depth: None,
                occlusion: None,
                empty: None,
            });
            b.draw(&kit::call(&k, &two, None, &vp, Topology::TriangleList, 3));
            b.end_pass();
            let rgba = kit::read(&mut b, &out, 0, 0, [1, 1, 1], 4, false);
            let bgrx = kit::read(&mut b, &t, 0, 0, [1, 1, 1], 4, false);
            assert_eq!(rgba[3], 64, "{format:?}: the RGBA target {rgba:?}");
            assert_eq!(bgrx[3], 255, "{format:?}: the BGRX target {bgrx:?}");
            b.destroy(Object::Program(two));
        }
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
        b.destroy(Object::Buffer(buf));
        b.destroy(Object::Texture(t2));
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(view));
        b.destroy(Object::TextureView(target));
        b.destroy(Object::Texture(t));
    }
}

/// Every level of a texture stays as written while level 1 is sampled, level 2's first layer is
/// cleared and then each layer (or slice) of level 2 is drawn from level 1 in turn, as the mip draws
/// do, and after every level is sampled again: 2D arrays of odd and even size, a 2D texture of odd
/// size and a 3D texture, each level read back after each step.
pub fn levels_stay_as_written_while_others_are_read_and_drawn<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let k = kit::Kit::new(&mut b);
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::SAMPLE
        | TextureUsage::RENDER
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    let scratch = kit::texture(&mut b, TextureDimension::D2, f, [1, 1, 1], 1, usage, false);
    let scratch_view = kit::target(&mut b, &scratch, f, 0, 0);
    let mut failures = Vec::new();
    for (dimension, size) in [
        (TextureDimension::D2, [13, 7, 2]),
        (TextureDimension::D2, [16, 8, 2]),
        (TextureDimension::D2, [13, 7, 1]),
        (TextureDimension::D3, [8, 8, 4]),
    ] {
        let three_d = dimension == TextureDimension::D3;
        let (own, ty, at) = match (three_d, size[2]) {
            (true, _) => (ViewDimension::D3, "texture_3d<f32>", "vec3<i32>(0), 0"),
            (false, 1) => (ViewDimension::D2, "texture_2d<f32>", "vec2<i32>(0), 0"),
            _ => (
                ViewDimension::D2Array,
                "texture_2d_array<f32>",
                "vec2<i32>(0), 0, 0",
            ),
        };
        let wgsl = format!(
            "@group(0) @binding(32) var t: {ty};
            @vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {{
                return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
            }}
            @fragment fn fs() -> @location(0) vec4<f32> {{
                return textureLoad(t, {at});
            }}"
        );
        let program = kit::program(&mut b, &wgsl, "vs", "fs");
        let levels = 4;
        let t = kit::texture(&mut b, dimension, f, size, levels, usage, false);
        let shape = |mip: u32| {
            let n = |s: u32| (s >> mip).max(1);
            [
                n(size[0]),
                n(size[1]),
                if three_d { n(size[2]) } else { size[2] },
            ]
        };
        let mut want: Vec<Vec<u8>> = (0..levels)
            .map(|mip| {
                let s = shape(mip);
                let bytes = pattern((s[0] * s[1] * s[2] * 4) as usize, 11 + mip * 7 + size[0]);
                b.write_texture(
                    &TextureRegion {
                        texture: &t,
                        mip,
                        layer: 0,
                        origin: Origin::default(),
                        stencil: false,
                    },
                    Extent {
                        width: s[0],
                        height: s[1],
                        depth_or_layers: s[2],
                    },
                    &bytes,
                    s[0] * 4,
                    s[1],
                    WriteMode::Ordered,
                );
                bytes
            })
            .collect();
        let check = |b: &mut H::Backend, want: &[Vec<u8>], step: &str| {
            let mut failures = Vec::new();
            for mip in 0..levels {
                let s = shape(mip);
                let got = kit::read(b, &t, mip, 0, s, 4, false);
                let layer = (s[0] * s[1] * 4) as usize;
                let differ: Vec<usize> = got
                    .chunks(layer)
                    .zip(want[mip as usize].chunks(layer))
                    .map(|(g, w)| g.chunks(4).zip(w.chunks(4)).filter(|(a, b)| a != b).count())
                    .collect();
                if differ.iter().any(|&n| n > 0) {
                    failures.push(format!(
                        "{dimension:?} {size:?} after {step}: level {mip} differs in {differ:?} of {} texels per layer",
                        s[0] * s[1]
                    ));
                }
            }
            failures
        };
        failures.extend(check(&mut b, &want, "the writes"));
        // Level 1 sampled alone, as the mip draws' source.
        let source = kit::view(
            &mut b,
            &t,
            f,
            own,
            1,
            1,
            0,
            if three_d { 1 } else { size[2] },
        );
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&source),
            }])
            .expect("create_binding_set");
        let one = [kit::viewport(0.0, 0.0, 1.0, 1.0)];
        b.begin_pass(&kit::pass(&scratch_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &one,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        let loaded = kit::read(&mut b, &scratch, 0, 0, [1, 1, 1], 4, false);
        if loaded != want[1][..4] {
            failures.push(format!(
                "{dimension:?} {size:?}: level 1 loaded {loaded:?}, written {:?}",
                &want[1][..4]
            ));
        }
        failures.extend(check(&mut b, &want, "sampling level 1"));
        // Level 2's first layer (slice) cleared, the source still bound as the mip draws leave it.
        let s2 = shape(2);
        let level_target = |b: &mut H::Backend, layer| {
            if three_d {
                kit::view(b, &t, f, ViewDimension::D3, 2, 1, 0, 1)
            } else {
                kit::target(b, &t, f, 2, layer)
            }
        };
        let target = level_target(&mut b, 0);
        let clear = [0.2f32, 0.4, 0.6, 1.0].map(f32::to_bits);
        b.begin_pass(&kit::pass(&target, clear, None));
        b.end_pass();
        for texel in want[2][..(s2[0] * s2[1] * 4) as usize].chunks_mut(4) {
            texel.copy_from_slice(&[51, 102, 153, 255]);
        }
        failures.extend(check(&mut b, &want, "clearing level 2's first layer"));
        // Every layer (or slice) of level 2 drawn from level 1 in turn, as the mip draws do.
        let vp = [kit::viewport(0.0, 0.0, s2[0] as f32, s2[1] as f32)];
        let layer = (s2[0] * s2[1] * 4) as usize;
        let first = want[1][..4].to_vec();
        for n in 0..s2[2] {
            let (at, slice) = if three_d { (0, n) } else { (n, 0) };
            let target = level_target(&mut b, at);
            let mut pass = kit::pass(&target, [0; 4], None);
            if let Some(c) = pass.colors[0].as_mut() {
                c.depth_slice = slice;
                c.load = LoadOp::DontCare;
            }
            b.begin_pass(&pass);
            b.draw(&kit::call(
                &k,
                &program,
                Some(&set),
                &vp,
                Topology::TriangleList,
                3,
            ));
            b.end_pass();
            b.destroy(Object::TextureView(target));
            let from = n as usize * layer;
            for texel in want[2][from..from + layer].chunks_mut(4) {
                texel.copy_from_slice(&first);
            }
        }
        failures.extend(check(&mut b, &want, "drawing level 2 from level 1"));
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(source));
        // Every level sampled again.
        let layers = if three_d { 1 } else { size[2] };
        let whole = kit::view(&mut b, &t, f, own, 0, levels, 0, layers);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&whole),
            }])
            .expect("create_binding_set");
        b.begin_pass(&kit::pass(&scratch_view, [0; 4], None));
        b.draw(&kit::call(
            &k,
            &program,
            Some(&set),
            &one,
            Topology::TriangleList,
            3,
        ));
        b.end_pass();
        failures.extend(check(&mut b, &want, "sampling every level"));
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::TextureView(whole));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A pass without attachments rasterises: occlusion queries answer that a triangle covering it
/// passed and one wholly outside it did not, at one sample and at four, and it leaves the device
/// whole.
pub fn passes_without_attachments_rasterise<H: Host>() {
    use crate::kit;
    let mut b = H::make();
    let k = kit::Kit::new(&mut b);
    // A triangle over the pass, or right of it.
    let wgsl = "
    struct P { x: vec4<f32> }
    @group(0) @binding(0) var<uniform> p: P;
    @vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
        return vec4<f32>(f32((v & 1u) << 2u) - 1.0 + p.x.x, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
    }
    @fragment fn fs() -> @location(0) vec4<f32> {
        return vec4<f32>(1.0);
    }";
    let program = kit::program(&mut b, wgsl, "vs", "fs");
    let align = b.caps().limits.min_uniform_offset_alignment as u64;
    let params = b
        .create_buffer(&BufferDesc {
            size: align * 2,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            intent: MapIntent::Often,
        })
        .expect("create_buffer");
    b.write_buffer(&params, 0, &0f32.to_le_bytes(), WriteMode::Ordered);
    b.write_buffer(&params, align, &8f32.to_le_bytes(), WriteMode::Ordered);
    let sets: Vec<_> = [0, align]
        .into_iter()
        .map(|offset| {
            b.create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &params,
                    offset,
                    size: 16,
                }),
            }])
            .expect("create_binding_set")
        })
        .collect();
    let vp = [kit::viewport(0.0, 0.0, 8.0, 4.0)];
    for samples in [1, 4] {
        if !b.caps().formats.takes_samples(Format::R8Unorm, samples) {
            continue;
        }
        let queries = b
            .create_query_set(QueryKind::OcclusionAny, 2)
            .expect("create_query_set");
        b.begin_pass(&PassDesc {
            colors: Default::default(),
            depth: None,
            empty: Some(EmptyPass {
                width: 8,
                height: 4,
                samples,
            }),
            occlusion: Some(&queries),
        });
        for (query, set) in sets.iter().enumerate() {
            b.begin_query(query as u32);
            b.draw(&kit::call(
                &k,
                &program,
                Some(set),
                &vp,
                Topology::TriangleList,
                3,
            ));
            b.end_query();
        }
        b.end_pass();
        let rb = b.read_queries(&queries, 0, 2).expect("read_queries");
        b.submit();
        b.wait_read(&rb);
        let mut out = [0u8; 16];
        b.finish_read(rb, &mut out);
        let passed: Vec<bool> = out.chunks(8).map(|c| c.iter().any(|&x| x != 0)).collect();
        assert_eq!(passed, [true, false], "{samples} samples");
        assert_eq!(b.status(), DeviceStatus::Ok, "{samples} samples");
    }
}

/// Immediates of every kind of type a struct holds: a float, a signed vector, an unsigned vector
/// read in the vertex stage, a matrix (columns 16 bytes apart) and a vector of three.
const IMMEDIATES: &str = "
struct Imm {
    a: f32,
    b: vec2<i32>,
    c: vec4<u32>,
    m: mat3x3<f32>,
    d: vec3<f32>,
    e: u32,
}
var<immediate> k: Imm;
struct Out {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) c: u32,
}
@vertex fn vs(@builtin(vertex_index) v: u32) -> Out {
    return Out(vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0), k.c.x);
}
@fragment fn fs(i: Out) -> @location(0) vec4<u32> {
    return vec4<u32>(i.c, bitcast<u32>(k.a), bitcast<u32>(k.b.y) ^ k.e, bitcast<u32>(k.m[2][1] + k.d.z));
}
";

/// Immediates reach both stages as plain uniforms, at the offsets the WGSL layout gives (`c` at 16,
/// `m` at 32 with columns 16 apart, `d` at 80, `e` at 92), set again only when they change; bytes a
/// draw does not give read zero. Each draw writes one texel of an `rgba32uint` target.
pub fn immediates_reach_both_stages<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    assert!(b.caps().limits.immediate_bytes >= 96);
    let modules = ["vs", "fs"].map(|entry| {
        let module = naga::front::wgsl::parse_str(IMMEDIATES).expect("the WGSL parses");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::IMMEDIATES,
        )
        .validate(&module)
        .expect("the WGSL validates");
        ShaderModule {
            module,
            info,
            entry_point: entry.into(),
            stage: if entry == "vs" {
                ShaderStage::Vertex
            } else {
                ShaderStage::Fragment
            },
            key: [0; 32],
        }
    });
    let v = b.create_shader(&modules[0]).expect("create_shader");
    let f = b.create_shader(&modules[1]).expect("create_shader");
    let program = b
        .create_program(&Program::Render {
            vertex: &v,
            fragment: Some(&f),
        })
        .expect("create_program");
    let k = Kit::new(&mut b);
    let format = Format::Rgba32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        format,
        [3, 1, 1],
        1,
        usage,
        false,
    );
    let target = kit::target(&mut b, &t, format, 0, 0);
    let immediates = |a: f32, cx: u32| {
        let mut bytes = vec![0u8; 96];
        let mut put = |at: usize, w: [u8; 4]| bytes[at..at + 4].copy_from_slice(&w);
        put(0, a.to_le_bytes());
        put(8, (-3i32).to_le_bytes());
        put(12, (-7i32).to_le_bytes());
        for (i, c) in [cx, 12, 13, 14].iter().enumerate() {
            put(16 + 4 * i, c.to_le_bytes());
        }
        // m[2] = (0.25, 0.5, 0.75), at 32 + 2 * 16.
        for (i, x) in [0.25f32, 0.5, 0.75].iter().enumerate() {
            put(64 + 4 * i, x.to_le_bytes());
        }
        for (i, x) in [1.0f32, 2.0, 4.0].iter().enumerate() {
            put(80 + 4 * i, x.to_le_bytes());
        }
        put(92, 0x100u32.to_le_bytes());
        bytes
    };
    let first = immediates(1.5, 11);
    let second = immediates(2.5, 21);
    let short = 1.25f32.to_le_bytes();
    let draws: [&[u8]; 4] = [&first, &second, &second, &short];
    b.begin_pass(&kit::pass(&target, [0; 4], None));
    let mut want = Vec::new();
    for (i, bytes) in draws.iter().enumerate().take(3) {
        let vp = [kit::viewport(i as f32, 0.0, 1.0, 1.0)];
        let mut call = kit::call(&k, &program, None, &vp, Topology::TriangleList, 3);
        call.immediates = bytes;
        b.draw(&call);
    }
    // The short draw over the third texel: everything past its four bytes reads zero.
    let vp = [kit::viewport(2.0, 0.0, 1.0, 1.0)];
    let mut call = kit::call(&k, &program, None, &vp, Topology::TriangleList, 3);
    call.immediates = draws[3];
    b.draw(&call);
    b.end_pass();
    for (a, cx) in [(1.5f32, 11u32), (2.5, 21)] {
        want.extend([cx, a.to_bits(), (-7i32 as u32) ^ 0x100, 4.5f32.to_bits()]);
    }
    want.extend([0, 1.25f32.to_bits(), 0, 0]);
    let got: Vec<u32> = kit::read(&mut b, &t, 0, 0, [3, 1, 1], 16, false)
        .chunks(4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect();
    assert_eq!(
        got, want,
        "texels: the first immediates, the second, the short ones"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Reads the three vertex formats GL ES cannot read as stored and writes what it read: row 0 the
/// BGRA colour (as bytes), the 10-10-10-2 integers (packed again) and the 11-11-10 red's bits; row
/// 1 the green's and blue's bits.
const PACKED_ATTRIBUTES: &str = "
struct In {
    @builtin(vertex_index) v: u32,
    @location(1) colour: vec4<f32>,
    @location(2) ints: vec4<u32>,
    @location(3) floats: vec3<f32>,
}
struct Out {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) colour: vec4<f32>,
    @location(1) @interpolate(flat) ints: vec4<u32>,
    @location(2) @interpolate(flat) floats: vec3<f32>,
}
fn at(i: In, y: f32) -> Out {
    let x = (f32(i.v) * 2.0 + 1.0) / 3.0 - 1.0;
    return Out(vec4<f32>(x, y, 0.0, 1.0), i.colour, i.ints, i.floats);
}
@vertex fn vs_a(i: In) -> Out { return at(i, 0.5); }
@vertex fn vs_b(i: In) -> Out { return at(i, -0.5); }
@fragment fn fs_a(o: Out) -> @location(0) vec4<u32> {
    let c = vec4<u32>(round(o.colour * 255.0));
    let u = o.ints;
    return vec4<u32>(c.x | (c.y << 8u) | (c.z << 16u) | (c.w << 24u),
        u.x | (u.y << 10u) | (u.z << 20u) | (u.w << 30u), bitcast<u32>(o.floats.x), 0u);
}
@fragment fn fs_b(o: Out) -> @location(0) vec4<u32> {
    return vec4<u32>(bitcast<u32>(o.floats.y), bitcast<u32>(o.floats.z), 0u, 0u);
}
";

/// `Bgra8Unorm`, `Uint10_10_10_2` and `Float11_11_10` attributes (which GL ES reads as bytes or one
/// word, converted in a program variant: `VertexFormatConverted`) reach the program with Direct3D's
/// meaning: red from the third byte; red in the low ten bits; red the low eleven bits as a float of
/// five exponent and six mantissa bits (a normal, a denormal, zero, infinity), blue the top ten.
pub fn packed_vertex_formats_are_converted<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let mut layout = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    layout.streams[0] = Some(VertexStream {
        stride: 12,
        step: StepMode::Vertex,
    });
    for (i, format) in [
        VertexFormat::Bgra8Unorm,
        VertexFormat::Uint10_10_10_2,
        VertexFormat::Float11_11_10,
    ]
    .into_iter()
    .enumerate()
    {
        layout.attributes[i] = Some(VertexAttribute {
            location: 1 + i as u32,
            stream: 0,
            offset: 4 * i as u32,
            format,
        });
    }
    let layout = b
        .create_vertex_layout(&layout)
        .expect("create_vertex_layout");
    // Per vertex: green of 1.5, 2^-20 * 3 (a denormal), 2, 0; blue 0.75, 100, infinity.
    let small = |r: u32, g: u32, bl: u32| r | g << 11 | bl << 22;
    let floats = [
        small(15 << 6 | 32, 3, 14 << 5 | 16),
        small(15 << 6 | 32, 16 << 6, 21 << 5 | 18),
        small(15 << 6 | 32, 0, 31 << 5),
    ];
    let mut bytes = Vec::new();
    let mut want_a = Vec::new();
    for (i, f) in floats.iter().enumerate() {
        let i = i as u32;
        bytes.extend([10 + i as u8, 20, 30, 40]);
        let ints = (1000 - i) | 513 << 10 | 7 << 20 | i << 30;
        bytes.extend(ints.to_le_bytes());
        bytes.extend(f.to_le_bytes());
        want_a.extend([
            30 | 20 << 8 | (10 + i) << 16 | 40 << 24,
            ints,
            1.5f32.to_bits(),
            0,
        ]);
    }
    let want_b: Vec<u32> = [
        (3.0 * 2f32.powi(-20), 0.75),
        (2.0, 100.0),
        (0.0, f32::INFINITY),
    ]
    .iter()
    .flat_map(|(g, bl): &(f32, f32)| [g.to_bits(), bl.to_bits(), 0, 0])
    .collect();
    let vb = buffer(&mut b, BufferUsage::VERTEX, &bytes);
    let k = Kit::new(&mut b);
    let format = Format::Rgba32Uint;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(
        &mut b,
        TextureDimension::D2,
        format,
        [3, 2, 1],
        1,
        usage,
        false,
    );
    let target = kit::target(&mut b, &t, format, 0, 0);
    let programs = [("vs_a", "fs_a"), ("vs_b", "fs_b")]
        .map(|(v, f)| kit::program(&mut b, PACKED_ATTRIBUTES, v, f));
    let vp = [kit::viewport(0.0, 0.0, 3.0, 2.0)];
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let before = b.slow_paths().count(SlowPath::VertexFormatConverted);
    b.begin_pass(&kit::pass(&target, [0; 4], None));
    for p in &programs {
        let mut call = kit::call(&k, p, None, &vp, Topology::PointList, 3);
        call.vertex_layout = &layout;
        call.vertex_buffers = &streams;
        b.draw(&call);
    }
    b.end_pass();
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(
        b.slow_paths().count(SlowPath::VertexFormatConverted) - before,
        2
    );
    let got: Vec<u32> = kit::read(&mut b, &t, 0, 0, [3, 2, 1], 16, false)
        .chunks(4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect();
    assert_eq!(got[..12], want_a[..], "row 0: colour, integers, red");
    assert_eq!(got[12..], want_b[..], "row 1: green and blue");
}
