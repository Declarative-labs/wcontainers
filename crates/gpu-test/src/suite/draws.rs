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
fn call<'a, B: Gpu>(
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

fn vertices<'a, B: Gpu>(first: u32, count: u32) -> DrawRange<'a, B> {
    DrawRange::Vertices {
        first,
        count,
        first_instance: 0,
        instances: 1,
    }
}

fn vertex_buffer<B: Gpu>(b: &mut B, values: &[f32]) -> B::Buffer {
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

/// A scissor reaching past the target, to the largest offsets, is clipped to the target.
pub fn a_scissor_past_the_target_is_clipped_to_it<H: Host>() {
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
        width: u32::MAX,
        height: u32::MAX,
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
            let inside = x >= 1 && y >= 2;
            expected.extend_from_slice(&if inside { ROUNDED_BYTES } else { [0; 4] });
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

/// `clear_rects` clears each rectangle, clipped to the view, one reaching to the largest offsets
/// included, and leaves the rest.
pub fn clear_rects_clear_each_rectangle_clipped_to_the_view<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    b.end_pass();
    let rects = [
        ScissorRect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        ScissorRect {
            x: 2,
            y: 1,
            width: u32::MAX,
            height: u32::MAX,
        },
    ];
    let value = ClearValue::Color(f32_bits([0.25, 0.85, 0.55, 1.0]));
    b.clear_rects(&v, Format::Rgba8Unorm, value, &rects)
        .expect("clear_rects");
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let mut expected = Vec::new();
    for y in 0..N {
        for x in 0..N {
            let inside = (x == 0 && y == 0) || (x >= 2 && y >= 1);
            expected.extend_from_slice(&if inside { ROUNDED_BYTES } else { [0; 4] });
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

/// A copy between `depth24plus-stencil8` textures moves both aspects. The source holds depth 0.75
/// and stencil 7 on its left half and depth 0.25 and stencil 0 on its right; the destination holds
/// depth 1 and stencil 3 before the copy. Draws tested against the destination then find the
/// source's values: a depth test at 0.5 passes on the left only, a stencil test for 7 on the left
/// only, one for 0 on the right only.
pub fn copies_between_depth_stencil_textures_move_both_aspects<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let f = Format::Depth24PlusStencil8;
    needs(&b, f, FormatUse::DEPTH_STENCIL);
    let (t, v) = rgba8_target(&mut b, N);
    let src = texture(
        &mut b,
        f,
        (N, N),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let dst = texture(
        &mut b,
        f,
        (N, N),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_DST,
    );
    let sv = view(&mut b, &src, f);
    let dv = view(&mut b, &dst, f);
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
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let block = |b: &mut H::Backend,
                 depth: Option<(CompareFn, bool)>,
                 stencil: Option<(CompareFn, StencilOp)>| {
        let face = |(compare, pass)| StencilFace {
            compare,
            fail: StencilOp::Keep,
            depth_fail: StencilOp::Keep,
            pass,
        };
        b.create_depth_stencil(&DepthStencilDesc {
            depth_compare: depth.map(|d| d.0),
            depth_write: depth.is_some_and(|d| d.1),
            stencil: stencil.map(|s| (face(s), face(s))),
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
    let attach = |view, depth, stencil, store| DepthAttachment::<H::Backend> {
        view,
        depth: Some(AspectOps {
            load: depth,
            store,
            read_only: false,
        }),
        stencil: Some(AspectOps {
            load: stencil,
            store,
            read_only: false,
        }),
    };
    let black = LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0]));

    // The source: depth 0.75 (z 0.5 through the depth range 0.75 to 0.75) and stencil 7 on the
    // left half.
    let at = |depth: f32| {
        [Viewport {
            min_depth: depth.to_bits(),
            max_depth: depth.to_bits(),
            ..viewport(N, N)[0]
        }]
    };
    let setup = block(
        &mut b,
        Some((CompareFn::Always, true)),
        Some((CompareFn::Always, StencilOp::Replace)),
    );
    let none = writes(&mut b, ColorWrites::empty());
    let mut pass = color_pass::<H::Backend>(&v, black);
    pass.depth = Some(attach(
        &sv,
        LoadOp::Clear(0.25f32.to_bits()),
        LoadOp::Clear(0),
        StoreOp::Store,
    ));
    let three_quarters = at(0.75);
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        stencil_reference: [7; 2],
        ..call(
            &p,
            &none,
            &setup,
            &rs,
            &vl,
            &streams,
            &three_quarters,
            Topology::TriangleStrip,
            vertices(0, 4),
        )
    });
    b.end_pass();
    // The destination: depth 1 and stencil 3, then the copy.
    let mut pass = color_pass::<H::Backend>(&v, black);
    pass.depth = Some(attach(
        &dv,
        LoadOp::Clear(1.0f32.to_bits()),
        LoadOp::Clear(3),
        StoreOp::Store,
    ));
    b.begin_pass(&pass);
    b.end_pass();
    b.copy_texture(
        &region(&src, 0, 0, 0, 0),
        &region(&dst, 0, 0, 0, 0),
        extent(N, N, 1),
    );

    // Red where 0.5 is less than the depth, green where the stencil is 7, blue where it is 0.
    let tests = [
        (
            block(&mut b, Some((CompareFn::Less, false)), None),
            ColorWrites::R,
            0,
        ),
        (
            block(&mut b, None, Some((CompareFn::Equal, StencilOp::Keep))),
            ColorWrites::G,
            7,
        ),
        (
            block(&mut b, None, Some((CompareFn::Equal, StencilOp::Keep))),
            ColorWrites::B,
            0,
        ),
    ];
    let blocks: Vec<_> = tests.iter().map(|(_, w, _)| writes(&mut b, *w)).collect();
    let vp = viewport(N, N);
    let mut pass = color_pass::<H::Backend>(&v, black);
    pass.depth = Some(attach(&dv, LoadOp::Load, LoadOp::Load, StoreOp::Discard));
    b.begin_pass(&pass);
    for ((ds, _, reference), bl) in tests.iter().zip(&blocks) {
        b.draw(&DrawCall {
            stencil_reference: [*reference; 2],
            ..call(
                &p,
                bl,
                ds,
                &rs,
                &vl,
                &streams,
                &vp,
                Topology::TriangleStrip,
                vertices(4, 4),
            )
        });
    }
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let mut expected = Vec::new();
    for _ in 0..N {
        for x in 0..N {
            expected.extend_from_slice(&if x < N / 2 {
                [64, 217, 0, 255]
            } else {
                [0, 0, 140, 255]
            });
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

/// A vertex stream of stride zero gives every vertex and instance its first element: a quad drawn
/// twice, its colour from a stream of three different colours of which only the first is read.
pub fn a_stream_of_stride_zero_reads_its_first_element<H: Host>() {
    let mut b = H::make();
    const N: u32 = 2;
    const COLOURED: &str = "
    struct Out { @builtin(position) pos: vec4<f32>, @location(0) c: vec4<f32> }
    @vertex fn vs(@location(0) p: vec2<f32>, @location(1) c: vec4<f32>) -> Out {
        return Out(vec4<f32>(p, 0.5, 1.0), c);
    }
    @fragment fn fs(i: Out) -> @location(0) vec4<f32> {
        return i.c;
    }
    ";
    let (t, v) = rgba8_target(&mut b, N);
    let quad = vertex_buffer(&mut b, &QUAD);
    let colours = vertex_buffer(
        &mut b,
        &[0.25, 0.5, 0.75, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
    );
    let p = program(&mut b, COLOURED, "vs", "fs");
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
        stride: 0,
        step: StepMode::Vertex,
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
        format: VertexFormat::Float32x4,
    });
    let vl = b.create_vertex_layout(&desc).expect("create_vertex_layout");
    let vp = viewport(N, N);
    let streams = [
        Some(VertexBinding {
            buffer: &quad,
            offset: 0,
        }),
        Some(VertexBinding {
            buffer: &colours,
            offset: 0,
        }),
    ];
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    b.draw(&call(
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
            first_instance: 0,
            instances: 2,
        },
    ));
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let expected: Vec<u8> = (0..N * N).flat_map(|_| [64, 128, 191, 255]).collect();
    assert_close(&got, &expected, 1, 4, N as usize);
    ok(&mut b);
}

/// A comparison sampler compares the reference with the stored depth by its function: a
/// `depth32float` texel cleared to 0.5, compared at each of four pixels with the pixel's x over 4
/// (0.125, 0.375, 0.625, 0.875) by `Less` into red and `GreaterEqual` into green.
pub fn comparison_samplers_compare_with_the_stored_depth<H: Host>() {
    let mut b = H::make();
    needs(
        &b,
        Format::Depth32Float,
        FormatUse::DEPTH_STENCIL | FormatUse::SAMPLE,
    );
    const N: u32 = 4;
    const COMPARED: &str = "
    @group(0) @binding(16) var less: sampler_comparison;
    @group(0) @binding(17) var greater_equal: sampler_comparison;
    @group(0) @binding(32) var depth: texture_depth_2d;
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
        let r = pos.x / 4.0;
        let at = vec2<f32>(0.5);
        return vec4<f32>(textureSampleCompareLevel(depth, less, at, r),
                         textureSampleCompareLevel(depth, greater_equal, at, r), 0.0, 1.0);
    }
    ";
    let d = texture(
        &mut b,
        Format::Depth32Float,
        (1, 1),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::SAMPLE,
    );
    let dv = view(&mut b, &d, Format::Depth32Float);
    b.begin_pass(&PassDesc {
        colors: Default::default(),
        depth: Some(DepthAttachment {
            view: &dv,
            depth: Some(AspectOps {
                load: LoadOp::Clear(0.5f32.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        empty: None,
        occlusion: None,
    });
    b.end_pass();
    let sampler = |b: &mut H::Backend, compare| {
        b.create_sampler(&SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [AddressMode::Clamp; 3],
            lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
            anisotropy: 1,
            compare: Some(compare),
            border: BorderColor::TransparentBlack,
        })
        .expect("create_sampler")
    };
    let less = sampler(&mut b, CompareFn::Less);
    let greater_equal = sampler(&mut b, CompareFn::GreaterEqual);
    let set = b
        .create_binding_set(&[
            BindingEntry {
                binding: 16,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(&less),
            },
            BindingEntry {
                binding: 17,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(&greater_equal),
            },
            BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&dv),
            },
        ])
        .expect("create_binding_set");
    let (t, v) = rgba8_target(&mut b, N);
    let quad = vertex_buffer(&mut b, &QUAD);
    let p = program(&mut b, COMPARED, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let vp = viewport(N, 1);
    let streams = [Some(VertexBinding {
        buffer: &quad,
        offset: 0,
    })];
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
    let expected = [
        255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
    ];
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

/// An indexed draw one of whose vertices lies past its stream's end, which the interface allows
/// (`DrawRange`'s doc): that vertex reads a value from the buffer, zero or (0, 0, 0, x), or the
/// whole draw is dropped. The stream's bytes are only 0x00, 0x80 and 0xbf, so any value read from
/// it is at most a denormal above zero, and the draw covers no pixel centre right of x = 0 or above
/// y = 0 whichever happens; a draw after it into the top right quarter lands, the device kept.
pub fn an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows<H: Host>() {
    let mut b = H::make();
    const N: u32 = 8;
    let low_left = vertex_buffer(&mut b, &[-1.0, -1.0, -1.0, 0.0, 0.0, -1.0]);
    let top_right = vertex_buffer(&mut b, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    let indices: Vec<u8> = [0u32, 1, 9].iter().flat_map(|i| i.to_le_bytes()).collect();
    let ib = buffer(&mut b, 12, BufferUsage::INDEX | BufferUsage::COPY_DST);
    b.write_buffer(&ib, 0, &indices, WriteMode::Discard);
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let (t, v) = rgba8_target(&mut b, N);
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    let streams = [Some(VertexBinding {
        buffer: &low_left,
        offset: 0,
    })];
    b.draw(&DrawCall {
        index: Some(IndexBinding {
            buffer: &ib,
            offset: 0,
            format: IndexFormat::U32,
        }),
        ..call(
            &p,
            &bl,
            &ds,
            &rs,
            &vl,
            &streams,
            &vp,
            Topology::TriangleList,
            DrawRange::Indexed {
                first: 0,
                count: 3,
                base_vertex: 0,
                first_instance: 0,
                instances: 1,
            },
        )
    });
    let streams = [Some(VertexBinding {
        buffer: &top_right,
        offset: 0,
    })];
    b.draw(&call(
        &p,
        &bl,
        &ds,
        &rs,
        &vl,
        &streams,
        &vp,
        Topology::TriangleList,
        DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
    ));
    b.end_pass();
    ok(&mut b);
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let at = |x: u32, y: u32| &got[((y * N + x) * 4) as usize..][..4];
    // Pixel (4, 3)'s centre is at (0.125, 0.125), inside the second triangle.
    assert_eq!(at(4, 3), ROUNDED_BYTES, "the draw after it is missing");
    for y in 0..N {
        for x in 0..N {
            let low_left = x < N / 2 && y >= N / 2;
            // The second triangle's pixels, and those whose centres lie on its long edge.
            let second = x >= N / 2 && y < N / 2 && (x - N / 2) + (N / 2 - 1 - y) < N / 2;
            if !low_left && !second {
                assert_eq!(at(x, y), [0; 4], "pixel ({x}, {y}) drawn");
            }
        }
    }
}

/// Fragment programs write storage buffers, where the back end declares
/// [`Features::FRAGMENT_STORAGE_WRITE`]: each pixel's fragment writes a value made from its position
/// into its own element, read back exactly.
pub fn fragments_write_storage_buffers<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::FRAGMENT_STORAGE_WRITE) {
        return;
    }
    const N: u32 = 4;
    const STORAGE: &str = "
    @group(0) @binding(160) var<storage, read_write> out: array<u32>;
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
        let x = u32(p.x);
        let y = u32(p.y);
        out[y * 4u + x] = x * 16u + y + 1u;
        return vec4<f32>(1.0);
    }
    ";
    let (_t, v) = rgba8_target(&mut b, N);
    let vb = vertex_buffer(&mut b, &QUAD);
    let out = buffer(
        &mut b,
        u64::from(N * N * 4),
        BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
    );
    b.write_buffer(&out, 0, &vec![0; (N * N * 4) as usize], WriteMode::Discard);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 160,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &out,
                    offset: 0,
                    size: u64::from(N * N * 4),
                },
                writable: true,
            },
        }])
        .expect("create_binding_set");
    let p = program(&mut b, STORAGE, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
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
    let got = read_buffer(&mut b, &out, 0, u64::from(N * N * 4));
    let expected: Vec<u8> = (0..N * N)
        .flat_map(|i| ((i % N) * 16 + i / N + 1).to_le_bytes())
        .collect();
    assert_eq!(got, expected);
    ok(&mut b);
}

const COVER: &str = "
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn white() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
";

const PER_SAMPLE: &str = "
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn index(@builtin(sample_index) i: u32) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i) / 3.0, 0.0, 0.0, 1.0);
}
";

/// Writes the red of samples 0 to 3 of a four-sample texture into red, green, blue and alpha.
const SAMPLES_OF: &str = "
@group(0) @binding(32) var samples: texture_multisampled_2d<f32>;
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let c = vec2<i32>(p.xy);
    return vec4<f32>(
        textureLoad(samples, c, 0).r,
        textureLoad(samples, c, 1).r,
        textureLoad(samples, c, 2).r,
        textureLoad(samples, c, 3).r,
    );
}
";

/// A four-sample `rgba8unorm` target of n by n, or none where the back end has no such texture.
fn four_sample_target<B: Gpu>(b: &mut B, n: u32) -> Option<(B::Texture, B::TextureView)> {
    if !b.caps().formats.takes_samples(Format::Rgba8Unorm, 4) {
        return None;
    }
    let t = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: n,
            height: n,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 4,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        })
        .expect("create_texture");
    let v = view(b, &t, Format::Rgba8Unorm);
    Some((t, v))
}

/// Reads sample 0 to 3 of each texel of a four-sample target through [`SAMPLES_OF`] into a
/// single-sampled one, and gives its bytes.
fn samples_of<B: Gpu>(b: &mut B, ms: &B::TextureView, n: u32) -> Vec<u8> {
    let (t, v) = rgba8_target(b, n);
    let vb = vertex_buffer(b, &QUAD);
    let p = program(b, SAMPLES_OF, "vs", "fs");
    let bl = blend(b, opaque());
    let ds = depth_test(b, None, false);
    let rs = b
        .create_raster(&raster_desc(ProvokingVertex::First))
        .expect("create_raster");
    let vl = positions(b);
    let vp = viewport(n, n);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(ms),
        }])
        .expect("create_binding_set");
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
    read_texture(b, region(&t, 0, 0, 0, 0), extent(n, n, 1), 4)
}

/// Draws white into the four-sample 4 by 4 `rgba8unorm` target `ms`, cleared to zero: all of row 3,
/// and rows 0 to 2 from the left edge to 1.25, 1.5 and 1.75 pixels, so pixel 1 of those rows has one,
/// two and three of its samples covered. The pass resolves into `resolve` where given.
fn cover_rows<B: Gpu>(b: &mut B, ms: &B::TextureView, resolve: Option<&B::TextureView>) {
    // Rows 0 to 3, each from x = 0 to its right edge, in pixels.
    let edges = [1.25f32, 1.5, 1.75, 4.0];
    let mut quads = Vec::new();
    for (row, edge) in edges.iter().enumerate() {
        let x = |px: f32| px / 2.0 - 1.0;
        let y = |py: f32| 1.0 - py / 2.0;
        let (top, bottom) = (y(row as f32), y(row as f32 + 1.0));
        quads.extend_from_slice(&[
            x(0.0),
            top,
            x(*edge),
            top,
            x(0.0),
            bottom,
            x(*edge),
            top,
            x(*edge),
            bottom,
            x(0.0),
            bottom,
        ]);
    }
    let vb = vertex_buffer(b, &quads);
    let p = program(b, COVER, "vs", "white");
    let bl = blend(b, opaque());
    let ds = depth_test(b, None, false);
    let rs = b
        .create_raster(&raster_desc(ProvokingVertex::First))
        .expect("create_raster");
    let vl = positions(b);
    let vp = viewport(4, 4);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let mut pass = color_pass::<B>(ms, LoadOp::Clear([0; 4]));
    if let Some(c) = &mut pass.colors[0] {
        c.resolve = resolve;
    }
    b.begin_pass(&pass);
    b.draw(&call(
        &p,
        &bl,
        &ds,
        &rs,
        &vl,
        &streams,
        &vp,
        Topology::TriangleList,
        vertices(0, 24),
    ));
    b.end_pass();
}

/// Four samples at the standard positions WebGPU, Direct3D and Vulkan share ((0.375, 0.125),
/// (0.875, 0.375), (0.125, 0.625), (0.625, 0.875) in the pixel): rectangles whose right edges cross
/// pixel 1 of rows 0 to 2 at 1.25, 1.5 and 1.75 cover one, two and three of its samples, so its
/// resolved value is 1/4, 1/2 and 3/4 of white, one step of tolerance; resolved at the pass's end and
/// by `resolve`, alike. Where the back end reads samples in programs, each sample's own coverage is
/// checked too: pixel 1 of row 0 covers sample 2 alone.
pub fn multisampled_targets_cover_their_samples_and_resolve<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let Some((ms, msv)) = four_sample_target(&mut b, N) else {
        return;
    };
    let (resolved, resolved_view) = rgba8_target(&mut b, N);
    let (again, _) = rgba8_target(&mut b, N);
    cover_rows(&mut b, &msv, Some(&resolved_view));
    let at = |t| region(t, 0, 0, 0, 0);
    b.resolve(&at(&ms), &at(&again), Format::Rgba8Unorm);
    let mut expected = Vec::new();
    for row in 0..N as usize {
        for x in 0..N {
            let v = match (row, x) {
                (_, 0) | (3, _) => 255,
                (_, 1) => [64, 128, 191][row],
                _ => 0,
            };
            expected.extend_from_slice(&[v; 4]);
        }
    }
    for t in [&resolved, &again] {
        let got = read_texture(&mut b, at(t), extent(N, N, 1), 4);
        assert_close(&got, &expected, 1, 4, N as usize);
    }
    if b.caps().features.contains(Features::MULTISAMPLE_READ) {
        let got = samples_of(&mut b, &msv, N);
        let mut expected = Vec::new();
        for row in 0..N as usize {
            for x in 0..N {
                expected.extend_from_slice(&match (row, x) {
                    (_, 0) | (3, _) => [255; 4],
                    (0, 1) => [0, 0, 255, 0],
                    (1, 1) => [255, 0, 255, 0],
                    (2, 1) => [255, 0, 255, 255],
                    _ => [0; 4],
                });
            }
        }
        assert_close(&got, &expected, 0, 4, N as usize);
    }
    ok(&mut b);
}

/// `resolve` averages in the format it names: the stored `rgba8unorm` on the stored values, its sRGB
/// twin in linear light (pixel 1's colour of a quarter, half and three quarters of white gives 137,
/// 188 and 225; alpha is linear in both). The bytes follow the format named, not the destination's,
/// so an `rgba8unorm` and an `rgba8unorm-srgb` destination hold the same. Where a back end averages
/// the stored values instead it counts `ResolveInStoredFormat`, a listed loss (plan 4.12), and the
/// stored averages are what it must give.
pub fn resolves_average_in_the_format_named<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let Some((ms, msv)) = four_sample_target(&mut b, N) else {
        return;
    };
    let uses = FormatUse::RENDER | FormatUse::COPY_OUT | FormatUse::RESOLVE;
    needs(&b, Format::Rgba8Unorm, uses);
    needs(&b, Format::Rgba8UnormSrgb, uses);
    cover_rows(&mut b, &msv, None);
    let (linear, _) = rgba8_target(&mut b, N);
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let srgb = texture(&mut b, Format::Rgba8UnormSrgb, (N, N), 1, 1, usage);
    let at = |t| region(t, 0, 0, 0, 0);
    for format in [Format::Rgba8Unorm, Format::Rgba8UnormSrgb] {
        for t in [&linear, &srgb] {
            let losses = b.slow_paths().counts[SlowPath::ResolveInStoredFormat as usize];
            b.resolve(&at(&ms), &at(t), format);
            let stored = format == Format::Rgba8Unorm
                || b.slow_paths().counts[SlowPath::ResolveInStoredFormat as usize] > losses;
            let mut expected = Vec::new();
            for row in 0..N as usize {
                for x in 0..N {
                    let (colour, alpha) = match (row, x) {
                        (_, 0) | (3, _) => (255, 255),
                        (_, 1) if stored => ([64, 128, 191][row], [64, 128, 191][row]),
                        (_, 1) => ([137, 188, 225][row], [64, 128, 191][row]),
                        _ => (0, 0),
                    };
                    expected.extend_from_slice(&[colour, colour, colour, alpha]);
                }
            }
            let got = read_texture(&mut b, at(t), extent(N, N, 1), 4);
            assert_close(&got, &expected, 1, 4, N as usize);
        }
    }
    ok(&mut b);
}

/// A program reading the sample index runs once per sample and writes that sample alone: each
/// sample i of every texel holds i / 3, read back by a program loading each sample.
pub fn programs_run_per_sample_and_read_samples<H: Host>() {
    let mut b = H::make();
    let features = b.caps().features;
    if !features.contains(Features::SAMPLE_SHADING | Features::MULTISAMPLE_READ) {
        return;
    }
    const N: u32 = 2;
    let Some((_ms, msv)) = four_sample_target(&mut b, N) else {
        return;
    };
    let vb = vertex_buffer(&mut b, &QUAD);
    let p = program(&mut b, PER_SAMPLE, "vs", "index");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = b
        .create_raster(&raster_desc(ProvokingVertex::First))
        .expect("create_raster");
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    b.begin_pass(&color_pass::<H::Backend>(&msv, LoadOp::Clear([0; 4])));
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
    let got = samples_of(&mut b, &msv, N);
    assert_close(&got, &[0, 85, 170, 255].repeat(4), 0, 4, N as usize);
    ok(&mut b);
}

/// Occlusion queries of "any" kind answer non-zero where a sample passed and zero where none did (a
/// quad drawn, a quad clipped away, no draw), read back by `read_queries` and written to a buffer at
/// an offset by `resolve_queries`, alike.
pub fn occlusion_queries_answer_whether_samples_passed<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    let (_t, v) = rgba8_target(&mut b, N);
    // On screen, then wholly right of it.
    let vb = vertex_buffer(
        &mut b,
        &[
            -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, //
            2.0, -1.0, 2.0, 1.0, 3.0, -1.0, 3.0, 1.0,
        ],
    );
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = b
        .create_raster(&raster_desc(ProvokingVertex::First))
        .expect("create_raster");
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let set = b
        .create_query_set(QueryKind::OcclusionAny, 3)
        .expect("create_query_set");
    let mut pass = color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4]));
    pass.occlusion = Some(&set);
    b.begin_pass(&pass);
    for (query, first) in [(0, Some(0)), (1, Some(4)), (2, None)] {
        b.begin_query(query);
        if let Some(first) = first {
            b.draw(&call(
                &p,
                &bl,
                &ds,
                &rs,
                &vl,
                &streams,
                &vp,
                Topology::TriangleStrip,
                vertices(first, 4),
            ));
        }
        b.end_query();
    }
    b.end_pass();
    let answers = |bytes: &[u8]| -> Vec<bool> {
        bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| u64::from_le_bytes(*c) != 0)
            .collect()
    };
    let rb = b.read_queries(&set, 0, 3).expect("read_queries");
    b.submit();
    b.wait_read(&rb);
    let mut out = [0u8; 24];
    b.finish_read(rb, &mut out);
    assert_eq!(answers(&out), [true, false, false], "read_queries");
    let dst = buffer(&mut b, 40, BufferUsage::COPY_DST | BufferUsage::COPY_SRC);
    b.write_buffer(&dst, 0, &[0xab; 40], WriteMode::Discard);
    b.resolve_queries(&set, 0, 3, &dst, 8)
        .expect("resolve_queries");
    let got = read_buffer(&mut b, &dst, 0, 40);
    assert_eq!(
        answers(&got[8..32]),
        [true, false, false],
        "resolve_queries"
    );
    assert_eq!(&got[..8], [0xab; 8]);
    assert_eq!(&got[32..], [0xab; 8]);
    ok(&mut b);
}

/// The occlusion counter's count on the back end: a quad over 37 by 21 pixels of a 40 by 24 target
/// (sizes no workgroup divides), with one sample and four, through `end` and through `end_into` (a
/// back end that computes sums it on the GPU and copies it; the default reads back).
pub fn the_occlusion_counter_counts_every_sample_that_passes<H: Host>() {
    let mut b = H::make();
    const COUNTED: &str = "
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs() -> @location(0) vec4<f32> {
        return vec4<f32>(0.5);
    }
    @fragment fn counting() -> @location(0) vec4<f32> {
        return vec4<f32>(1.0);
    }
    ";
    let format = Format::Rgba8Unorm;
    needs(&b, format, FormatUse::RENDER);
    let p = program(&mut b, COUNTED, "vs", "fs");
    let counting = program(&mut b, COUNTED, "vs", "counting");
    let vb = vertex_buffer(&mut b, &QUAD);
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let heap = buffer(&mut b, 24, BufferUsage::COPY_SRC | BufferUsage::COPY_DST);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let vp = [Viewport {
        width: 37f32.to_bits(),
        height: 21f32.to_bits(),
        ..viewport(40, 24)[0]
    }];
    for samples in [1, 4] {
        let texture = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: 40,
                height: 24,
                depth_or_layers: 1,
                mip_levels: 1,
                samples,
                usage: TextureUsage::RENDER,
                cube: false,
                reinterpret: false,
            })
            .expect("create_texture");
        let v = view(&mut b, &texture, format);
        let mut colors: [Option<ColorTarget<H::Backend>>; MAX_COLOR_TARGETS] = Default::default();
        colors[0] = Some(ColorTarget {
            target: Target {
                view: v,
                texture,
                format,
                mip: 0,
                layer: 0,
                depth_slice: 0,
                width: 40,
                height: 24,
                samples,
            },
            resolve: None,
        });
        let mut passes = PassTracker::default();
        let mut counter = OcclusionCounter::default();
        passes
            .set_targets(
                &mut b,
                PassTargets {
                    colors,
                    depth: None,
                    empty: None,
                },
            )
            .expect("set_targets");
        for into in [false, true] {
            counter.begin(&mut b, &mut passes).expect("begin");
            let c = call(
                &p,
                &bl,
                &ds,
                &rs,
                &vl,
                &streams,
                &vp,
                Topology::TriangleStrip,
                vertices(0, 4),
            );
            let facts = CountedDraw {
                counting: &counting,
                alpha_to_coverage: false,
                writes_depth_stencil: false,
                instances_step_by_one: true,
            };
            counter.draw(&mut b, &mut passes, &c, &facts).expect("draw");
            passes.before_draw(&mut b, &[]).expect("before_draw");
            b.draw(&c);
            let count = match into {
                false => counter.end(&mut b, &mut passes).expect("end"),
                true => {
                    counter
                        .end_into(&mut b, &mut passes, &heap, 8)
                        .expect("end_into");
                    passes.close(&mut b).expect("close");
                    let bytes = read_buffer(&mut b, &heap, 8, 8);
                    u64::from_le_bytes(bytes.try_into().unwrap())
                }
            };
            assert_eq!(
                count,
                37 * 21 * u64::from(samples),
                "{samples} samples, into {into}"
            );
        }
        passes.close(&mut b).expect("close");
    }
    ok(&mut b);
}
