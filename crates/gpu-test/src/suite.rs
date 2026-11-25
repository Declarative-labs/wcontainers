//! The shared back-end suite: tests through the interface that every back end runs. Each uses only
//! what the interface promises every back end, checks exact answers where the APIs promise them and a
//! stated tolerance where they do not, and ends by checking the device is still working. A back end
//! runs them all with [`crate::suite!`].

use wcontainers_gpu::*;

mod draws;
mod routes;
pub use draws::*;
pub use routes::*;

/// What the suite needs of a back end beyond the interface.
pub trait Host {
    type Backend: Gpu;
    /// A new device.
    fn make() -> Self::Backend;
    /// A window to present into, as the host's graphics hook names it.
    fn window(backend: &mut Self::Backend) -> u32;
    /// The texels last shown in `window`, rows top first, tightly packed in the surface's format. A host
    /// gives its back end a window it can see (a browser canvas read through a 2D canvas, a native
    /// window read by the host's graphics hook), since the interface reads no surface back.
    fn shown(backend: &mut Self::Backend, window: u32) -> Vec<u8>;
}

/// Declares one `#[test]` per suite test for a [`Host`]; `suite!(@each Host; test, ...)` declares the
/// named ones only, for a host that cannot run every test (one that cannot see its window leaves out
/// `present_shows_the_frame`).
#[macro_export]
macro_rules! suite {
    ($host:ty) => {
        $crate::suite!(@each $host;
            buffer_writes_land_in_order_and_read_back,
            texture_writes_land_and_read_back,
            copies_move_bytes_exactly,
            clears_fill_exactly,
            triangle_covers_the_pixels_whose_centres_it_holds,
            textured_draw_with_depth_and_blending,
            edges_through_pixel_centres_follow_the_top_left_rule,
            strips_take_flat_values_from_the_provoking_vertex,
            depth_writes_off_leave_the_depth,
            a_fragment_covering_no_sample_writes_nothing,
            srgb_targets_convert_colour_but_not_alpha,
            generate_mips_averages_in_the_values_format,
            values_are_interpolated_with_perspective,
            the_scissor_bounds_the_draw,
            a_scissor_past_the_target_is_clipped_to_it,
            clear_rects_clear_each_rectangle_clipped_to_the_view,
            copies_between_depth_stencil_textures_move_both_aspects,
            stencil_operations_and_tests,
            near_clipping_and_the_viewport_depth_range,
            linear_filtering_and_address_modes,
            instances_start_at_the_first_instance,
            indexed_strips_restart_and_add_the_base_vertex,
            an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows,
            a_stream_of_stride_zero_reads_its_first_element,
            comparison_samplers_compare_with_the_stored_depth,
            fragments_write_storage_buffers,
            multisampled_targets_cover_their_samples_and_resolve,
            resolves_average_in_the_format_named,
            programs_run_per_sample_and_read_samples,
            occlusion_queries_answer_whether_samples_passed,
            the_occlusion_counter_counts_every_sample_that_passes,
            fill_modes_draw_edges_or_corners,
            values_pass_between_programs_by_location,
            vertices_pulled_from_storage_match_the_fetched_ones,
            constants_read_through_a_texture_lie_256_rows_to_a_texture_row,
            missing_components_read_their_defaults,
            typed_buffers_through_storage_read_their_views_place_and_length,
            typed_buffers_through_a_texture_lie_2048_elements_to_a_row,
            captures_write_location_outputs_packed_in_draw_order,
            a_copy_within_one_subresource_moves_the_region,
            a_copy_within_one_depth_or_stencil_subresource_moves_the_region,
            a_copy_within_one_3d_or_compressed_subresource_moves_the_region,
            every_vertex_format_reads_its_element,
            indirect_draws_and_dispatches_read_their_arguments_when_they_run,
            timestamps_between_passes_read_back_in_order,
            programs_ask_a_texture_its_levels_and_samples,
            depth_copies_through_buffers_keep_their_pitches_and_values,
            bgra4_texels_sample_and_copy_in_direct3d_bit_order,
            depth_bounds_keep_fragments_whose_stored_depth_lies_within,
            present_shows_the_frame,
        );
    };
    (@each $host:ty; $($test:ident),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                $crate::suite::$test::<$host>();
            }
        )*
    };
}

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
        provoking: ProvokingVertex::First,
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
pub fn buffer_writes_land_in_order_and_read_back<H: Host>() {
    let mut b = H::make();
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
pub fn texture_writes_land_and_read_back<H: Host>() {
    let mut b = H::make();
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
pub fn copies_move_bytes_exactly<H: Host>() {
    let mut b = H::make();
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
pub fn clears_fill_exactly<H: Host>() {
    let mut b = H::make();
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
        b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Clear(value)));
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
pub fn triangle_covers_the_pixels_whose_centres_it_holds<H: Host>() {
    let mut b = H::make();
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
    b.begin_pass(&color_pass::<H::Backend>(
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
pub fn textured_draw_with_depth_and_blending<H: Host>() {
    let mut b = H::make();
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
    let set = |b: &mut H::Backend, k: u64, textured: bool| {
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

    let mut pass =
        color_pass::<H::Backend>(&color_view, LoadOp::Clear(f32_bits([0.0, 0.0, 1.0, 1.0])));
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

/// What is drawn into the surface's current texture is what the host shows, frame after frame.
pub fn present_shows_the_frame<H: Host>() {
    let mut b = H::make();
    needs(&b, Format::Rgba8Unorm, FormatUse::PRESENT);
    let window = H::window(&mut b);
    let surface = b.create_surface(window).expect("create_surface");
    b.configure_surface(&surface, Format::Rgba8Unorm, 4, 4, true)
        .expect("configure_surface");
    for (value, texel) in [
        ([0.2, 0.4, 0.6, 1.0], [51, 102, 153, 255]),
        ([1.0, 0.0, 0.2, 1.0], [255, 0, 51, 255]),
    ] {
        let v = b.current_texture(&surface).expect("current_texture");
        b.begin_pass(&color_pass::<H::Backend>(
            &v,
            LoadOp::Clear(f32_bits(value)),
        ));
        b.end_pass();
        b.present(&surface).expect("present");
        assert_eq!(H::shown(&mut b, window), texel.repeat(16));
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
pub fn edges_through_pixel_centres_follow_the_top_left_rule<H: Host>() {
    let mut b = H::make();
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
    b.begin_pass(&color_pass::<H::Backend>(
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
pub fn strips_take_flat_values_from_the_provoking_vertex<H: Host>() {
    let mut b = H::make();
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
pub fn depth_writes_off_leave_the_depth<H: Host>() {
    let mut b = H::make();
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
    let pass = color_pass::<H::Backend>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
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
pub fn a_fragment_covering_no_sample_writes_nothing<H: Host>() {
    let mut b = H::make();
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
    let pass = color_pass::<H::Backend>(&v, LoadOp::Clear(f32_bits([0.0, 0.0, 0.0, 1.0])));
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
pub fn srgb_targets_convert_colour_but_not_alpha<H: Host>() {
    let mut b = H::make();
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
    b.begin_pass(&color_pass::<H::Backend>(
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
    b.begin_pass(&color_pass::<H::Backend>(&v, LoadOp::Load));
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
pub fn generate_mips_averages_in_the_values_format<H: Host>() {
    // Columns of 0 and 200: stored, the average is 100; as sRGB, 200 is linear 0.5776, the
    // average 0.2888, which encodes to 146.31. Alpha 255 stays 255.
    for (values, expected) in [
        (Format::Rgba8UnormSrgb, [146u8, 146, 146, 255]),
        (Format::Rgba8Unorm, [100, 100, 100, 255]),
    ] {
        let mut b = H::make();
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
