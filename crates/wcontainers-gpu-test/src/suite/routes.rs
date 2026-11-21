//! Suite tests of what fronts route through the base: fill modes, values passed between programs
//! converted apart, vertices pulled from storage, constants read through a texture, and the
//! components a view of a format reads.

use super::*;

/// A full-screen quad as a strip of `vec2<f32>` positions.
const QUAD: [f32; 8] = [-1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0];

/// One draw of fixed state but for what each test names, into a viewport of `size`.
#[allow(clippy::too_many_arguments)]
fn draw_into<B: Backend>(
    b: &mut B,
    program: &B::Program,
    raster: &B::Raster,
    layout: &B::VertexLayout,
    bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS],
    streams: &[Option<VertexBinding<'_, B>>],
    topology: Topology,
    range: DrawRange<'_, B>,
    size: (u32, u32),
) {
    let bl = blend(b, opaque());
    let ds = depth_test(b, None, false);
    let vp = viewport(size.0, size.1);
    b.draw(&DrawCall {
        program,
        blend: &bl,
        depth_stencil: &ds,
        raster,
        vertex_layout: layout,
        bindings,
        vertex_buffers: streams,
        index: None,
        topology,
        restart: false,
        viewports: &vp,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    });
    b.destroy(Object::Blend(bl));
    b.destroy(Object::DepthStencil(ds));
}

fn quad<B: Backend>(b: &mut B) -> B::Buffer {
    let vb = buffer(b, 32, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&QUAD), WriteMode::Discard);
    vb
}

fn once<'a, B: Backend>(count: u32) -> DrawRange<'a, B> {
    DrawRange::Vertices {
        first: 0,
        count,
        first_instance: 0,
        instances: 1,
    }
}

const WHITE: &str = "
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
";

/// A triangle with corners at the centres of pixels (1, 1), (6, 1) and (1, 6), filled as lines,
/// covers its edges and not its inside, and filled as points only its corners, where the back end
/// declares `POLYGON_MODE` and `POLYGON_MODE_POINT`; where it does not, the raster block is refused
/// as absent of that capability. Line ends are left unchecked, since line rules differ there.
pub fn fill_modes_draw_edges_or_corners<H: Host>() {
    let mut b = H::make();
    const N: u32 = 8;
    let x = |p: f32| (p + 0.5) / N as f32 * 2.0 - 1.0;
    let y = |p: f32| 1.0 - (p + 0.5) / N as f32 * 2.0;
    let corners = [x(1.0), y(1.0), x(6.0), y(1.0), x(1.0), y(6.0)];
    let vb = buffer(&mut b, 24, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&corners), WriteMode::Discard);
    let p = program(&mut b, WHITE, "vs", "fs");
    let vl = positions(&mut b);
    let (t, v) = rgba8_target(&mut b, N);
    let cases = [
        (
            FillMode::Line,
            Features::POLYGON_MODE,
            [(3, 1), (1, 3), (3, 4)],
        ),
        (
            FillMode::Point,
            Features::POLYGON_MODE_POINT,
            [(1, 1), (6, 1), (1, 6)],
        ),
    ];
    for (fill, feature, drawn) in cases {
        let desc = RasterDesc {
            fill,
            cull: CullMode::None,
            front: FrontFace::CounterClockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking: if b.caps().features.contains(Features::PROVOKING_VERTEX_FIRST) {
                ProvokingVertex::First
            } else {
                ProvokingVertex::Last
            },
            depth_minus_one: false,
            sample_mask: !0,
        };
        let made = b.create_raster(&desc);
        if !b.caps().features.contains(feature) {
            assert!(
                matches!(made, Err(GpuError::Absent(f)) if f == feature),
                "{fill:?} without {feature:?} is not refused as absent of it"
            );
            continue;
        }
        let rs = made.expect("create_raster");
        b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
        let streams = [Some(VertexBinding {
            buffer: &vb,
            offset: 0,
        })];
        draw_into(
            &mut b,
            &p,
            &rs,
            &vl,
            [None; MAX_BINDING_SETS],
            &streams,
            Topology::TriangleList,
            once(3),
            (N, N),
        );
        b.end_pass();
        let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
        let red = |(px, py): (u32, u32)| got[((py * N + px) * 4) as usize];
        for at in drawn {
            assert_eq!(red(at), 255, "{fill:?} leaves {at:?} undrawn");
        }
        for at in [(2, 2), (3, 3), (6, 6)].into_iter().chain(match fill {
            FillMode::Point => Some((3, 1)),
            _ => None,
        }) {
            assert_eq!(red(at), 0, "{fill:?} draws {at:?}");
        }
        b.destroy(Object::Raster(rs));
    }
    ok(&mut b);
}

/// A vertex program writing locations 0, 2 and 4 and a fragment program reading 0 and 4 only,
/// as programs converted apart pass `VariantKey::linked` values past the rest: the fragment
/// program reads each value by its location, not by its place among those it reads.
pub fn values_pass_between_programs_by_location<H: Host>() {
    let mut b = H::make();
    const N: u32 = 4;
    const LINKED: &str = "
    struct V {
        @builtin(position) p: vec4<f32>,
        @location(0) a: vec4<f32>,
        @location(2) skipped: vec4<f32>,
        @location(4) linked: vec4<f32>,
    }
    @vertex fn vs(@location(0) p: vec2<f32>) -> V {
        return V(vec4<f32>(p, 0.5, 1.0), vec4<f32>(0.25), vec4<f32>(0.5), vec4<f32>(0.75));
    }
    struct F { @location(0) a: vec4<f32>, @location(4) linked: vec4<f32> }
    @fragment fn fs(i: F) -> @location(0) vec4<f32> {
        return vec4<f32>(i.a.x, i.linked.y, 0.0, 1.0);
    }
    ";
    let (t, v) = rgba8_target(&mut b, N);
    let vb = quad(&mut b);
    let p = program(&mut b, LINKED, "vs", "fs");
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    draw_into(
        &mut b,
        &p,
        &rs,
        &vl,
        [None; MAX_BINDING_SETS],
        &streams,
        Topology::TriangleStrip,
        once(4),
        (N, N),
    );
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    // 0.25 and 0.75 of 255 round to 64 and 191; the skipped value would give 128.
    assert_close(
        &got,
        &[64, 191, 0, 255].repeat((N * N) as usize),
        1,
        4,
        N as usize,
    );
    ok(&mut b);
}

/// Vertices fetched from streams and pulled by a program from storage buffers by its vertex and
/// instance index give the same picture: each pulled input has its own binding, from its stream's
/// offset rounded down to the storage offset alignment, and reads element (instance index + bias) /
/// divisor of a per-instance stream, the instance index counting from the first instance (step rate
/// one: divisor 1, bias 0; step rate two from first instance 1: divisor 2, bias 1). The left half
/// is drawn by instance data element 1 in red 0.25, the right by element 2 in red 0.75; element 0
/// is never read. Runs where vertex programs read two storage buffers (`VERTEX_STORAGE_READ`).
pub fn vertices_pulled_from_storage_match_the_fetched_ones<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::VERTEX_STORAGE_READ)
        || b.caps().limits.storage_buffers_per_stage < 2
    {
        return;
    }
    const N: u32 = 8;
    const PULL: &str = "
    struct Pulled { offset: u32, stride: u32, divisor: u32, bias: u32 }
    @group(0) @binding(0) var<uniform> pulled: array<Pulled, 2>;
    @group(1) @binding(160) var<storage> input0: array<u32>;
    @group(1) @binding(161) var<storage> input1: array<u32>;
    struct Out { @builtin(position) p: vec4<f32>, @location(0) @interpolate(flat) v: f32 }
    fn word(h: Pulled, vertex: u32, instance: u32) -> u32 {
        var element = vertex;
        if h.divisor != 0u {
            element = (instance + h.bias) / h.divisor;
        }
        return (h.offset + element * h.stride) / 4u;
    }
    @vertex fn pull(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Out {
        let a = word(pulled[0], vi, ii);
        let b = word(pulled[1], vi, ii);
        let p = vec2<f32>(bitcast<f32>(input0[a]), bitcast<f32>(input0[a + 1u]));
        let d = vec2<f32>(bitcast<f32>(input1[b]), bitcast<f32>(input1[b + 1u]));
        return Out(vec4<f32>(p.x + d.x, p.y, 0.5, 1.0), d.y);
    }
    @vertex fn fetch(@location(0) p: vec2<f32>, @location(1) d: vec2<f32>) -> Out {
        return Out(vec4<f32>(p.x + d.x, p.y, 0.5, 1.0), d.y);
    }
    @fragment fn fs(i: Out) -> @location(0) vec4<f32> {
        return vec4<f32>(i.v, 0.0, 0.0, 1.0);
    }
    ";
    // Positions of the left half as a strip at 0; instance data of two floats (x offset, red) at
    // 260, past a storage offset alignment.
    const AT: u64 = 260;
    let data = buffer(
        &mut b,
        512,
        BufferUsage::VERTEX | BufferUsage::STORAGE | BufferUsage::COPY_DST,
    );
    b.write_buffer(
        &data,
        0,
        &floats(&[-1.0, -1.0, -1.0, 1.0, 0.0, -1.0, 0.0, 1.0]),
        WriteMode::Discard,
    );
    b.write_buffer(
        &data,
        AT,
        &floats(&[0.5, 1.0, 0.0, 0.25, 1.0, 0.75]),
        WriteMode::Ordered,
    );
    let align = u64::from(b.caps().limits.min_storage_offset_alignment);
    let from = AT / align * align;
    let rs = raster(&mut b, CullMode::None);
    let range = |instances| DrawRange::Vertices {
        first: 0,
        count: 4,
        first_instance: 1,
        instances,
    };
    let mut pictures = Vec::new();

    // Fetched by the vertex layout.
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 8,
        step: StepMode::Vertex,
    });
    desc.streams[1] = Some(VertexStream {
        stride: 8,
        step: StepMode::Instance(1),
    });
    for (i, a) in desc.attributes[..2].iter_mut().enumerate() {
        *a = Some(VertexAttribute {
            location: i as u32,
            stream: i as u8,
            offset: 0,
            format: VertexFormat::Float32x2,
        });
    }
    let fetching = b.create_vertex_layout(&desc).expect("create_vertex_layout");
    let p = program(&mut b, PULL, "fetch", "fs");
    let (t, v) = rgba8_target(&mut b, N);
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
    let streams = [
        Some(VertexBinding {
            buffer: &data,
            offset: 0,
        }),
        Some(VertexBinding {
            buffer: &data,
            offset: AT,
        }),
    ];
    draw_into(
        &mut b,
        &p,
        &rs,
        &fetching,
        [None; MAX_BINDING_SETS],
        &streams,
        Topology::TriangleStrip,
        range(2),
        (N, N),
    );
    b.end_pass();
    pictures.push(read_texture(
        &mut b,
        region(&t, 0, 0, 0, 0),
        extent(N, N, 1),
        4,
    ));

    // Pulled, at step rates one and two.
    let empty = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        })
        .expect("create_vertex_layout");
    let p = program(&mut b, PULL, "pull", "fs");
    let uniforms = buffer(&mut b, 512, BufferUsage::UNIFORM | BufferUsage::COPY_DST);
    let storage = |offset, size| BindingResource::StorageBuffer {
        range: BufferRange {
            buffer: &data,
            offset,
            size,
        },
        writable: false,
    };
    let inputs = b
        .create_binding_set(&[
            BindingEntry {
                binding: 160,
                stages: StageSet::VERTEX,
                resource: storage(0, 32),
            },
            BindingEntry {
                binding: 161,
                stages: StageSet::VERTEX,
                resource: storage(from, 512 - from),
            },
        ])
        .expect("create_binding_set");
    let remainder = (AT - from) as u32;
    for (slot, (divisor, bias, instances)) in
        [(1u32, 0u32, 2u32), (2, 1, 4)].into_iter().enumerate()
    {
        let offset = slot as u64 * 256;
        let words = [0, 8, 0, 0, remainder, 8, divisor, bias];
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        b.write_buffer(&uniforms, offset, &bytes, WriteMode::Ordered);
        let helpers = b
            .create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &uniforms,
                    offset,
                    size: 32,
                }),
            }])
            .expect("create_binding_set");
        let (t, v) = rgba8_target(&mut b, N);
        b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
        draw_into(
            &mut b,
            &p,
            &rs,
            &empty,
            [Some(&helpers), Some(&inputs), None, None],
            &[],
            Topology::TriangleStrip,
            range(instances),
            (N, N),
        );
        b.end_pass();
        pictures.push(read_texture(
            &mut b,
            region(&t, 0, 0, 0, 0),
            extent(N, N, 1),
            4,
        ));
    }
    let mut expected = Vec::new();
    for _y in 0..N {
        for x in 0..N {
            expected.extend_from_slice(&[if x < N / 2 { 64 } else { 191 }, 0, 0, 255]);
        }
    }
    for got in &pictures {
        assert_close(got, &expected, 1, 4, N as usize);
    }
    ok(&mut b);
}

/// A constant buffer of 300 16-byte rows copied into an `rgba32uint` texture 256 texels wide, as
/// `CONSTANTS_TEXTURE_WIDTH` lays it out (two copies: a full texture row, then the rest), and read
/// by a program at texel (r % 256, r / 256): each read gives row r.
pub fn constants_read_through_a_texture_lie_256_rows_to_a_texture_row<H: Host>() {
    let mut b = H::make();
    needs(
        &b,
        Format::Rgba32Uint,
        FormatUse::SAMPLE | FormatUse::RENDER | FormatUse::COPY_OUT,
    );
    const ROWS: u32 = 300;
    const READ: [u32; 5] = [0, 1, 255, 256, 299];
    const CONSTANTS: &str = "
    @group(0) @binding(32) var constants: texture_2d<u32>;
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<u32> {
        var rows = array<u32, 5>(0u, 1u, 255u, 256u, 299u);
        let r = rows[u32(at.x)];
        return textureLoad(constants, vec2<u32>(r % 256u, r / 256u), 0);
    }
    ";
    let width = CONSTANTS_TEXTURE_WIDTH;
    let row = |r: u32| [r, r * 3 + 1, r ^ 0x55, 7];
    let words: Vec<u32> = (0..ROWS).flat_map(row).collect();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let src = buffer(
        &mut b,
        bytes.len() as u64,
        BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
    );
    b.write_buffer(&src, 0, &bytes, WriteMode::Discard);
    let tex = texture(
        &mut b,
        Format::Rgba32Uint,
        (width, ROWS.div_ceil(width)),
        1,
        1,
        TextureUsage::COPY_DST | TextureUsage::SAMPLE,
    );
    for (first, count) in [(0, width), (width, ROWS - width)] {
        b.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &src,
                offset: u64::from(first) * 16,
                row_pitch: width * 16,
                rows_per_layer: 1,
            },
            &region(&tex, 0, 0, 0, first / width),
            extent(count, 1, 1),
        );
    }
    let tv = view(&mut b, &tex, Format::Rgba32Uint);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&tv),
        }])
        .expect("create_binding_set");
    let out = texture(
        &mut b,
        Format::Rgba32Uint,
        (READ.len() as u32, 1),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let ov = view(&mut b, &out, Format::Rgba32Uint);
    let vb = quad(&mut b);
    let p = program(&mut b, CONSTANTS, "vs", "fs");
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    b.begin_pass(&color_pass::<H::Backend>(&ov, LoadOp::Clear([0; 4])));
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    draw_into(
        &mut b,
        &p,
        &rs,
        &vl,
        [Some(&set), None, None, None],
        &streams,
        Topology::TriangleStrip,
        once(4),
        (READ.len() as u32, 1),
    );
    b.end_pass();
    let got = read_texture(
        &mut b,
        region(&out, 0, 0, 0, 0),
        extent(READ.len() as u32, 1, 1),
        16,
    );
    let expected: Vec<u8> = READ
        .iter()
        .flat_map(|&r| row(r))
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(got, expected);
    ok(&mut b);
}

/// A texel of 0.5 in every component a format holds, loaded by a program: the components
/// `Format::components` names read 0.5, the others their defaults, zero and one for alpha, as a
/// border colour applied in the program reads them. The packed, shared-exponent, BGR and alpha-only
/// formats where the back end samples them.
pub fn missing_components_read_their_defaults<H: Host>() {
    let mut b = H::make();
    const LOAD: &str = "
    @group(0) @binding(32) var t: texture_2d<f32>;
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs() -> @location(0) vec4<f32> {
        return textureLoad(t, vec2<u32>(0u, 0u), 0);
    }
    ";
    let half = 0x3800u16.to_le_bytes();
    let packed = (0x380u32 | 0x380 << 11 | 0x1c0 << 22).to_le_bytes();
    // Each texel holds 0.5 in every channel (all ones in `b5g6r5unorm`, whose components then read
    // 255), the padding byte of `bgrx8unorm` too.
    let mut cases: Vec<(Format, Vec<u8>)> = vec![
        (Format::R8Unorm, vec![128]),
        (Format::Rg8Unorm, vec![128; 2]),
        (Format::Rgba8Unorm, vec![128; 4]),
        (Format::R16Float, half.to_vec()),
        (Format::Rg16Float, half.repeat(2)),
    ];
    for (f, _) in &cases {
        needs(&b, *f, FormatUse::SAMPLE);
    }
    let shared = (256u32 | 256 << 9 | 256 << 18 | 15 << 27).to_le_bytes();
    for (format, texel) in [
        (Format::Rg11b10Float, packed.to_vec()),
        (Format::Rgb9e5Float, shared.to_vec()),
        (Format::Bgra8Unorm, vec![128; 4]),
        (Format::Bgrx8Unorm, vec![128; 4]),
        (Format::A8Unorm, vec![128]),
        (Format::B5g6r5Unorm, vec![0xff; 2]),
    ] {
        if b.caps().formats.get(format).contains(FormatUse::SAMPLE) {
            cases.push((format, texel));
        }
    }
    let (t, v) = rgba8_target(&mut b, 1);
    let vb = quad(&mut b);
    let p = program(&mut b, LOAD, "vs", "fs");
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    for (format, texel) in cases {
        let tex = texture(
            &mut b,
            format,
            (1, 1),
            1,
            1,
            TextureUsage::COPY_DST | TextureUsage::SAMPLE,
        );
        b.write_texture(
            &region(&tex, 0, 0, 0, 0),
            extent(1, 1, 1),
            &texel,
            texel.len() as u32,
            1,
            WriteMode::Ordered,
        );
        let tv = view(&mut b, &tex, format);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&tv),
            }])
            .expect("create_binding_set");
        b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear([0; 4])));
        let streams = [Some(VertexBinding {
            buffer: &vb,
            offset: 0,
        })];
        draw_into(
            &mut b,
            &p,
            &rs,
            &vl,
            [Some(&set), None, None, None],
            &streams,
            Topology::TriangleStrip,
            once(4),
            (1, 1),
        );
        b.end_pass();
        let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(1, 1, 1), 4);
        let present = if format == Format::B5g6r5Unorm {
            255
        } else {
            128
        };
        let expected: Vec<u8> = (0..4)
            .map(|c| match (format.components() >> c & 1, c) {
                (1, _) => present,
                (_, 3) => 255,
                _ => 0,
            })
            .collect();
        assert_close(&got, &expected, 1, 4, 1);
        b.destroy(Object::BindingSet(set));
    }
    ok(&mut b);
}

/// Where the back end captures (`TRANSFORM_FEEDBACK`), a strip of two triangles captured outside a
/// pass writes its location outputs, packed 32-bit and in location order, as the lists it draws
/// (the odd triangle (1, 3, 2) with the first vertex provoking, (2, 1, 3) with the last):
/// interleaved into one target, and one output per target where it has two.
pub fn captures_write_location_outputs_packed_in_draw_order<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::TRANSFORM_FEEDBACK) {
        return;
    }
    const CAPTURED: &str = "
    struct V {
        @builtin(position) p: vec4<f32>,
        @location(0) at: vec2<f32>,
        @location(1) @interpolate(flat) i: u32,
    }
    @vertex fn vs(@location(0) p: vec2<f32>, @builtin(vertex_index) i: u32) -> V {
        return V(vec4<f32>(p, 0.5, 1.0), p, i);
    }
    ";
    const CORNERS: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
    let first = b.caps().features.contains(Features::PROVOKING_VERTEX_FIRST);
    let order: [u32; 6] = if first {
        [0, 1, 2, 1, 3, 2]
    } else {
        [0, 1, 2, 2, 1, 3]
    };
    let vb = buffer(&mut b, 32, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&CORNERS), WriteMode::Discard);
    let v = shader(&mut b, CAPTURED, "vs", ShaderStage::Vertex);
    let p = b
        .create_program(&Program::Render {
            vertex: &v,
            fragment: None,
        })
        .expect("create_program");
    let rs = b
        .create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::CounterClockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking: if first {
                ProvokingVertex::First
            } else {
                ProvokingVertex::Last
            },
            depth_minus_one: false,
            sample_mask: !0,
        })
        .expect("create_raster");
    let vl = positions(&mut b);
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let vp = viewport(2, 2);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    let call = DrawCall {
        program: &p,
        blend: &bl,
        depth_stencil: &ds,
        raster: &rs,
        vertex_layout: &vl,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::TriangleStrip,
        restart: false,
        viewports: &vp,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: once(4),
    };
    let (mut both, mut at, mut index) = (Vec::new(), Vec::new(), Vec::new());
    for i in order {
        let corner = floats(&CORNERS[i as usize * 2..][..2]);
        both.extend(&corner);
        both.extend(i.to_le_bytes());
        at.extend(corner);
        index.extend(i.to_le_bytes());
    }
    let usage = BufferUsage::CAPTURE | BufferUsage::COPY_SRC;
    let one = buffer(&mut b, 72, usage);
    let range = |buffer, size| BufferRange {
        buffer,
        offset: 0,
        size,
    };
    b.capture_vertices(&call, &[range(&one, 72)], false)
        .expect("capture_vertices");
    assert_eq!(read_buffer(&mut b, &one, 0, 72), both);
    if b.caps().limits.capture_buffers >= 2 {
        let (ta, ti) = (buffer(&mut b, 48, usage), buffer(&mut b, 24, usage));
        b.capture_vertices(&call, &[range(&ta, 48), range(&ti, 24)], false)
            .expect("capture_vertices");
        assert_eq!(read_buffer(&mut b, &ta, 0, 48), at);
        assert_eq!(read_buffer(&mut b, &ti, 0, 24), index);
    }
    ok(&mut b);
}

/// A copy between two regions of one subresource that do not overlap, as Vulkan and OpenGL allow
/// (a back end whose API refuses it copies through a buffer, `CopyWithinTexture`): the right half
/// of a 4 by 2 texture takes the left half's texels, which stay.
pub fn a_copy_within_one_subresource_moves_the_region<H: Host>() {
    let mut b = H::make();
    needs(&b, Format::Rgba8Unorm, FormatUse::COPY_OUT);
    copy_within_one_subresource(&mut b, Format::Rgba8Unorm, 4, pattern(32, 9));
}

/// The same within one level of a 3D texture, whose one subresource holds every slice: the left
/// half of slice 0 of a 4 by 2 by 2 `rgba8unorm` texture onto the right half of slice 1; and
/// within one level of each compressed format the back end samples and copies out, whole blocks
/// moved: the left column of blocks of a texture two blocks wide and high onto its right column.
pub fn a_copy_within_one_3d_or_compressed_subresource_moves_the_region<H: Host>() {
    let mut b = H::make();
    needs(&b, Format::Rgba8Unorm, FormatUse::COPY_OUT);
    let t = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D3,
            format: Format::Rgba8Unorm,
            width: 4,
            height: 2,
            depth_or_layers: 2,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .expect("create_texture");
    let bytes = pattern(64, 5);
    b.write_texture(
        &region(&t, 0, 0, 0, 0),
        extent(4, 2, 2),
        &bytes,
        16,
        2,
        WriteMode::Ordered,
    );
    let mut to = region(&t, 0, 0, 2, 0);
    to.origin.z = 1;
    b.copy_texture(&region(&t, 0, 0, 0, 0), &to, extent(2, 2, 1));
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(4, 2, 2), 4);
    let mut expected = bytes;
    for y in 0..2 {
        expected.copy_within(y * 16..y * 16 + 8, 32 + y * 16 + 8);
    }
    assert_eq!(got, expected, "3D");
    ok(&mut b);
    for format in [
        Format::Bc1RgbaUnorm,
        Format::Bc7RgbaUnorm,
        Format::Etc2Rgba8Unorm,
        Format::Astc4x4Unorm,
    ] {
        if !b
            .caps()
            .formats
            .get(format)
            .contains(FormatUse::SAMPLE | FormatUse::COPY_OUT)
        {
            continue;
        }
        let (w, h, size) = format.block();
        let t = texture(
            &mut b,
            format,
            (2 * w, 2 * h),
            1,
            1,
            TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
        );
        let bytes = pattern(4 * size as usize, 11);
        b.write_texture(
            &region(&t, 0, 0, 0, 0),
            extent(2 * w, 2 * h, 1),
            &bytes,
            2 * size,
            2,
            WriteMode::Ordered,
        );
        b.copy_texture(
            &region(&t, 0, 0, 0, 0),
            &region(&t, 0, 0, w, 0),
            extent(w, 2 * h, 1),
        );
        let got = read(
            &mut b,
            &ReadSource::Texture {
                region: region(&t, 0, 0, 0, 0),
                extent: extent(2 * w, 2 * h, 1),
            },
            bytes.len(),
        );
        let (row, block) = (2 * size as usize, size as usize);
        let mut expected = bytes;
        for y in 0..2 {
            expected.copy_within(y * row..y * row + block, y * row + block);
        }
        assert_eq!(got, expected, "{format:?}");
        ok(&mut b);
    }
}

/// The same for the depth and stencil formats a back end copies out, each written with values a
/// draw writes exactly (float depth in eighths, 16-bit depth and stencil any), 24-bit depth and the
/// formats with both, each aspect read back: a back end whose API copies these only between
/// subresources goes through a temporary texture of the same format, counted `CopyWithinTexture`.
pub fn a_copy_within_one_depth_or_stencil_subresource_moves_the_region<H: Host>() {
    let mut b = H::make();
    let depth32 = (0..8u32)
        .flat_map(|i| (i as f32 / 8.0).to_le_bytes())
        .collect();
    let depth16 = (0..8u16)
        .flat_map(|i| (i * 9000 + 7).to_le_bytes())
        .collect();
    for (format, texel, bytes) in [
        (Format::Depth32Float, 4, depth32),
        (Format::Depth16Unorm, 2, depth16),
        (Format::Stencil8, 1, pattern(8, 3)),
    ] {
        if b.caps().formats.get(format).contains(FormatUse::COPY_OUT) {
            copy_within_one_subresource(&mut b, format, texel, bytes);
        }
    }
    for format in [
        Format::Depth24Plus,
        Format::Depth24PlusStencil8,
        Format::Depth32FloatStencil8,
    ] {
        if b.caps().formats.get(format).contains(FormatUse::COPY_OUT) {
            copy_within_one_depth_subresource(&mut b, format);
        }
    }
}

/// As [`copy_within_one_subresource`] for a format whose depth reads back as a value, not its
/// bytes, and whose one copy moves its stencil too where it has one: depth written and read in
/// eighths (24-bit depth within one step of 2^-24), stencil any.
fn copy_within_one_depth_subresource<B: Backend>(b: &mut B, format: Format) {
    let has_stencil = format != Format::Depth24Plus;
    let t = texture(
        b,
        format,
        (4, 2),
        1,
        1,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    );
    let aspect = |stencil| TextureRegion {
        stencil,
        ..region(&t, 0, 0, 0, 0)
    };
    let eighth = |i: usize| i as f64 / 8.0;
    let depth: Vec<u8> = (0..8)
        .flat_map(|i| {
            let mut texel = [0; 4];
            crate::texel::encode_depth(format, eighth(i), &mut texel);
            texel
        })
        .collect();
    let stencil = pattern(8, 5);
    b.write_texture(
        &aspect(false),
        extent(4, 2, 1),
        &depth,
        16,
        2,
        WriteMode::Ordered,
    );
    if has_stencil {
        b.write_texture(
            &aspect(true),
            extent(4, 2, 1),
            &stencil,
            4,
            2,
            WriteMode::Ordered,
        );
    }
    b.copy_texture(
        &region(&t, 0, 0, 0, 0),
        &region(&t, 0, 0, 2, 0),
        extent(2, 2, 1),
    );
    // Texel i holds what texel `from(i)` held: the right half of each row takes its left half.
    let from = |i: usize| if i % 4 >= 2 { i - 2 } else { i };
    let got = read_texture(b, aspect(false), extent(4, 2, 1), 4);
    let step = match format {
        Format::Depth24Plus | Format::Depth24PlusStencil8 => 1.0 / 16_777_215.0,
        _ => 0.0,
    };
    for i in 0..8 {
        let g = crate::texel::decode_depth(format, &got[i * 4..i * 4 + 4]);
        let e = eighth(from(i));
        assert!(
            (g - e).abs() <= step,
            "{format:?} depth of texel {i}: {g}, not {e}"
        );
    }
    if has_stencil {
        let got = read_texture(b, aspect(true), extent(4, 2, 1), 1);
        let expected: Vec<u8> = (0..8).map(|i| stencil[from(i)]).collect();
        assert_eq!(got, expected, "{format:?} stencil");
    }
    ok(b);
}

/// Writes a 4 by 2 texture of `format`, copies its left half onto its right half, and reads back
/// the left half twice.
fn copy_within_one_subresource<B: Backend>(b: &mut B, format: Format, texel: u32, bytes: Vec<u8>) {
    let t = texture(
        b,
        format,
        (4, 2),
        1,
        1,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    );
    b.write_texture(
        &region(&t, 0, 0, 0, 0),
        extent(4, 2, 1),
        &bytes,
        4 * texel,
        2,
        WriteMode::Ordered,
    );
    b.copy_texture(
        &region(&t, 0, 0, 0, 0),
        &region(&t, 0, 0, 2, 0),
        extent(2, 2, 1),
    );
    let got = read_texture(b, region(&t, 0, 0, 0, 0), extent(4, 2, 1), texel as usize);
    let (row, half) = (4 * texel as usize, 2 * texel as usize);
    let mut expected = bytes;
    for y in 0..2 {
        expected.copy_within(y * row..y * row + half, y * row + half);
    }
    assert_eq!(got, expected, "{format:?}");
    ok(b);
}

/// A typed buffer read through a storage buffer (`TexelBufferRoute::Storage`): a read-only storage
/// buffer of words at its texture slot's binding, bound from an aligned offset, read by a fragment
/// program with the view's place in the bound range and its length from the fragment stage's
/// helper constants (`HelperValue::TexelBufferView`). An `rgba8unorm` view of three elements from
/// byte 8: pixels 0 to 2 read them, pixel 3 reads past the length, zero, though the bound range
/// holds a fourth element. Runs where fragment programs read storage buffers.
pub fn typed_buffers_through_storage_read_their_views_place_and_length<H: Host>() {
    let mut b = H::make();
    if b.caps().limits.storage_buffers_per_stage == 0 {
        return;
    }
    const VIEW: &str = "
    @group(0) @binding(33) var<storage> texels: array<u32>;
    @group(3) @binding(12) var<uniform> view: vec4<u32>;
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
        let i = u32(at.x);
        if i >= view.y {
            return vec4<f32>(0.0);
        }
        let w = texels[view.x / 4u + i];
        return vec4<f32>(vec4<u32>(w, w >> 8u, w >> 16u, w >> 24u) & vec4<u32>(255u)) / 255.0;
    }
    ";
    let element = |i: u8| [16 * (i + 1), 255 - 16 * i, 7, 200];
    let from = u64::from(b.caps().limits.min_storage_offset_alignment);
    let data = buffer(
        &mut b,
        from + 64,
        BufferUsage::STORAGE | BufferUsage::COPY_DST,
    );
    let mut bytes = vec![0xee; 8];
    bytes.extend((0..4).flat_map(element));
    b.write_buffer(&data, from, &bytes, WriteMode::Discard);
    let uniforms = buffer(&mut b, 16, BufferUsage::UNIFORM | BufferUsage::COPY_DST);
    let words: Vec<u8> = [8u32, 3, 0, 0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    b.write_buffer(&uniforms, 0, &words, WriteMode::Discard);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 33,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &data,
                    offset: from,
                    size: 32,
                },
                writable: false,
            },
        }])
        .expect("create_binding_set");
    let helpers = b
        .create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Fragment),
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &uniforms,
                offset: 0,
                size: 16,
            }),
        }])
        .expect("create_binding_set");
    let out = texture(
        &mut b,
        Format::Rgba8Unorm,
        (4, 1),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let ov = view(&mut b, &out, Format::Rgba8Unorm);
    let vb = quad(&mut b);
    let p = program(&mut b, VIEW, "vs", "fs");
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    b.begin_pass(&color_pass::<H::Backend>(&ov, LoadOp::Clear([0; 4])));
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    draw_into(
        &mut b,
        &p,
        &rs,
        &vl,
        [Some(&set), None, None, Some(&helpers)],
        &streams,
        Topology::TriangleStrip,
        once(4),
        (4, 1),
    );
    b.end_pass();
    let got = read_texture(&mut b, region(&out, 0, 0, 0, 0), extent(4, 1, 1), 4);
    let mut expected: Vec<u8> = (0..3).flat_map(element).collect();
    expected.extend([0; 4]);
    assert_close(&got, &expected, 1, 4, 4);
    ok(&mut b);
}

/// A typed buffer read through a texture (`TexelBufferRoute::Texture`): the buffer copied into a
/// 2D texture of the view's format `TEXEL_BUFFER_TEXTURE_WIDTH` texels wide, element e counted from
/// the buffer's start at texel (e % width, e / width), and the view's place from the fragment
/// helper constants. An `r32uint` buffer of 2100 elements, a view from byte 16, read at view
/// elements 0, 1, 2043, 2044 and 2095 (buffer elements 4, 5, 2047, 2048 and 2099).
pub fn typed_buffers_through_a_texture_lie_2048_elements_to_a_row<H: Host>() {
    let mut b = H::make();
    needs(&b, Format::R32Uint, FormatUse::SAMPLE);
    needs(
        &b,
        Format::Rgba32Uint,
        FormatUse::RENDER | FormatUse::COPY_OUT,
    );
    const ELEMENTS: u32 = 2100;
    const READ: [u32; 5] = [0, 1, 2043, 2044, 2095];
    const TEXELS: &str = "
    @group(0) @binding(32) var texels: texture_2d<u32>;
    @group(3) @binding(12) var<uniform> view: vec4<u32>;
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<u32> {
        var read = array<u32, 5>(0u, 1u, 2043u, 2044u, 2095u);
        let e = view.x / 4u + read[u32(at.x)];
        return vec4<u32>(textureLoad(texels, vec2<u32>(e % 2048u, e / 2048u), 0).x, e, 0u, 1u);
    }
    ";
    let width = TEXEL_BUFFER_TEXTURE_WIDTH;
    let element = |e: u32| e * 7 + 3;
    let bytes: Vec<u8> = (0..ELEMENTS)
        .flat_map(|e| element(e).to_le_bytes())
        .collect();
    let src = buffer(
        &mut b,
        bytes.len() as u64,
        BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
    );
    b.write_buffer(&src, 0, &bytes, WriteMode::Discard);
    let tex = texture(
        &mut b,
        Format::R32Uint,
        (width, ELEMENTS.div_ceil(width)),
        1,
        1,
        TextureUsage::COPY_DST | TextureUsage::SAMPLE,
    );
    for (first, count) in [(0, width), (width, ELEMENTS - width)] {
        b.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &src,
                offset: u64::from(first) * 4,
                row_pitch: width * 4,
                rows_per_layer: 1,
            },
            &region(&tex, 0, 0, 0, first / width),
            extent(count, 1, 1),
        );
    }
    let uniforms = buffer(&mut b, 16, BufferUsage::UNIFORM | BufferUsage::COPY_DST);
    let words: Vec<u8> = [16u32, ELEMENTS - 4, 0, 0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    b.write_buffer(&uniforms, 0, &words, WriteMode::Discard);
    let tv = view(&mut b, &tex, Format::R32Uint);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&tv),
        }])
        .expect("create_binding_set");
    let helpers = b
        .create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Fragment),
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &uniforms,
                offset: 0,
                size: 16,
            }),
        }])
        .expect("create_binding_set");
    let out = texture(
        &mut b,
        Format::Rgba32Uint,
        (READ.len() as u32, 1),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let ov = view(&mut b, &out, Format::Rgba32Uint);
    let vb = quad(&mut b);
    let p = program(&mut b, TEXELS, "vs", "fs");
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    b.begin_pass(&color_pass::<H::Backend>(&ov, LoadOp::Clear([0; 4])));
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    draw_into(
        &mut b,
        &p,
        &rs,
        &vl,
        [Some(&set), None, None, Some(&helpers)],
        &streams,
        Topology::TriangleStrip,
        once(4),
        (READ.len() as u32, 1),
    );
    b.end_pass();
    let got = read_texture(
        &mut b,
        region(&out, 0, 0, 0, 0),
        extent(READ.len() as u32, 1, 1),
        16,
    );
    let expected: Vec<u8> = READ
        .iter()
        .flat_map(|&r| [element(r + 4), r + 4, 0, 1])
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(got, expected);
    ok(&mut b);
}

/// Every vertex format is taken by every back end and reads its element (a back end whose API lacks
/// one converts it itself, `VertexFormatConverted`): one element, at attribute offset 12 of a stride
/// of 28 whose first bytes are filler, read as a `vec4` of the format's number type by a one-pixel
/// quad and written to an `rgba32` target of that type. Missing components read 0, 0 and 1 (the
/// integer 1 for integer formats). Normalised values are held within 2^-20, the others exactly, and
/// the packed floats' infinities and NaNs read as such.
pub fn every_vertex_format_reads_its_element<H: Host>() {
    use VertexFormat::*;
    const FORMATS: [VertexFormat; 52] = [
        Uint8x1,
        Uint8x2,
        Uint8x3,
        Uint8x4,
        Sint8x1,
        Sint8x2,
        Sint8x3,
        Sint8x4,
        Unorm8x1,
        Unorm8x2,
        Unorm8x3,
        Unorm8x4,
        Snorm8x1,
        Snorm8x2,
        Snorm8x3,
        Snorm8x4,
        Uint16x1,
        Uint16x2,
        Uint16x3,
        Uint16x4,
        Sint16x1,
        Sint16x2,
        Sint16x3,
        Sint16x4,
        Unorm16x1,
        Unorm16x2,
        Unorm16x3,
        Unorm16x4,
        Snorm16x1,
        Snorm16x2,
        Snorm16x3,
        Snorm16x4,
        Float16x1,
        Float16x2,
        Float16x3,
        Float16x4,
        Float32x1,
        Float32x2,
        Float32x3,
        Float32x4,
        Uint32x1,
        Uint32x2,
        Uint32x3,
        Uint32x4,
        Sint32x1,
        Sint32x2,
        Sint32x3,
        Sint32x4,
        Bgra8Unorm,
        Unorm10_10_10_2,
        Uint10_10_10_2,
        Float11_11_10,
    ];
    let mut b = H::make();
    let mut routes = Vec::new();
    for (kind, format) in [
        ("f32", Format::Rgba32Float),
        ("u32", Format::Rgba32Uint),
        ("i32", Format::Rgba32Sint),
    ] {
        needs(&b, format, FormatUse::RENDER | FormatUse::COPY_OUT);
        let source = format!(
            "struct Out {{ @builtin(position) pos: vec4<f32>, @location(0) @interpolate(flat) v: vec4<{kind}> }}
            @vertex fn vs(@location(0) p: vec2<f32>, @location(1) v: vec4<{kind}>) -> Out {{
                return Out(vec4<f32>(p, 0.5, 1.0), v);
            }}
            @fragment fn fs(i: Out) -> @location(0) vec4<{kind}> {{
                return i.v;
            }}"
        );
        let p = program(&mut b, &source, "vs", "fs");
        let t = texture(
            &mut b,
            format,
            (1, 1),
            1,
            1,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        );
        let v = view(&mut b, &t, format);
        routes.push((p, t, v));
    }
    let rs = raster(&mut b, CullMode::None);
    let vb = quad(&mut b);
    const OFFSET: u32 = 12;
    let values = buffer(&mut b, 128, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    for (format, (element, expected)) in FORMATS
        .into_iter()
        .flat_map(|f| vertex_elements(f).into_iter().map(move |e| (f, e)))
    {
        let bytes: Vec<u8> = (0..4)
            .flat_map(|_| [0xa5; OFFSET as usize].into_iter().chain(element))
            .collect();
        b.write_buffer(&values, 0, &bytes, WriteMode::Ordered);
        let mut desc = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        for (s, stride) in [(0, 8), (1, 16 + OFFSET)] {
            desc.streams[s] = Some(VertexStream {
                stride,
                step: StepMode::Vertex,
            });
        }
        desc.attributes[0] = Some(VertexAttribute {
            location: 0,
            stream: 0,
            offset: 0,
            format: Float32x2,
        });
        desc.attributes[1] = Some(VertexAttribute {
            location: 1,
            stream: 1,
            offset: OFFSET,
            format,
        });
        let vl = b.create_vertex_layout(&desc).expect("create_vertex_layout");
        let (p, t, v) = &routes[usize::from(matches!(expected, Read::Uint(_)))
            + 2 * usize::from(matches!(expected, Read::Sint(_)))];
        let streams = [
            Some(VertexBinding {
                buffer: &vb,
                offset: 0,
            }),
            Some(VertexBinding {
                buffer: &values,
                offset: 0,
            }),
        ];
        b.begin_pass(&color_pass::<H::Backend>(v, LoadOp::Clear([0; 4])));
        draw_into(
            &mut b,
            p,
            &rs,
            &vl,
            [None; MAX_BINDING_SETS],
            &streams,
            Topology::TriangleStrip,
            once(4),
            (1, 1),
        );
        b.end_pass();
        b.destroy(Object::VertexLayout(vl));
        let got = read_texture(&mut b, region(t, 0, 0, 0, 0), extent(1, 1, 1), 16);
        let word = |c: usize| u32::from_le_bytes(got[c * 4..c * 4 + 4].try_into().unwrap());
        match expected {
            Read::Float(e, tolerance) => {
                for (c, e) in e.iter().enumerate() {
                    let g = f32::from_bits(word(c));
                    let close = match e.is_finite() {
                        true => (g - e).abs() <= tolerance,
                        false => g == *e || g.is_nan() && e.is_nan(),
                    };
                    assert!(close, "{format:?} component {c}: {g}, not {e}");
                }
            }
            Read::Uint(e) => assert_eq!([0, 1, 2, 3].map(word), e, "{format:?}"),
            Read::Sint(e) => assert_eq!([0, 1, 2, 3].map(|c| word(c) as i32), e, "{format:?}"),
        }
    }
    ok(&mut b);
}

/// What a vertex program reads from an element.
enum Read {
    Float([f32; 4], f32),
    Uint([u32; 4]),
    Sint([i32; 4]),
}

/// Elements of `format` (16 bytes, the format's own first) and what each reads.
fn vertex_elements(format: VertexFormat) -> Vec<([u8; 16], Read)> {
    use VertexFormat::*;
    let mut element = [0u8; 16];
    let values = [0.5f32, -2.0, 3.25, 100.0];
    let read = match format {
        Float32x1 | Float32x2 | Float32x3 | Float32x4 | Float16x1 | Float16x2 | Float16x3
        | Float16x4 => {
            let n = (format as usize - Float16x1 as usize) % 4 + 1;
            let half = (format as usize) < Float32x1 as usize;
            for (c, v) in values.iter().take(n).enumerate() {
                match half {
                    true => element[c * 2..c * 2 + 2]
                        .copy_from_slice(&half::f16::from_f32(*v).to_le_bytes()),
                    false => element[c * 4..c * 4 + 4].copy_from_slice(&v.to_le_bytes()),
                }
            }
            let mut e = [0.0, 0.0, 0.0, 1.0];
            e[..n].copy_from_slice(&values[..n]);
            Read::Float(e, 0.0)
        }
        Bgra8Unorm => {
            element[..4].copy_from_slice(&[10, 20, 30, 255]);
            Read::Float(
                [30.0 / 255.0, 20.0 / 255.0, 10.0 / 255.0, 1.0],
                1.0 / 1048576.0,
            )
        }
        Unorm10_10_10_2 | Uint10_10_10_2 => {
            let (r, g, bl, a) = (1023u32, 512, 3, 2);
            element[..4].copy_from_slice(&(r | g << 10 | bl << 20 | a << 30).to_le_bytes());
            match format {
                Uint10_10_10_2 => Read::Uint([r, g, bl, a]),
                _ => Read::Float(
                    [1.0, 512.0 / 1023.0, 3.0 / 1023.0, 2.0 / 3.0],
                    1.0 / 1048576.0,
                ),
            }
        }
        Float11_11_10 => {
            // 5-bit exponents biased by 15: the least 11-bit denormal (2^-20), the greatest 11-bit
            // float, and 0.75 as a 10-bit float.
            // Then the all-ones exponent: infinity with a zero mantissa, NaN otherwise, in both
            // widths.
            let word = 1u32 | (30u32 << 6 | 63) << 11 | (14u32 << 5 | 16) << 22;
            let special = |r: u32, g: u32, b: u32| {
                let mut e = [0u8; 16];
                e[..4].copy_from_slice(
                    &(31 << 6 | r | (31 << 6 | g) << 11 | (31 << 5 | b) << 22).to_le_bytes(),
                );
                e
            };
            let (inf, nan) = (f32::INFINITY, f32::NAN);
            element[..4].copy_from_slice(&word.to_le_bytes());
            return vec![
                (
                    element,
                    Read::Float([2f32.powi(-20), 65024.0, 0.75, 1.0], 0.0),
                ),
                (special(0, 1, 0), Read::Float([inf, nan, inf, 1.0], 0.0)),
                (special(32, 0, 16), Read::Float([nan, inf, nan, 1.0], 0.0)),
            ];
        }
        _ => {
            // The 8, 16 and 32-bit integer and normalised formats: bytes of a fixed pattern, with
            // the most negative value in the first component of the signed ones.
            let i = format as usize;
            let n = i % 4 + 1;
            // The enum's order: uint, sint, unorm and snorm of 8 bits, then of 16, then two
            // float groups, then uint and sint of 32.
            let (size, kind) = match i {
                0..16 => (1, i / 4),
                16..32 => (2, (i - 16) / 4),
                _ => (4, (i - 40) / 4),
            };
            element.copy_from_slice(&pattern(16, 0x5b)[..]);
            element[size - 1] = 0x80;
            element[..size - 1].fill(0);
            let max = (1u64 << (size * 8)) - 1;
            let raw = |c: usize| {
                element[c * size..c * size + size]
                    .iter()
                    .rev()
                    .fold(0u64, |a, &x| (a << 8) | u64::from(x))
            };
            let signed = |c: usize| ((raw(c) << (64 - size * 8)) as i64) >> (64 - size * 8);
            match kind {
                0 => {
                    let mut e = [0, 0, 0, 1];
                    (0..n).for_each(|c| e[c] = raw(c) as u32);
                    Read::Uint(e)
                }
                1 => {
                    let mut e = [0, 0, 0, 1];
                    (0..n).for_each(|c| e[c] = signed(c) as i32);
                    Read::Sint(e)
                }
                2 => {
                    let mut e = [0.0, 0.0, 0.0, 1.0];
                    (0..n).for_each(|c| e[c] = (raw(c) as f64 / max as f64) as f32);
                    Read::Float(e, 1.0 / 1048576.0)
                }
                _ => {
                    let mut e = [0.0, 0.0, 0.0, 1.0];
                    let top = (max / 2) as f64;
                    (0..n).for_each(|c| e[c] = (signed(c) as f64 / top).max(-1.0) as f32);
                    Read::Float(e, 1.0 / 1048576.0)
                }
            }
        }
    };
    vec![(element, read)]
}
