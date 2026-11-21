//! Every fast path's default, run on the recording back end, which overrides none: the recorded
//! stream shows each is made of base calls only, and its answer is right.

use wcontainers_gpu::*;
use wcontainers_gpu_test::{Call, Id, Recorder};

const PROGRAM: &str = "
@vertex fn vs(@location(0) v: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(v, 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// The calls recorded since `from`, by name.
fn names(r: &Recorder, from: usize) -> Vec<&'static str> {
    r.calls[from..].iter().map(Call::name).collect()
}

fn healthy(r: &mut Recorder) {
    assert_eq!(r.violations, Vec::<String>::new());
    assert_eq!(r.status(), DeviceStatus::Ok);
}

fn buffer(r: &mut Recorder, size: u64, usage: BufferUsage) -> Id {
    r.create_buffer(&BufferDesc {
        size,
        usage,
        intent: MapIntent::Rarely,
    })
    .unwrap()
}

fn texture(r: &mut Recorder, size: u32, layers: u32, mips: u32) -> (Id, TextureDesc) {
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: size,
        height: size,
        depth_or_layers: layers,
        mip_levels: mips,
        samples: 1,
        usage: TextureUsage::all().difference(TextureUsage::STORAGE),
        cube: false,
        reinterpret: false,
    };
    (r.create_texture(&desc).unwrap(), desc)
}

fn view(r: &mut Recorder, t: &Id) -> Id {
    r.create_view(
        t,
        &TextureViewDesc {
            format: Format::Rgba8Unorm,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil: false,
        },
    )
    .unwrap()
}

fn read(r: &mut Recorder, t: &Id, mip: u32, layer: u32, n: u32) -> Vec<u8> {
    let mut out = vec![0; (n * n * 4) as usize];
    r.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: t,
                mip,
                layer,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: n,
                height: n,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    out
}

fn red() -> [u32; 4] {
    [1.0f32, 0.0, 0.0, 1.0].map(f32::to_bits)
}

/// The blocks and program of a plain draw.
struct Blocks {
    program: Id,
    blend: Id,
    depth: Id,
    raster: Id,
    layout: Id,
}

fn blocks(r: &mut Recorder) -> Blocks {
    let module = naga::front::wgsl::parse_str(PROGRAM).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    let mut shader = |entry: &str, stage| {
        r.create_shader(&ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: entry.into(),
            stage,
            key: *blake3::hash(entry.as_bytes()).as_bytes(),
        })
        .unwrap()
    };
    let (v, f) = (
        shader("vs", ShaderStage::Vertex),
        shader("fs", ShaderStage::Fragment),
    );
    let program = r
        .create_program(&Program::Render {
            vertex: &v,
            fragment: Some(&f),
        })
        .unwrap();
    let blend = r
        .create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; MAX_COLOR_TARGETS],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .unwrap();
    let depth = r
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .unwrap();
    let raster = r
        .create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::Back,
            front: FrontFace::CounterClockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking: ProvokingVertex::First,
            depth_minus_one: false,
            sample_mask: !0,
        })
        .unwrap();
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 8,
        step: StepMode::Vertex,
    });
    desc.attributes[0] = Some(VertexAttribute {
        location: 0,
        stream: 0,
        offset: 0,
        format: VertexFormat::Float32x2,
    });
    let layout = r.create_vertex_layout(&desc).unwrap();
    Blocks {
        program,
        blend,
        depth,
        raster,
        layout,
    }
}

fn pass_on(view: &Id, load: LoadOp<[u32; 4]>) -> PassDesc<'_, Recorder> {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view,
        depth_slice: 0,
        resolve: None,
        load,
        store: StoreOp::Store,
    });
    pass
}

#[test]
fn read_back_now_reads_back_later_and_waits() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 8, BufferUsage::COPY_SRC | BufferUsage::COPY_DST);
    r.write_buffer(&b, 0, &[1, 2, 3, 4, 5, 6, 7, 8], WriteMode::Discard);
    let from = r.calls.len();
    let mut got = [0; 4];
    r.read_back_now(
        &ReadSource::Buffer {
            buffer: &b,
            offset: 2,
            size: 4,
        },
        &mut got,
    )
    .unwrap();
    assert_eq!(got, [3, 4, 5, 6]);
    assert_eq!(names(&r, from), ["read_back", "wait_read", "finish_read"]);
    assert_eq!(r.slow_paths().count(SlowPath::ReadBackWait), 1);
    healthy(&mut r);
}

#[test]
fn prepare_does_nothing() {
    let mut r = Recorder::new();
    let k = blocks(&mut r);
    let from = r.calls.len();
    r.prepare(&PrepareDesc {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth,
        raster: &k.raster,
        vertex_layout: &k.layout,
        targets: TargetLayout {
            colors: [
                Some(Format::Rgba8Unorm),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            depth_stencil: None,
            samples: 1,
        },
        topology: Topology::TriangleList,
        layouts: [BindingLayoutKey::default(); MAX_BINDING_SETS],
    });
    assert_eq!(names(&r, from), Vec::<&str>::new());
}

#[test]
fn clear_texture_is_a_pass_per_level_and_layer() {
    let mut r = Recorder::new();
    let (t, desc) = texture(&mut r, 4, 2, 2);
    let from = r.calls.len();
    r.clear_texture(
        &t,
        &desc,
        Format::Rgba8Unorm,
        0..2,
        0..2,
        ClearValue::Color(red()),
    )
    .unwrap();
    let called = names(&r, from);
    assert_eq!(called.iter().filter(|&&n| n == "begin_pass").count(), 4);
    assert!(
        called
            .iter()
            .all(|n| ["create_view", "begin_pass", "end_pass", "destroy"].contains(n)),
        "{called:?}"
    );
    for (mip, n) in [(0, 4), (1, 2)] {
        for layer in 0..2 {
            assert_eq!(
                read(&mut r, &t, mip, layer, n),
                [255, 0, 0, 255].repeat((n * n) as usize)
            );
        }
    }
    healthy(&mut r);
}

#[test]
fn clear_rects_draws_one_triangle_per_rectangle_in_one_pass() {
    let mut r = Recorder::new();
    let (t, _) = texture(&mut r, 4, 1, 1);
    let v = view(&mut r, &t);
    let from = r.calls.len();
    let rects = [
        ScissorRect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        },
        // Clipped to the view.
        ScissorRect {
            x: 2,
            y: 2,
            width: 5,
            height: 9,
        },
    ];
    r.clear_rects(&v, Format::Rgba8Unorm, ClearValue::Color(red()), &rects)
        .unwrap();
    let called = names(&r, from);
    let count = |name| called.iter().filter(|&&n| n == name).count();
    assert_eq!(
        (count("begin_pass"), count("draw"), count("end_pass")),
        (1, 2, 1)
    );
    assert!(called.iter().all(|n| n.starts_with("create_")
        || ["begin_pass", "draw", "end_pass", "write_buffer", "destroy"].contains(n)));
    let got = read(&mut r, &t, 0, 0, 4);
    for y in 0..4 {
        for x in 0..4 {
            let inside = (y == 0 && x < 2) || (y >= 2 && x >= 2);
            let want: &[u8] = if inside { &[255, 0, 0, 255] } else { &[0; 4] };
            assert_eq!(&got[(y * 4 + x) * 4..][..4], want, "({x}, {y})");
        }
    }
    healthy(&mut r);
}

/// The built-in programs draw with the provoking vertex the back end has: on one without
/// `PROVOKING_VERTEX_FIRST`, a draw-made clear asks for the last and clears.
#[test]
fn built_ins_take_the_provoking_vertex_the_back_end_has() {
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::PROVOKING_VERTEX_FIRST));
    let (t, _) = texture(&mut r, 2, 1, 1);
    let v = view(&mut r, &t);
    let whole = ScissorRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    r.clear_rects(&v, Format::Rgba8Unorm, ClearValue::Color(red()), &[whole])
        .unwrap();
    assert_eq!(read(&mut r, &t, 0, 0, 2), [255, 0, 0, 255].repeat(4));
    healthy(&mut r);
}

/// A rectangle reaching to the largest offsets is cleared to the view's edges, through a viewport
/// within the bounds the recorder checks (`max_texture_2d` wide at most).
#[test]
fn clear_rects_reaching_past_the_largest_offset_clear_to_the_views_edges() {
    let mut r = Recorder::new();
    let (t, _) = texture(&mut r, 4, 1, 1);
    let v = view(&mut r, &t);
    let rect = [ScissorRect {
        x: 1,
        y: 2,
        width: u32::MAX,
        height: u32::MAX,
    }];
    r.clear_rects(&v, Format::Rgba8Unorm, ClearValue::Color(red()), &rect)
        .unwrap();
    let got = read(&mut r, &t, 0, 0, 4);
    for y in 0..4 {
        for x in 0..4 {
            let inside = x >= 1 && y >= 2;
            let want: &[u8] = if inside { &[255, 0, 0, 255] } else { &[0; 4] };
            assert_eq!(&got[(y * 4 + x) * 4..][..4], want, "({x}, {y})");
        }
    }
    healthy(&mut r);
}

/// A depth-stencil texture of `format` and its view, 4 by 4.
fn depth_texture(r: &mut Recorder, format: Format) -> (Id, Id) {
    let t = r
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: 4,
            height: 4,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let v = r
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
        .unwrap();
    (t, v)
}

/// One aspect of a 4 by 4 depth-stencil texture, as 4-byte depth words or stencil bytes.
fn read_aspect(r: &mut Recorder, t: &Id, stencil: bool) -> Vec<u8> {
    let mut out = vec![0; if stencil { 16 } else { 64 }];
    r.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: t,
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
        &mut out,
    )
    .unwrap();
    out
}

/// The depth route writes the value as the fragment's depth, and the stencil route replaces with
/// the value as the reference; the other aspect is left as it was.
#[test]
fn clear_rects_writes_depth_and_stencil_by_draws() {
    let mut r = Recorder::new();
    let (t, v) = depth_texture(&mut r, Format::Depth32Float);
    let from = r.calls.len();
    let rect = [ScissorRect {
        x: 1,
        y: 1,
        width: 2,
        height: 2,
    }];
    let depth = |d: Option<f32>, s| ClearValue::DepthStencil {
        depth: d.map(f32::to_bits),
        stencil: s,
    };
    r.clear_rects(&v, Format::Depth32Float, depth(Some(0.25), None), &rect)
        .unwrap();
    let called = names(&r, from);
    assert_eq!(called.iter().filter(|&&n| n == "draw").count(), 1);
    assert!(called.iter().all(|n| n.starts_with("create_")
        || ["begin_pass", "draw", "end_pass", "write_buffer", "destroy"].contains(n)));
    let got = read_aspect(&mut r, &t, false);
    for y in 0..4 {
        for x in 0..4 {
            let inside = (1..3).contains(&x) && (1..3).contains(&y);
            let want = if inside { 0.25f32 } else { 0.0 };
            let at = (y * 4 + x) * 4;
            let d = f32::from_le_bytes(got[at..at + 4].try_into().unwrap());
            assert_eq!(d, want, "({x}, {y})");
        }
    }

    let (t, v) = depth_texture(&mut r, Format::Depth24PlusStencil8);
    let whole = [ScissorRect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    }];
    let column = [ScissorRect {
        x: 0,
        y: 0,
        width: 1,
        height: 4,
    }];
    r.clear_rects(
        &v,
        Format::Depth24PlusStencil8,
        depth(Some(0.5), None),
        &whole,
    )
    .unwrap();
    let before = read_aspect(&mut r, &t, false);
    r.clear_rects(
        &v,
        Format::Depth24PlusStencil8,
        depth(None, Some(7)),
        &column,
    )
    .unwrap();
    let stencil = read_aspect(&mut r, &t, true);
    for (i, &s) in stencil.iter().enumerate() {
        assert_eq!(s, if i % 4 == 0 { 7 } else { 0 }, "texel {i}");
    }
    assert_eq!(read_aspect(&mut r, &t, false), before);
    let word = u32::from_le_bytes(before[..4].try_into().unwrap());
    assert_eq!(word, (0.5f64 * f64::from((1u32 << 24) - 1)).round() as u32);
    healthy(&mut r);
}

#[test]
fn generate_mips_is_a_pass_per_level_and_layer() {
    let mut r = Recorder::new();
    let (t, desc) = texture(&mut r, 4, 2, 3);
    let white = [255u8; 4].repeat(16);
    for layer in 0..2 {
        r.write_texture(
            &TextureRegion {
                texture: &t,
                mip: 0,
                layer,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: 4,
                height: 4,
                depth_or_layers: 1,
            },
            &white,
            16,
            4,
            WriteMode::Ordered,
        );
    }
    let from = r.calls.len();
    r.generate_mips(&t, &desc, Format::Rgba8Unorm, 1..3, 0..2)
        .unwrap();
    let called = names(&r, from);
    let count = |name| called.iter().filter(|&&n| n == name).count();
    assert_eq!((count("begin_pass"), count("draw")), (4, 4));
    assert!(called.iter().all(|n| n.starts_with("create_")
        || ["begin_pass", "draw", "end_pass", "write_buffer", "destroy"].contains(n)));
    assert_eq!(read(&mut r, &t, 2, 1, 1), [255; 4]);
    healthy(&mut r);
}

/// A 4 by 4 texture of `format` with one level below, its texels `texels` (row by row, `size`
/// bytes each), mipped by the default; gives level 1's bytes and the calls the default made.
fn mipped(r: &mut Recorder, format: Format, values: Format, texels: &[u8], size: u32) -> Vec<u8> {
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: 4,
        height: 4,
        depth_or_layers: 1,
        mip_levels: 2,
        samples: 1,
        usage: TextureUsage::all().difference(TextureUsage::STORAGE),
        cube: false,
        reinterpret: false,
    };
    let t = r.create_texture(&desc).unwrap();
    let at = |mip| TextureRegion {
        texture: &t,
        mip,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    let extent = |n| Extent {
        width: n,
        height: n,
        depth_or_layers: 1,
    };
    r.write_texture(&at(0), extent(4), texels, 4 * size, 4, WriteMode::Ordered);
    r.generate_mips(&t, &desc, values, 1..2, 0..1).unwrap();
    let mut out = vec![0; 4 * size as usize];
    r.read_back_now(
        &ReadSource::Texture {
            region: at(1),
            extent: extent(2),
        },
        &mut out,
    )
    .unwrap();
    out
}

/// Each texel of a level is the average of the four below it: through the linear sampler, and on
/// formats without filtering in the program, from texel centres (floats) or integer loads decoded
/// as the values the texture holds (an integer twin holding normalised values).
#[test]
fn generate_mips_averages_the_four_texels_below_on_every_route() {
    // Texel k of 16: red 16 k in an rgba8unorm texture, through the linear sampler.
    let mut r = Recorder::new();
    let texels: Vec<u8> = (0..16u8).flat_map(|k| [16 * k, 0, 0, 255]).collect();
    let got = mipped(&mut r, Format::Rgba8Unorm, Format::Rgba8Unorm, &texels, 4);
    let reds: Vec<u8> = got.chunks(4).map(|c| c[0]).collect();
    assert_eq!(reds, [40, 72, 168, 200]);
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 0);
    healthy(&mut r);

    // r32float holding k, on a back end that does not filter it.
    let mut r =
        Recorder::with_caps(|c| c.formats.uses[Format::R32Float.index()].remove(FormatUse::FILTER));
    let texels: Vec<u8> = (0..16).flat_map(|k| (k as f32).to_le_bytes()).collect();
    let got = mipped(&mut r, Format::R32Float, Format::R32Float, &texels, 4);
    let values: Vec<f32> = got
        .chunks(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(values, [2.5, 4.5, 10.5, 12.5]);
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 1);
    healthy(&mut r);

    // rgba16uint holding rgba16unorm values 4000 k in every channel.
    let mut r = Recorder::new();
    let texels: Vec<u8> = (0..16u16)
        .flat_map(|k| (4000 * k).to_le_bytes().repeat(4))
        .collect();
    let got = mipped(&mut r, Format::Rgba16Uint, Format::Rgba16Unorm, &texels, 8);
    let values: Vec<u16> = got
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let expected: Vec<u16> = [10_000u16, 18_000, 42_000, 50_000]
        .iter()
        .flat_map(|&v| [v; 4])
        .collect();
    assert_eq!(values, expected);
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 1);
    healthy(&mut r);
}

#[test]
fn fans_become_lists_in_the_ring() {
    let mut r = Recorder::new();
    let k = blocks(&mut r);
    let (t, _) = texture(&mut r, 4, 1, 1);
    let v = view(&mut r, &t);
    // A square as a fan, counter-clockwise from the bottom left.
    let vb = buffer(&mut r, 32, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    let corners: Vec<u8> = [-1.0f32, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    r.write_buffer(&vb, 0, &corners, WriteMode::Discard);
    let viewport = [Viewport {
        x: 0f32.to_bits(),
        y: 0f32.to_bits(),
        width: 4f32.to_bits(),
        height: 4f32.to_bits(),
        min_depth: 0f32.to_bits(),
        max_depth: 1f32.to_bits(),
    }];
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let call = DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth,
        raster: &k.raster,
        vertex_layout: &k.layout,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::TriangleFan,
        restart: false,
        viewports: &viewport,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 4,
            first_instance: 0,
            instances: 1,
        },
    };
    r.begin_pass(&pass_on(&v, LoadOp::Clear([0; 4])));
    let from = r.calls.len();
    r.draw_fan_or_loop(
        &call,
        FanOrLoop::TriangleFan,
        IndexSource::Sequential { first: 0, count: 4 },
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    assert_eq!(names(&r, from), ["create_buffer", "write_buffer", "draw"]);
    let Some(Call::Draw(d)) = r.calls.last() else {
        unreachable!()
    };
    assert_eq!(d.topology, Topology::TriangleList);
    assert!(d.index.is_some());
    // The ring is made once.
    let from = r.calls.len();
    r.draw_fan_or_loop(
        &call,
        FanOrLoop::TriangleFan,
        IndexSource::Sequential { first: 0, count: 4 },
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    assert_eq!(names(&r, from), ["write_buffer", "draw"]);
    r.end_pass();
    assert_eq!(r.slow_paths().count(SlowPath::FanConverted), 2);
    // Both triangles keep the fan's winding, so the back-face culling keeps them: all covered.
    assert_eq!(read(&mut r, &t, 0, 0, 4), [255, 0, 0, 255].repeat(16));
    healthy(&mut r);
}

/// A line loop becomes a line list in the ring, closed back to its first vertex. The recording back
/// end draws no lines, so the stream is shown and the draw itself refused.
#[test]
fn loops_become_line_lists_in_the_ring() {
    let mut r = Recorder::new();
    let k = blocks(&mut r);
    let (t, _) = texture(&mut r, 4, 1, 1);
    let v = view(&mut r, &t);
    let vb = buffer(&mut r, 32, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    let viewport = [Viewport {
        x: 0f32.to_bits(),
        y: 0f32.to_bits(),
        width: 4f32.to_bits(),
        height: 4f32.to_bits(),
        min_depth: 0f32.to_bits(),
        max_depth: 1f32.to_bits(),
    }];
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let call = DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth,
        raster: &k.raster,
        vertex_layout: &k.layout,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::LineLoop,
        restart: false,
        viewports: &viewport,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 4,
            first_instance: 0,
            instances: 1,
        },
    };
    r.begin_pass(&pass_on(&v, LoadOp::Clear([0; 4])));
    let from = r.calls.len();
    r.draw_fan_or_loop(
        &call,
        FanOrLoop::LineLoop,
        IndexSource::Sequential { first: 2, count: 4 },
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    assert_eq!(names(&r, from), ["create_buffer", "write_buffer", "draw"]);
    let Some(Call::WriteBuffer(ring, at, data, _)) = r.calls.get(r.calls.len() - 2).cloned() else {
        unreachable!()
    };
    let Some(Call::Draw(d)) = r.calls.last() else {
        unreachable!()
    };
    assert_eq!(d.topology, Topology::LineList);
    let (buffer, offset, format) = d.index.unwrap();
    assert_eq!((buffer, offset, format), (ring, at, IndexFormat::U32));
    let indices: Vec<u32> = data
        .chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(indices, [2, 3, 3, 4, 4, 5, 5, 2]);
    assert_eq!(r.slow_paths().count(SlowPath::LoopConverted), 1);
    assert_eq!(
        r.violations,
        ["LineList is not drawn by the recording back end yet"]
    );
}

#[test]
fn the_ring_writes_without_waiting() {
    let mut r = Recorder::new();
    let mut offsets = [0u64; 2];
    let from = r.calls.len();
    let ring = r
        .write_buffer_ring(RingKind::Data, &[&[1, 2, 3], &[4, 5]], 16, &mut offsets)
        .unwrap();
    // One write per part.
    assert_eq!(
        names(&r, from),
        ["create_buffer", "write_buffer", "write_buffer"]
    );
    assert_eq!(offsets[1] - offsets[0], 16);
    let from = r.calls.len();
    let mut more = [0u64; 1];
    let again = r
        .write_buffer_ring(RingKind::Data, &[&[9]], 4, &mut more)
        .unwrap();
    assert_eq!(names(&r, from), ["write_buffer"]);
    assert_eq!(again, ring);
    let Some(Call::WriteBuffer(_, _, _, mode)) = r.calls.last() else {
        unreachable!()
    };
    assert_eq!(*mode, WriteMode::NoOverwrite);
    healthy(&mut r);
}

/// Where the parts do not fit after the head, the ring starts again at zero with a discard, so its
/// later parts land over bytes an unfinished draw read without waiting, and that draw keeps what it
/// read.
#[test]
fn the_ring_wraps_with_a_discard() {
    let mut r = Recorder::new();
    let k = blocks(&mut r);
    let (t, _) = texture(&mut r, 2, 1, 1);
    let v = view(&mut r, &t);
    // A triangle over the target's left column, then one over all of it.
    let left: Vec<u8> = [-1.0f32, -1.0, 0.0, -1.0, -1.0, 3.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let all: Vec<u8> = [-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let mut first = vec![0u8; (4 << 20) - 16];
    first[..24].copy_from_slice(&left);
    let mut at = [0u64; 1];
    let ring = r
        .write_buffer_ring(RingKind::Data, &[&first], 16, &mut at)
        .unwrap();
    assert_eq!(at, [0]);
    let draw = |r: &mut Recorder, offset| {
        let viewport = [Viewport {
            x: 0f32.to_bits(),
            y: 0f32.to_bits(),
            width: 2f32.to_bits(),
            height: 2f32.to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }];
        let streams = [Some(VertexBinding {
            buffer: &ring,
            offset,
        })];
        r.begin_pass(&pass_on(&v, LoadOp::Clear([0; 4])));
        r.draw(&DrawCall {
            program: &k.program,
            blend: &k.blend,
            depth_stencil: &k.depth,
            raster: &k.raster,
            vertex_layout: &k.layout,
            bindings: [None; MAX_BINDING_SETS],
            vertex_buffers: &streams,
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &viewport,
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
        r.end_pass();
    };
    draw(&mut r, 0);
    let mut copy = [0u8; 16];
    let rb = r
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture: &t,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: 2,
                height: 2,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    let from = r.calls.len();
    let mut two = [0u64; 2];
    let again = r
        .write_buffer_ring(RingKind::Data, &[&[5; 8], &all], 16, &mut two)
        .unwrap();
    assert_eq!((again, two), (ring, [0, 16]));
    let modes: Vec<WriteMode> = r.calls[from..]
        .iter()
        .map(|c| match c {
            Call::WriteBuffer(_, _, _, mode) => *mode,
            other => panic!("{} in the ring's write", other.name()),
        })
        .collect();
    assert_eq!(modes, [WriteMode::Discard, WriteMode::NoOverwrite]);
    healthy(&mut r);
    // The draw before the wrap drew the left column; the one after draws the new part.
    r.submit();
    r.wait_read(&rb);
    r.finish_read(rb, &mut copy);
    let red = [255, 0, 0, 255];
    let none = [0; 4];
    assert_eq!(copy[..], [red, none, red, none].concat());
    draw(&mut r, 16);
    assert_eq!(read(&mut r, &t, 0, 0, 2), red.repeat(4));
    healthy(&mut r);
}

#[test]
fn resolve_queries_reads_them_back_and_writes_them_in_order() {
    let mut r = Recorder::new();
    let k = blocks(&mut r);
    let (t, _) = texture(&mut r, 2, 1, 1);
    let v = view(&mut r, &t);
    let set = r.create_query_set(QueryKind::OcclusionAny, 2).unwrap();
    let vb = buffer(&mut r, 24, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    let corners: Vec<u8> = [-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    r.write_buffer(&vb, 0, &corners, WriteMode::Discard);
    let mut pass = pass_on(&v, LoadOp::Clear([0; 4]));
    pass.occlusion = Some(&set);
    r.begin_pass(&pass);
    let viewport = [Viewport {
        x: 0f32.to_bits(),
        y: 0f32.to_bits(),
        width: 2f32.to_bits(),
        height: 2f32.to_bits(),
        min_depth: 0f32.to_bits(),
        max_depth: 1f32.to_bits(),
    }];
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    r.begin_query(1);
    r.draw(&DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth,
        raster: &k.raster,
        vertex_layout: &k.layout,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport,
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
    r.end_query();
    r.end_pass();
    let dst = buffer(&mut r, 24, BufferUsage::COPY_SRC | BufferUsage::COPY_DST);
    // Results go to multiples of 8 bytes; the refusal makes no call.
    let from = r.calls.len();
    assert_eq!(
        r.resolve_queries(&set, 0, 2, &dst, 4),
        Err(GpuError::Invalid(
            "query results go to multiples of 8 bytes"
        ))
    );
    assert_eq!(r.calls.len(), from);
    r.resolve_queries(&set, 0, 2, &dst, 8).unwrap();
    assert_eq!(
        names(&r, from),
        ["read_queries", "wait_read", "finish_read", "write_buffer"]
    );
    let Some(Call::WriteBuffer(_, 8, _, WriteMode::Ordered)) = r.calls.last() else {
        panic!("the results are not an ordered write at the offset")
    };
    assert_eq!(r.slow_paths().count(SlowPath::QueryResolveWait), 1);
    let mut got = [0u8; 16];
    r.read_back_now(
        &ReadSource::Buffer {
            buffer: &dst,
            offset: 8,
            size: 16,
        },
        &mut got,
    )
    .unwrap();
    // Query 0 never ran; query 1 saw samples pass.
    assert_eq!(got[..8], [0; 8]);
    assert_ne!(got[8..], [0; 8]);
    healthy(&mut r);
}
