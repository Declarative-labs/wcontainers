//! Copies WebGPU has no single command for, exact: between formats of one typeless group (through a
//! buffer), and into depth (by draws), from a buffer at a pitch WebGPU refuses, from a write, and
//! from a texture of the group, into part of the destination; and texture reads into a buffer that
//! leave the bytes between rows as they were.
//!
//! Needs a GPU; `WEBGPU_API` as in unit_tests/scenes.rs.

use super::{depth16, draw, gaps};

use crate::probe::Depth24;
use wcontainers_gpu::{
    Gpu, caps::FormatUse, draw::AspectOps, draw::ColorAttachment, draw::DepthAttachment,
    draw::DrawCall, draw::DrawRange, draw::LoadOp, draw::PassDesc, draw::StoreOp, draw::Viewport,
    error::DeviceStatus, format::FORMAT_FACTS, format::Format, format::FormatKind,
    resource::BindingEntry, resource::BindingResource, resource::BufferDesc, resource::BufferUsage,
    resource::CopyBufferTexture, resource::Extent, resource::MapIntent, resource::Origin,
    resource::ReadSource, resource::TextureRegion, resource::TextureUsage, resource::WriteMode,
    shader::StageSet, slow::SlowPath, state::Topology,
};
use wcontainers_gpu_webgpu::WebGpu;

type Tex = <WebGpu as Gpu>::Texture;
type View = <WebGpu as Gpu>::TextureView;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    pollster::block_on(draw::device(backends))
        .expect("a GPU adapter")
        .0
}

use depth16::{H, W};

fn region(texture: &Tex, x: u32, y: u32) -> TextureRegion<'_, WebGpu> {
    TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin { x, y, z: 0 },
        stencil: false,
    }
}

fn extent(width: u32, height: u32) -> Extent {
    Extent {
        width,
        height,
        depth_or_layers: 1,
    }
}

fn read(b: &mut WebGpu, texture: &Tex, texel: u32) -> Vec<u8> {
    let rb = b
        .read_back(&ReadSource::Texture {
            region: region(texture, 0, 0),
            extent: extent(W, H),
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (W * H * texel) as usize];
    b.finish_read(rb, &mut out);
    out
}

/// Reads every texel of a depth texture as a float, by a draw loading it into an `r32float` target.
const DEPTH_READ_WGSL: &str = r#"
@group(1) @binding(32) var depth: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return vec4f(textureLoad(depth, vec2i(p.xy), 0).x, 0.0, 0.0, 1.0);
}
"#;

fn depth_values(b: &mut WebGpu, view: &View) -> Vec<f32> {
    let (target, target_view) = draw::texture_2d(
        b,
        Format::R32Float,
        W,
        H,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let program = draw::program(b, DEPTH_READ_WGSL).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let vertex_layout = b.create_vertex_layout(&draw::layout(&[], &[])).unwrap();
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(view),
        }])
        .unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &target_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear([0; 4]),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [None, Some(&set), None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x: 0,
            y: 0,
            width: (W as f32).to_bits(),
            height: (H as f32).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }],
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
    b.end_pass();
    read(b, &target, 4)
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Clears a depth texture's depth (and stencil, where it has one) by a pass.
fn clear_depth(b: &mut WebGpu, view: &View, format: Format, value: f32) {
    let stencil = matches!(
        format,
        Format::Depth24PlusStencil8 | Format::Depth32FloatStencil8
    );
    b.begin_pass(&PassDesc {
        colors: Default::default(),
        depth: Some(DepthAttachment {
            view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(value.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: stencil.then_some(AspectOps {
                load: LoadOp::Clear(0),
                store: StoreOp::Store,
                read_only: false,
            }),
        }),
        empty: None,
        occlusion: None,
    });
    b.end_pass();
}

/// The words a front hands over for each texel: float depth, or 24-bit unorm depth under a byte the
/// copy ignores (Direct3D's stencil byte).
fn source_words(format: Format) -> Vec<u32> {
    (0..W * H)
        .map(|i| match format {
            Format::Depth24Plus | Format::Depth24PlusStencil8 => {
                0xab00_0000 | (i.wrapping_mul(0x0012_3457) & Format::UNORM24_MAX)
            }
            _ => (i as f32 / 31.0 + if i % 3 == 0 { 1.0 / 3.0 } else { 0.0 })
                .min(1.0)
                .to_bits(),
        })
        .collect()
}

/// The depth a word stands for.
fn depth_of(format: Format, word: u32) -> f32 {
    match format {
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            (f64::from(word & Format::UNORM24_MAX) / f64::from(Format::UNORM24_MAX)) as f32
        }
        _ => f32::from_bits(word),
    }
}

/// Float depth to the bit. 24-bit depth within one of its steps: that is as finely as a program
/// reads it (the RTX 3080 reads a clear to 0.5, stored as 2^23, as 0.50000006, one step above 0.5,
/// and in the upper half a step is one float bit).
fn same_depth(format: Format, got: f32, want: f32) -> bool {
    match format {
        Format::Depth24Plus | Format::Depth24PlusStencil8 => {
            (f64::from(got) - f64::from(want)).abs() * f64::from(Format::UNORM24_MAX) < 1.001
        }
        _ => got.to_bits() == want.to_bits(),
    }
}

fn depth_formats(b: &WebGpu) -> Vec<Format> {
    [
        Format::Depth32Float,
        Format::Depth32FloatStencil8,
        Format::Depth24Plus,
        Format::Depth24PlusStencil8,
    ]
    .into_iter()
    .filter(|f| {
        let held = b.caps().formats.uses[*f as usize]
            .contains(FormatUse::DEPTH_STENCIL | FormatUse::SAMPLE);
        if !held {
            eprintln!("{f:?}: not held by this device");
        }
        held
    })
    .collect()
}

fn depth_texture(b: &mut WebGpu, format: Format) -> (Tex, View) {
    draw::texture_2d(
        b,
        format,
        W,
        H,
        TextureUsage::RENDER
            | TextureUsage::SAMPLE
            | TextureUsage::COPY_SRC
            | TextureUsage::COPY_DST,
    )
    .unwrap()
}

/// A buffer copied into depth at a row pitch WebGPU refuses, and the same bytes written: every texel
/// is the depth its word stands for, on every depth format.
#[test]
fn buffers_and_writes_fill_depth_exactly() {
    let mut b = device();
    for format in depth_formats(&b) {
        let words = source_words(format);
        // Rows of W words and one word of padding: 36 bytes, not a multiple of 256.
        let pitch = (W + 1) * 4;
        let mut bytes = vec![0xee; (pitch * H) as usize];
        for y in 0..H {
            for x in 0..W {
                let at = (y * pitch + x * 4) as usize;
                bytes[at..at + 4].copy_from_slice(&words[(y * W + x) as usize].to_le_bytes());
            }
        }
        let buffer = b
            .create_buffer(&BufferDesc {
                size: bytes.len() as u64,
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .unwrap();
        b.write_buffer(&buffer, 0, &bytes, WriteMode::Discard);
        let (copied, copied_view) = depth_texture(&mut b, format);
        let (written, written_view) = depth_texture(&mut b, format);
        let before = (
            b.slow_paths().counts[SlowPath::DepthWrittenByDraw as usize],
            b.slow_paths().counts[SlowPath::RowsRepacked as usize],
        );
        b.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &buffer,
                offset: 0,
                row_pitch: pitch,
                rows_per_layer: H,
            },
            &region(&copied, 0, 0),
            extent(W, H),
        );
        b.write_texture(
            &region(&written, 0, 0),
            extent(W, H),
            &bytes,
            pitch,
            H,
            WriteMode::Ordered,
        );
        let want: Vec<f32> = words.iter().map(|w| depth_of(format, *w)).collect();
        for (name, v) in [("copied", &copied_view), ("written", &written_view)] {
            let got = depth_values(&mut b, v);
            let wrong: Vec<_> = (0..want.len())
                .filter(|i| !same_depth(format, got[*i], want[*i]))
                .map(|i| (i, got[i], want[i]))
                .collect();
            assert!(wrong.is_empty(), "{format:?} {name}: {wrong:?}");
        }
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
        let after = (
            b.slow_paths().counts[SlowPath::DepthWrittenByDraw as usize],
            b.slow_paths().counts[SlowPath::RowsRepacked as usize],
        );
        assert_eq!(
            (after.0 - before.0, after.1 - before.1),
            (2, 1),
            "{format:?}: two depth draws, one repacking"
        );
    }
}

/// Part of a texture of the group copied into part of a depth texture by a draw: `r32float`, and
/// `r32uint` and `r32sint` holding float bits, into 32-bit depth, and 24-bit depth into 24-bit depth.
/// Texels outside the rectangle keep their clear.
#[test]
fn group_textures_copy_into_part_of_depth_exactly() {
    let mut b = device();
    for format in depth_formats(&b) {
        let words = source_words(format);
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let sources: &[Format] = match format {
            Format::Depth32Float => &[
                Format::R32Float,
                Format::R32Uint,
                Format::R32Sint,
                Format::Depth32Float,
            ],
            // A combined format is copied with both aspects, only from itself.
            _ => &[format],
        };
        for &source in sources {
            let src = if FORMAT_FACTS[source as usize].depth_stencil {
                let (t, _) = depth_texture(&mut b, source);
                b.write_texture(
                    &region(&t, 0, 0),
                    extent(W, H),
                    &bytes,
                    W * 4,
                    H,
                    WriteMode::Ordered,
                );
                t
            } else {
                let t = draw::texture_2d(
                    &mut b,
                    source,
                    W,
                    H,
                    TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                )
                .unwrap()
                .0;
                b.write_texture(
                    &region(&t, 0, 0),
                    extent(W, H),
                    &bytes,
                    W * 4,
                    H,
                    WriteMode::Ordered,
                );
                t
            };
            let (dst, dst_view) = depth_texture(&mut b, format);
            clear_depth(&mut b, &dst_view, format, 0.5);
            let before = (
                b.slow_paths().counts[SlowPath::DepthWrittenByDraw as usize],
                b.slow_paths().counts[SlowPath::StencilWrittenByDraw as usize],
            );
            // Source texels (1..6, 1..3) to destination texels (2..7, 2..4).
            b.copy_texture(&region(&src, 1, 1), &region(&dst, 2, 2), extent(5, 2));
            let got = depth_values(&mut b, &dst_view);
            let mut wrong = Vec::new();
            for y in 0..H {
                for x in 0..W {
                    let inside = (2..7).contains(&x) && (2..4).contains(&y);
                    let want = if inside {
                        depth_of(format, words[((y - 1) * W + x - 1) as usize])
                    } else {
                        0.5
                    };
                    let g = got[(y * W + x) as usize];
                    if !same_depth(format, g, want) {
                        wrong.push((x, y, g, want));
                    }
                }
            }
            assert!(wrong.is_empty(), "{source:?} into {format:?}: {wrong:?}");
            assert_eq!(b.status(), DeviceStatus::Ok, "{source:?} into {format:?}");
            let stencil = FORMAT_FACTS[format as usize].kind == FormatKind::DepthStencil;
            assert_eq!(
                (
                    b.slow_paths().counts[SlowPath::DepthWrittenByDraw as usize] - before.0,
                    b.slow_paths().counts[SlowPath::StencilWrittenByDraw as usize] - before.1
                ),
                (1, u64::from(stencil)),
                "{source:?} into {format:?}"
            );
        }
    }
}

/// Whether 32-bit float depth copied in holds what the loss plan 4.12 lists: a draw writes it, so
/// values in 0 to 1 keep their bits, values outside are clamped and NaN becomes zero; denormals and
/// -0 are kept or become zero (the RTX 3080 keeps -0 and flushes denormals, Metal on Apple M5 the
/// opposite). Direct3D's `CopyResource` keeps every bit.
fn drawable(word: u32, got: u32) -> bool {
    let v = f32::from_bits(word);
    if v.is_nan() {
        got == 0
    } else if v.is_subnormal() || v == 0.0 {
        got == word || got == 0
    } else {
        got == v.clamp(0.0, 1.0).to_bits()
    }
}

/// Float words a draw cannot write as depth (above 1, below 0, a denormal, NaN) beside ones it can,
/// copied into `depth32float` from an `r32float` texture and from a buffer: each texel holds what
/// the listed loss says.
#[test]
fn float_depth_copies_keep_what_a_draw_can_write() {
    let mut b = device();
    let mut words: Vec<u32> = vec![
        0.25f32.to_bits(),
        2.0f32.to_bits(),
        (-0.5f32).to_bits(),
        (-0.0f32).to_bits(),
        0x0000_1234, // a denormal
        0x7fc0_0001, // NaN
        1.0f32.to_bits(),
        0.1234f32.to_bits(),
    ];
    words.resize((W * H) as usize, 0.5f32.to_bits());
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let (src, _) = draw::texture_2d(
        &mut b,
        Format::R32Float,
        W,
        H,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    )
    .unwrap();
    b.write_texture(
        &region(&src, 0, 0),
        extent(W, H),
        &bytes,
        W * 4,
        H,
        WriteMode::Ordered,
    );
    let buffer = b
        .create_buffer(&BufferDesc {
            size: bytes.len() as u64,
            usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    b.write_buffer(&buffer, 0, &bytes, WriteMode::Ordered);
    let (from_texture, _) = depth_texture(&mut b, Format::Depth32Float);
    let (from_buffer, _) = depth_texture(&mut b, Format::Depth32Float);
    b.copy_texture(
        &region(&src, 0, 0),
        &region(&from_texture, 0, 0),
        extent(W, H),
    );
    b.copy_buffer_to_texture(
        &CopyBufferTexture {
            buffer: &buffer,
            offset: 0,
            row_pitch: W * 4,
            rows_per_layer: H,
        },
        &region(&from_buffer, 0, 0),
        extent(W, H),
    );
    for (name, t) in [
        ("from a texture", &from_texture),
        ("from a buffer", &from_buffer),
    ] {
        let got: Vec<u32> = read(&mut b, t, 4)
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        eprintln!("{name}: {:08x?}", &got[..8]);
        let wrong: Vec<_> = (0..words.len())
            .filter(|i| !drawable(words[*i], got[*i]))
            .map(|i| (i, words[i], got[i]))
            .collect();
        assert!(wrong.is_empty(), "{name}: {wrong:x?}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Copies between colour formats of one typeless group move the bits unchanged, through a buffer
/// where the formats differ and natively between sRGB twins.
#[test]
fn typeless_group_copies_keep_the_bits() {
    let mut b = device();
    let bytes: Vec<u8> = (0..W * H * 4).map(|i| (i * 37 + 11) as u8).collect();
    for (from, to, through_buffer) in [
        (Format::Rgba8Unorm, Format::Rgba8Uint, true),
        (Format::Rgba8Uint, Format::Rgba8Snorm, true),
        (Format::Rgba8Unorm, Format::Rgba8UnormSrgb, false),
        (Format::R32Float, Format::R32Uint, true),
        (Format::R32Sint, Format::R32Float, true),
        (Format::Rg16Float, Format::Rg16Uint, true),
        (Format::Bgra8Unorm, Format::Bgra8UnormSrgb, false),
    ] {
        let usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
        let src = draw::texture_2d(&mut b, from, W, H, usage).unwrap().0;
        let dst = draw::texture_2d(&mut b, to, W, H, usage).unwrap().0;
        b.write_texture(
            &region(&src, 0, 0),
            extent(W, H),
            &bytes,
            W * 4,
            H,
            WriteMode::Ordered,
        );
        b.write_texture(
            &region(&dst, 0, 0),
            extent(W, H),
            &[0x5a; (W * H * 4) as usize],
            W * 4,
            H,
            WriteMode::Ordered,
        );
        let before = b.slow_paths().counts[SlowPath::CopyThroughBuffer as usize];
        // The whole texture, then a part moved by one texel each way.
        b.copy_texture(&region(&src, 0, 0), &region(&dst, 0, 0), extent(W, H));
        b.copy_texture(&region(&src, 0, 0), &region(&dst, 1, 1), extent(3, 2));
        let got = read(&mut b, &dst, 4);
        let mut want = bytes.clone();
        for y in 0..2u32 {
            for x in 0..3u32 {
                let (s, d) = (
                    ((y * W + x) * 4) as usize,
                    (((y + 1) * W + x + 1) * 4) as usize,
                );
                want[d..d + 4].copy_from_slice(&bytes[s..s + 4]);
            }
        }
        assert_eq!(got, want, "{from:?} to {to:?}");
        assert_eq!(b.status(), DeviceStatus::Ok, "{from:?} to {to:?}");
        assert_eq!(
            b.slow_paths().counts[SlowPath::CopyThroughBuffer as usize] - before,
            if through_buffer { 2 } else { 0 },
            "{from:?} to {to:?}"
        );
    }
}

/// The bytes of `extent` texels of `region`, `texel` bytes each, read back.
fn read_part(
    b: &mut WebGpu,
    region: TextureRegion<'_, WebGpu>,
    extent: Extent,
    texel: u32,
) -> Vec<u8> {
    let rb = b
        .read_back(&ReadSource::Texture { region, extent })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (extent.width * extent.height * texel) as usize];
    b.finish_read(rb, &mut out);
    out
}

fn stencil_of(texture: &Tex, x: u32, y: u32) -> TextureRegion<'_, WebGpu> {
    TextureRegion {
        stencil: true,
        ..region(texture, x, y)
    }
}

/// What WebGPU copies only whole, done by draws and exact: part of a stencil from a buffer at a
/// 9-byte pitch (`StencilWrittenByDraw`), part of a combined texture copied from another (both
/// aspects), part of each aspect and all of 24-bit depth read back (`DepthReadByDraw`), against
/// the bytes written and against WebGPU's own whole copies; whole copies of a combined texture
/// are native, both aspects.
#[test]
fn depth_and_stencil_parts_copy_and_read_back_exactly() {
    let mut b = device();
    let count = |b: &mut WebGpu| {
        [
            SlowPath::DepthWrittenByDraw,
            SlowPath::StencilWrittenByDraw,
            SlowPath::DepthReadByDraw,
        ]
        .map(|p| b.slow_paths().counts[p as usize])
    };
    let delta = |b: &mut WebGpu, before: [u64; 3]| {
        let now = count(b);
        [0, 1, 2].map(|i| now[i] - before[i])
    };
    for format in depth_formats(&b) {
        let stencil = FORMAT_FACTS[format as usize].kind == FormatKind::DepthStencil;
        let unorm24 = matches!(format, Format::Depth24Plus | Format::Depth24PlusStencil8);
        let (a, a_view) = depth_texture(&mut b, format);
        clear_depth(&mut b, &a_view, format, 0.5);

        // Depth, whole, from a buffer; read back whole and in part.
        let words = source_words(format);
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let buffer = b
            .create_buffer(&BufferDesc {
                size: 4096,
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .unwrap();
        b.write_buffer(&buffer, 0, &bytes, WriteMode::Ordered);
        b.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &buffer,
                offset: 0,
                row_pitch: W * 4,
                rows_per_layer: H,
            },
            &region(&a, 0, 0),
            extent(W, H),
        );
        let drawn = depth_values(&mut b, &a_view);
        let before = count(&mut b);
        let whole: Vec<u32> = read_part(&mut b, region(&a, 0, 0), extent(W, H), 4)
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(
            delta(&mut b, before),
            [0, 0, u64::from(unorm24)],
            "{format:?}"
        );
        // What a read by a draw must give exactly: 24-bit depth's stored values, which the write
        // stores as given and the read finds through the device's own conversion (a device may
        // sample a value as a float nearer the next one: llvmpipe's product with the rounded
        // reciprocal), else the bits of the depth a program reads.
        let want: Vec<u32> = if unorm24 {
            words.iter().map(|w| w & Format::UNORM24_MAX).collect()
        } else {
            drawn.iter().map(|d| d.to_bits()).collect()
        };
        // Where `depth24plus` is stored as a float, the word is the float's nearest 24-bit step,
        // which the program's float arithmetic may miss by one at a near tie.
        let stored_as_float = matches!(b.depth24(), Depth24::Float(_));
        if unorm24 && stored_as_float {
            let off: Vec<_> = (0..want.len())
                .filter(|i| want[*i].abs_diff(whole[*i]) > 1)
                .collect();
            assert!(off.is_empty(), "{format:?}: whole depth read back, {off:?}");
        } else {
            assert_eq!(whole, want, "{format:?}: whole depth read back");
        }
        if unorm24 {
            let off: Vec<_> = (0..words.len())
                .filter(|i| (words[*i] & Format::UNORM24_MAX).abs_diff(whole[*i]) > 1)
                .collect();
            assert!(
                off.is_empty(),
                "{format:?}: more than a step from the words"
            );
        } else {
            assert_eq!(whole, words, "{format:?}: 32-bit depth as written");
        }
        let before = count(&mut b);
        let part: Vec<u32> = read_part(&mut b, region(&a, 3, 1), extent(4, 2), 4)
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(delta(&mut b, before), [0, 0, 1], "{format:?}");
        let want_part: Vec<u32> = (1..3)
            .flat_map(|y| (3..7).map(move |x| (y * W + x) as usize))
            .map(|i| whole[i])
            .collect();
        assert_eq!(part, want_part, "{format:?}: part of the depth read back");
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
        if !stencil {
            continue;
        }

        // Stencil: part from a buffer at a pitch of 9 bytes, read back whole natively and in part
        // by a draw.
        let pitch = W + 1;
        let stencil_bytes: Vec<u8> = (0..pitch * H).map(|i| (i * 37 + 5) as u8).collect();
        b.write_buffer(&buffer, 1024, &stencil_bytes, WriteMode::Ordered);
        let before = count(&mut b);
        b.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &buffer,
                offset: 1024,
                row_pitch: pitch,
                rows_per_layer: H,
            },
            &stencil_of(&a, 2, 1),
            extent(5, 2),
        );
        assert_eq!(delta(&mut b, before), [0, 1, 0], "{format:?}");
        let mut want_stencil = vec![0u8; (W * H) as usize];
        for y in 0..2 {
            for x in 0..5 {
                want_stencil[((y + 1) * W + x + 2) as usize] =
                    stencil_bytes[(y * pitch + x) as usize];
            }
        }
        let got = read_part(&mut b, stencil_of(&a, 0, 0), extent(W, H), 1);
        assert_eq!(got, want_stencil, "{format:?}: stencil written in part");
        let before = count(&mut b);
        let got = read_part(&mut b, stencil_of(&a, 1, 1), extent(6, 3), 1);
        assert_eq!(delta(&mut b, before), [0, 0, 1], "{format:?}");
        let want_part: Vec<u8> = (1..4)
            .flat_map(|y| (1..7).map(move |x| (y * W + x) as usize))
            .map(|i| want_stencil[i])
            .collect();
        assert_eq!(got, want_part, "{format:?}: part of the stencil read back");

        // The combined texture copied whole (native) and in part (by draws), both aspects.
        let (c, c_view) = depth_texture(&mut b, format);
        let before = count(&mut b);
        b.copy_texture(&region(&a, 0, 0), &region(&c, 0, 0), extent(W, H));
        assert_eq!(
            delta(&mut b, before),
            [0, 0, 0],
            "{format:?}: a whole copy is native"
        );
        let got = read_part(&mut b, stencil_of(&c, 0, 0), extent(W, H), 1);
        assert_eq!(got, want_stencil, "{format:?}: stencil copied whole");
        assert_eq!(
            depth_values(&mut b, &c_view),
            drawn,
            "{format:?}: depth copied whole"
        );

        let (d, d_view) = depth_texture(&mut b, format);
        clear_depth(&mut b, &d_view, format, 0.25);
        let before = count(&mut b);
        // Texels (2..7, 1..3) of `a` to (1..6, 2..4).
        b.copy_texture(&region(&a, 2, 1), &region(&d, 1, 2), extent(5, 2));
        assert_eq!(delta(&mut b, before), [1, 1, 0], "{format:?}");
        let got_stencil = read_part(&mut b, stencil_of(&d, 0, 0), extent(W, H), 1);
        let got_depth = depth_values(&mut b, &d_view);
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) as usize;
                let inside = (1..6).contains(&x) && (2..4).contains(&y);
                let from = if inside {
                    ((y - 1) * W + x + 1) as usize
                } else {
                    i
                };
                let (want_s, want_d) = if inside {
                    (want_stencil[from], drawn[from])
                } else {
                    (0, 0.25)
                };
                assert_eq!(got_stencil[i], want_s, "{format:?}: stencil at {x}, {y}");
                assert!(
                    same_depth(format, got_depth[i], want_d),
                    "{format:?}: depth at {x}, {y}: {} for {want_d}",
                    got_depth[i]
                );
            }
        }
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
    }
}

/// A copy between regions of one subresource (as Vulkan and OpenGL allow) goes through a buffer,
/// counted; between two levels of one texture it is one copy.
#[test]
fn copies_within_one_subresource_go_through_a_buffer() {
    let mut b = device();
    let b = &mut b;
    let t = b
        .create_texture(&wcontainers_gpu::resource::TextureDesc {
            dimension: wcontainers_gpu::resource::TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: W,
            height: H,
            depth_or_layers: 1,
            mip_levels: 2,
            samples: 1,
            usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let texels: Vec<u8> = (0..W * H * 4).map(|i| (i * 13 + 5) as u8).collect();
    b.write_texture(
        &region(&t, 0, 0),
        extent(W, H),
        &texels,
        W * 4,
        H,
        WriteMode::Ordered,
    );
    // Mip 0 at (2, 1) into mip 1 at (0, 0): different subresources.
    let mut level1 = TextureRegion {
        mip: 1,
        ..region(&t, 0, 0)
    };
    b.copy_texture(&region(&t, 2, 1), &level1, extent(4, 2));
    assert_eq!(
        b.slow_paths().counts[SlowPath::CopyWithinTexture as usize],
        0
    );
    // Mip 0 at (0, 0) into mip 0 at (4, 2), 3 by 2: one subresource.
    b.copy_texture(&region(&t, 0, 0), &region(&t, 4, 2), extent(3, 2));
    assert_eq!(
        b.slow_paths().counts[SlowPath::CopyWithinTexture as usize],
        1
    );
    let mut want = texels.clone();
    for y in 0..2 {
        for x in 0..3 {
            let (from, to) = ((y * W + x) * 4, ((y + 2) * W + x + 4) * 4);
            let texel = texels[from as usize..][..4].to_vec();
            want[to as usize..][..4].copy_from_slice(&texel);
        }
    }
    assert_eq!(read(b, &t, 4), want, "mip 0");
    level1.origin = Origin::default();
    let rb = b
        .read_back(&ReadSource::Texture {
            region: level1,
            extent: extent(4, 2),
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut got = vec![0; 32];
    b.finish_read(rb, &mut got);
    let mut want1 = Vec::new();
    for y in 1..3 {
        want1.extend_from_slice(&texels[((y * W + 2) * 4) as usize..][..16]);
    }
    assert_eq!(got, want1, "mip 1");
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A copy between regions of one depth subresource, over two layers at once, goes through a
/// temporary texture, counted, and moves every value exactly.
#[test]
fn depth_copies_within_one_subresource_go_through_a_texture() {
    let mut b = device();
    let b = &mut b;
    let t = b
        .create_texture(&wcontainers_gpu::resource::TextureDesc {
            dimension: wcontainers_gpu::resource::TextureDimension::D2,
            format: Format::Depth32Float,
            width: W,
            height: H,
            depth_or_layers: 2,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let words: Vec<u32> = (0..W * H * 2)
        .map(|i| (i as f32 / 64.0).to_bits())
        .collect();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    b.write_texture(
        &region(&t, 0, 0),
        Extent {
            depth_or_layers: 2,
            ..extent(W, H)
        },
        &bytes,
        W * 4,
        H,
        WriteMode::Ordered,
    );
    // Both layers' 3 by 2 at (1, 0) into (4, 2): one subresource each.
    b.copy_texture(
        &region(&t, 1, 0),
        &region(&t, 4, 2),
        Extent {
            depth_or_layers: 2,
            ..extent(3, 2)
        },
    );
    assert_eq!(
        b.slow_paths().counts[SlowPath::CopyWithinTexture as usize],
        1
    );
    for layer in 0..2 {
        let mut want = bytes[(layer * W * H * 4) as usize..][..(W * H * 4) as usize].to_vec();
        let from = want.clone();
        for y in 0..2 {
            for x in 0..3 {
                let (s, d) = ((y * W + x + 1) * 4, ((y + 2) * W + x + 4) * 4);
                want[d as usize..][..4].copy_from_slice(&from[s as usize..][..4]);
            }
        }
        let rb = b
            .read_back(&ReadSource::Texture {
                region: TextureRegion {
                    layer,
                    ..region(&t, 0, 0)
                },
                extent: extent(W, H),
            })
            .unwrap();
        b.submit();
        b.wait_read(&rb);
        let mut got = vec![0; (W * H * 4) as usize];
        b.finish_read(rb, &mut got);
        assert_eq!(got, want, "layer {layer}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// `r16unorm` and `r16uint` texels copied into `depth16unorm` (Direct3D's `R16_TYPELESS` group)
/// keep their bits.
#[test]
fn sixteen_bit_group_copies_into_depth_keep_the_bits() {
    let mut b = device();
    let b = &mut b;
    let words: Vec<u16> = (0..W * H)
        .map(|i| {
            [0, 1, 0x7fff, 0x8000, 0xfffe, 0xffff, 12345, 54321][i as usize % 8] ^ (i as u16 * 3)
        })
        .collect();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    for source in [Format::R16Unorm, Format::R16Uint] {
        if !b.caps().formats.uses[source as usize].contains(FormatUse::SAMPLE) {
            eprintln!("{source:?} is absent on this device: not copied");
            continue;
        }
        let (s, _) = draw::texture_2d(
            b,
            source,
            W,
            H,
            TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
        )
        .unwrap();
        b.write_texture(
            &region(&s, 0, 0),
            extent(W, H),
            &bytes,
            W * 2,
            H,
            WriteMode::Ordered,
        );
        let (d, _) = depth_texture(b, Format::Depth16Unorm);
        let before = b.slow_paths().counts[SlowPath::DepthWrittenByDraw as usize];
        b.copy_texture(&region(&s, 0, 0), &region(&d, 0, 0), extent(W, H));
        assert!(b.slow_paths().counts[SlowPath::DepthWrittenByDraw as usize] > before);
        assert_eq!(read(b, &d, 2), bytes, "{source:?} into depth16unorm");
        assert_eq!(b.status(), DeviceStatus::Ok, "{source:?}");
    }
}

/// Every colour format of Direct3D's `R16_TYPELESS` group copied into `depth16unorm` and back, whole
/// and in part, keeps the bits (tests/scenes/depth16.rs; also run in Chrome by web/check).
#[test]
fn sixteen_bit_group_copies_with_depth_both_ways_keep_the_bits() {
    let mut b = device();
    eprintln!("{}", depth16::group_copies(&mut b).unwrap());
}

/// Query results resolved past the buffer's end are refused with a reason, the device kept.
#[test]
fn query_results_past_the_buffer_are_refused() {
    let mut b = device();
    let set = b
        .create_query_set(wcontainers_gpu::resource::QueryKind::OcclusionAny, 4)
        .unwrap();
    let dst = b
        .create_buffer(&BufferDesc {
            size: 24,
            usage: BufferUsage::COPY_DST | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    assert!(matches!(
        b.resolve_queries(&set, 0, 4, &dst, 0),
        Err(wcontainers_gpu::error::GpuError::Invalid(_))
    ));
    assert!(matches!(
        b.resolve_queries(&set, 2, 4, &dst, 0),
        Err(wcontainers_gpu::error::GpuError::Invalid(_))
    ));
    assert_eq!(b.resolve_queries(&set, 0, 3, &dst, 0), Ok(()));
    let fence = b.submit();
    b.wait_fence(&fence);
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A 3D level written with more rows to a slice in the data than the extent has keeps every slice.
#[test]
fn volume_writes_with_padded_slices_keep_every_slice() {
    let mut b = device();
    let b = &mut b;
    let t = b
        .create_texture(&wcontainers_gpu::resource::TextureDesc {
            dimension: wcontainers_gpu::resource::TextureDimension::D3,
            format: Format::Rgba8Unorm,
            width: 2,
            height: 2,
            depth_or_layers: 2,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    // Rows of 8 bytes, 3 rows to a slice, the third of each slice padding.
    let data: Vec<u8> = (0..48).map(|i| i as u8 + 1).collect();
    let whole = Extent {
        width: 2,
        height: 2,
        depth_or_layers: 2,
    };
    b.write_texture(&region(&t, 0, 0), whole, &data, 8, 3, WriteMode::Ordered);
    let rb = b
        .read_back(&ReadSource::Texture {
            region: region(&t, 0, 0),
            extent: whole,
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut got = vec![0; 32];
    b.finish_read(rb, &mut got);
    let want: Vec<u8> = [&data[0..16], &data[24..40]].concat();
    assert_eq!(got, want);
    assert_eq!(b.slow_paths().counts[SlowPath::RowsRepacked as usize], 1);
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Texture reads into a buffer leave the bytes between rows and between layers as they were, at
/// an offset off Direct3D 12's placement alignment too (wgpu's Direct3D 12 overwrote them there).
/// `WEBGPU_SEED` picks the texels (default 1).
#[test]
fn texture_reads_into_a_buffer_keep_the_bytes_between_rows() {
    let mut b = device();
    let seed = std::env::var("WEBGPU_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let line = gaps::reads_keep_the_bytes_between_rows(&mut b, seed).unwrap();
    eprintln!("seed {seed}: {line}; slow paths {:?}", b.slow_paths());
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Copies from a buffer into several layers or slices whose rows lie further apart in the buffer
/// than the region's height take each layer from its own rows (wgpu's Direct3D 12 read them at the
/// region's height). `WEBGPU_SEED` picks the bytes (default 1).
#[test]
fn buffer_copies_take_each_layer_from_its_rows() {
    let mut b = device();
    let seed = std::env::var("WEBGPU_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let line = gaps::writes_take_each_layer_from_its_rows(&mut b, seed).unwrap();
    eprintln!("seed {seed}: {line}; slow paths {:?}", b.slow_paths());
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A surface handle the back end never made is refused by each call naming it, with the reason every
/// such handle is refused for (plan 4.10, `GpuError::GONE`).
#[test]
fn a_surface_never_made_is_refused_by_configure_and_present() {
    let mut b = device();
    let never = crate::Surface(u64::MAX);
    assert_eq!(
        b.configure_surface(&never, Format::Rgba8Unorm, 4, 4, false),
        Err(wcontainers_gpu::error::GpuError::Invalid(
            wcontainers_gpu::error::GpuError::GONE
        ))
    );
    assert_eq!(
        b.present(&never),
        Err(wcontainers_gpu::error::GpuError::Invalid(
            wcontainers_gpu::error::GpuError::GONE
        ))
    );
}

/// On a device sampling some 24-bit words alike, the comparison pass's pipeline is made once for each
/// 24-bit depth format a read compares against (`fill::decide_colliding` keeps it by format): a read of
/// a format already read makes no pipeline, the first read of the other 24-bit format exactly one.
#[test]
fn the_comparison_pass_is_made_once_a_depth_format() {
    let mut b = device();
    if b.depth24_colliding.is_empty() {
        eprintln!("this device samples every 24-bit word as its own float: no comparison pass");
        return;
    }
    let (plain, _) = depth_texture(&mut b, Format::Depth24Plus);
    let (stencilled, _) = depth_texture(&mut b, Format::Depth24PlusStencil8);
    read(&mut b, &plain, 4);
    let before = b.counters().pipelines;
    read(&mut b, &plain, 4);
    assert_eq!(b.counters().pipelines, before, "a format read before");
    read(&mut b, &stencilled, 4);
    assert_eq!(
        b.counters().pipelines,
        before + 1,
        "the other 24-bit format"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
}
