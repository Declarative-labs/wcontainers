//! Suite tests of the draw's fixed stages: interpolation, scissor, stencil, clipping and the depth
//! range, sampler addressing and filtering, instancing and indices.

use super::*;

/// What [`CONSTANT`]'s colour 0.25, 0.85, 0.55, 1 stores in 8-bit unorm: 63.75, 216.75 and 140.25
/// steps round to nearest, and truncating or rounding up is off by more than the 0.6 step Direct3D
/// allows.
const ROUNDED_BYTES: [u8; 4] = [64, 217, 140, 255];

/// A full-screen quad as a strip of `vec2<f32>` positions.
const QUAD: [f32; 8] = [-1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0];

const CONSTANT: &str = "
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(0.25, 0.85, 0.55, 1.0);
}
";

/// A draw call of fixed state but for what each test names.
#[allow(clippy::too_many_arguments)]
fn call<'a, B: Backend>(
    program: &'a B::Program,
    blend: &'a B::Blend,
    depth_stencil: &'a B::DepthStencil,
    raster: &'a B::Raster,
    layout: &'a B::VertexLayout,
    streams: &'a [Option<VertexBinding<'a, B>>],
    viewports: &'a [Viewport],
    topology: Topology,
    range: DrawRange<'a, B>,
) -> DrawCall<'a, B> {
    DrawCall {
        program,
        blend,
        depth_stencil,
        raster,
        vertex_layout: layout,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: streams,
        index: None,
        topology,
        restart: false,
        viewports,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    }
}

fn vertices<'a, B: Backend>(first: u32, count: u32) -> DrawRange<'a, B> {
    DrawRange::Vertices {
        first,
        count,
        first_instance: 0,
        instances: 1,
    }
}

fn vertex_buffer<B: Backend>(b: &mut B, values: &[f32]) -> B::Buffer {
    let vb = buffer(
        b,
        values.len() as u64 * 4,
        BufferUsage::VERTEX | BufferUsage::COPY_DST,
    );
    b.write_buffer(&vb, 0, &floats(values), WriteMode::Discard);
    vb
}

/// Values a program passes to its fragments without `@interpolate` are interpolated with perspective:
/// a quad whose right edge has w = 4 holds, at a pixel a fraction t of the way across, the value
/// (t / 4) / ((1 - t) + t / 4), not t. One step of tolerance; the two differ by up to 64 steps here.
pub fn values_are_interpolated_with_perspective<H: Host>() {
    let mut b = H::make();
    const N: u32 = 8;
    let (t, v) = rgba8_target(&mut b, N);
    const PERSPECTIVE: &str = "
    struct Out { @builtin(position) pos: vec4<f32>, @location(0) u: f32 }
    @vertex fn vs(@location(0) p: vec2<f32>, @location(1) u: f32) -> Out {
        let w = 1.0 + 3.0 * u;
        return Out(vec4<f32>(p * w, 0.5 * w, w), u);
    }
    @fragment fn fs(i: Out) -> @location(0) vec4<f32> {
        return vec4<f32>(i.u, 0.0, 0.0, 1.0);
    }
    ";
    let vb = vertex_buffer(
        &mut b,
        &[
            -1.0, -1.0, 0.0, -1.0, 1.0, 0.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0,
        ],
    );
    let p = program(&mut b, PERSPECTIVE, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions_and_values(&mut b);
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    draw(
        &mut b,
        &p,
        &bl,
        &ds,
        &rs,
        &vl,
        &vb,
        Topology::TriangleStrip,
        0,
        4,
        N,
    );
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let mut expected = Vec::new();
    for _ in 0..N {
        for x in 0..N {
            let t = (f64::from(x) + 0.5) / f64::from(N);
            let u = (t / 4.0) / ((1.0 - t) + t / 4.0);
            expected.extend_from_slice(&[(u * 255.0).round() as u8, 0, 0, 255]);
        }
    }
    assert_close(&got, &expected, 1, 4, N as usize);
    ok(&mut b);
}

/// A draw with the scissor switched on writes only inside the scissor rectangle, and its colour
/// values convert to unorm by rounding to nearest.
pub fn the_scissor_bounds_the_draw<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    let vb = vertex_buffer(&mut b, &QUAD);
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = b
        .create_raster(&RasterDesc {
            scissor: true,
            ..raster_desc(ProvokingVertex::First)
        })
        .expect("create_raster");
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let scissor = [ScissorRect {
        x: 1,
        y: 2,
        width: 2,
        height: 1,
    }];
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    b.draw(&DrawCall {
        scissors: &scissor,
        ..call(
            &p,
            &bl,
            &ds,
            &rs,
            &vl,
            &streams,
            &vp,
            Topology::TriangleStrip,
            vertices(0, 4),
        )
    });
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let mut expected = Vec::new();
    for y in 0..N {
        for x in 0..N {
            let inside = (1..3).contains(&x) && y == 2;
            expected.extend_from_slice(&if inside { ROUNDED_BYTES } else { [0; 4] });
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

/// The raster block every test of this file starts from.
fn raster_desc(provoking: ProvokingVertex) -> RasterDesc {
    RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::CounterClockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking,
        depth_minus_one: false,
        sample_mask: !0,
    }
}

/// Stencil operations and comparisons with the draw's reference: a replace on the left half, a
/// draw passing where the stencil equals the reference, an increment where it does not and an
/// invert where the test fails, each then found by an equal test.
pub fn stencil_operations_and_tests<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    needs(&b, Format::Depth24PlusStencil8, FormatUse::DEPTH_STENCIL);
    let (t, v) = rgba8_target(&mut b, N);
    let st = texture(
        &mut b,
        Format::Depth24PlusStencil8,
        (N, N),
        1,
        1,
        TextureUsage::RENDER,
    );
    let sv = view(&mut b, &st, Format::Depth24PlusStencil8);
    let vb = vertex_buffer(
        &mut b,
        &[
            -1.0, -1.0, -1.0, 1.0, 0.0, -1.0, 0.0, 1.0, // left half
            -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, // all
        ],
    );
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let stencil = |b: &mut H::Backend, compare, fail, pass| {
        let face = StencilFace {
            compare,
            fail,
            depth_fail: StencilOp::Keep,
            pass,
        };
        b.create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: Some((face, face)),
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .expect("create_depth_stencil")
    };
    let writes = |b: &mut H::Backend, writes| {
        blend(
            b,
            RenderTargetBlend {
                blend: None,
                writes,
            },
        )
    };
    use StencilOp::{IncrementClamp, Invert, Keep, Replace};
    let draws = [
        // (compare, fail, pass, reference, writes, first vertex)
        (CompareFn::Always, Keep, Replace, 5, ColorWrites::empty(), 0),
        (CompareFn::Equal, Keep, Keep, 5, ColorWrites::R, 4),
        (
            CompareFn::NotEqual,
            Invert,
            IncrementClamp,
            5,
            ColorWrites::empty(),
            4,
        ),
        (CompareFn::Equal, Keep, Keep, 1, ColorWrites::G, 4),
        (CompareFn::Equal, Keep, Keep, !5, ColorWrites::B, 4),
    ];
    let mut pass = color_pass::<H::Backend>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
    pass.depth = Some(DepthAttachment {
        view: &sv,
        depth: Some(AspectOps {
            load: LoadOp::Clear(1.0f32.to_bits()),
            store: StoreOp::Discard,
            read_only: false,
        }),
        stencil: Some(AspectOps {
            load: LoadOp::Clear(0),
            store: StoreOp::Discard,
            read_only: false,
        }),
    });
    let blocks: Vec<_> = draws
        .iter()
        .map(|&(compare, fail, pass_op, _, w, _)| {
            (stencil(&mut b, compare, fail, pass_op), writes(&mut b, w))
        })
        .collect();
    b.begin_pass(&pass);
    for (&(.., reference, _, first), (ds, bl)) in draws.iter().zip(&blocks) {
        b.draw(&DrawCall {
            stencil_reference: [reference; 2],
            ..call(
                &p,
                bl,
                ds,
                &rs,
                &vl,
                &streams,
                &vp,
                Topology::TriangleStrip,
                vertices(first, 4),
            )
        });
    }
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let mut expected = Vec::new();
    for _ in 0..N {
        for x in 0..N {
            // Left: 5, so red; then inverted to 250, so blue. Right: 0, incremented to 1, so green.
            expected.extend_from_slice(&if x < N / 2 {
                [64, 0, 140, 255]
            } else {
                [0, 217, 0, 255]
            });
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

/// Primitives are clipped where z < 0 (the clip volume's near side, depth from 0 to 1), and the depth
/// written is the viewport's depth range applied to z: a quad whose z runs from -0.5 on the left to
/// 0.5 on the right, drawn with depths 0.25 to 0.75, covers only the right half, at 0.25 + z / 2.
pub fn near_clipping_and_the_viewport_depth_range<H: Host>() {
    let mut b = H::make();
    const N: u32 = 8;
    needs(
        &b,
        Format::Depth32Float,
        FormatUse::DEPTH_STENCIL | FormatUse::COPY_OUT,
    );
    let (t, v) = rgba8_target(&mut b, N);
    let d = texture(
        &mut b,
        Format::Depth32Float,
        (N, N),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let dv = view(&mut b, &d, Format::Depth32Float);
    let vb = vertex_buffer(
        &mut b,
        &[
            -1.0, -1.0, -0.5, -1.0, 1.0, -0.5, 1.0, -1.0, 0.5, 1.0, 1.0, 0.5,
        ],
    );
    let p = program(&mut b, VALUES, "at_depth", "white");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, Some(CompareFn::Always), true);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions_and_values(&mut b);
    let vp = [Viewport {
        min_depth: 0.25f32.to_bits(),
        max_depth: 0.75f32.to_bits(),
        ..viewport(N, N)[0]
    }];
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let pass = color_pass::<H::Backend>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
    b.begin_pass(&with_depth(pass, &dv));
    b.draw(&call(
        &p,
        &bl,
        &ds,
        &rs,
        &vl,
        &streams,
        &vp,
        Topology::TriangleStrip,
        vertices(0, 4),
    ));
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let depth = read_texture(&mut b, region(&d, 0, 0, 0, 0), extent(N, N, 1), 4);
    for y in 0..N as usize {
        for x in 0..N as usize {
            let at = (y * N as usize + x) * 4;
            let z = 0.5 * ((x as f64 + 0.5) / f64::from(N) * 2.0 - 1.0);
            let (colour, want) = if z < 0.0 {
                ([0, 0, 0, 255], 1.0)
            } else {
                ([255; 4], 0.25 + z * 0.5)
            };
            assert_eq!(got[at..at + 4], colour, "colour at ({x}, {y})");
            let have = f32::from_le_bytes(depth[at..at + 4].try_into().unwrap());
            assert!(
                (f64::from(have) - want).abs() < 1e-6,
                "depth at ({x}, {y}) is {have}, expected {want}"
            );
        }
    }
    ok(&mut b);
}

/// Linear filtering weighs the two nearest texels by distance, and the address mode picks the texel
/// beyond an edge: a 2-texel row of 0 and 255 read at the centres of 4 pixels gives 0, 63.75,
/// 191.25 and 255 clamped and 63.75, 63.75, 191.25 and 191.25 repeated. Two steps of tolerance,
/// since filter weights have the hardware's sub-texel precision.
pub fn linear_filtering_and_address_modes<H: Host>() {
    let mut b = H::make();
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::SAMPLE | FormatUse::FILTER,
    );
    const N: u32 = 4;
    let tex = texture(
        &mut b,
        Format::Rgba8Unorm,
        (2, 1),
        1,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    );
    b.write_texture(
        &region(&tex, 0, 0, 0, 0),
        extent(2, 1, 1),
        &[0, 0, 0, 255, 255, 0, 0, 255],
        8,
        1,
        WriteMode::Ordered,
    );
    let tv = view(&mut b, &tex, Format::Rgba8Unorm);
    let vb = vertex_buffer(&mut b, &QUAD);
    let p = program(&mut b, TEXTURED, "vs", "textured");
    let uniforms = buffer(&mut b, 32, BufferUsage::UNIFORM | BufferUsage::COPY_DST);
    b.write_buffer(&uniforms, 0, &[0; 32], WriteMode::Discard);
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let vp = viewport(N, 1);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    for (mode, expected) in [
        (AddressMode::Clamp, [0u8, 64, 191, 255]),
        (AddressMode::Repeat, [64, 64, 191, 191]),
    ] {
        let sampler = b
            .create_sampler(&SamplerDesc {
                min: Filter::Linear,
                mag: Filter::Linear,
                mip: Filter::Nearest,
                address: [mode, AddressMode::Clamp, AddressMode::Clamp],
                lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
                anisotropy: 1,
                compare: None,
                border: BorderColor::TransparentBlack,
            })
            .expect("create_sampler");
        let set = b
            .create_binding_set(&[
                BindingEntry {
                    binding: 0,
                    stages: StageSet::VERTEX | StageSet::FRAGMENT,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &uniforms,
                        offset: 0,
                        size: 32,
                    }),
                },
                BindingEntry {
                    binding: 16,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&tv),
                },
            ])
            .expect("create_binding_set");
        let t = texture(
            &mut b,
            Format::Rgba8Unorm,
            (N, 1),
            1,
            1,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        );
        let v = view(&mut b, &t, Format::Rgba8Unorm);
        b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
        b.draw(&DrawCall {
            bindings: [Some(&set), None, None, None],
            ..call(
                &p,
                &bl,
                &ds,
                &rs,
                &vl,
                &streams,
                &vp,
                Topology::TriangleStrip,
                vertices(0, 4),
            )
        });
        b.end_pass();
        let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, 1, 1), 4);
        let want: Vec<u8> = expected.iter().flat_map(|&r| [r, 0, 0, 255]).collect();
        assert_close(&got, &want, 2, 4, N as usize);
    }
    ok(&mut b);
}

/// Per-instance streams start at the draw's first instance, and the program's `instance_index`
/// includes it: two instances from instance 2 of a stream of four values, each drawn into its own half.
/// Where the back end has no [`Features::BASE_VERTEX_INSTANCE`] the first instance is 0 and the test
/// checks the two instances from there.
pub fn instances_start_at_the_first_instance<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let first = if b.caps().features.contains(Features::BASE_VERTEX_INSTANCE) {
        2
    } else {
        0
    };
    const INSTANCED: &str = "
    struct Out { @builtin(position) pos: vec4<f32>, @location(0) @interpolate(flat) v: f32 }
    struct Params { first: f32 }
    @group(0) @binding(0) var<uniform> params: Params;
    @vertex fn vs(@location(0) p: vec2<f32>, @location(1) v: f32,
                  @builtin(instance_index) i: u32) -> Out {
        let x = p.x * 0.5 + (f32(i) - params.first - 0.5);
        return Out(vec4<f32>(x, p.y, 0.5, 1.0), v);
    }
    @fragment fn fs(i: Out) -> @location(0) vec4<f32> {
        return vec4<f32>(i.v, 0.0, 0.0, 1.0);
    }
    ";
    let (t, v) = rgba8_target(&mut b, N);
    let quad = vertex_buffer(&mut b, &QUAD);
    let values = vertex_buffer(&mut b, &[0.2, 0.4, 0.6, 1.0]);
    let uniforms = buffer(&mut b, 16, BufferUsage::UNIFORM | BufferUsage::COPY_DST);
    b.write_buffer(
        &uniforms,
        0,
        &floats(&[first as f32, 0.0, 0.0, 0.0]),
        WriteMode::Discard,
    );
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::VERTEX,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &uniforms,
                offset: 0,
                size: 16,
            }),
        }])
        .expect("create_binding_set");
    let p = program(&mut b, INSTANCED, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 8,
        step: StepMode::Vertex,
    });
    desc.streams[1] = Some(VertexStream {
        stride: 4,
        step: StepMode::Instance(1),
    });
    desc.attributes[0] = Some(VertexAttribute {
        location: 0,
        stream: 0,
        offset: 0,
        format: VertexFormat::Float32x2,
    });
    desc.attributes[1] = Some(VertexAttribute {
        location: 1,
        stream: 1,
        offset: 0,
        format: VertexFormat::Float32x1,
    });
    let vl = b.create_vertex_layout(&desc).expect("create_vertex_layout");
    let vp = viewport(N, N);
    let streams = [
        Some(VertexBinding {
            buffer: &quad,
            offset: 0,
        }),
        Some(VertexBinding {
            buffer: &values,
            offset: 0,
        }),
    ];
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    b.draw(&DrawCall {
        bindings: [Some(&set), None, None, None],
        ..call(
            &p,
            &bl,
            &ds,
            &rs,
            &vl,
            &streams,
            &vp,
            Topology::TriangleStrip,
            DrawRange::Vertices {
                first: 0,
                count: 4,
                first_instance: first,
                instances: 2,
            },
        )
    });
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let shades = [51u8, 102, 153, 255];
    let mut expected = Vec::new();
    for _ in 0..N {
        for x in 0..N {
            let r = shades[(first + x / 2) as usize];
            expected.extend_from_slice(&[r, 0, 0, 255]);
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

/// Indexed strips with 16 and 32-bit indices read from the index buffer's offset, restart at the
/// largest index value, and add the base vertex: two quads, left and right, from one strip of nine
/// indices. Where the back end has no [`Features::BASE_VERTEX_INSTANCE`] the base vertex is 0.
pub fn indexed_strips_restart_and_add_the_base_vertex<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let base = if b.caps().features.contains(Features::BASE_VERTEX_INSTANCE) {
        3
    } else {
        0
    };
    // `base` unused vertices, then a quad on the left of value 0.2 and one on the right of 0.6.
    let mut verts = vec![9.0f32; base as usize * 3];
    verts.extend_from_slice(&[
        -1.0, -1.0, 0.2, -1.0, 1.0, 0.2, 0.0, -1.0, 0.2, 0.0, 1.0, 0.2, //
        0.0, -1.0, 0.6, 0.0, 1.0, 0.6, 1.0, -1.0, 0.6, 1.0, 1.0, 0.6,
    ]);
    let vb = vertex_buffer(&mut b, &verts);
    let p = program(&mut b, VALUES, "flat", "red");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions_and_values(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let strip = [0u32, 1, 2, 3, u32::MAX, 4, 5, 6, 7];
    for format in [IndexFormat::U16, IndexFormat::U32] {
        // Two unused indices before the strip, so it starts at a non-zero offset.
        let mut bytes = Vec::new();
        for i in [7u32, 7].iter().chain(&strip) {
            match format {
                IndexFormat::U16 => bytes.extend_from_slice(&(*i as u16).to_le_bytes()),
                _ => bytes.extend_from_slice(&i.to_le_bytes()),
            }
        }
        let size = bytes.len() / (strip.len() + 2);
        let ib = buffer(
            &mut b,
            bytes.len() as u64,
            BufferUsage::INDEX | BufferUsage::COPY_DST,
        );
        b.write_buffer(&ib, 0, &bytes, WriteMode::Discard);
        let (t, v) = rgba8_target(&mut b, N);
        b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
        b.draw(&DrawCall {
            index: Some(IndexBinding {
                buffer: &ib,
                offset: size as u64,
                format,
            }),
            restart: true,
            ..call(
                &p,
                &bl,
                &ds,
                &rs,
                &vl,
                &streams,
                &vp,
                Topology::TriangleStrip,
                DrawRange::Indexed {
                    first: 1,
                    count: strip.len() as u32,
                    base_vertex: base,
                    first_instance: 0,
                    instances: 1,
                },
            )
        });
        b.end_pass();
        let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
        let mut expected = Vec::new();
        for _ in 0..N {
            for x in 0..N {
                expected.extend_from_slice(&[if x < N / 2 { 51 } else { 153 }, 0, 0, 255]);
            }
        }
        assert_close(&got, &expected, 0, 4, N as usize);
    }
    ok(&mut b);
}
