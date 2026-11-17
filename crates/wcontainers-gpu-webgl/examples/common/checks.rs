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
