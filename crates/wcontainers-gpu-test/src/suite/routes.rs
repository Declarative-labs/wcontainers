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
/// border colour applied in the program reads them. `rg11b10ufloat` where the back end samples it.
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
    if b.caps()
        .formats
        .get(Format::Rg11b10Float)
        .contains(FormatUse::SAMPLE)
    {
        cases.push((Format::Rg11b10Float, packed.to_vec()));
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
        let expected: Vec<u8> = (0..4)
            .map(|c| match (format.components() >> c & 1, c) {
                (1, _) => 128,
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
