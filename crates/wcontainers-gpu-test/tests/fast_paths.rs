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
        usage: TextureUsage::all(),
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
        ScissorRect {
            x: 2,
            y: 2,
            width: 2,
            height: 2,
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
    let from = r.calls.len();
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
