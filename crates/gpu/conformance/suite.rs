//! The shared back-end suite: tests through the interface that every back end runs. Each uses only
//! what the interface promises every back end, checks exact answers where the APIs promise them and a
//! stated tolerance where they do not, and ends by checking the device is still working. A back end
//! runs them all with [`crate::tests!`].

use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, occlusion::*, passes::*, reflect::*, resource::*,
    shader::*, slow::*, state::*, *,
};

/// Reads the depth of a depth texel, as the recording back end stores it.
fn decode_depth(format: Format, bytes: &[u8]) -> f64 {
    let word =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    match format {
        Format::Depth16Unorm => f64::from(u16::from_le_bytes([bytes[0], bytes[1]])) / 65535.0,
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            f64::from(word(0) & 0xff_ffff) / 16_777_215.0
        }
        Format::Depth32Float | Format::Depth32FloatStencil8 => f64::from(f32::from_bits(word(0))),
        _ => 0.0,
    }
}

/// Writes the depth of a depth texel, leaving its stencil as it is.
fn encode_depth(format: Format, depth: f64, bytes: &mut [u8]) {
    let depth = depth.clamp(0.0, 1.0);
    match format {
        Format::Depth16Unorm => {
            let v = (depth * 65535.0).round_ties_even() as u16;
            bytes[..2].copy_from_slice(&v.to_le_bytes());
        }
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            let v = (depth * 16_777_215.0).round_ties_even() as u32;
            bytes[..3].copy_from_slice(&v.to_le_bytes()[..3]);
        }
        Format::Depth32Float | Format::Depth32FloatStencil8 => {
            bytes[..4].copy_from_slice(&(depth as f32).to_bits().to_le_bytes());
        }
        _ => {}
    }
}

/// A quad covering the whole target as a strip: four corners in clip space.
const QUAD: [f32; 8] = [-1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0];

/// Bytes no test writes by accident: a fixed pattern.
fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(37).wrapping_add(seed))
        .collect()
}

fn ok<B: Gpu>(b: &mut B) {
    assert_eq!(b.status(), DeviceStatus::Ok, "the device was lost");
}

fn needs<B: Gpu>(b: &B, format: Format, uses: FormatUse) {
    let has = b.caps().formats.uses[format as usize];
    assert!(
        has.contains(uses),
        "{format:?} lacks {:?}, which every back end has",
        uses - has
    );
}

fn buffer<B: Gpu>(b: &mut B, size: u64, usage: BufferUsage) -> B::Buffer {
    b.create_buffer(&BufferDesc {
        size,
        usage,
        intent: MapIntent::Rarely,
    })
    .expect("create_buffer")
}

fn texture<B: Gpu>(
    b: &mut B,
    format: Format,
    size: (u32, u32),
    layers: u32,
    mips: u32,
    usage: TextureUsage,
) -> B::Texture {
    b.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: size.0,
        height: size.1,
        depth_or_layers: layers,
        mip_levels: mips,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    })
    .expect("create_texture")
}

/// A 2D view of one level and one layer.
fn view<B: Gpu>(b: &mut B, texture: &B::Texture, format: Format) -> B::TextureView {
    b.create_view(
        texture,
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
    .expect("create_view")
}

fn region<B: Gpu>(
    texture: &B::Texture,
    mip: u32,
    layer: u32,
    x: u32,
    y: u32,
) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip,
        layer,
        origin: Origin { x, y, z: 0 },
        stencil: false,
    }
}

fn extent(width: u32, height: u32, layers: u32) -> Extent {
    Extent {
        width,
        height,
        depth_or_layers: layers,
    }
}

/// Reads back later: records the read, submits, waits and copies the answer out.
fn read<B: Gpu>(b: &mut B, src: &ReadSource<'_, B>, len: usize) -> Vec<u8> {
    let rb = b.read_back(src).expect("read_back");
    b.submit();
    b.wait_read(&rb);
    assert_eq!(
        b.read_state(&rb),
        ReadState::Ready,
        "a read back not ready after wait_read"
    );
    let mut out = vec![0; len];
    b.finish_read(rb, &mut out);
    out
}

fn read_buffer<B: Gpu>(b: &mut B, buffer: &B::Buffer, offset: u64, size: u64) -> Vec<u8> {
    read(
        b,
        &ReadSource::Buffer {
            buffer,
            offset,
            size,
        },
        size as usize,
    )
}

fn read_texture<B: Gpu>(
    b: &mut B,
    region: TextureRegion<'_, B>,
    e: Extent,
    texel: usize,
) -> Vec<u8> {
    let len = (e.width * e.height * e.depth_or_layers) as usize * texel;
    read(b, &ReadSource::Texture { region, extent: e }, len)
}

fn shader<B: Gpu>(b: &mut B, source: &str, entry: &str, stage: ShaderStage) -> B::Shader {
    let module = naga::front::wgsl::parse_str(source).expect("test program parses");
    // Programs reading the sample index run only where the back end declares `SAMPLE_SHADING`.
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::MULTISAMPLED_SHADING,
    )
    .validate(&module)
    .expect("test program validates");
    let mut hasher = blake3::Hasher::new();
    hasher.update(source.as_bytes());
    hasher.update(entry.as_bytes());
    b.create_shader(&ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: *hasher.finalize().as_bytes(),
    })
    .expect("create_shader")
}

fn program<B: Gpu>(b: &mut B, source: &str, vs: &str, fs: &str) -> B::Program {
    let v = shader(b, source, vs, ShaderStage::Vertex);
    let f = shader(b, source, fs, ShaderStage::Fragment);
    let p = b
        .create_program(&Program::Render {
            vertex: &v,
            fragment: Some(&f),
        })
        .expect("create_program");
    b.destroy(Object::Shader(v));
    b.destroy(Object::Shader(f));
    p
}

fn f32_bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

fn opaque() -> RenderTargetBlend {
    RenderTargetBlend {
        blend: None,
        writes: ColorWrites::all(),
    }
}

fn blend<B: Gpu>(b: &mut B, target: RenderTargetBlend) -> B::Blend {
    b.create_blend(&BlendDesc {
        targets: [target; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    })
    .expect("create_blend")
}

fn depth_test<B: Gpu>(b: &mut B, compare: Option<CompareFn>, write: bool) -> B::DepthStencil {
    b.create_depth_stencil(&DepthStencilDesc {
        depth_compare: compare,
        depth_write: write,
        stencil: None,
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    })
    .expect("create_depth_stencil")
}

/// The provoking vertex a test that draws no flat value asks for: Direct3D's first where the back end
/// has it, else the last, which every back end with either has (WebGL 2 without
/// `ANGLE_provoking_vertex`, OpenGL ES).
fn any_provoking<B: Gpu>(b: &B) -> ProvokingVertex {
    if b.caps().features.contains(Features::PROVOKING_VERTEX_FIRST) {
        ProvokingVertex::First
    } else {
        ProvokingVertex::Last
    }
}

fn raster<B: Gpu>(b: &mut B, cull: CullMode) -> B::Raster {
    b.create_raster(&RasterDesc {
        fill: FillMode::Solid,
        cull,
        front: FrontFace::CounterClockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: any_provoking(b),
        depth_minus_one: false,
        sample_mask: !0,
    })
    .expect("create_raster")
}

/// A layout of one stream of `vec2<f32>` positions at location 0.
fn positions<B: Gpu>(b: &mut B) -> B::VertexLayout {
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
    b.create_vertex_layout(&desc).expect("create_vertex_layout")
}

fn viewport(width: u32, height: u32) -> [Viewport; 1] {
    [Viewport {
        x: 0.0f32.to_bits(),
        y: 0.0f32.to_bits(),
        width: (width as f32).to_bits(),
        height: (height as f32).to_bits(),
        min_depth: 0.0f32.to_bits(),
        max_depth: 1.0f32.to_bits(),
    }]
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn color_pass<'a, B: Gpu>(view: &'a B::TextureView, load: LoadOp<[u32; 4]>) -> PassDesc<'a, B> {
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

/// Asserts each byte is within `tolerance` of the expected one, naming the first texel that is not.
fn assert_close(got: &[u8], expected: &[u8], tolerance: u8, texel: usize, width: usize) {
    assert_eq!(got.len(), expected.len());
    for (i, (g, e)) in got.iter().zip(expected).enumerate() {
        if g.abs_diff(*e) > tolerance {
            let t = i / texel;
            panic!(
                "texel ({}, {}) is {:?}, expected {:?} within {tolerance}",
                t % width,
                t / width,
                &got[t * texel..t * texel + texel],
                &expected[t * texel..t * texel + texel]
            );
        }
    }
}

/// Buffer writes at any byte offset land in the order they were recorded, a read back sees the bytes
/// as they were when it was recorded, and reading back now gives the same.
pub fn buffer_writes_land_in_order_and_read_back<B: Gpu>(mut b: B) {
    let usage = BufferUsage::VERTEX | BufferUsage::COPY_SRC | BufferUsage::COPY_DST;
    let buf = buffer(&mut b, 64, usage);
    let mut expected = pattern(64, 11);
    b.write_buffer(&buf, 0, &expected, WriteMode::Discard);
    b.write_buffer(&buf, 3, &[1, 2, 3, 4, 5], WriteMode::Ordered);
    b.write_buffer(&buf, 5, &[9], WriteMode::Ordered);
    b.write_buffer(&buf, 40, &[7; 6], WriteMode::NoOverwrite);
    expected[3..8].copy_from_slice(&[1, 2, 9, 4, 5]);
    expected[40..46].fill(7);
    assert_eq!(read_buffer(&mut b, &buf, 0, 64), expected);

    let mut part = vec![0; 7];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &buf,
            offset: 9,
            size: 7,
        },
        &mut part,
    )
    .expect("read_back_now");
    assert_eq!(part, expected[9..16]);

    // A write recorded after a read back does not change what it reads.
    let rb = b
        .read_back(&ReadSource::Buffer {
            buffer: &buf,
            offset: 0,
            size: 8,
        })
        .expect("read_back");
    b.write_buffer(&buf, 0, &[0xee; 8], WriteMode::Ordered);
    b.submit();
    b.wait_read(&rb);
    let mut first = [0; 8];
    b.finish_read(rb, &mut first);
    assert_eq!(first, expected[..8]);
    assert_eq!(read_buffer(&mut b, &buf, 0, 8), [0xee; 8]);
    ok(&mut b);
}

/// Texture writes with padded rows and layers land in the right texels of the right level and layer,
/// and read back tightly packed.
pub fn texture_writes_land_and_read_back<B: Gpu>(mut b: B) {
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::SAMPLE | FormatUse::COPY_OUT,
    );
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let t = texture(&mut b, Format::Rgba8Unorm, (5, 3), 2, 2, usage);
    // Both layers of level 0, rows 24 bytes apart (20 used), layers 4 rows apart (3 used).
    let data = pattern(24 * 4 * 2, 3);
    b.write_texture(
        &region(&t, 0, 0, 0, 0),
        extent(5, 3, 2),
        &data,
        24,
        4,
        WriteMode::Ordered,
    );
    let level1 = pattern(16, 99);
    b.write_texture(
        &region(&t, 1, 1, 0, 0),
        extent(2, 1, 1),
        &level1,
        16,
        1,
        WriteMode::Ordered,
    );
    let patch = pattern(12 * 2, 200);
    b.write_texture(
        &region(&t, 0, 0, 1, 1),
        extent(3, 2, 1),
        &patch,
        12,
        2,
        WriteMode::Ordered,
    );

    let layer = |l: usize| -> Vec<u8> {
        (0..3)
            .flat_map(|y| data[l * 96 + y * 24..l * 96 + y * 24 + 20].to_vec())
            .collect()
    };
    let mut layer0 = layer(0);
    for y in 0..2 {
        let at = (1 + y) * 20 + 4;
        layer0[at..at + 12].copy_from_slice(&patch[y * 12..y * 12 + 12]);
    }
    let layer1 = layer(1);
    assert_eq!(
        read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(5, 3, 1), 4),
        layer0
    );
    assert_eq!(
        read_texture(&mut b, region(&t, 0, 1, 0, 0), extent(5, 3, 1), 4),
        layer1
    );
    assert_eq!(
        read_texture(&mut b, region(&t, 1, 1, 0, 0), extent(2, 1, 1), 4),
        level1[..8]
    );
    let both: Vec<u8> = layer0.iter().chain(&layer1).copied().collect();
    assert_eq!(
        read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(5, 3, 2), 4),
        both
    );
    let corner: Vec<u8> = (0..2)
        .flat_map(|y| layer0[y * 20 + 8..y * 20 + 16].to_vec())
        .collect();
    assert_eq!(
        read_texture(&mut b, region(&t, 0, 0, 2, 0), extent(2, 2, 1), 4),
        corner
    );

    needs(
        &b,
        Format::R32Float,
        FormatUse::SAMPLE | FormatUse::COPY_OUT,
    );
    let f = texture(&mut b, Format::R32Float, (3, 2), 1, 1, usage);
    let values = floats(&[1.5, -2.25, 1e-3, 65504.0, -0.0, 3.0e38]);
    b.write_texture(
        &region(&f, 0, 0, 0, 0),
        extent(3, 2, 1),
        &values,
        12,
        2,
        WriteMode::Discard,
    );
    assert_eq!(
        read_texture(&mut b, region(&f, 0, 0, 0, 0), extent(3, 2, 1), 4),
        values
    );
    ok(&mut b);
}

/// Copies between buffers (at byte offsets, and within one buffer), from a buffer into a texture at a
/// 256-byte and at a tight row pitch over several rows, between textures, and from a texture into a
/// buffer (at offset 0, and at an offset over two layers) move exactly the bytes they name.
pub fn copies_move_bytes_exactly<B: Gpu>(mut b: B) {
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::SAMPLE | FormatUse::COPY_OUT,
    );
    let usage = BufferUsage::COPY_SRC | BufferUsage::COPY_DST;
    let src = buffer(&mut b, 1024, usage);
    let dst = buffer(&mut b, 1024, usage);
    let src_bytes = pattern(1024, 5);
    let mut dst_bytes = pattern(1024, 77);
    b.write_buffer(&src, 0, &src_bytes, WriteMode::Discard);
    b.write_buffer(&dst, 0, &dst_bytes, WriteMode::Discard);
    b.copy_buffer(&src, 1, &dst, 2, 6);
    dst_bytes[2..8].copy_from_slice(&src_bytes[1..7]);
    b.copy_buffer(&dst, 100, &dst, 200, 16);
    dst_bytes.copy_within(100..116, 200);
    assert_eq!(read_buffer(&mut b, &dst, 0, 1024), dst_bytes);

    let tusage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let a = texture(&mut b, Format::Rgba8Unorm, (4, 4), 1, 1, tusage);
    let mut a_bytes = pattern(64, 140);
    b.write_texture(
        &region(&a, 0, 0, 0, 0),
        extent(4, 4, 1),
        &a_bytes,
        16,
        4,
        WriteMode::Ordered,
    );
    // Two rows of three texels from offset 256 at a pitch of 256, then two rows at a tight pitch of
    // 12 bytes (which WebGPU refuses, so its back end repacks them).
    b.copy_buffer_to_texture(
        &CopyBufferTexture {
            buffer: &src,
            offset: 256,
            row_pitch: 256,
            rows_per_layer: 2,
        },
        &region(&a, 0, 0, 1, 1),
        extent(3, 2, 1),
    );
    for y in 0..2 {
        let at = (1 + y) * 16 + 4;
        a_bytes[at..at + 12].copy_from_slice(&src_bytes[256 + y * 256..256 + y * 256 + 12]);
    }
    b.copy_buffer_to_texture(
        &CopyBufferTexture {
            buffer: &src,
            offset: 12,
            row_pitch: 12,
            rows_per_layer: 2,
        },
        &region(&a, 0, 0, 0, 2),
        extent(3, 2, 1),
    );
    for y in 0..2 {
        let at = (2 + y) * 16;
        a_bytes[at..at + 12].copy_from_slice(&src_bytes[12 + y * 12..24 + y * 12]);
    }
    assert_eq!(
        read_texture(&mut b, region(&a, 0, 0, 0, 0), extent(4, 4, 1), 4),
        a_bytes
    );

    let c = texture(&mut b, Format::Rgba8Unorm, (4, 4), 1, 1, tusage);
    let mut c_bytes = pattern(64, 33);
    b.write_texture(
        &region(&c, 0, 0, 0, 0),
        extent(4, 4, 1),
        &c_bytes,
        16,
        4,
        WriteMode::Ordered,
    );
    b.copy_texture(
        &region(&a, 0, 0, 1, 1),
        &region(&c, 0, 0, 0, 2),
        extent(2, 2, 1),
    );
    for y in 0..2 {
        let (from, to) = ((1 + y) * 16 + 4, (2 + y) * 16);
        c_bytes[to..to + 8].copy_from_slice(&a_bytes[from..from + 8]);
    }
    b.copy_texture_to_buffer(
        &region(&c, 0, 0, 0, 0),
        &CopyBufferTexture {
            buffer: &dst,
            offset: 0,
            row_pitch: 256,
            rows_per_layer: 4,
        },
        extent(4, 4, 1),
    );
    for y in 0..4 {
        dst_bytes[y * 256..y * 256 + 16].copy_from_slice(&c_bytes[y * 16..y * 16 + 16]);
    }
    assert_eq!(read_buffer(&mut b, &dst, 0, 1024), dst_bytes);

    // Two layers of two rows of three texels from (1, 1) into a buffer at offset 12, rows 20 bytes
    // apart and layers three rows apart: the bytes between rows and layers keep their values.
    let layered = texture(&mut b, Format::Rgba8Unorm, (4, 4), 2, 1, tusage);
    let l_bytes = pattern(128, 201);
    b.write_texture(
        &region(&layered, 0, 0, 0, 0),
        extent(4, 4, 2),
        &l_bytes,
        16,
        4,
        WriteMode::Ordered,
    );
    let out = buffer(&mut b, 160, usage);
    let mut out_bytes = pattern(160, 3);
    b.write_buffer(&out, 0, &out_bytes, WriteMode::Discard);
    b.copy_texture_to_buffer(
        &region(&layered, 0, 0, 1, 1),
        &CopyBufferTexture {
            buffer: &out,
            offset: 12,
            row_pitch: 20,
            rows_per_layer: 3,
        },
        extent(3, 2, 2),
    );
    for layer in 0..2 {
        for y in 0..2 {
            let to = 12 + layer * 60 + y * 20;
            let from = layer * 64 + (1 + y) * 16 + 4;
            out_bytes[to..to + 12].copy_from_slice(&l_bytes[from..from + 12]);
        }
    }
    assert_eq!(read_buffer(&mut b, &out, 0, 160), out_bytes);
    ok(&mut b);
}

/// A pass's clear load writes the value in each target's format exactly (unorm rounded to nearest), and a buffer clear repeats
/// its pattern over exactly its range.
pub fn clears_fill_exactly<B: Gpu>(mut b: B) {
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    for (format, value, expected) in [
        // 63.75, 216.75 and 140.25 steps: rounded to nearest, as WebGPU does and within the 0.6
        // step Direct3D allows.
        (
            Format::Rgba8Unorm,
            f32_bits([0.25, 0.85, 0.55, 1.0]),
            vec![64u8, 217, 140, 255],
        ),
        (
            Format::R32Uint,
            [0xdead_beef, 0, 0, 0],
            0xdead_beefu32.to_le_bytes().to_vec(),
        ),
        (
            Format::R32Float,
            f32_bits([-1.5, 0.0, 0.0, 0.0]),
            (-1.5f32).to_le_bytes().to_vec(),
        ),
    ] {
        needs(&b, format, FormatUse::RENDER | FormatUse::COPY_OUT);
        let t = texture(&mut b, format, (3, 2), 1, 1, usage);
        let v = view(&mut b, &t, format);
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear(value)));
        b.end_pass();
        let texel = expected.len();
        let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(3, 2, 1), texel);
        assert_eq!(got, expected.repeat(6), "{format:?}");
    }
    let buf = buffer(&mut b, 32, BufferUsage::COPY_SRC | BufferUsage::COPY_DST);
    let mut bytes = pattern(32, 9);
    b.write_buffer(&buf, 0, &bytes, WriteMode::Discard);
    b.clear_buffer(&buf, 4, 16, &[1, 2, 3, 4]);
    for i in 0..4 {
        bytes[4 + i * 4..8 + i * 4].copy_from_slice(&[1, 2, 3, 4]);
    }
    assert_eq!(read_buffer(&mut b, &buf, 0, 32), bytes);
    ok(&mut b);
}

const TRIANGLE: &str = "
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.6, 1.0, 1.0);
}
";

/// A counter-clockwise triangle colours exactly the pixels whose centres it holds (no centre lies near
/// an edge, so every rule agrees), and a clockwise one is culled as a back face.
pub fn triangle_covers_the_pixels_whose_centres_it_holds<B: Gpu>(mut b: B) {
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::RENDER | FormatUse::COPY_OUT,
    );
    const N: u32 = 16;
    let front = [[-0.927f32, -0.819], [0.855, -0.526], [-0.337, 0.904]];
    let back = [[0.3f32, 0.95], [0.95, 0.95], [0.95, 0.3]];
    let verts: Vec<f32> = front.iter().chain(&back).flatten().copied().collect();
    let vb = buffer(&mut b, 48, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&verts), WriteMode::Discard);
    let t = texture(
        &mut b,
        Format::Rgba8Unorm,
        (N, N),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let v = view(&mut b, &t, Format::Rgba8Unorm);
    let p = program(&mut b, TRIANGLE, "vs", "fs");
    let (bl, ds, rs, vl) = (
        blend(&mut b, opaque()),
        depth_test(&mut b, None, false),
        raster(&mut b, CullMode::Back),
        positions(&mut b),
    );
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    b.begin_pass(&color_pass::<B>(
        &v,
        LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])),
    ));
    b.draw(&DrawCall {
        program: &p,
        blend: &bl,
        depth_stencil: &ds,
        raster: &rs,
        vertex_layout: &vl,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &vp,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 6,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);

    // Each centre's signed distance in pixels to each edge of the front triangle.
    let px = |c: [f32; 2]| {
        (
            (f64::from(c[0]) + 1.0) * 0.5 * f64::from(N),
            (1.0 - f64::from(c[1])) * 0.5 * f64::from(N),
        )
    };
    let corners: Vec<(f64, f64)> = front.iter().map(|&c| px(c)).collect();
    let mut expected = Vec::new();
    for y in 0..N {
        for x in 0..N {
            let q = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let d: Vec<f64> = (0..3)
                .map(|i| {
                    let (a, c) = (corners[i], corners[(i + 1) % 3]);
                    let len = ((c.0 - a.0).powi(2) + (c.1 - a.1).powi(2)).sqrt();
                    ((c.0 - a.0) * (q.1 - a.1) - (c.1 - a.1) * (q.0 - a.0)) / len
                })
                .collect();
            assert!(
                d.iter().all(|e| e.abs() > 0.01),
                "test triangle passes near pixel ({x}, {y})"
            );
            let inside = d.iter().all(|&e| e < 0.0) || d.iter().all(|&e| e > 0.0);
            expected.extend_from_slice(if inside {
                &[51, 153, 255, 255]
            } else {
                &[0, 0, 0, 255]
            });
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

const TEXTURED: &str = "
struct Params { color: vec4<f32>, depth: f32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(16) var samp: sampler;
@group(0) @binding(32) var tex: texture_2d<f32>;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs(@location(0) p: vec2<f32>) -> Out {
    var o: Out;
    o.pos = vec4<f32>(p, params.depth, 1.0);
    o.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}
@fragment fn flat() -> @location(0) vec4<f32> {
    return params.color;
}
@fragment fn textured(i: Out) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv);
}
";

/// Three draws into a colour and a depth target: an opaque left half nearer the viewer; a textured
/// quad over all of it, behind the left half, blended over the clear colour by the texture's alpha;
/// then a quad between the two depths, tested `Greater` and writing green only, which lands on the
/// left half alone. Exact where nothing is blended, within one step where it is.
pub fn textured_draw_with_depth_and_blending<B: Gpu>(mut b: B) {
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::RENDER | FormatUse::BLEND | FormatUse::SAMPLE | FormatUse::COPY_OUT,
    );
    needs(&b, Format::Depth32Float, FormatUse::DEPTH_STENCIL);
    const N: u32 = 4;
    let color = texture(
        &mut b,
        Format::Rgba8Unorm,
        (N, N),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let color_view = view(&mut b, &color, Format::Rgba8Unorm);
    let depth = texture(
        &mut b,
        Format::Depth32Float,
        (N, N),
        1,
        1,
        TextureUsage::RENDER,
    );
    let depth_view = view(&mut b, &depth, Format::Depth32Float);

    let mut texels = Vec::new();
    for j in 0..N {
        for i in 0..N {
            texels.extend_from_slice(&[
                (i * 60 + 15) as u8,
                (j * 60 + 15) as u8,
                ((i + j) * 20 + 10) as u8,
                128,
            ]);
        }
    }
    let tex = texture(
        &mut b,
        Format::Rgba8Unorm,
        (N, N),
        1,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    );
    b.write_texture(
        &region(&tex, 0, 0, 0, 0),
        extent(N, N, 1),
        &texels,
        N * 4,
        N,
        WriteMode::Ordered,
    );
    let tex_view = view(&mut b, &tex, Format::Rgba8Unorm);
    let sampler = b
        .create_sampler(&SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [AddressMode::Clamp; 3],
            lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .expect("create_sampler");

    // Parameters of the three draws, one per 256-byte step: colour, then depth.
    let step = u64::from(b.caps().limits.min_uniform_offset_alignment.max(256));
    let uniforms = buffer(
        &mut b,
        step * 3,
        BufferUsage::UNIFORM | BufferUsage::COPY_DST,
    );
    for (k, params) in [
        [1.0, 0.0, 0.0, 1.0, 0.25],
        [0.0, 0.0, 0.0, 0.0, 0.5],
        [0.0, 1.0, 0.0, 1.0, 0.375],
    ]
    .iter()
    .enumerate()
    {
        b.write_buffer(
            &uniforms,
            step * k as u64,
            &floats(params),
            WriteMode::Ordered,
        );
    }
    let set = |b: &mut B, k: u64, textured: bool| {
        let mut entries = vec![BindingEntry {
            binding: 0,
            stages: StageSet::VERTEX | StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &uniforms,
                offset: step * k,
                size: 32,
            }),
        }];
        if textured {
            entries.push(BindingEntry {
                binding: 16,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(&sampler),
            });
            entries.push(BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&tex_view),
            });
        }
        b.create_binding_set(&entries).expect("create_binding_set")
    };
    let sets = [
        set(&mut b, 0, false),
        set(&mut b, 1, true),
        set(&mut b, 2, false),
    ];

    let quads = [
        -1.0f32, -1.0, 0.0, -1.0, -1.0, 1.0, 0.0, 1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0,
    ];
    let vb = buffer(&mut b, 64, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&quads), WriteMode::Discard);
    let flat = program(&mut b, TEXTURED, "vs", "flat");
    let textured = program(&mut b, TEXTURED, "vs", "textured");
    let over = blend(
        &mut b,
        RenderTargetBlend {
            blend: Some((
                BlendComponent {
                    src: BlendFactor::SrcAlpha,
                    dst: BlendFactor::OneMinusSrcAlpha,
                    op: BlendOp::Add,
                },
                BlendComponent {
                    src: BlendFactor::One,
                    dst: BlendFactor::Zero,
                    op: BlendOp::Add,
                },
            )),
            writes: ColorWrites::all(),
        },
    );
    let opaque_blend = blend(&mut b, opaque());
    let green_only = blend(
        &mut b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::G,
        },
    );
    let nearer = depth_test(&mut b, Some(CompareFn::Less), true);
    let further = depth_test(&mut b, Some(CompareFn::Greater), false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];

    let mut pass = color_pass::<B>(&color_view, LoadOp::Clear(f32_bits([0.0, 0.0, 1.0, 1.0])));
    pass.depth = Some(DepthAttachment {
        view: &depth_view,
        depth: Some(AspectOps {
            load: LoadOp::Clear(1.0f32.to_bits()),
            store: StoreOp::Store,
            read_only: false,
        }),
        stencil: None,
    });
    b.begin_pass(&pass);
    let draws = [
        (&flat, &opaque_blend, &nearer, &sets[0], 0),
        (&textured, &over, &nearer, &sets[1], 4),
        (&flat, &green_only, &further, &sets[2], 4),
    ];
    for (program, blend, depth_stencil, set, first) in draws {
        b.draw(&DrawCall {
            program,
            blend,
            depth_stencil,
            raster: &rs,
            vertex_layout: &vl,
            bindings: [Some(set), None, None, None],
            vertex_buffers: &streams,
            index: None,
            topology: Topology::TriangleStrip,
            restart: false,
            viewports: &vp,
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first,
                count: 4,
                first_instance: 0,
                instances: 1,
            },
        });
    }
    b.end_pass();
    let got = read_texture(&mut b, region(&color, 0, 0, 0, 0), extent(N, N, 1), 4);

    let a = 128.0 / 255.0;
    let mut expected = Vec::new();
    for y in 0..N as usize {
        for x in 0..N as usize {
            if x < 2 {
                expected.extend_from_slice(&[255, 255, 0, 255]);
            } else {
                let t = &texels[(y * N as usize + x) * 4..][..4];
                let mix = |c: u8, d: f64| {
                    ((f64::from(c) / 255.0 * a + d * (1.0 - a)) * 255.0).round() as u8
                };
                expected.extend_from_slice(&[mix(t[0], 0.0), mix(t[1], 0.0), mix(t[2], 1.0), 128]);
            }
        }
    }
    assert_close(&got, &expected, 1, 4, N as usize);
    ok(&mut b);
}

/// The window a back end given to [`tests!`] presents into where its host gave it one.
const PRESENT_WINDOW: u32 = 1;

/// A surface of the host's window takes cleared frames one after another, and the device keeps working;
/// a back end built alone whose host named no window answers an error instead, and keeps working too.
///
/// The interface has no call that gives a back end a window: a host names its windows to the
/// implementation itself, so on a machine without a display (the build machines) no value given to
/// [`tests!`] has one. What the host then shows is read by each implementation's own test, since only
/// the host can see its window.
pub fn presents_cleared_frames<B: Gpu>(mut b: B) {
    needs(&b, Format::Rgba8Unorm, FormatUse::PRESENT);
    let surface = match b.create_surface(PRESENT_WINDOW) {
        Ok(surface) => surface,
        Err(_) => {
            assert_eq!(
                b.status(),
                DeviceStatus::Ok,
                "a window refused loses no device"
            );
            ok(&mut b);
            return;
        }
    };
    b.configure_surface(&surface, Format::Rgba8Unorm, 4, 4, true)
        .expect("configure_surface");
    for value in [[0.2, 0.4, 0.6, 1.0], [1.0, 0.0, 0.2, 1.0]] {
        let v = b.current_texture(&surface).expect("current_texture");
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear(f32_bits(value))));
        b.end_pass();
        b.present(&surface).expect("present");
        assert_eq!(b.status(), DeviceStatus::Ok);
    }
    ok(&mut b);
}

/// Draws `count` vertices from `first` of `vb` with fixed state into an `n` by `n` viewport.
#[allow(clippy::too_many_arguments)]
fn draw<B: Gpu>(
    b: &mut B,
    program: &B::Program,
    blend: &B::Blend,
    depth_stencil: &B::DepthStencil,
    raster: &B::Raster,
    layout: &B::VertexLayout,
    vb: &B::Buffer,
    topology: Topology,
    first: u32,
    count: u32,
    n: u32,
) {
    let vp = viewport(n, n);
    let streams = [Some(VertexBinding {
        buffer: vb,
        offset: 0,
    })];
    b.draw(&DrawCall {
        program,
        blend,
        depth_stencil,
        raster,
        vertex_layout: layout,
        bindings: [None; MAX_BINDING_SETS],
        vertex_buffers: &streams,
        index: None,
        topology,
        restart: false,
        viewports: &vp,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first,
            count,
            first_instance: 0,
            instances: 1,
        },
    });
}

/// A layout of one stream of a `vec2<f32>` position at location 0 and one `f32` at location 1.
fn positions_and_values<B: Gpu>(b: &mut B) -> B::VertexLayout {
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 12,
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
        stream: 0,
        offset: 8,
        format: VertexFormat::Float32x1,
    });
    b.create_vertex_layout(&desc).expect("create_vertex_layout")
}

fn raster_with<B: Gpu>(b: &mut B, provoking: ProvokingVertex) -> B::Raster {
    b.create_raster(&RasterDesc {
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
    })
    .expect("create_raster")
}

/// An `n` by `n` `rgba8unorm` target that can be read back, and its view.
fn rgba8_target<B: Gpu>(b: &mut B, n: u32) -> (B::Texture, B::TextureView) {
    needs(
        b,
        Format::Rgba8Unorm,
        FormatUse::RENDER | FormatUse::COPY_OUT,
    );
    let t = texture(
        b,
        Format::Rgba8Unorm,
        (n, n),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let v = view(b, &t, Format::Rgba8Unorm);
    (t, v)
}

/// An `n` by `n` `depth32float` target and its view, cleared to 1 by the pass that uses it.
fn depth_target<B: Gpu>(b: &mut B, n: u32) -> (B::Texture, B::TextureView) {
    needs(b, Format::Depth32Float, FormatUse::DEPTH_STENCIL);
    let t = texture(b, Format::Depth32Float, (n, n), 1, 1, TextureUsage::RENDER);
    let v = view(b, &t, Format::Depth32Float);
    (t, v)
}

fn with_depth<'a, B: Gpu>(mut pass: PassDesc<'a, B>, depth: &'a B::TextureView) -> PassDesc<'a, B> {
    pass.depth = Some(DepthAttachment {
        view: depth,
        depth: Some(AspectOps {
            load: LoadOp::Clear(1.0f32.to_bits()),
            store: StoreOp::Store,
            read_only: false,
        }),
        stencil: None,
    });
    pass
}

const VALUES: &str = "
struct Out { @builtin(position) pos: vec4<f32>, @location(0) @interpolate(flat) v: f32 }
@vertex fn flat(@location(0) p: vec2<f32>, @location(1) v: f32) -> Out {
    return Out(vec4<f32>(p, 0.5, 1.0), v);
}
@vertex fn at_depth(@location(0) p: vec2<f32>, @location(1) z: f32) -> Out {
    return Out(vec4<f32>(p, z, 1.0), 0.0);
}
@fragment fn red(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(i.v, 0.0, 0.0, 1.0);
}
@fragment fn white() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
struct Masked { @location(0) color: vec4<f32>, @builtin(sample_mask) mask: u32 }
@fragment fn masked() -> Masked {
    return Masked(vec4<f32>(1.0, 0.0, 0.0, 1.0), 0u);
}
";

/// Two triangles sharing a diagonal through pixel centres, with their other edges along rows and
/// columns of centres: every centre on an edge belongs to exactly one triangle, the one for which the
/// edge is a top or a left edge (Direct3D's top-left rule). Triangle A is written to red, B to green.
pub fn edges_through_pixel_centres_follow_the_top_left_rule<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    // In pixels, A is (0.5, 0.5), (3.5, 0.5), (0.5, 3.5): a top edge and a left edge through centres
    // and its right edge the diagonal. B is (3.5, 0.5), (3.5, 3.5), (0.5, 3.5): a right and a bottom
    // edge through centres and its left edge the diagonal.
    let verts = [
        -0.75f32, 0.75, 0.0, 0.75, 0.75, 0.0, -0.75, -0.75, 0.0, //
        0.75, 0.75, 0.0, 0.75, -0.75, 0.0, -0.75, -0.75, 0.0,
    ];
    let vb = buffer(&mut b, 72, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&verts), WriteMode::Discard);
    let p = program(&mut b, VALUES, "flat", "white");
    let red = blend(
        &mut b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::R,
        },
    );
    let green = blend(
        &mut b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::G,
        },
    );
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions_and_values(&mut b);
    b.begin_pass(&color_pass::<B>(
        &v,
        LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])),
    ));
    let list = Topology::TriangleList;
    draw(&mut b, &p, &red, &ds, &rs, &vl, &vb, list, 0, 3, N);
    draw(&mut b, &p, &green, &ds, &rs, &vl, &vb, list, 3, 3, N);
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    let mut expected = Vec::new();
    for y in 0..N {
        for x in 0..N {
            // A owns its top row and left column and the inside below the diagonal; B the diagonal
            // itself and the inside above it, not its right column or bottom row.
            let a = x + y < 3;
            let on_b = x + y >= 3 && x < 3 && y < 3;
            expected.extend_from_slice(&[
                if a { 255 } else { 0 },
                if on_b { 255 } else { 0 },
                0,
                255,
            ]);
        }
    }
    assert_close(&got, &expected, 0, 4, N as usize);
    ok(&mut b);
}

/// A strip of two triangles with a flat value per vertex: triangle i takes vertex i's value with the
/// first vertex provoking and vertex i + 2's with the last, odd triangles included.
pub fn strips_take_flat_values_from_the_provoking_vertex<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    // Vertices 0 bottom left, 1 top left, 2 bottom right, 3 top right; values 0, 0.2, 0.6, 1.
    let verts = [
        -1.0f32, -1.0, 0.0, -1.0, 1.0, 0.2, 1.0, -1.0, 0.6, 1.0, 1.0, 1.0,
    ];
    let vb = buffer(&mut b, 48, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&verts), WriteMode::Discard);
    let p = program(&mut b, VALUES, "flat", "red");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let vl = positions_and_values(&mut b);
    let mut ran = 0;
    for (feature, provoking, values) in [
        (
            Features::PROVOKING_VERTEX_FIRST,
            ProvokingVertex::First,
            [0u8, 51],
        ),
        (
            Features::PROVOKING_VERTEX_LAST,
            ProvokingVertex::Last,
            [153, 255],
        ),
    ] {
        if !b.caps().features.contains(feature) {
            continue;
        }
        ran += 1;
        let (t, v) = rgba8_target(&mut b, N);
        let rs = raster_with(&mut b, provoking);
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
        // Triangle 0 lies below the diagonal, triangle 1 above it; centres on it are left out.
        for y in 0..N as usize {
            for x in 0..N as usize {
                if x != y {
                    let want = values[usize::from(x > y)];
                    assert_eq!(
                        got[(y * N as usize + x) * 4],
                        want,
                        "{provoking:?}: texel ({x}, {y})"
                    );
                }
            }
        }
    }
    assert!(ran > 0, "a back end declares no provoking vertex");
    ok(&mut b);
}

/// A draw that tests depth with writes off leaves the depth as it was: a later draw behind it but in
/// front of the cleared depth still passes.
pub fn depth_writes_off_leave_the_depth<B: Gpu>(mut b: B) {
    const N: u32 = 2;
    let (t, v) = rgba8_target(&mut b, N);
    let (_d, dv) = depth_target(&mut b, N);
    let quads = [
        -1.0f32, -1.0, 0.25, -1.0, 1.0, 0.25, 1.0, -1.0, 0.25, 1.0, 1.0, 0.25, //
        -1.0, -1.0, 0.5, -1.0, 1.0, 0.5, 1.0, -1.0, 0.5, 1.0, 1.0, 0.5,
    ];
    let vb = buffer(&mut b, 96, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&quads), WriteMode::Discard);
    let p = program(&mut b, VALUES, "at_depth", "white");
    let red = blend(
        &mut b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::R,
        },
    );
    let green = blend(
        &mut b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::G,
        },
    );
    let test_only = depth_test(&mut b, Some(CompareFn::Less), false);
    let test_and_write = depth_test(&mut b, Some(CompareFn::Less), true);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions_and_values(&mut b);
    let pass = color_pass::<B>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
    b.begin_pass(&with_depth(pass, &dv));
    let strip = Topology::TriangleStrip;
    draw(&mut b, &p, &red, &test_only, &rs, &vl, &vb, strip, 0, 4, N);
    draw(
        &mut b,
        &p,
        &green,
        &test_and_write,
        &rs,
        &vl,
        &vb,
        strip,
        4,
        4,
        N,
    );
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    assert_close(&got, &[255, 255, 0, 255].repeat(4), 0, 4, N as usize);
    ok(&mut b);
}

/// A fragment whose program clears every bit of its sample mask covers no sample, so it writes no
/// colour and no depth: a later draw behind it but in front of the cleared depth still passes.
/// Where the back end has no [`Features::SAMPLE_SHADING`], programs cannot write the mask and the
/// test checks nothing beyond that.
pub fn a_fragment_covering_no_sample_writes_nothing<B: Gpu>(mut b: B) {
    if !b.caps().features.contains(Features::SAMPLE_SHADING) {
        return;
    }
    const N: u32 = 2;
    let (t, v) = rgba8_target(&mut b, N);
    let (_d, dv) = depth_target(&mut b, N);
    let quads = [
        -1.0f32, -1.0, 0.25, -1.0, 1.0, 0.25, 1.0, -1.0, 0.25, 1.0, 1.0, 0.25, //
        -1.0, -1.0, 0.5, -1.0, 1.0, 0.5, 1.0, -1.0, 0.5, 1.0, 1.0, 0.5,
    ];
    let vb = buffer(&mut b, 96, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&quads), WriteMode::Discard);
    let masked = program(&mut b, VALUES, "at_depth", "masked");
    let white = program(&mut b, VALUES, "at_depth", "white");
    let all = blend(&mut b, opaque());
    let green = blend(
        &mut b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::G,
        },
    );
    let less = depth_test(&mut b, Some(CompareFn::Less), true);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions_and_values(&mut b);
    let pass = color_pass::<B>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
    b.begin_pass(&with_depth(pass, &dv));
    let strip = Topology::TriangleStrip;
    draw(&mut b, &masked, &all, &less, &rs, &vl, &vb, strip, 0, 4, N);
    draw(&mut b, &white, &green, &less, &rs, &vl, &vb, strip, 4, 4, N);
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    assert_close(&got, &[0, 255, 0, 255].repeat(4), 0, 4, N as usize);
    ok(&mut b);
}

/// An sRGB target stores red, green and blue encoded by the sRGB curve and alpha as it is, both from
/// a clear and from a draw (the stored values computed from the curve, one step of tolerance).
pub fn srgb_targets_convert_colour_but_not_alpha<B: Gpu>(mut b: B) {
    needs(
        &b,
        Format::Rgba8UnormSrgb,
        FormatUse::RENDER | FormatUse::COPY_OUT,
    );
    const N: u32 = 2;
    let t = texture(
        &mut b,
        Format::Rgba8UnormSrgb,
        (N, N),
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let v = view(&mut b, &t, Format::Rgba8UnormSrgb);
    // Linear 0.25, 0.6 and 0.05 encode to 136.96, 203.42 and 63.19; alpha 0.4 is 102.
    b.begin_pass(&color_pass::<B>(
        &v,
        LoadOp::Clear(f32_bits([0.25, 0.6, 0.05, 0.4])),
    ));
    b.end_pass();
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    assert_close(&got, &[137, 203, 63, 102].repeat(4), 1, 4, N as usize);

    const SRGB: &str = "
    @vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
        return vec4<f32>(p, 0.5, 1.0);
    }
    @fragment fn fs() -> @location(0) vec4<f32> {
        return vec4<f32>(0.6, 0.05, 0.25, 0.4);
    }
    ";
    let quad = [-1.0f32, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0];
    let vb = buffer(&mut b, 32, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&quad), WriteMode::Discard);
    let p = program(&mut b, SRGB, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = raster(&mut b, CullMode::None);
    let vl = positions(&mut b);
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Load));
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
    assert_close(&got, &[203, 63, 137, 102].repeat(4), 1, 4, N as usize);
    ok(&mut b);
}

/// `generate_mips` of a texture that may be reinterpreted averages in the format its values are given
/// in: through the sRGB twin in linear light, through the stored format on the stored numbers.
pub fn generate_mips_averages_in_the_values_format<B: Gpu>(mut b: B) {
    // Columns of 0 and 200: stored, the average is 100; as sRGB, 200 is linear 0.5776, the
    // average 0.2888, which encodes to 146.31. Alpha 255 stays 255.
    for (values, expected) in [
        (Format::Rgba8UnormSrgb, [146u8, 146, 146, 255]),
        (Format::Rgba8Unorm, [100, 100, 100, 255]),
    ] {
        for f in [Format::Rgba8Unorm, Format::Rgba8UnormSrgb] {
            needs(
                &b,
                f,
                FormatUse::RENDER | FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT,
            );
        }
        let desc = TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: 2,
            height: 2,
            depth_or_layers: 1,
            mip_levels: 2,
            samples: 1,
            usage: TextureUsage::RENDER
                | TextureUsage::SAMPLE
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: true,
        };
        let t = b.create_texture(&desc).expect("create_texture");
        let row = [0u8, 0, 0, 255, 200, 200, 200, 255];
        b.write_texture(
            &region(&t, 0, 0, 0, 0),
            extent(2, 2, 1),
            &row.repeat(2),
            8,
            2,
            WriteMode::Ordered,
        );
        b.generate_mips(&t, &desc, values, 1..2, 0..1)
            .expect("generate_mips");
        let got = read_texture(&mut b, region(&t, 1, 0, 0, 0), extent(1, 1, 1), 4);
        assert_close(&got, &expected, 1, 4, 1);
        ok(&mut b);
    }
}

// Suite tests of the draw's fixed stages: interpolation, scissor, stencil, clipping and the depth
// range, sampler addressing and filtering, instancing and indices.

/// What [`CONSTANT`]'s colour 0.25, 0.85, 0.55, 1 stores in 8-bit unorm: 63.75, 216.75 and 140.25
/// steps round to nearest, and truncating or rounding up is off by more than the 0.6 step Direct3D
/// allows.
const ROUNDED_BYTES: [u8; 4] = [64, 217, 140, 255];

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
pub fn values_are_interpolated_with_perspective<B: Gpu>(mut b: B) {
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn the_scissor_bounds_the_draw<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    let vb = vertex_buffer(&mut b, &QUAD);
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = b
        .create_raster(&RasterDesc {
            scissor: true,
            ..raster_desc(any_provoking(&b))
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn a_scissor_past_the_target_is_clipped_to_it<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    let vb = vertex_buffer(&mut b, &QUAD);
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let rs = b
        .create_raster(&RasterDesc {
            scissor: true,
            ..raster_desc(any_provoking(&b))
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn clear_rects_clear_each_rectangle_clipped_to_the_view<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn copies_between_depth_stencil_textures_move_both_aspects<B: Gpu>(mut b: B) {
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
    let block =
        |b: &mut B, depth: Option<(CompareFn, bool)>, stencil: Option<(CompareFn, StencilOp)>| {
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
    let writes = |b: &mut B, writes| {
        blend(
            b,
            RenderTargetBlend {
                blend: None,
                writes,
            },
        )
    };
    let attach = |view, depth, stencil, store| DepthAttachment::<B> {
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
    let mut pass = color_pass::<B>(&v, black);
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
    let mut pass = color_pass::<B>(&v, black);
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
    let mut pass = color_pass::<B>(&v, black);
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
pub fn stencil_operations_and_tests<B: Gpu>(mut b: B) {
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
    let stencil = |b: &mut B, compare, fail, pass| {
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
    let writes = |b: &mut B, writes| {
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
    let mut pass = color_pass::<B>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
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
pub fn near_clipping_and_the_viewport_depth_range<B: Gpu>(mut b: B) {
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
    let pass = color_pass::<B>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
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
pub fn linear_filtering_and_address_modes<B: Gpu>(mut b: B) {
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
pub fn instances_start_at_the_first_instance<B: Gpu>(mut b: B) {
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
pub fn a_stream_of_stride_zero_reads_its_first_element<B: Gpu>(mut b: B) {
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn comparison_samplers_compare_with_the_stored_depth<B: Gpu>(mut b: B) {
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
    let sampler = |b: &mut B, compare| {
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
pub fn indexed_strips_restart_and_add_the_base_vertex<B: Gpu>(mut b: B) {
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
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows<B: Gpu>(mut b: B) {
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn fragments_write_storage_buffers<B: Gpu>(mut b: B) {
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
        .create_raster(&raster_desc(any_provoking(b)))
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
        .create_raster(&raster_desc(any_provoking(b)))
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
pub fn multisampled_targets_cover_their_samples_and_resolve<B: Gpu>(mut b: B) {
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
pub fn resolves_average_in_the_format_named<B: Gpu>(mut b: B) {
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
pub fn programs_run_per_sample_and_read_samples<B: Gpu>(mut b: B) {
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
        .create_raster(&raster_desc(any_provoking(&b)))
        .expect("create_raster");
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    b.begin_pass(&color_pass::<B>(&msv, LoadOp::Clear([0; 4])));
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
pub fn occlusion_queries_answer_whether_samples_passed<B: Gpu>(mut b: B) {
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
        .create_raster(&raster_desc(any_provoking(&b)))
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
    let mut pass = color_pass::<B>(&v, LoadOp::Clear([0; 4]));
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
pub fn the_occlusion_counter_counts_every_sample_that_passes<B: Gpu>(mut b: B) {
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
        let mut colors: [Option<ColorTarget<B>>; MAX_COLOR_TARGETS] = Default::default();
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

// Suite tests of what fronts route through the base: fill modes, values passed between programs
// converted apart, vertices pulled from storage, constants read through a texture, and the
// components a view of a format reads.

/// One draw of fixed state but for what each test names, into a viewport of `size`.
#[allow(clippy::too_many_arguments)]
fn draw_into<B: Gpu>(
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

fn quad<B: Gpu>(b: &mut B) -> B::Buffer {
    let vb = buffer(b, 32, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    b.write_buffer(&vb, 0, &floats(&QUAD), WriteMode::Discard);
    vb
}

fn once<'a, B: Gpu>(count: u32) -> DrawRange<'a, B> {
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
pub fn fill_modes_draw_edges_or_corners<B: Gpu>(mut b: B) {
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
            provoking: any_provoking(&b),
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
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn values_pass_between_programs_by_location<B: Gpu>(mut b: B) {
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn vertices_pulled_from_storage_match_the_fetched_ones<B: Gpu>(mut b: B) {
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
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
pub fn constants_read_through_a_texture_lie_256_rows_to_a_texture_row<B: Gpu>(mut b: B) {
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
    b.begin_pass(&color_pass::<B>(&ov, LoadOp::Clear([0; 4])));
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
pub fn missing_components_read_their_defaults<B: Gpu>(mut b: B) {
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
        if b.caps().formats.uses[format as usize].contains(FormatUse::SAMPLE) {
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
        b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
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
            .map(|c| {
                match (
                    wcontainers_gpu::format::FORMAT_FACTS[format as usize].components >> c & 1,
                    c,
                ) {
                    (1, _) => present,
                    (_, 3) => 255,
                    _ => 0,
                }
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
pub fn captures_write_location_outputs_packed_in_draw_order<B: Gpu>(mut b: B) {
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
pub fn a_copy_within_one_subresource_moves_the_region<B: Gpu>(mut b: B) {
    needs(&b, Format::Rgba8Unorm, FormatUse::COPY_OUT);
    copy_within_one_subresource(&mut b, Format::Rgba8Unorm, 4, pattern(32, 9));
}

/// The same within one level of a 3D texture, whose one subresource holds every slice: the left
/// half of slice 0 of a 4 by 2 by 2 `rgba8unorm` texture onto the right half of slice 1; and
/// within one level of each compressed format the back end samples and copies out, whole blocks
/// moved: the left column of blocks of a texture two blocks wide and high onto its right column.
pub fn a_copy_within_one_3d_or_compressed_subresource_moves_the_region<B: Gpu>(mut b: B) {
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
        if !b.caps().formats.uses[format as usize].contains(FormatUse::SAMPLE | FormatUse::COPY_OUT)
        {
            continue;
        }
        let (w, h, size) = wcontainers_gpu::format::FORMAT_FACTS[format as usize].block;
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
pub fn a_copy_within_one_depth_or_stencil_subresource_moves_the_region<B: Gpu>(mut b: B) {
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
        if b.caps().formats.uses[format as usize].contains(FormatUse::COPY_OUT) {
            copy_within_one_subresource(&mut b, format, texel, bytes);
        }
    }
    for format in [
        Format::Depth24Plus,
        Format::Depth24PlusStencil8,
        Format::Depth32FloatStencil8,
    ] {
        if b.caps().formats.uses[format as usize].contains(FormatUse::COPY_OUT) {
            copy_within_one_depth_subresource(&mut b, format);
        }
    }
}

/// As [`copy_within_one_subresource`] for a format whose depth reads back as a value, not its
/// bytes, and whose one copy moves its stencil too where it has one: depth written and read in
/// eighths (24-bit depth within one step of 2^-24), stencil any.
fn copy_within_one_depth_subresource<B: Gpu>(b: &mut B, format: Format) {
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
            encode_depth(format, eighth(i), &mut texel);
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
        let g = decode_depth(format, &got[i * 4..i * 4 + 4]);
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
fn copy_within_one_subresource<B: Gpu>(b: &mut B, format: Format, texel: u32, bytes: Vec<u8>) {
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
pub fn typed_buffers_through_storage_read_their_views_place_and_length<B: Gpu>(mut b: B) {
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
            binding: ShaderStage::Fragment.helper_binding(),
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
    b.begin_pass(&color_pass::<B>(&ov, LoadOp::Clear([0; 4])));
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
pub fn typed_buffers_through_a_texture_lie_2048_elements_to_a_row<B: Gpu>(mut b: B) {
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
            binding: ShaderStage::Fragment.helper_binding(),
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
    b.begin_pass(&color_pass::<B>(&ov, LoadOp::Clear([0; 4])));
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
pub fn every_vertex_format_reads_its_element<B: Gpu>(mut b: B) {
    use VertexFormat::*;
    const FORMATS: [VertexFormat; 53] = [
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
        Bgrx8Unorm,
    ];
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
        b.begin_pass(&color_pass::<B>(v, LoadOp::Clear([0; 4])));
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
        Bgra8Unorm | Bgrx8Unorm => {
            // B8G8R8X8's fourth byte is not read: zero there still reads as alpha one.
            let fourth = if format == Bgrx8Unorm { 0 } else { 255 };
            element[..4].copy_from_slice(&[10, 20, 30, fourth]);
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

/// Slow paths are counted one by one, and the first use since the host took the warnings queues one.
pub fn slow_paths_are_counted_and_warned_once<B: Gpu>(mut b: B) {
    let path = SlowPath::ReinterpretCopy;
    let earlier = b.take_slow_path_warnings();
    assert!(
        earlier.len() <= SLOW_PATH_COUNT,
        "a warning per path at most"
    );
    let before = b.slow_paths().counts[path as usize];
    b.record_slow_path(path);
    b.record_slow_path(path);
    assert_eq!(b.slow_paths().counts[path as usize], before + 2);
    let warned = b.take_slow_path_warnings();
    let expected = usize::from(before == 0);
    assert_eq!(warned.iter().filter(|p| **p == path).count(), expected);
    assert!(b.take_slow_path_warnings().is_empty());
    ok(&mut b);
}

/// The defaults' state holds a ring for each [`RingKind`], and a ring write places its parts at
/// offsets that are multiples of the alignment asked for, each part apart from the others.
pub fn ring_writes_place_each_part_at_an_aligned_offset<B: Gpu>(mut b: B) {
    assert_eq!(
        b.helpers().rings.len(),
        2,
        "a ring for indices and one for data"
    );
    let parts = [pattern(16, 1), pattern(32, 2)];
    let mut offsets = [0; 2];
    let align = b.caps().limits.min_uniform_offset_alignment;
    let ring = b
        .write_buffer_ring(RingKind::Data, &[&parts[0], &parts[1]], align, &mut offsets)
        .expect("write_buffer_ring");
    for offset in offsets {
        assert_eq!(
            offset % u64::from(align),
            0,
            "offset {offset} is aligned to {align}"
        );
    }
    let [(a, la), (c, lc)] = [0, 1].map(|i| (offsets[i], parts[i].len() as u64));
    assert!(a + la <= c || c + lc <= a, "parts at {a} and {c} overlap");
    // The first part is a uniform block a binding set can name.
    b.create_binding_set(&[BindingEntry {
        binding: 0,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer: &ring,
            offset: a,
            size: la,
        }),
    }])
    .expect("a binding set of the ring's first part");
    ok(&mut b);
}

/// Binding sets of the same entries have the same layout key, which a state combination names.
pub fn binding_sets_of_one_layout_share_its_key<B: Gpu>(mut b: B) {
    let uniform = buffer(&mut b, 256, BufferUsage::UNIFORM | BufferUsage::COPY_DST);
    let entries = [BindingEntry {
        binding: 0,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer: &uniform,
            offset: 0,
            size: 16,
        }),
    }];
    let one = b.create_binding_set(&entries).expect("create_binding_set");
    let two = b.create_binding_set(&entries).expect("create_binding_set");
    assert_eq!(b.binding_layout_key(&one), b.binding_layout_key(&two));
    ok(&mut b);
}

/// A submit's fence is done once waited on, and a write submitted before it has landed.
pub fn fences_are_done_once_waited_on<B: Gpu>(mut b: B) {
    let data = pattern(32, 5);
    let dst = buffer(&mut b, 32, BufferUsage::COPY_DST | BufferUsage::COPY_SRC);
    b.write_buffer(&dst, 0, &data, WriteMode::Ordered);
    let fence = b.submit();
    b.wait_fence(&fence);
    assert!(b.fence_done(&fence), "a fence waited on is done");
    assert_eq!(read_buffer(&mut b, &dst, 0, 32), data);
    ok(&mut b);
}

/// A triangle fan covers what its triangles cover, after its combination was prepared ahead.
pub fn fans_cover_their_triangles<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    let (t, v) = rgba8_target(&mut b, N);
    // Corners in fan order: the fan covers the whole target.
    let vb = vertex_buffer(&mut b, &[-1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0]);
    let p = program(&mut b, CONSTANT, "vs", "fs");
    let bl = blend(&mut b, opaque());
    let ds = depth_test(&mut b, None, false);
    let provoking = any_provoking(&b);
    let rs = b
        .create_raster(&raster_desc(provoking))
        .expect("create_raster");
    let vl = positions(&mut b);
    let vp = viewport(N, N);
    let mut colors = [None; MAX_COLOR_TARGETS];
    colors[0] = Some(Format::Rgba8Unorm);
    b.prepare(&PrepareDesc {
        program: &p,
        blend: &bl,
        depth_stencil: &ds,
        raster: &rs,
        vertex_layout: &vl,
        targets: TargetLayout {
            colors,
            depth_stencil: None,
            samples: 1,
        },
        topology: Topology::TriangleList,
        layouts: [BindingLayoutKey::default(); MAX_BINDING_SETS],
    });
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    b.begin_pass(&color_pass::<B>(&v, LoadOp::Clear([0; 4])));
    let drawn = b.draw_fan_or_loop(
        &call(
            &p,
            &bl,
            &ds,
            &rs,
            &vl,
            &streams,
            &vp,
            Topology::TriangleFan,
            vertices(0, 4),
        ),
        FanOrLoop::TriangleFan,
        IndexSource::Sequential { first: 0, count: 4 },
        provoking,
        0,
        1,
    );
    b.end_pass();
    drawn.expect("draw_fan_or_loop");
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    assert_eq!(got, ROUNDED_BYTES.repeat((N * N) as usize));
    ok(&mut b);
}

/// A colour clear of a whole texture fills every texel of every level asked for.
pub fn clear_texture_fills_every_texel<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::RENDER | FormatUse::COPY_OUT,
    );
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: N,
        height: N,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
        cube: false,
        reinterpret: false,
    };
    let t = b.create_texture(&desc).expect("create_texture");
    let red = f32_bits([1.0, 0.0, 0.0, 1.0]);
    b.clear_texture(
        &t,
        &desc,
        Format::Rgba8Unorm,
        0..1,
        0..1,
        ClearValue::Color(red),
    )
    .expect("clear_texture");
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    assert_eq!(got, [255, 0, 0, 255].repeat((N * N) as usize));
    ok(&mut b);
}

/// Each present-or-absent ability answers [`GpuError::Absent`] exactly where the caps leave it out.
pub fn absent_abilities_answer_absent<B: Gpu>(mut b: B) {
    let features = b.caps().features;
    let table = b.create_resource_table(4);
    match table {
        Err(GpuError::Absent(f)) => {
            assert!(!features.contains(Features::RESOURCE_TABLES));
            assert_eq!(f, Features::RESOURCE_TABLES);
        }
        Ok(table) => {
            assert!(features.contains(Features::RESOURCE_TABLES));
            b.write_resource_table(&table, 0, &[])
                .expect("an empty write of a table");
        }
        Err(e) => panic!("create_resource_table: {e:?}"),
    }
    let disjoint = b.timestamps_disjoint();
    assert_eq!(
        matches!(disjoint, Err(GpuError::Absent(_))),
        !features.contains(Features::TIMESTAMPS),
        "timestamps_disjoint: {disjoint:?}"
    );
    if features.contains(Features::TIMESTAMPS) {
        let set = b
            .create_query_set(QueryKind::Timestamp, 2)
            .expect("create_query_set");
        b.write_timestamp(&set, 0).expect("write_timestamp");
        b.write_timestamp(&set, 1).expect("write_timestamp");
        let rb = b.read_queries(&set, 0, 2).expect("read_queries");
        b.submit();
        b.wait_read(&rb);
        let mut out = [0u8; 16];
        b.finish_read(rb, &mut out);
        let [first, second] =
            [&out[..8], &out[8..]].map(|w| u64::from_le_bytes(w.try_into().expect("eight bytes")));
        assert!(second >= first, "time runs forward: {first} then {second}");
    }
    if features.contains(Features::COMPUTE) {
        let s = shader(
            &mut b,
            "@compute @workgroup_size(1) fn main() {}",
            "main",
            ShaderStage::Compute,
        );
        let p = b
            .create_program(&Program::Compute(&s))
            .expect("create_program");
        b.dispatch(&DispatchCall {
            program: &p,
            bindings: [None; MAX_BINDING_SETS],
            immediates: &[],
            groups: DispatchGroups::Direct([1, 1, 1]),
        })
        .expect("dispatch");
    } else {
        let p = program(&mut b, CONSTANT, "vs", "fs");
        let answer = b.dispatch(&DispatchCall {
            program: &p,
            bindings: [None; MAX_BINDING_SETS],
            immediates: &[],
            groups: DispatchGroups::Direct([1, 1, 1]),
        });
        assert!(
            matches!(answer, Err(GpuError::Absent(f)) if f == Features::COMPUTE),
            "dispatch without COMPUTE: {answer:?}"
        );
    }
    ok(&mut b);
}

/// Bytes another party changes while the back end holds them: each byte goes up by one after every
/// time they are read, and before they are written over.
struct Drifting {
    bytes: core::cell::RefCell<Vec<u8>>,
}

impl Drifting {
    fn new(bytes: Vec<u8>) -> Self {
        Drifting {
            bytes: core::cell::RefCell::new(bytes),
        }
    }

    fn drift(&self) {
        for byte in self.bytes.borrow_mut().iter_mut() {
            *byte = byte.wrapping_add(1);
        }
    }

    /// Checks that each byte of `got` is a value byte `i` of `first` held during the call.
    fn held(first: &[u8], got: &[u8]) {
        assert_eq!(first.len(), got.len());
        for (i, (f, g)) in first.iter().zip(got).enumerate() {
            assert!(
                *g == *f || *g == f.wrapping_add(1),
                "byte {i}: {g}, which it never held ({f} or the next)"
            );
        }
    }
}

impl wcontainers_volatile_bytes::VolatileBytes for Drifting {
    fn len(&self) -> u64 {
        self.bytes.borrow().len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = {
            let held = self.bytes.borrow();
            let count = into.len().min(held.len());
            into[..count].copy_from_slice(&held[..count]);
            count
        };
        self.drift();
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.drift();
        let mut held = self.bytes.borrow_mut();
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(
        &self,
        sink: &mut Sink,
    ) -> Result<usize, Sink::Error> {
        let written = sink.write(&self.bytes.borrow());
        self.drift();
        written
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(
        &self,
        source: &mut Source,
    ) -> Result<usize, Source::Error> {
        self.drift();
        source.read(&mut self.bytes.borrow_mut())
    }
}

/// A buffer written from bytes another party changes during the call holds, of each byte, a value
/// it held then.
pub fn buffer_writes_from_changing_bytes_land_as_held<B: Gpu>(mut b: B) {
    let first = pattern(256, 11);
    let guest = Drifting::new(first.clone());
    let dst = buffer(&mut b, 256, BufferUsage::COPY_DST | BufferUsage::COPY_SRC);
    b.write_buffer_from(&dst, 0, &guest, WriteMode::Ordered);
    assert_ne!(
        guest.bytes.borrow()[0],
        first[0],
        "the bytes were read during the call"
    );
    Drifting::held(&first, &read_buffer(&mut b, &dst, 0, 256));
    ok(&mut b);
}

/// A texture written from texels another party changes during the call holds, of each byte, a value
/// it held then.
pub fn texture_writes_from_changing_bytes_land_as_held<B: Gpu>(mut b: B) {
    const N: u32 = 4;
    needs(
        &b,
        Format::Rgba8Unorm,
        FormatUse::SAMPLE | FormatUse::COPY_OUT,
    );
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let t = texture(&mut b, Format::Rgba8Unorm, (N, N), 1, 1, usage);
    let first = pattern((N * N * 4) as usize, 23);
    let guest = Drifting::new(first.clone());
    b.write_texture_from(
        &region(&t, 0, 0, 0, 0),
        extent(N, N, 1),
        &guest,
        N * 4,
        N,
        WriteMode::Ordered,
    );
    assert_ne!(
        guest.bytes.borrow()[0],
        first[0],
        "the texels were read during the call"
    );
    let got = read_texture(&mut b, region(&t, 0, 0, 0, 0), extent(N, N, 1), 4);
    Drifting::held(&first, &got);
    ok(&mut b);
}

/// A read back finished into bytes another party changes during the call leaves them holding what
/// was read back.
pub fn read_backs_finish_into_changing_bytes<B: Gpu>(mut b: B) {
    let data = pattern(256, 31);
    let src = buffer(&mut b, 256, BufferUsage::COPY_DST | BufferUsage::COPY_SRC);
    b.write_buffer(&src, 0, &data, WriteMode::Ordered);
    let rb = b
        .read_back(&ReadSource::Buffer {
            buffer: &src,
            offset: 0,
            size: 256,
        })
        .expect("read_back");
    b.submit();
    b.wait_read(&rb);
    let guest = Drifting::new(vec![0; 256]);
    b.finish_read_into(rb, &guest);
    assert_eq!(*guest.bytes.borrow(), data);
    ok(&mut b);
}

/// Runs each suite function as its own test on a back end made anew for every test.
///
/// A back end is built alone through [`StandaloneGpu`], so every implementation says `StandaloneGpu: yes`.
#[macro_export]
macro_rules! tests {
    (@StandaloneGpu yes ($value:expr)) => {
        $crate::tests!(@StandaloneGpu yes ($value) ($value));
    };
    (@StandaloneGpu yes ($base:expr) ($value:expr)) => {
        mod standalone_gpu {}
    };
    (@StandaloneGpu no ($value:expr)) => {
        mod standalone_gpu {
            #[test]
            fn lacks_standalone_gpu() {
                struct Probe<T>(::core::marker::PhantomData<T>);
                impl<T> ::core::clone::Clone for Probe<T> {
                    fn clone(&self) -> Self {
                        Probe(::core::marker::PhantomData)
                    }
                }
                struct Implemented;
                #[allow(dead_code)]
                impl<T: ::wcontainers_gpu::StandaloneGpu> Probe<T> {
                    fn clone(&self) -> Implemented {
                        Implemented
                    }
                }
                fn probe_of<T>(_value: &T) -> Probe<T> {
                    Probe(::core::marker::PhantomData)
                }
                let _: Probe<_> = probe_of(&$value).clone();
            }
        }
    };
    ($value:expr; StandaloneGpu: $standalone_gpu:ident $(($standalone_gpu_value:expr))?) => {
        #[test]
        fn buffer_writes_land_in_order_and_read_back() {
            $crate::buffer_writes_land_in_order_and_read_back($value);
        }

        #[test]
        fn texture_writes_land_and_read_back() {
            $crate::texture_writes_land_and_read_back($value);
        }

        #[test]
        fn copies_move_bytes_exactly() {
            $crate::copies_move_bytes_exactly($value);
        }

        #[test]
        fn clears_fill_exactly() {
            $crate::clears_fill_exactly($value);
        }

        #[test]
        fn triangle_covers_the_pixels_whose_centres_it_holds() {
            $crate::triangle_covers_the_pixels_whose_centres_it_holds($value);
        }

        #[test]
        fn textured_draw_with_depth_and_blending() {
            $crate::textured_draw_with_depth_and_blending($value);
        }

        #[test]
        fn presents_cleared_frames() {
            $crate::presents_cleared_frames($value);
        }

        #[test]
        fn edges_through_pixel_centres_follow_the_top_left_rule() {
            $crate::edges_through_pixel_centres_follow_the_top_left_rule($value);
        }

        #[test]
        fn strips_take_flat_values_from_the_provoking_vertex() {
            $crate::strips_take_flat_values_from_the_provoking_vertex($value);
        }

        #[test]
        fn depth_writes_off_leave_the_depth() {
            $crate::depth_writes_off_leave_the_depth($value);
        }

        #[test]
        fn a_fragment_covering_no_sample_writes_nothing() {
            $crate::a_fragment_covering_no_sample_writes_nothing($value);
        }

        #[test]
        fn srgb_targets_convert_colour_but_not_alpha() {
            $crate::srgb_targets_convert_colour_but_not_alpha($value);
        }

        #[test]
        fn generate_mips_averages_in_the_values_format() {
            $crate::generate_mips_averages_in_the_values_format($value);
        }

        #[test]
        fn values_are_interpolated_with_perspective() {
            $crate::values_are_interpolated_with_perspective($value);
        }

        #[test]
        fn the_scissor_bounds_the_draw() {
            $crate::the_scissor_bounds_the_draw($value);
        }

        #[test]
        fn a_scissor_past_the_target_is_clipped_to_it() {
            $crate::a_scissor_past_the_target_is_clipped_to_it($value);
        }

        #[test]
        fn clear_rects_clear_each_rectangle_clipped_to_the_view() {
            $crate::clear_rects_clear_each_rectangle_clipped_to_the_view($value);
        }

        #[test]
        fn copies_between_depth_stencil_textures_move_both_aspects() {
            $crate::copies_between_depth_stencil_textures_move_both_aspects($value);
        }

        #[test]
        fn stencil_operations_and_tests() {
            $crate::stencil_operations_and_tests($value);
        }

        #[test]
        fn near_clipping_and_the_viewport_depth_range() {
            $crate::near_clipping_and_the_viewport_depth_range($value);
        }

        #[test]
        fn linear_filtering_and_address_modes() {
            $crate::linear_filtering_and_address_modes($value);
        }

        #[test]
        fn instances_start_at_the_first_instance() {
            $crate::instances_start_at_the_first_instance($value);
        }

        #[test]
        fn a_stream_of_stride_zero_reads_its_first_element() {
            $crate::a_stream_of_stride_zero_reads_its_first_element($value);
        }

        #[test]
        fn comparison_samplers_compare_with_the_stored_depth() {
            $crate::comparison_samplers_compare_with_the_stored_depth($value);
        }

        #[test]
        fn indexed_strips_restart_and_add_the_base_vertex() {
            $crate::indexed_strips_restart_and_add_the_base_vertex($value);
        }

        #[test]
        fn an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows() {
            $crate::an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows($value);
        }

        #[test]
        fn fragments_write_storage_buffers() {
            $crate::fragments_write_storage_buffers($value);
        }

        #[test]
        fn multisampled_targets_cover_their_samples_and_resolve() {
            $crate::multisampled_targets_cover_their_samples_and_resolve($value);
        }

        #[test]
        fn resolves_average_in_the_format_named() {
            $crate::resolves_average_in_the_format_named($value);
        }

        #[test]
        fn programs_run_per_sample_and_read_samples() {
            $crate::programs_run_per_sample_and_read_samples($value);
        }

        #[test]
        fn occlusion_queries_answer_whether_samples_passed() {
            $crate::occlusion_queries_answer_whether_samples_passed($value);
        }

        #[test]
        fn the_occlusion_counter_counts_every_sample_that_passes() {
            $crate::the_occlusion_counter_counts_every_sample_that_passes($value);
        }

        #[test]
        fn fill_modes_draw_edges_or_corners() {
            $crate::fill_modes_draw_edges_or_corners($value);
        }

        #[test]
        fn values_pass_between_programs_by_location() {
            $crate::values_pass_between_programs_by_location($value);
        }

        #[test]
        fn vertices_pulled_from_storage_match_the_fetched_ones() {
            $crate::vertices_pulled_from_storage_match_the_fetched_ones($value);
        }

        #[test]
        fn constants_read_through_a_texture_lie_256_rows_to_a_texture_row() {
            $crate::constants_read_through_a_texture_lie_256_rows_to_a_texture_row($value);
        }

        #[test]
        fn missing_components_read_their_defaults() {
            $crate::missing_components_read_their_defaults($value);
        }

        #[test]
        fn captures_write_location_outputs_packed_in_draw_order() {
            $crate::captures_write_location_outputs_packed_in_draw_order($value);
        }

        #[test]
        fn a_copy_within_one_subresource_moves_the_region() {
            $crate::a_copy_within_one_subresource_moves_the_region($value);
        }

        #[test]
        fn a_copy_within_one_3d_or_compressed_subresource_moves_the_region() {
            $crate::a_copy_within_one_3d_or_compressed_subresource_moves_the_region($value);
        }

        #[test]
        fn a_copy_within_one_depth_or_stencil_subresource_moves_the_region() {
            $crate::a_copy_within_one_depth_or_stencil_subresource_moves_the_region($value);
        }

        #[test]
        fn typed_buffers_through_storage_read_their_views_place_and_length() {
            $crate::typed_buffers_through_storage_read_their_views_place_and_length($value);
        }

        #[test]
        fn typed_buffers_through_a_texture_lie_2048_elements_to_a_row() {
            $crate::typed_buffers_through_a_texture_lie_2048_elements_to_a_row($value);
        }

        #[test]
        fn every_vertex_format_reads_its_element() {
            $crate::every_vertex_format_reads_its_element($value);
        }

        #[test]
        fn slow_paths_are_counted_and_warned_once() {
            $crate::slow_paths_are_counted_and_warned_once($value);
        }

        #[test]
        fn ring_writes_place_each_part_at_an_aligned_offset() {
            $crate::ring_writes_place_each_part_at_an_aligned_offset($value);
        }

        #[test]
        fn binding_sets_of_one_layout_share_its_key() {
            $crate::binding_sets_of_one_layout_share_its_key($value);
        }

        #[test]
        fn fences_are_done_once_waited_on() {
            $crate::fences_are_done_once_waited_on($value);
        }

        #[test]
        fn fans_cover_their_triangles() {
            $crate::fans_cover_their_triangles($value);
        }

        #[test]
        fn clear_texture_fills_every_texel() {
            $crate::clear_texture_fills_every_texel($value);
        }

        #[test]
        fn absent_abilities_answer_absent() {
            $crate::absent_abilities_answer_absent($value);
        }

        #[test]
        fn buffer_writes_from_changing_bytes_land_as_held() {
            $crate::buffer_writes_from_changing_bytes_land_as_held($value);
        }

        #[test]
        fn texture_writes_from_changing_bytes_land_as_held() {
            $crate::texture_writes_from_changing_bytes_land_as_held($value);
        }

        #[test]
        fn read_backs_finish_into_changing_bytes() {
            $crate::read_backs_finish_into_changing_bytes($value);
        }

        $crate::tests!(@StandaloneGpu $standalone_gpu ($value) $(($standalone_gpu_value))?);
    };
}
