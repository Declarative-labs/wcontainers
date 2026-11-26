//! The third check's cases of format usages, step rates and captures, and the cases of calls inside a pass.
//!
//! The calls inside a pass are those that would wait for, commit or nest work while a pass is open.
//! Each prints a line starting HOSTILE with what the device did and what the reference did. The capture after a draw
//! of its own buffers, which the check found losing the device, is in the shared suite, as the reference takes it.

#![cfg(target_os = "macos")]

use wcontainers_gpu::{
    BlendDesc, BufferDesc, BufferRange, BufferUsage, ColorAttachment, ColorWrites, CullMode,
    DepthStencilDesc, DeviceStatus, DispatchCall, DispatchGroups, DrawCall, DrawRange, Extent,
    FORMAT_COUNT, Features, FillMode, Format, FormatUse, FrontFace, Gpu, GpuError, IndexBinding,
    IndexFormat, LoadOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, MapIntent, Origin, PassDesc,
    Program, ProvokingVertex, QueryKind, RasterDesc, ReadSource, RenderTargetBlend, ShaderModule,
    ShaderStage, StepMode, StoreOp, TextureDesc, TextureDimension, TextureRegion, TextureUsage,
    TextureViewDesc, Topology, VertexAttribute, VertexBinding, VertexFormat, VertexLayoutDesc,
    VertexStream, ViewDimension, Viewport, WriteMode,
};
use wcontainers_gpu_metal::MetalGpu;
use wcontainers_gpu_test::Recorder;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;

/// The side of every texture the cases make.
const CASE_TEXTURE_SIDE: u32 = 8;
/// The byte a target is filled with before a case, which only what the case may change changes.
const CANARY: u8 = u8::MAX;

fn device() -> MetalGpu<DirectMetal> {
    MetalGpu::new(DirectMetal::new(()))
}

fn reference() -> Recorder {
    Recorder::with_caps(|caps| {
        caps.features |= Features::TRANSFORM_FEEDBACK
            | Features::INSTANCE_STEP_RATE
            | Features::BASE_VERTEX_INSTANCE
    })
}

fn why(r: &Recorder) -> String {
    r.violations.join(" | ")
}

fn ok<B: Gpu>(b: &mut B) -> bool {
    b.status() == wcontainers_gpu::DeviceStatus::Ok
}

fn buffer<B: Gpu>(b: &mut B, size: u64, usage: BufferUsage) -> B::Buffer {
    b.create_buffer(&BufferDesc {
        size,
        usage: usage | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        intent: MapIntent::Rarely,
    })
    .expect("create_buffer")
}

fn read_bytes<B: Gpu>(b: &mut B, buffer: &B::Buffer, size: u64) -> Vec<u8> {
    let readback = b
        .read_back(&ReadSource::Buffer {
            buffer,
            offset: 0,
            size,
        })
        .expect("read_back");
    b.submit();
    let mut bytes = vec![0; size as usize];
    b.finish_read(readback, &mut bytes);
    bytes
}

fn tex_desc(format: Format, usage: TextureUsage, samples: u32, reinterpret: bool) -> TextureDesc {
    TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: CASE_TEXTURE_SIDE,
        height: CASE_TEXTURE_SIDE,
        depth_or_layers: 1,
        mip_levels: 1,
        samples,
        usage,
        cube: false,
        reinterpret,
    }
}

fn view_desc(format: Format) -> TextureViewDesc {
    TextureViewDesc {
        format,
        dimension: ViewDimension::D2,
        base_mip: 0,
        mip_count: 1,
        base_layer: 0,
        layer_count: 1,
        stencil: false,
    }
}

fn region<B: Gpu>(texture: &B::Texture) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin { x: 0, y: 0, z: 0 },
        stencil: false,
    }
}

fn extent() -> Extent {
    Extent {
        width: CASE_TEXTURE_SIDE,
        height: CASE_TEXTURE_SIDE,
        depth_or_layers: 1,
    }
}

fn read_texture<B: Gpu>(b: &mut B, texture: &B::Texture, texel: usize) -> Vec<u8> {
    let mut out = vec![0; CASE_TEXTURE_SIDE as usize * CASE_TEXTURE_SIDE as usize * texel];
    b.submit();
    b.read_back_now(
        &ReadSource::Texture {
            region: region(texture),
            extent: extent(),
        },
        &mut out,
    )
    .expect("read_back_now");
    out
}

fn color_pass<'a, B: Gpu>(target: &'a B::TextureView) -> PassDesc<'a, B> {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: target,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear([0; 4]),
        store: StoreOp::Store,
    });
    pass
}

fn rgba8_target<B: Gpu>(b: &mut B) -> (B::Texture, B::TextureView) {
    let t = b
        .create_texture(&tex_desc(
            Format::Rgba8Unorm,
            TextureUsage::RENDER | TextureUsage::SAMPLE | TextureUsage::COPY_SRC,
            1,
            false,
        ))
        .expect("create_texture");
    let v = b
        .create_view(&t, &view_desc(Format::Rgba8Unorm))
        .expect("create_view");
    (t, v)
}

struct Fixed<B: Gpu> {
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
}

fn fixed<B: Gpu>(b: &mut B) -> Fixed<B> {
    Fixed {
        blend: b
            .create_blend(&BlendDesc {
                targets: [RenderTargetBlend {
                    blend: None,
                    writes: ColorWrites::all(),
                }; MAX_COLOR_TARGETS],
                same_for_all: true,
                alpha_to_coverage: false,
                logic_op: None,
            })
            .expect("create_blend"),
        depth_stencil: b
            .create_depth_stencil(&DepthStencilDesc {
                depth_compare: None,
                depth_write: false,
                stencil: None,
                stencil_read_mask: [u8::MAX; 2],
                stencil_write_mask: [u8::MAX; 2],
                depth_bounds: None,
            })
            .expect("create_depth_stencil"),
        raster: b
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
                provoking: ProvokingVertex::First,
                depth_minus_one: false,
                sample_mask: !0,
            })
            .expect("create_raster"),
    }
}

fn viewport() -> [Viewport; 1] {
    [Viewport {
        x: 0,
        y: 0,
        width: (CASE_TEXTURE_SIDE as f32).to_bits(),
        height: (CASE_TEXTURE_SIDE as f32).to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }]
}

fn program<B: Gpu>(b: &mut B, source: &str) -> B::Program {
    let module = |entry, stage| ShaderModule::from_wgsl(source, entry, stage).expect("parses");
    let vertex = b
        .create_shader(&module("vs", ShaderStage::Vertex))
        .expect("create_shader");
    let fragment = b
        .create_shader(&module("fs", ShaderStage::Fragment))
        .expect("create_shader");
    b.create_program(&Program::Render {
        vertex: &vertex,
        fragment: Some(&fragment),
    })
    .expect("create_program")
}

fn outcome<T>(r: &Result<T, GpuError>) -> String {
    match r {
        Ok(_) => "taken".into(),
        Err(GpuError::Invalid(why)) => format!("refused ({why})"),
        Err(e) => format!("refused ({e:?})"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Feature 1: texture usages a format lacks.

/// Checks that a texture is made exactly where the device's format caps take its format, usage and sample count.
///
/// H1a: every format, every usage alone and with others, one and four samples.
#[test]
fn h1a_every_format_and_usage_is_made_only_where_the_caps_allow() {
    let mut b = device();
    let caps = b.caps().formats.clone();
    let usages = [
        TextureUsage::RENDER,
        TextureUsage::SAMPLE,
        TextureUsage::STORAGE,
        TextureUsage::RENDER | TextureUsage::SAMPLE,
        TextureUsage::STORAGE | TextureUsage::SAMPLE,
    ];
    let (mut made_against_caps, mut tried, mut made) = (0, 0, 0);
    use Format as F;
    let all = [
        F::R8Unorm,
        F::R8Snorm,
        F::R8Uint,
        F::R8Sint,
        F::R16Unorm,
        F::R16Snorm,
        F::R16Uint,
        F::R16Sint,
        F::R16Float,
        F::Rg8Unorm,
        F::Rg8Snorm,
        F::Rg8Uint,
        F::Rg8Sint,
        F::R32Uint,
        F::R32Sint,
        F::R32Float,
        F::Rg16Unorm,
        F::Rg16Snorm,
        F::Rg16Uint,
        F::Rg16Sint,
        F::Rg16Float,
        F::Rgba8Unorm,
        F::Rgba8UnormSrgb,
        F::Rgba8Snorm,
        F::Rgba8Uint,
        F::Rgba8Sint,
        F::Bgra8Unorm,
        F::Bgra8UnormSrgb,
        F::Bgrx8Unorm,
        F::Bgrx8UnormSrgb,
        F::Rgb10a2Unorm,
        F::Rgb10a2Uint,
        F::Rg11b10Float,
        F::Rgb9e5Float,
        F::B5g6r5Unorm,
        F::Bgr5a1Unorm,
        F::Bgra4Unorm,
        F::Rgbg8Unorm,
        F::Grgb8Unorm,
        F::Rg32Uint,
        F::Rg32Sint,
        F::Rg32Float,
        F::Rgba16Unorm,
        F::Rgba16Snorm,
        F::Rgba16Uint,
        F::Rgba16Sint,
        F::Rgba16Float,
        F::Rgb32Uint,
        F::Rgb32Sint,
        F::Rgb32Float,
        F::Rgba32Uint,
        F::Rgba32Sint,
        F::Rgba32Float,
        F::A8Unorm,
        F::Stencil8,
        F::Depth16Unorm,
        F::Depth24Plus,
        F::Depth24PlusStencil8,
        F::Depth32Float,
        F::Depth32FloatStencil8,
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
        F::Etc2Rgb8Unorm,
        F::Etc2Rgb8UnormSrgb,
        F::Etc2Rgba8Unorm,
        F::Etc2Rgba8UnormSrgb,
        F::Astc4x4Unorm,
        F::Astc4x4UnormSrgb,
    ];
    assert_eq!(all.len(), FORMAT_COUNT);
    for format in all {
        let uses = caps.uses[format as usize];
        for usage in usages {
            for samples in [1u32, 4] {
                tried += 1;
                let allowed = !uses.is_empty()
                    && (!usage.contains(TextureUsage::RENDER)
                        || uses.intersects(FormatUse::RENDER | FormatUse::DEPTH_STENCIL))
                    && (!usage.contains(TextureUsage::SAMPLE) || uses.contains(FormatUse::SAMPLE))
                    && (!usage.contains(TextureUsage::STORAGE)
                        || uses.intersects(FormatUse::STORAGE_READ | FormatUse::STORAGE_WRITE))
                    && caps.takes_samples(format, samples);
                let got = b.create_texture(&tex_desc(
                    format,
                    usage | TextureUsage::COPY_DST,
                    samples,
                    false,
                ));
                if got.is_ok() {
                    made += 1;
                    if !allowed {
                        made_against_caps += 1;
                        println!("HOSTILE h1a made against caps: {format:?} {usage:?} x{samples}");
                    }
                }
                if !ok(&mut b) {
                    println!("HOSTILE h1a device lost at {format:?} {usage:?} x{samples}");
                    b = device();
                }
            }
        }
    }
    println!("HOSTILE h1a tried {tried}, made {made}, made against caps {made_against_caps}");
    assert_eq!(made_against_caps, 0);
    assert!(ok(&mut b));
}

/// Checks views of a drawn texture in formats whose caps lack RENDER, and passes into them, against the reference.
///
/// H1b: a drawn `Rgba8Unorm` texture made for views in other formats, seen as each 32-bit format whose caps lack
/// RENDER, then drawn into.
#[test]
fn h1b_a_view_in_a_format_that_is_not_drawn_into_is_refused_or_not_drawn() {
    for format in [Format::Rgb9e5Float, Format::Rg16Uint, Format::R32Float] {
        let run = |name: &str, b: &mut dyn FnMut() -> (String, String, bool)| {
            let (view, pass, alive) = b();
            println!(
                "HOSTILE h1b {name} {format:?}: view {view}, pass {pass}, device ok after {alive}"
            );
            (view, alive)
        };
        let mut mb = device();
        let (dv, dalive) = run("device", &mut || scene_h1b(&mut mb, format));
        let mut r = reference();
        let (rv, ralive) = run("reference", &mut || scene_h1b(&mut r, format));
        let agree = (dv.starts_with("taken"), dalive) == (rv.starts_with("taken"), ralive);
        println!(
            "HOSTILE h1b {format:?}: agree {agree}; reference says: {}",
            why(&r)
        );
        assert!(dalive, "{format:?}: the device was lost");
        // The reference makes a view only in a format it holds; where it holds none, the device's own caps decide.
        if !r.caps().formats.uses[format as usize].is_empty() {
            assert!(agree, "{format:?}: the device and the reference disagree");
        }
    }
}

fn scene_h1b<B: Gpu>(b: &mut B, format: Format) -> (String, String, bool) {
    let caps_render = b.caps().formats.uses[format as usize].contains(FormatUse::RENDER);
    let t = b
        .create_texture(&tex_desc(
            Format::Rgba8Unorm,
            TextureUsage::RENDER | TextureUsage::SAMPLE | TextureUsage::COPY_SRC,
            1,
            true,
        ))
        .expect("create_texture");
    let v = b.create_view(&t, &view_desc(format));
    let view = format!("{} (caps RENDER {caps_render})", outcome(&v));
    let Ok(v) = v else {
        return (view, "not tried".into(), ok(b));
    };
    b.begin_pass(&color_pass::<B>(&v));
    b.end_pass();
    b.submit();
    (view, "begun and ended".into(), ok(b))
}

/// Checks that `Bgra4Unorm` texels written from the host read back unchanged, and copies them to another format.
///
/// H1c: the bit order is the interface's both ways; the copy is into an `Rg8Unorm` texture of the same block size.
#[test]
fn h1c_bgra4_texels_written_and_read_back_keep_the_interfaces_bit_order() {
    let pattern: Vec<u8> = (0..CASE_TEXTURE_SIDE * CASE_TEXTURE_SIDE * 2)
        .map(|n| (n * 37 + 11) as u8)
        .collect();
    let mut b = device();
    let got = scene_h1c(&mut b, &pattern);
    let r = reference();
    let held = !r.caps().formats.uses[Format::Bgra4Unorm as usize].is_empty();
    println!(
        "HOSTILE h1c device: read back equal to written {}, copy {}; reference holds Bgra4Unorm {held}",
        got.0 == pattern,
        got.1,
    );
    assert_eq!(got.0, pattern, "read back");
}

fn scene_h1c<B: Gpu>(b: &mut B, pattern: &[u8]) -> (Vec<u8>, String) {
    let t = b
        .create_texture(&tex_desc(
            Format::Bgra4Unorm,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC,
            1,
            false,
        ))
        .expect("create_texture");
    b.write_texture(
        &region(&t),
        extent(),
        pattern,
        CASE_TEXTURE_SIDE * 2,
        CASE_TEXTURE_SIDE,
        WriteMode::Ordered,
    );
    let back = read_texture(b, &t, 2);
    let other = b
        .create_texture(&tex_desc(
            Format::Rg8Unorm,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC,
            1,
            false,
        ))
        .expect("create_texture");
    b.copy_texture(&region(&t), &region(&other), extent());
    b.submit();
    let copy = if ok(b) {
        "kept the device"
    } else {
        "lost the device"
    };
    (back, copy.into())
}

// ---------------------------------------------------------------------------------------------------------------
// Feature 2: instance step rates.

const STEPPED: &str = "
struct Out { @builtin(position) pos: vec4<f32>, @location(0) @interpolate(flat) c: vec4<f32> }
@vertex fn vs(@location(0) p: vec2<f32>, @location(1) c: vec4<f32>, @builtin(instance_index) i: u32) -> Out {
    let column = f32(i % 8u);
    let x = (p.x + 1.0 + 2.0 * column) / 8.0 - 1.0;
    return Out(vec4<f32>(x, p.y, 0.5, 1.0), c);
}
@fragment fn fs(i: Out) -> @location(0) vec4<f32> { return i.c; }
";

const STRIP: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];

/// Draws eight columns colored from a per-instance stream, and returns whether the device stayed and the pixels.
///
/// Each column is an instance from `first_instance`; the stream is of `format` at `rate`, bound at `offset` of a buffer
/// of `stream` bytes.
#[allow(clippy::too_many_arguments)]
fn stepped_scene<B: Gpu>(
    b: &mut B,
    format: VertexFormat,
    stride: u32,
    rate: u32,
    first_instance: u32,
    instances: u32,
    stream: &[u8],
    offset: u64,
) -> (bool, Vec<u8>) {
    let (t, v) = rgba8_target(b);
    let quad = buffer(b, 32, BufferUsage::VERTEX);
    let q: Vec<u8> = STRIP.iter().flat_map(|f| f.to_le_bytes()).collect();
    b.write_buffer(&quad, 0, &q, WriteMode::Ordered);
    let colors = buffer(b, stream.len() as u64, BufferUsage::VERTEX);
    b.write_buffer(&colors, 0, stream, WriteMode::Ordered);
    let p = program(b, STEPPED);
    let f = fixed(b);
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
    desc.streams[1] = Some(VertexStream {
        stride,
        step: StepMode::Instance(rate),
    });
    desc.attributes[1] = Some(VertexAttribute {
        location: 1,
        stream: 1,
        offset: 0,
        format,
    });
    let layout = b.create_vertex_layout(&desc).expect("create_vertex_layout");
    let streams = [
        Some(VertexBinding {
            buffer: &quad,
            offset: 0,
        }),
        Some(VertexBinding {
            buffer: &colors,
            offset,
        }),
    ];
    let vp = viewport();
    b.begin_pass(&color_pass::<B>(&v));
    b.draw(&DrawCall {
        program: &p,
        blend: &f.blend,
        depth_stencil: &f.depth_stencil,
        raster: &f.raster,
        vertex_layout: &layout,
        bindings: [None; 4],
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
            first: 0,
            count: 4,
            first_instance,
            instances,
        },
    });
    b.end_pass();
    b.submit();
    let alive = ok(b);
    let pixels = if alive {
        read_texture(b, &t, 4)
    } else {
        Vec::new()
    };
    (alive, pixels)
}

fn float_colors(n: u32) -> Vec<u8> {
    (0..n)
        .flat_map(|k| {
            [k as f32 / 16.0, 1.0 - k as f32 / 16.0, 0.25, 1.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
        })
        .collect()
}

/// Returns whether two images agree within one step per channel.
fn close(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 1)
}

/// Checks per-instance streams read from a first instance near the largest `u32` against the reference.
///
/// S1: rates 0, 1, 2 and `u32::MAX`, so that every element read lies past a one-element stream; Metal's shader
/// validation reports a read outside.
#[test]
fn s1_rates_from_a_first_instance_near_the_largest_u32_read_nothing_outside() {
    let stream = float_colors(1);
    for rate in [0, 1, 2, u32::MAX] {
        for first in [u32::MAX - 8, u32::MAX - 16] {
            let mut b = device();
            let got = stepped_scene(
                &mut b,
                VertexFormat::Float32x4,
                16,
                rate,
                first,
                8,
                &stream,
                0,
            );
            let mut r = reference();
            let wanted = stepped_scene(
                &mut r,
                VertexFormat::Float32x4,
                16,
                rate,
                first,
                8,
                &stream,
                0,
            );
            println!(
                "HOSTILE s1 rate {rate} first {first}: device drew {} ok {}, black {}, reference ok {} ({})",
                got.0 && !got.1.is_empty(),
                got.0,
                got.1.iter().all(|v| *v == 0),
                wanted.0,
                why(&r)
            );
            assert!(got.0, "rate {rate} first {first}: device lost");
            if wanted.0 {
                // Rate 0 reads element `first`, past the stream: both read zeros. The others likewise.
                assert!(close(&got.1, &wanted.1), "rate {rate} first {first}");
            }
        }
    }
}

/// Checks that a stream widened on the host at a step rate places each element where the rate's index reads it.
///
/// S2: a stream of a format naga lacks (`Unorm8x3`) at rates 2, 3 and 0 from first instance 5.
#[test]
fn s2_a_widened_stream_at_a_rate_from_a_late_first_instance_matches_the_reference() {
    // Twelve elements of three bytes and one of padding.
    let stream: Vec<u8> = (0..12u8)
        .flat_map(|k| [k * 20, 255 - k * 20, 128, 0])
        .collect();
    for rate in [0, 2, 3, 7] {
        let mut b = device();
        let got = stepped_scene(&mut b, VertexFormat::Unorm8x3, 4, rate, 5, 8, &stream, 0);
        let mut r = reference();
        let wanted = stepped_scene(&mut r, VertexFormat::Unorm8x3, 4, rate, 5, 8, &stream, 0);
        let same = close(&got.1, &wanted.1);
        println!(
            "HOSTILE s2 rate {rate}: device ok {}, reference ok {}, same pixels {same}",
            got.0, wanted.0
        );
        if !same {
            let row = |p: &[u8]| {
                p.chunks(4)
                    .take(8)
                    .map(|c| format!("{:?}", &c[..3]))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            println!("HOSTILE s2   device    {}", row(&got.1));
            println!("HOSTILE s2   reference {}", row(&wanted.1));
        }
        assert!(got.0 && same, "rate {rate}");
    }
}

/// Checks a stepped stream bound near its buffer's end against the reference.
///
/// S3: a per-instance stream at rate 3 bound at offsets that leave less than one element (off its stride), none, or
/// exactly the elements read.
#[test]
fn s3_a_stepped_stream_bound_near_its_end_reads_zeros_past_it() {
    let stream = float_colors(3);
    let len = stream.len() as u64;
    for offset in [len - 3, len, len - 16, 0, 4] {
        let mut b = device();
        let got = stepped_scene(
            &mut b,
            VertexFormat::Float32x4,
            16,
            3,
            0,
            8,
            &stream,
            offset,
        );
        let mut r = reference();
        let wanted = stepped_scene(
            &mut r,
            VertexFormat::Float32x4,
            16,
            3,
            0,
            8,
            &stream,
            offset,
        );
        let same = close(&got.1, &wanted.1);
        println!(
            "HOSTILE s3 offset {offset} of {len}: device ok {}, reference ok {} ({}), same pixels {same}",
            got.0,
            wanted.0,
            why(&r)
        );
        assert!(got.0, "offset {offset}: device lost");
        if got.0 && wanted.0 {
            assert!(same, "offset {offset}");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Feature 3: captures.

const CAPTURED: &str = "
struct V {
    @builtin(position) p: vec4<f32>,
    @location(0) @interpolate(flat) n: vec2<u32>,
    @location(1) x: f32,
    @location(2) y: f32,
}
@vertex fn vs(@location(0) p: vec2<f32>, @builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> V {
    return V(vec4<f32>(p, 0.5, 1.0), vec2<u32>(v, i), p.x, p.y);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 1.0, 0.0, 1.0); }
";

struct CaptureScene<B: Gpu> {
    program: B::Program,
    fixed: Fixed<B>,
    layout: B::VertexLayout,
    stream: B::Buffer,
    index: B::Buffer,
}

fn capture_scene<B: Gpu>(b: &mut B) -> CaptureScene<B> {
    let program = program(b, CAPTURED);
    let fixed = fixed(b);
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
    let layout = b.create_vertex_layout(&desc).expect("create_vertex_layout");
    let stream = buffer(b, 48, BufferUsage::VERTEX);
    let q: Vec<u8> = STRIP
        .iter()
        .chain(&[0.5, 0.25, -0.5, 0.75])
        .flat_map(|f| f.to_le_bytes())
        .collect();
    b.write_buffer(&stream, 0, &q, WriteMode::Ordered);
    let index = buffer(b, 16, BufferUsage::INDEX);
    let i: Vec<u8> = [0u16, 1, 2, 2, 1, 3, 0, 0]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    b.write_buffer(&index, 0, &i, WriteMode::Ordered);
    CaptureScene {
        program,
        fixed,
        layout,
        stream,
        index,
    }
}

fn capture_call<'a, B: Gpu>(
    s: &'a CaptureScene<B>,
    streams: &'a [Option<VertexBinding<'a, B>>],
    vp: &'a [Viewport],
    indexed: bool,
    count: u32,
) -> DrawCall<'a, B> {
    DrawCall {
        program: &s.program,
        blend: &s.fixed.blend,
        depth_stencil: &s.fixed.depth_stencil,
        raster: &s.fixed.raster,
        vertex_layout: &s.layout,
        bindings: [None; 4],
        vertex_buffers: streams,
        index: indexed.then_some(IndexBinding {
            buffer: &s.index,
            offset: 0,
            format: IndexFormat::U16,
        }),
        topology: Topology::TriangleList,
        restart: false,
        viewports: vp,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: if indexed {
            DrawRange::Indexed {
                first: 0,
                count,
                base_vertex: 0,
                first_instance: 0,
                instances: 2,
            }
        } else {
            DrawRange::Vertices {
                first: 0,
                count,
                first_instance: 0,
                instances: 2,
            }
        },
    }
}

const TARGET_BYTES: u64 = 1024;

/// Checks that a capture of `u32::MAX` indices from a small index buffer is refused before the indices are held.
///
/// C2: 32-bit indices from a 16-byte index buffer into a small target, whose 16 GiB would be held first.
#[test]
fn c2_an_indexed_capture_of_u32_max_indices_from_a_small_buffer_is_refused_at_once() {
    let mut b = device();
    let s = capture_scene(&mut b);
    let target = buffer(&mut b, TARGET_BYTES, BufferUsage::CAPTURE);
    let streams = [Some(VertexBinding {
        buffer: &s.stream,
        offset: 0,
    })];
    let vp = viewport();
    for (format, count) in [
        (IndexFormat::U32, u32::MAX),
        (IndexFormat::U16, u32::MAX),
        (IndexFormat::U32, 1 << 30),
    ] {
        let mut call = capture_call(&s, &streams, &vp, true, count);
        call.index = Some(IndexBinding {
            buffer: &s.index,
            offset: 0,
            format,
        });
        let clock = std::time::Instant::now();
        let r = b.capture_vertices(
            &call,
            &[BufferRange {
                buffer: &target,
                offset: 0,
                size: TARGET_BYTES,
            }],
            false,
        );
        let took = clock.elapsed();
        println!(
            "HOSTILE c2 {format:?} x{count}: {} in {took:?}, device ok {}",
            outcome(&r),
            ok(&mut b)
        );
        assert!(r.is_err());
        assert!(
            took < std::time::Duration::from_secs(1),
            "{format:?} x{count}: {took:?}"
        );
    }
    assert!(ok(&mut b));
}

/// Checks aliased, overlapping, ragged and unmarked capture targets against the reference.
///
/// C3: separate targets that are one range of one buffer, targets that overlap by a word, a target whose size is not
/// whole words, and a target in a buffer made without CAPTURE; nothing outside the ranges may change.
#[test]
fn c3_aliased_overlapping_ragged_and_unmarked_targets_agree_with_the_reference() {
    for case in 0..4 {
        let mut b = device();
        let got = scene_c3(&mut b, case);
        let mut r = reference();
        let wanted = scene_c3(&mut r, case);
        println!(
            "HOSTILE c3 case {case} ({}): device {} ok {} outside-changed {}; reference {} ok {}; same bytes {}",
            got.0,
            got.1,
            got.2,
            got.4,
            wanted.1,
            wanted.2,
            got.3 == wanted.3
        );
        println!("HOSTILE c3 case {case} reference says: {}", why(&r));
        assert!(got.2, "case {case}: device lost");
        // A target of a buffer made without CAPTURE is refused, as the reference refuses it.
        assert_eq!(
            got.1 == "taken",
            wanted.1 == "taken",
            "case {case}: taken by one of the two"
        );
        assert_eq!(got.4, 0, "case {case}: bytes outside the ranges changed");
        if case >= 2 && got.1 == wanted.1 && got.1 == "taken" {
            assert_eq!(got.3, wanted.3, "case {case}: bytes");
        }
    }
}

fn scene_c3<B: Gpu>(b: &mut B, case: u32) -> (&'static str, String, bool, Vec<u8>, usize) {
    let s = capture_scene(b);
    let usage = if case == 3 {
        BufferUsage::UNIFORM
    } else {
        BufferUsage::CAPTURE
    };
    let target = buffer(b, TARGET_BYTES, usage);
    b.write_buffer(
        &target,
        0,
        &vec![CANARY; TARGET_BYTES as usize],
        WriteMode::Ordered,
    );
    let streams = [Some(VertexBinding {
        buffer: &s.stream,
        offset: 0,
    })];
    let vp = viewport();
    // Six vertices, two instances: twelve vertices of four words packed; per output 2, 1, 1 words.
    let call = capture_call(&s, &streams, &vp, false, 6);
    let at = |offset, size| BufferRange {
        buffer: &target,
        offset,
        size,
    };
    let (name, ranges, lo, hi): (&str, Vec<_>, u64, u64) = match case {
        0 => (
            "three targets one range",
            vec![at(256, 96), at(256, 96), at(256, 96)],
            256,
            352,
        ),
        1 => (
            "overlap by a word",
            vec![at(256, 96), at(348, 48), at(392, 48)],
            256,
            440,
        ),
        2 => (
            "ragged size",
            vec![at(256, 12 * 16 + 3)],
            256,
            256 + 12 * 16 + 3,
        ),
        _ => (
            "no CAPTURE usage",
            vec![at(256, 12 * 16)],
            256,
            256 + 12 * 16,
        ),
    };
    let r = b.capture_vertices(&call, &ranges, false);
    b.submit();
    let alive = ok(b);
    let bytes = if alive {
        read_bytes(b, &target, TARGET_BYTES)
    } else {
        Vec::new()
    };
    let outside = bytes
        .iter()
        .enumerate()
        .filter(|(n, v)| ((*n as u64) < lo || (*n as u64) >= hi) && **v != CANARY)
        .count();
    (name, outcome(&r), alive, bytes, outside)
}

// ---------------------------------------------------------------------------------------------------------------
// The places that would wait for, commit or nest work while a pass is open. The reference counts each call here a
// broken rule (it is outside a pass in the interface, or submits inside one); the device must keep every draw of the
// pass and its device, or refuse the call.

const GREEN: &str = "
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 1.0, 0.0, 1.0); }
";

/// The left half of the target as a strip from vertex 0, the right half from vertex 4.
const HALVES: [f32; 16] = [
    -1.0, -1.0, -1.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0,
];

/// A target and what draws each half of it green.
struct Halves<B: Gpu> {
    target: B::Texture,
    view: B::TextureView,
    program: B::Program,
    fixed: Fixed<B>,
    layout: B::VertexLayout,
    positions: B::Buffer,
}

fn halves<B: Gpu>(b: &mut B) -> Halves<B> {
    let (target, view) = rgba8_target(b);
    let program = program(b, GREEN);
    let fixed = fixed(b);
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
    let layout = b.create_vertex_layout(&desc).expect("create_vertex_layout");
    let positions = buffer(b, 64, BufferUsage::VERTEX);
    let bytes: Vec<u8> = HALVES.iter().flat_map(|f| f.to_le_bytes()).collect();
    b.write_buffer(&positions, 0, &bytes, WriteMode::Ordered);
    Halves {
        target,
        view,
        program,
        fixed,
        layout,
        positions,
    }
}

/// Draws half `half` (0 left, 1 right) of the target green.
fn draw_half<B: Gpu>(b: &mut B, h: &Halves<B>, half: u32) {
    let streams = [Some(VertexBinding {
        buffer: &h.positions,
        offset: 0,
    })];
    let vp = viewport();
    b.draw(&DrawCall {
        program: &h.program,
        blend: &h.fixed.blend,
        depth_stencil: &h.fixed.depth_stencil,
        raster: &h.fixed.raster,
        vertex_layout: &h.layout,
        bindings: [None; 4],
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
            first: half * 4,
            count: 4,
            first_instance: 0,
            instances: 1,
        },
    });
}

/// Returns whether the device stayed and every texel is green after `call` ran between the draws of one pass.
///
/// The pass clears the target and draws its left half; `call` runs; the right half is drawn and the pass ends.
fn around_a_call<B: Gpu>(b: &mut B, h: &Halves<B>, call: impl FnOnce(&mut B)) -> (bool, bool) {
    b.begin_pass(&color_pass::<B>(&h.view));
    draw_half(b, h, 0);
    call(b);
    draw_half(b, h, 1);
    b.end_pass();
    b.submit();
    let alive = ok(b);
    let green = alive
        && read_texture(b, &h.target, 4)
            == [0, 255, 0, 255].repeat((CASE_TEXTURE_SIDE * CASE_TEXTURE_SIDE) as usize);
    (alive, green)
}

/// Prints and checks one place's outcome on the device, where the pass goes on around the call.
fn kept(name: &str, (alive, green): (bool, bool), reference: &Recorder) {
    println!(
        "HOSTILE {name}: device ok {alive}, every texel drawn {green}; reference says: {}",
        why(reference)
    );
    assert!(alive, "{name}: the device was lost");
    assert!(green, "{name}: a draw of the pass was lost");
}

/// Bytes a buffer is written with before the pass, which a read inside it must find.
fn written<B: Gpu>(b: &mut B) -> (B::Buffer, Vec<u8>) {
    let bytes: Vec<u8> = (0..64u8)
        .map(|n| n.wrapping_mul(29).wrapping_add(7))
        .collect();
    let source = buffer(b, 64, BufferUsage::empty());
    b.write_buffer(&source, 0, &bytes, WriteMode::Ordered);
    (source, bytes)
}

/// Checks that a submit inside a pass commits the draws so far, ending the pass and opening it again after.
#[test]
fn p1_a_submit_inside_a_pass_keeps_the_pass_and_its_draws() {
    let mut b = device();
    let h = halves(&mut b);
    let got = around_a_call(&mut b, &h, |b| {
        b.submit();
    });
    let mut r = reference();
    let hr = halves(&mut r);
    around_a_call(&mut r, &hr, |r| {
        r.submit();
    });
    kept("p1 submit", got, &r);
}

/// Checks that a wait inside a pass for a read back not yet submitted commits it there and keeps the pass.
#[test]
fn p2_a_wait_for_a_read_back_inside_a_pass_keeps_the_pass_and_reads_the_bytes() {
    let mut b = device();
    let h = halves(&mut b);
    let (source, bytes) = written(&mut b);
    let readback = b
        .read_back(&ReadSource::Buffer {
            buffer: &source,
            offset: 0,
            size: 64,
        })
        .expect("read_back");
    let got = around_a_call(&mut b, &h, |b| b.wait_read(&readback));
    let mut out = vec![0; 64];
    b.finish_read(readback, &mut out);
    let r = reference();
    kept("p2 wait_read", got, &r);
    assert_eq!(out, bytes, "p2: the bytes read back");
}

/// Checks that finishing a read back not yet submitted inside a pass commits it there and keeps the pass.
#[test]
fn p3_a_finished_read_back_inside_a_pass_keeps_the_pass_and_reads_the_bytes() {
    let mut b = device();
    let h = halves(&mut b);
    let (source, bytes) = written(&mut b);
    let readback = b
        .read_back(&ReadSource::Buffer {
            buffer: &source,
            offset: 0,
            size: 64,
        })
        .expect("read_back");
    let mut out = vec![0; 64];
    let mut readback = Some(readback);
    let got = around_a_call(&mut b, &h, |b| {
        if let Some(readback) = readback.take() {
            b.finish_read(readback, &mut out);
        }
    });
    let r = reference();
    kept("p3 finish_read", got, &r);
    assert_eq!(out, bytes, "p3: the bytes read back");
}

/// Checks that reads back at once inside a pass wait for the work before, committing it there.
///
/// One reads a buffer a copy recorded before the pass writes, the other a texture written before it.
#[test]
fn p4_a_read_back_now_inside_a_pass_keeps_the_pass_and_reads_the_bytes() {
    let mut b = device();
    let h = halves(&mut b);
    let (source, bytes) = written(&mut b);
    let copy = buffer(&mut b, 64, BufferUsage::empty());
    b.copy_buffer(&source, 0, &copy, 0, 64);
    let texels: Vec<u8> = (0..CASE_TEXTURE_SIDE * CASE_TEXTURE_SIDE * 4)
        .map(|n| (n * 13 + 1) as u8)
        .collect();
    let texture = b
        .create_texture(&tex_desc(
            Format::Rgba8Unorm,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC,
            1,
            false,
        ))
        .expect("create_texture");
    b.write_texture(
        &region(&texture),
        extent(),
        &texels,
        CASE_TEXTURE_SIDE * 4,
        CASE_TEXTURE_SIDE,
        WriteMode::Ordered,
    );
    let (mut from_buffer, mut from_texture) = (vec![0; 64], vec![0; texels.len()]);
    let mut outcomes = Vec::new();
    let got = around_a_call(&mut b, &h, |b| {
        outcomes.push(outcome(&b.read_back_now(
            &ReadSource::Buffer {
                buffer: &copy,
                offset: 0,
                size: 64,
            },
            &mut from_buffer,
        )));
        outcomes.push(outcome(&b.read_back_now(
            &ReadSource::Texture {
                region: region(&texture),
                extent: extent(),
            },
            &mut from_texture,
        )));
    });
    let r = reference();
    println!("HOSTILE p4 reads: {outcomes:?}");
    kept("p4 read_back_now", got, &r);
    assert_eq!(from_buffer, bytes, "p4: the buffer's bytes");
    assert_eq!(from_texture, texels, "p4: the texture's texels");
}

/// Checks that timestamps resolved inside a pass wait for the work that writes them, committing it there.
#[test]
fn p5_timestamps_resolved_inside_a_pass_keep_the_pass() {
    let mut b = device();
    if !b.caps().features.contains(Features::TIMESTAMPS) {
        println!("HOSTILE p5: no timestamps on this device");
        return;
    }
    let h = halves(&mut b);
    let set = b
        .create_query_set(QueryKind::Timestamp, 1)
        .expect("create_query_set");
    b.write_timestamp(&set, 0).expect("write_timestamp");
    let results = buffer(&mut b, 8, BufferUsage::empty());
    let mut resolved = String::new();
    let got = around_a_call(&mut b, &h, |b| {
        resolved = outcome(&b.resolve_queries(&set, 0, 1, &results, 0));
    });
    let r = reference();
    println!("HOSTILE p5 resolve_queries: {resolved}");
    kept("p5 resolve_queries", got, &r);
    assert_eq!(resolved, "taken");
    assert_ne!(read_bytes(&mut b, &results, 8), [0; 8], "p5: the timestamp");
}

/// Checks that a dispatch inside a pass, which Metal would run before the pass's draws, is refused.
#[test]
fn p6_a_dispatch_inside_a_pass_is_refused_and_the_pass_goes_on() {
    let mut b = device();
    let h = halves(&mut b);
    let module = ShaderModule::from_wgsl(
        "@compute @workgroup_size(1) fn cs() {}",
        "cs",
        ShaderStage::Compute,
    )
    .expect("parses");
    let shader = b.create_shader(&module).expect("create_shader");
    let compute = b
        .create_program(&Program::Compute(&shader))
        .expect("create_program");
    let call = DispatchCall {
        program: &compute,
        bindings: [None; 4],
        immediates: &[],
        groups: DispatchGroups::Direct([1, 1, 1]),
    };
    let mut inside = String::new();
    let got = around_a_call(&mut b, &h, |b| inside = outcome(&b.dispatch(&call)));
    let outside = outcome(&b.dispatch(&call));
    let r = reference();
    println!("HOSTILE p6 dispatch inside: {inside}, outside: {outside}");
    kept("p6 dispatch", got, &r);
    assert!(inside.starts_with("refused"), "p6: {inside}");
    assert_eq!(outside, "taken");
}

/// Checks that mipmaps generated inside a pass, which may draw each level in a pass of its own, are refused.
#[test]
fn p7_mipmaps_generated_inside_a_pass_are_refused_and_the_pass_goes_on() {
    let mut b = device();
    let h = halves(&mut b);
    let desc = TextureDesc {
        mip_levels: 2,
        ..tex_desc(
            Format::Rgba8Unorm,
            TextureUsage::RENDER | TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            1,
            false,
        )
    };
    let texture = b.create_texture(&desc).expect("create_texture");
    let mut inside = String::new();
    let got = around_a_call(&mut b, &h, |b| {
        inside = outcome(&b.generate_mips(&texture, &desc, Format::Rgba8Unorm, 1..2, 0..1));
    });
    let outside = outcome(&b.generate_mips(&texture, &desc, Format::Rgba8Unorm, 1..2, 0..1));
    let r = reference();
    println!("HOSTILE p7 generate_mips inside: {inside}, outside: {outside}");
    kept("p7 generate_mips", got, &r);
    assert!(inside.starts_with("refused"), "p7: {inside}");
    assert_eq!(outside, "taken");
}

/// Checks that a resolve or a pass begun inside a pass, which would drop the open pass's draws, is refused.
///
/// Both lose the device, as the reference loses it, and nothing reaches Metal that its checks stop.
#[test]
fn p8_a_resolve_or_a_pass_begun_inside_a_pass_is_refused() {
    for nested in ["resolve", "begin_pass"] {
        let run = |b: &mut MetalGpu<DirectMetal>| {
            let h = halves(b);
            let multisampled = b
                .create_texture(&tex_desc(
                    Format::Rgba8Unorm,
                    TextureUsage::RENDER,
                    4,
                    false,
                ))
                .expect("create_texture");
            b.begin_pass(&color_pass::<MetalGpu<DirectMetal>>(&h.view));
            draw_half(b, &h, 0);
            if nested == "resolve" {
                b.resolve(
                    &region(&multisampled),
                    &region(&h.target),
                    Format::Rgba8Unorm,
                );
            } else {
                b.begin_pass(&color_pass::<MetalGpu<DirectMetal>>(&h.view));
            }
            b.end_pass();
            b.submit();
            ok(b)
        };
        let mut b = device();
        let alive = run(&mut b);
        println!("HOSTILE p8 {nested} inside a pass: device ok after {alive}");
        assert_eq!(
            b.status(),
            DeviceStatus::Lost(wcontainers_gpu::LossReason::Error),
            "p8 {nested}"
        );
        // A new device works.
        let mut fresh = device();
        let h = halves(&mut fresh);
        assert_eq!(
            around_a_call(&mut fresh, &h, |_| ()),
            (true, true),
            "p8 {nested}: a new device"
        );
    }
}
