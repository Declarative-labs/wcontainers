//! Copies WebGPU has no single command for, exact: between formats of one typeless group (through a
//! buffer), and into depth (by draws), from a buffer at a pitch WebGPU refuses, from a write, and
//! from a texture of the group, into part of the destination.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BufferDesc, BufferUsage, ColorAttachment,
    CopyBufferTexture, DepthAttachment, DeviceStatus, DrawCall, DrawRange, Extent, Format,
    FormatKind, FormatUse, LoadOp, MapIntent, Origin, PassDesc, ReadSource, SlowPath, StageSet,
    StoreOp, TextureRegion, TextureUsage, Topology, Viewport, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

type Tex = <WebGpu as Backend>::Texture;
type View = <WebGpu as Backend>::TextureView;

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

const W: u32 = 8;
const H: u32 = 4;

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
                0xab00_0000 | (i.wrapping_mul(0x0012_3457) & 0xff_ffff)
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
            (f64::from(word & 0xff_ffff) / 16_777_215.0) as f32
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
            (f64::from(got) - f64::from(want)).abs() * 16_777_215.0 < 1.001
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
        let held = b
            .caps()
            .formats
            .get(*f)
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
            b.slow_paths().count(SlowPath::DepthWrittenByDraw),
            b.slow_paths().count(SlowPath::RowsRepacked),
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
            b.slow_paths().count(SlowPath::DepthWrittenByDraw),
            b.slow_paths().count(SlowPath::RowsRepacked),
        );
        assert_eq!(
            (after.0 - before.0, after.1 - before.1),
            (2, 1),
            "{format:?}: two depth draws, one repacking"
        );
    }
}

/// Part of a texture of the group copied into part of a depth texture by a draw: `r32float` and
/// `r32uint` holding float bits into 32-bit depth, and 24-bit depth into 24-bit depth. Texels
/// outside the rectangle keep their clear.
#[test]
fn group_textures_copy_into_part_of_depth_exactly() {
    let mut b = device();
    for format in depth_formats(&b) {
        let words = source_words(format);
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let sources: &[Format] = match format {
            Format::Depth32Float => &[Format::R32Float, Format::R32Uint, Format::Depth32Float],
            // A combined format is copied with both aspects, only from itself.
            _ => &[format],
        };
        for &source in sources {
            let src = if source.is_depth_stencil() {
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
                b.slow_paths().count(SlowPath::DepthWrittenByDraw),
                b.slow_paths().count(SlowPath::StencilWrittenByDraw),
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
            let stencil = format.kind() == FormatKind::DepthStencil;
            assert_eq!(
                (
                    b.slow_paths().count(SlowPath::DepthWrittenByDraw) - before.0,
                    b.slow_paths().count(SlowPath::StencilWrittenByDraw) - before.1
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
        0x3e80_0000, // 0.25
        0x4000_0000, // 2
        0xbf00_0000, // -0.5
        0x8000_0000, // -0
        0x0000_1234, // a denormal
        0x7fc0_0001, // NaN
        0x3f80_0000, // 1
        0x3dfc_d6ea, // 0.1234
    ];
    words.resize((W * H) as usize, 0x3f00_0000);
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
        let before = b.slow_paths().count(SlowPath::CopyThroughBuffer);
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
            b.slow_paths().count(SlowPath::CopyThroughBuffer) - before,
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
        .map(|p| b.slow_paths().count(p))
    };
    let delta = |b: &mut WebGpu, before: [u64; 3]| {
        let now = count(b);
        [0, 1, 2].map(|i| now[i] - before[i])
    };
    for format in depth_formats(&b) {
        let stencil = format.kind() == FormatKind::DepthStencil;
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
        // The words of the depth a program reads: what a read by a draw must give exactly.
        let want: Vec<u32> = drawn
            .iter()
            .map(|d| {
                if unorm24 {
                    (f64::from(*d) * 16_777_215.0).round() as u32
                } else {
                    d.to_bits()
                }
            })
            .collect();
        // Where `depth24plus` is stored as a float, the word is the float's nearest 24-bit step,
        // which the program's float arithmetic may miss by one at a near tie.
        let stored_as_float = matches!(b.depth24(), wcontainers_gpu_webgpu::Depth24::Float(_));
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
                .filter(|i| (words[*i] & 0xff_ffff).abs_diff(whole[*i]) > 1)
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
