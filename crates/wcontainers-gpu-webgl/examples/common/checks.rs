//! Checks of the WebGL 2 back end's own routes, beside the shared suite and run by the same hosts
//! (tests/suite.rs on ANGLE, examples/browser.rs in Chrome): each panics on a wrong answer.

use wcontainers_gpu::*;
use wcontainers_gpu_test::Host;

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
/// element]); per-instance elements are 100 plus their number.
fn expected(range: &Range, first_instance: u32, instances: u32) -> Vec<(usize, [u32; 4])> {
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
        for v in &vertices {
            out.push((((i % 4) * 8 + v % 8) as usize, [*v, i, *v, 100 + i]));
        }
    }
    out
}

/// `vertex_index` and `instance_index` include the draw's first or base vertex and first instance,
/// and vertex streams are read from those elements on, in non-indexed and indexed (8, 16 and 32-bit)
/// instanced draws. WebGL 2 draws with no bases: the streams are bound at an offset
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
        step: StepMode::Instance(1),
    });
    for (i, location) in [1, 2].into_iter().enumerate() {
        layout.attributes[i] = Some(VertexAttribute {
            location,
            stream: i as u8,
            offset: 0,
            format: VertexFormat::Uint32x1,
        });
    }
    let layout = b
        .create_vertex_layout(&layout)
        .expect("create_vertex_layout");
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
        ("non-indexed", Range::Vertices { first: 3, count: 4 }, 2, 2),
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
            2,
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
            3,
            1,
        ),
    ];
    for (name, range, first_instance, instances) in cases {
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
        let want = expected(&range, first_instance, instances);
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
/// to depth 0.25 and stencil 0x5a, then `clear_rects` sets the middle 2 by 2 to 0.75 and 0xc3. Depth
/// in unorm formats within one step (GL lets the conversion round either way). Formats the back end
/// cannot read back (no `COPY_OUT`) are left out: stencil needs WEBGL_stencil_texturing.
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
                depth: Some(0.75f32.to_bits()),
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
            Format::Depth16Unorm => (2, 65535.0, 1),
            Format::Depth24Plus | Format::Depth24PlusStencil8 => (4, 16_777_215.0, 1),
            _ => (4, 0.0, 0),
        };
        let depth = read(&mut b, false, bytes);
        for p in 0..16 {
            let want = if inside(p) { 0.75f64 } else { 0.25 };
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
    let mut x = seed | 1;
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
/// another place, and read back. Pairs whose destination does not read back here are skipped; so is a
/// pair whose formats the back end lacks.
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
        (Format::R32Float, Format::R32Uint),
        (Format::R32Uint, Format::R32Float),
        (Format::R32Sint, Format::R32Uint),
        (Format::Rgba32Float, Format::Rgba32Sint),
        (Format::Rgba32Uint, Format::Rgba32Float),
        (Format::Rgba8Unorm, Format::Rgba8UnormSrgb),
        (Format::Rgba8UnormSrgb, Format::Rgba8Uint),
    ];
    let usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    for (i, (from, to)) in pairs.into_iter().enumerate() {
        let caps = &b.caps().formats;
        if caps.get(from).is_empty() || !caps.get(to).contains(FormatUse::COPY_OUT) {
            continue;
        }
        let texel = from.block().2 as usize;
        let make = |b: &mut H::Backend, format| {
            kit::texture(b, TextureDimension::D2, format, [4, 4, 2], 1, usage, false)
        };
        let src = make(&mut b, from);
        let dst = make(&mut b, to);
        let bytes = pattern(32 * texel, 0x9e37_79b9 ^ i as u32);
        let region = |texture, x, y| TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin { x, y, z: 0 },
            stencil: false,
        };
        let whole = Extent {
            width: 4,
            height: 4,
            depth_or_layers: 2,
        };
        b.write_texture(
            &region(&src, 0, 0),
            whole,
            &bytes,
            4 * texel as u32,
            4,
            WriteMode::Ordered,
        );
        b.write_texture(
            &region(&dst, 0, 0),
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
        b.copy_texture(&region(&src, 1, 1), &region(&dst, 0, 2), part);
        let got = kit::read(&mut b, &dst, 0, 0, [4, 4, 2], texel, false);
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
";

/// Loads and sizes count levels from the view's first, as WGSL's `textureLoad` and
/// `textureDimensions` do: each level of an `rgba8uint` texture holds its own number, and views
/// starting at each level are loaded and sized at each of their levels.
pub fn loads_count_levels_from_the_view<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let program = kit::program(&mut b, LEVEL_LOAD, "vs", "fs");
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
            b.begin_pass(&kit::pass(&out_view, [9; 4], None));
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
            let at = base + level;
            assert_eq!(
                got,
                vec![at as u8, 4 >> at, 0, 0],
                "level {level} of the view from level {base}: its number and width"
            );
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
/// largest index value, instance after instance, interleaved and separate, with and without drawing.
pub fn strips_fans_loops_and_indexed_draws_captured_in_list_order<H: Host>() {
    use crate::kit::{self, Kit};
    let mut b = H::make();
    if !b.caps().has(Features::TRANSFORM_FEEDBACK) {
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
        call.restart = ib.is_some();
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
