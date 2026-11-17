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
    FormatUse, LoadOp, MapIntent, Origin, PassDesc, ReadSource, SlowPath, StageSet, StoreOp,
    TextureRegion, TextureUsage, Topology, Viewport, WriteMode,
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
            Format::Depth32Float | Format::Depth32FloatStencil8 => {
                &[Format::R32Float, Format::R32Uint, Format::Depth32Float]
            }
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
            let before = b.slow_paths().count(SlowPath::DepthWrittenByDraw);
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
            assert_eq!(
                b.slow_paths().count(SlowPath::DepthWrittenByDraw) - before,
                1,
                "{source:?} into {format:?}"
            );
        }
    }
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
