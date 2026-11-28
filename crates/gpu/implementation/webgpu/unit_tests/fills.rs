//! The back end's own draws of fill.rs that no call reaches on this host, run directly (a packed
//! target's rounding, the present and resolve draws), the programs those draws make (each made once
//! and counted), the copies by draws across layers, and the clears by draws.
//!
//! Needs a GPU, or lavapipe as in unit_tests/scenes.rs.

use super::draw;

use crate::fill::{self, PackedTarget};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wcontainers_gpu::{
    Gpu, draw::ColorAttachment, draw::LoadOp, draw::PassDesc, draw::StoreOp, error::DeviceStatus,
    format::Format, resource::BufferDesc, resource::BufferUsage, resource::CopyBufferTexture,
    resource::Extent, resource::MapIntent, resource::Origin, resource::ReadSource,
    resource::TextureDesc, resource::TextureDimension, resource::TextureRegion,
    resource::TextureUsage, resource::TextureViewDesc, resource::ViewDimension,
    resource::WriteMode, slow::SlowPath,
};
use wcontainers_gpu_webgpu::WebGpu;

type Tex = <WebGpu as Gpu>::Texture;

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

/// The width and height of level 0 of every texture here.
const N: u32 = 8;
/// 0.4 as `rgba8unorm` stores it, exactly (0.4 255 = 102).
const HELD: u8 = 102;
/// 0.4 in `b5g6r5unorm`, widened back to 8 bits by repeating its bits: red and blue round(0.4 31) =
/// 12 = 0b01100, so 0b01100011; green round(0.4 63) = 25 = 0b011001, so 0b01100101; no alpha, so 255.
const ROUNDED: [u8; 4] = [0b0110_0011, 0b0110_0101, 0b0110_0011, 255];
/// The bytes each row of a read keeps in its buffer (WebGPU's row alignment).
const ROW: u32 = 256;

/// A GPU texture of `rgba8unorm` (`bgra8unorm` where `bgra`), `layers` layers and `mips` levels.
fn raw_texture(
    gpu: &WebGpu,
    bgra: bool,
    layers: u32,
    mips: u32,
    samples: u32,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format: if bgra {
            wgpu::TextureFormat::Bgra8Unorm
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage,
        view_formats: &[],
    })
}

/// Fills level `mip`, layer `layer` of `texture` with `texel` in every texel.
fn fill_level(gpu: &WebGpu, texture: &wgpu::Texture, mip: u32, layer: u32, texel: [u8; 4]) {
    fill_texels(gpu, texture, mip, layer, &level_of(mip, texel));
}

/// Writes `bytes` (4 a texel, row after row) into level `mip`, layer (or depth slice) `layer` of
/// `texture`.
fn fill_texels(gpu: &WebGpu, texture: &wgpu::Texture, mip: u32, layer: u32, bytes: &[u8]) {
    let n = N >> mip;
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: mip,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(n * 4),
            rows_per_image: Some(n),
        },
        wgpu::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
    );
}

/// Submits what the back end recorded, then reads level `mip`, layer `layer` of `texture` (4 bytes
/// a texel), row after row.
fn read_level(gpu: &mut WebGpu, texture: &wgpu::Texture, mip: u32, layer: u32) -> Vec<u8> {
    gpu.submit();
    let n = N >> mip;
    let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(ROW * n),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: mip,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW),
                rows_per_image: Some(n),
            },
        },
        wgpu::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
    );
    let index = gpu.queue.submit([encoder.finish()]);
    let mapped = Arc::new(AtomicBool::new(false));
    let flag = mapped.clone();
    read.slice(..).map_async(wgpu::MapMode::Read, move |_| {
        flag.store(true, Ordering::Release);
    });
    crate::wait::until(&gpu.device, Some(index), || mapped.load(Ordering::Acquire));
    let bytes = read
        .slice(..)
        .get_mapped_range()
        .expect("the read buffer maps")
        .to_vec();
    (0..n)
        .flat_map(|y| bytes[(y * ROW) as usize..(y * ROW + n * 4) as usize].to_vec())
        .collect()
}

/// Every texel of a level holding `texel`.
fn level_of(mip: u32, texel: [u8; 4]) -> Vec<u8> {
    let n = N >> mip;
    (0..n * n).flat_map(|_| texel).collect()
}

/// Clears all of `view` to `color` in a pass of its own, resolving into `resolve` where given.
fn clear_raw(
    gpu: &WebGpu,
    view: &wgpu::TextureView,
    resolve: Option<&wgpu::TextureView>,
    load: wgpu::LoadOp<wgpu::Color>,
) {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: resolve,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    }));
    gpu.queue.submit([encoder.finish()]);
}

/// 0.4 in every channel, which `rgba8unorm` stores as [`HELD`].
const POINT_FOUR: wgpu::Color = wgpu::Color {
    r: 0.4,
    g: 0.4,
    b: 0.4,
    a: 0.4,
};

/// A `b5g6r5unorm` target held as `rgba8unorm` (level 1, layer 1 of three levels and three layers)
/// is rounded to its bits there and nowhere else: every texel of that level and layer reads
/// [`ROUNDED`], every texel of the levels and layers around it keeps [`HELD`].
#[test]
fn a_packed_target_is_rounded_to_its_bits_at_its_level_and_layer_alone() {
    let mut gpu = device();
    let usage = wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST
        | wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING;
    let t = raw_texture(&gpu, false, 3, 3, 1, usage);
    for mip in 0..3 {
        for layer in 0..3 {
            fill_level(&gpu, &t, mip, layer, [HELD; 4]);
        }
    }
    fill::quantise(
        &mut gpu,
        &PackedTarget {
            format: Format::B5g6r5Unorm,
            texture: t.clone(),
            mip: 1,
            z: 1,
        },
    );
    for mip in 0..3 {
        for layer in 0..3 {
            let want = if (mip, layer) == (1, 1) {
                ROUNDED
            } else {
                [HELD; 4]
            };
            assert_eq!(
                read_level(&mut gpu, &t, mip, layer),
                level_of(mip, want),
                "level {mip}, layer {layer}"
            );
        }
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A `bgr5a1unorm` target held as `rgba8unorm`, every channel 0.4, is rounded to its bits: red, green
/// and blue round(0.4 31) = 12 = 0b01100, widened to 0b01100011 = 99; alpha round(0.4) = 0.
#[test]
fn a_packed_target_with_one_bit_of_alpha_is_rounded_in_every_channel() {
    let mut gpu = device();
    let t = raw_texture(
        &gpu,
        false,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    fill_level(&gpu, &t, 0, 0, [HELD; 4]);
    fill::quantise(
        &mut gpu,
        &PackedTarget {
            format: Format::Bgr5a1Unorm,
            texture: t.clone(),
            mip: 0,
            z: 0,
        },
    );
    assert_eq!(read_level(&mut gpu, &t, 0, 0), level_of(0, [99, 99, 99, 0]));
    // Alpha 0.6 rounds to one bit set, widened to 255.
    fill_level(&gpu, &t, 0, 0, [HELD, HELD, HELD, 153]);
    fill::quantise(
        &mut gpu,
        &PackedTarget {
            format: Format::Bgr5a1Unorm,
            texture: t.clone(),
            mip: 0,
            z: 0,
        },
    );
    assert_eq!(
        read_level(&mut gpu, &t, 0, 0),
        level_of(0, [99, 99, 99, 255])
    );
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A 3D `b5g6r5unorm` target held as `rgba8unorm` (three slices) is rounded at depth slice 1 alone:
/// that slice reads [`ROUNDED`], slices 0 and 2 keep [`HELD`].
#[test]
fn a_packed_3d_target_is_rounded_at_its_slice_alone() {
    let mut gpu = device();
    let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 3,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    for slice in 0..3 {
        fill_level(&gpu, &t, 0, slice, [HELD; 4]);
    }
    fill::quantise(
        &mut gpu,
        &PackedTarget {
            format: Format::B5g6r5Unorm,
            texture: t.clone(),
            mip: 0,
            z: 1,
        },
    );
    for slice in 0..3 {
        let want = if slice == 1 { ROUNDED } else { [HELD; 4] };
        assert_eq!(
            read_level(&mut gpu, &t, 0, slice),
            level_of(0, want),
            "slice {slice}"
        );
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A four-sample `b5g6r5unorm` target held as `rgba8unorm` is rounded sample by sample: every sample
/// cleared to 0.4 becomes [`ROUNDED`], so its resolve does too.
#[test]
fn a_multisampled_packed_target_is_rounded_in_every_sample() {
    let mut gpu = device();
    let ms = raw_texture(
        &gpu,
        false,
        1,
        1,
        4,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let single = raw_texture(
        &gpu,
        false,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let ms_view = ms.create_view(&Default::default());
    clear_raw(&gpu, &ms_view, None, wgpu::LoadOp::Clear(POINT_FOUR));
    fill::quantise(
        &mut gpu,
        &PackedTarget {
            format: Format::B5g6r5Unorm,
            texture: ms.clone(),
            mip: 0,
            z: 0,
        },
    );
    gpu.submit();
    clear_raw(
        &gpu,
        &ms_view,
        Some(&single.create_view(&Default::default())),
        wgpu::LoadOp::Load,
    );
    assert_eq!(read_level(&mut gpu, &single, 0, 0), level_of(0, ROUNDED));
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// The texel at column `x`, row `y` of a level whose texels all differ: red the column, green the row,
/// blue both, alpha 255 less the column.
fn texel_at(x: u32, y: u32) -> [u8; 4] {
    [
        (x * 30 + 5) as u8,
        (y * 30 + 7) as u8,
        (x * 3 + y * 25) as u8,
        (255 - x) as u8,
    ]
}

/// Every texel of a level of `n` by `n`, each [`texel_at`] its place.
fn positioned(n: u32) -> Vec<u8> {
    (0..n)
        .flat_map(|y| (0..n).flat_map(move |x| texel_at(x, y)))
        .collect()
}

/// A frame drawn into a window texture of another format (`PresentConverted`) holds the frame's
/// colours there: `rgba8unorm` texels into `bgra8unorm`, each channel in its place.
#[test]
fn a_frame_presented_by_a_draw_keeps_its_colours() {
    let mut gpu = device();
    let frame = raw_texture(
        &gpu,
        false,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let window = raw_texture(
        &gpu,
        true,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    fill_level(&gpu, &frame, 0, 0, [10, 20, 30, 255]);
    fill::present_by_draw(&mut gpu, &frame, &window);
    assert_eq!(
        read_level(&mut gpu, &window, 0, 0),
        level_of(0, [30, 20, 10, 255])
    );
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A frame whose texels all differ ([`texel_at`]) presented by a draw holds each texel at its own
/// place in the window texture, its channels in `bgra8unorm`'s order.
#[test]
fn a_frame_presented_by_a_draw_keeps_each_texel_in_its_place() {
    let mut gpu = device();
    let frame = raw_texture(
        &gpu,
        false,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let window = raw_texture(
        &gpu,
        true,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    fill_texels(&gpu, &frame, 0, 0, &positioned(N));
    fill::present_by_draw(&mut gpu, &frame, &window);
    let want: Vec<u8> = (0..N)
        .flat_map(|y| {
            (0..N).flat_map(move |x| {
                let [r, g, b, a] = texel_at(x, y);
                [b, g, r, a]
            })
        })
        .collect();
    assert_eq!(read_level(&mut gpu, &window, 0, 0), want);
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A resolve by a draw (`ResolveByDraw`) writes every sample's colour where all samples hold one:
/// a four-sample texture cleared to (0.4, 0.2, 0.6, 1) resolves to (102, 51, 153, 255).
#[test]
fn a_resolve_by_a_draw_writes_the_samples_colour() {
    let mut gpu = device();
    let ms = raw_texture(
        &gpu,
        false,
        1,
        1,
        4,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let dst = raw_texture(
        &gpu,
        false,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let ms_view = ms.create_view(&Default::default());
    clear_raw(
        &gpu,
        &ms_view,
        None,
        wgpu::LoadOp::Clear(wgpu::Color {
            r: 0.4,
            g: 0.2,
            b: 0.6,
            a: 1.0,
        }),
    );
    fill::resolve_by_draw(
        &mut gpu,
        &ms_view,
        &dst.create_view(&Default::default()),
        wgpu::TextureFormat::Rgba8Unorm,
    );
    assert_eq!(
        read_level(&mut gpu, &dst, 0, 0),
        level_of(0, [102, 51, 153, 255])
    );
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// Red, green, blue and alpha (of 255) each sample `i` of the resolve test is drawn with, before red
/// adds 8 times the column and green 8 times the row: their averages are red 60, green 76, blue 254,
/// alpha 255.
const SAMPLE_BASES: [[u32; 4]; 4] = [
    [0, 200, 255, 255],
    [40, 100, 255, 255],
    [80, 0, 255, 255],
    [120, 4, 251, 255],
];

/// Draws sample `i` of every texel of the four-sample `rgba8unorm` `view` alone (its sample mask)
/// with [`SAMPLE_BASES`]`[i]`, red and green adding 8 times the texel's column and row.
fn draw_samples_apart(gpu: &WebGpu, view: &wgpu::TextureView) {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let pipelines: Vec<wgpu::RenderPipeline> = (0..4)
        .map(|i| {
            let [r, g, b, a] = SAMPLE_BASES[i];
            let module = gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(
                        format!(
                        "@vertex fn v(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn f(@builtin(position) p: vec4f) -> @location(0) vec4f {{
    return vec4f(floor(p.x) * 8.0 + {r}.0, floor(p.y) * 8.0 + {g}.0, {b}.0, {a}.0) / 255.0;
}}"
                    )
                        .into(),
                    ),
                });
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: None,
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 4,
                        mask: 1 << i,
                        alpha_to_coverage_enabled: false,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: None,
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        })
        .collect();
    {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        for pipeline in &pipelines {
            rp.set_pipeline(pipeline);
            rp.draw(0..3, 0..1);
        }
    }
    gpu.queue.submit([encoder.finish()]);
}

/// A resolve by a draw of four samples that all differ, and differ by place, writes each texel the
/// average of its own samples: red 60 and green 76 (each plus 8 times the column, the row), blue 254,
/// alpha 255.
#[test]
fn a_resolve_by_a_draw_averages_samples_that_differ_at_each_place() {
    let mut gpu = device();
    let ms = raw_texture(
        &gpu,
        false,
        1,
        1,
        4,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let dst = raw_texture(
        &gpu,
        false,
        1,
        1,
        1,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let ms_view = ms.create_view(&Default::default());
    draw_samples_apart(&gpu, &ms_view);
    fill::resolve_by_draw(
        &mut gpu,
        &ms_view,
        &dst.create_view(&Default::default()),
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let want: Vec<u8> = (0..N)
        .flat_map(|y| (0..N).flat_map(move |x| [(60 + 8 * x) as u8, (76 + 8 * y) as u8, 254, 255]))
        .collect();
    assert_eq!(read_level(&mut gpu, &dst, 0, 0), want);
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

fn texture(gpu: &mut WebGpu, format: Format, layers: u32, mips: u32, usage: TextureUsage) -> Tex {
    gpu.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: N,
        height: N,
        depth_or_layers: layers,
        mip_levels: mips,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    })
    .expect("create_texture")
}

fn at(texture: &Tex, mip: u32, layer: u32, stencil: bool) -> TextureRegion<'_, WebGpu> {
    TextureRegion {
        texture,
        mip,
        layer,
        origin: Origin { x: 0, y: 0, z: 0 },
        stencil,
    }
}

fn extent(n: u32, layers: u32) -> Extent {
    Extent {
        width: n,
        height: n,
        depth_or_layers: layers,
    }
}

fn read_back(
    gpu: &mut WebGpu,
    region: TextureRegion<'_, WebGpu>,
    extent: Extent,
    texel: u32,
) -> Vec<u8> {
    let rb = gpu
        .read_back(&ReadSource::Texture { region, extent })
        .expect("read_back");
    gpu.submit();
    gpu.wait_read(&rb);
    let mut out = vec![0; (extent.width * extent.height * extent.depth_or_layers * texel) as usize];
    gpu.finish_read(rb, &mut out);
    out
}

fn upload(gpu: &mut WebGpu, bytes: &[u8]) -> <WebGpu as Gpu>::Buffer {
    let buffer = gpu
        .create_buffer(&BufferDesc {
            size: bytes.len().next_multiple_of(4) as u64,
            usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    gpu.write_buffer(&buffer, 0, bytes, WriteMode::Ordered);
    buffer
}

/// Reads `size` bytes of `buffer` from its start.
fn read_buffer(gpu: &mut WebGpu, buffer: &<WebGpu as Gpu>::Buffer, size: u64) -> Vec<u8> {
    let rb = gpu
        .read_back(&ReadSource::Buffer {
            buffer,
            offset: 0,
            size,
        })
        .expect("read_back");
    gpu.submit();
    gpu.wait_read(&rb);
    let mut out = vec![0; size as usize];
    gpu.finish_read(rb, &mut out);
    out
}

/// A converted format (`b5g6r5unorm`, held as `rgba8unorm`) copied from a buffer into level 1,
/// layer 1 of a texture of three levels and three layers, and copied from there into a buffer (both
/// draws, into that level and layer and from it), comes back as written; the level and layer around
/// it keep their zeros.
#[test]
fn a_converted_format_copies_into_and_out_of_one_level_and_layer() {
    let mut gpu = device();
    let t = texture(
        &mut gpu,
        Format::B5g6r5Unorm,
        3,
        3,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    );
    let n = N >> 1;
    let bytes: Vec<u8> = (0..n * n * 2).map(|i| (i * 37 + 11) as u8).collect();
    let src = upload(&mut gpu, &bytes);
    let copy = |buffer| CopyBufferTexture {
        buffer,
        offset: 0,
        row_pitch: n * 2,
        rows_per_layer: n,
    };
    gpu.copy_buffer_to_texture(&copy(&src), &at(&t, 1, 1, false), extent(n, 1));
    let dst = upload(&mut gpu, &vec![0; bytes.len()]);
    gpu.copy_texture_to_buffer(&at(&t, 1, 1, false), &copy(&dst), extent(n, 1));
    assert_eq!(read_buffer(&mut gpu, &dst, bytes.len() as u64), bytes);
    assert_eq!(
        read_back(&mut gpu, at(&t, 1, 0, false), extent(n, 1), 2),
        vec![0; bytes.len()],
        "level 1, layer 0"
    );
    assert_eq!(
        read_back(&mut gpu, at(&t, 0, 1, false), extent(N, 1), 2),
        vec![0; (N * N * 2) as usize],
        "level 0, layer 1"
    );
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// Reads level 0, layer 0 of `texture` (`texel` bytes a texel), row after row.
fn read_raw(gpu: &mut WebGpu, texture: &wgpu::Texture, texel: u32) -> Vec<u8> {
    gpu.submit();
    let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(ROW * N),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW),
                rows_per_image: Some(N),
            },
        },
        wgpu::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
    );
    let index = gpu.queue.submit([encoder.finish()]);
    let mapped = Arc::new(AtomicBool::new(false));
    let flag = mapped.clone();
    read.slice(..).map_async(wgpu::MapMode::Read, move |_| {
        flag.store(true, Ordering::Release);
    });
    crate::wait::until(&gpu.device, Some(index), || mapped.load(Ordering::Acquire));
    let bytes = read
        .slice(..)
        .get_mapped_range()
        .expect("the read buffer maps")
        .to_vec();
    (0..N)
        .flat_map(|y| bytes[(y * ROW) as usize..(y * ROW + N * texel) as usize].to_vec())
        .collect()
}

/// Every converted format (the 8-bit alpha, BGRX, 16-bit packed, two-pixel and three-channel
/// 32-bit ones): blocks of distinct bytes copied from a buffer into a region off the origin, at a
/// column and a row that differ (by a draw widening them), leave in the held texture what the host
/// widens those blocks to there and zero elsewhere; and that region copied into a buffer (by a draw
/// packing it) gives the blocks back, but for BGRX's padding byte, which is packed as 255.
#[test]
fn converted_formats_copy_by_draws_as_the_host_widens_and_packs() {
    use crate::packed;
    let mut gpu = device();
    let (x, y, w, h) = (2u32, 1u32, 4u32, 2u32);
    for format in [
        Format::A8Unorm,
        Format::Bgrx8Unorm,
        Format::Bgrx8UnormSrgb,
        Format::B5g6r5Unorm,
        Format::Bgr5a1Unorm,
        Format::Bgra4Unorm,
        Format::Rgbg8Unorm,
        Format::Grgb8Unorm,
        Format::Rgb32Float,
        Format::Rgb32Uint,
        Format::Rgb32Sint,
    ] {
        let t = texture(
            &mut gpu,
            format,
            1,
            1,
            TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
        );
        let row = packed::packed_bytes(format, u64::from(w)) as u32;
        // Distinct bytes, no float of them a NaN (a draw may make a NaN canonical).
        let blocks: Vec<u8> = (0..row * h).map(|i| (i * 29 + 7) as u8 & 0x7f).collect();
        let src = upload(&mut gpu, &blocks);
        let region = TextureRegion {
            origin: Origin { x, y, z: 0 },
            ..at(&t, 0, 0, false)
        };
        let copy = |buffer| CopyBufferTexture {
            buffer,
            offset: 0,
            row_pitch: row,
            rows_per_layer: h,
        };
        let size = Extent {
            width: w,
            height: h,
            depth_or_layers: 1,
        };
        gpu.copy_buffer_to_texture(&copy(&src), &region, size);
        let held = packed::held_bytes(format);
        let mut want = vec![0; (N * N * held) as usize];
        let mut packed_back = Vec::new();
        for r in 0..h {
            let mut texels = Vec::new();
            packed::widen(
                format,
                &blocks[(r * row) as usize..((r + 1) * row) as usize],
                &mut texels,
            );
            let at = (((y + r) * N + x) * held) as usize;
            want[at..at + texels.len()].copy_from_slice(&texels);
            packed::pack(format, &texels, &mut packed_back);
        }
        let padded = matches!(format, Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb);
        assert_eq!(
            packed_back,
            blocks
                .iter()
                .enumerate()
                .map(|(i, &b)| if padded && i % 4 == 3 { u8::MAX } else { b })
                .collect::<Vec<u8>>(),
            "{format:?} packed on the host"
        );
        let raw = crate::texture_of(&gpu, &t).expect("held").raw();
        assert_eq!(
            read_raw(&mut gpu, &raw, held),
            want,
            "{format:?} widened by a draw"
        );
        let dst = upload(&mut gpu, &vec![0; blocks.len()]);
        gpu.copy_texture_to_buffer(&region, &copy(&dst), size);
        assert_eq!(
            read_buffer(&mut gpu, &dst, blocks.len() as u64),
            packed_back,
            "{format:?} packed by a draw"
        );
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A region of an `rgb32uint` texture off the origin copied into its typeless twin `rgb32float` at
/// another place (by draws through their raw words, each at a column and a row that differ) holds
/// each word at its new place, and the twin's alpha one; everything else stays zero.
#[test]
fn three_channel_formats_copy_into_their_twins_word_for_word() {
    let mut gpu = device();
    let usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
    let src = texture(&mut gpu, Format::Rgb32Uint, 1, 1, usage);
    let dst = texture(&mut gpu, Format::Rgb32Float, 1, 1, usage);
    // Distinct words, none of them a NaN or a denormal as a float.
    let word = |x: u32, y: u32, c: u32| 0x3f00_0000 | (y * N + x) << 4 | c;
    let words: Vec<u8> = (0..N)
        .flat_map(|y| {
            (0..N).flat_map(move |x| (0..3).flat_map(move |c| word(x, y, c).to_le_bytes()))
        })
        .collect();
    gpu.write_texture(
        &at(&src, 0, 0, false),
        extent(N, 1),
        &words,
        N * 12,
        N,
        WriteMode::Ordered,
    );
    let (w, h) = (4, 2);
    let from = TextureRegion {
        origin: Origin { x: 2, y: 1, z: 0 },
        ..at(&src, 0, 0, false)
    };
    let to = TextureRegion {
        origin: Origin { x: 1, y: 3, z: 0 },
        ..at(&dst, 0, 0, false)
    };
    gpu.copy_texture(
        &from,
        &to,
        Extent {
            width: w,
            height: h,
            depth_or_layers: 1,
        },
    );
    let mut want = vec![0u8; (N * N * 16) as usize];
    for r in 0..h {
        for i in 0..w {
            let at = (((3 + r) * N + 1 + i) * 16) as usize;
            let texel: Vec<u8> = (0..3)
                .map(|c| word(2 + i, 1 + r, c))
                .chain([1f32.to_bits()])
                .flat_map(u32::to_le_bytes)
                .collect();
            want[at..at + 16].copy_from_slice(&texel);
        }
    }
    let raw = crate::texture_of(&gpu, &dst).expect("held").raw();
    assert_eq!(read_raw(&mut gpu, &raw, 16), want);
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A `b5g6r5unorm` and a `bgr5a1unorm` target held as `rgba8unorm`, whose texels all differ by place
/// and by channel ([`texel_at`]), are rounded texel by texel and channel by channel to what the host's
/// packing and widening make of each texel.
#[test]
fn packed_targets_are_rounded_at_each_texel_and_channel() {
    use crate::packed;
    let mut gpu = device();
    let usage = wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST
        | wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING;
    for format in [Format::B5g6r5Unorm, Format::Bgr5a1Unorm] {
        let t = raw_texture(&gpu, false, 1, 1, 1, usage);
        let texels = positioned(N);
        fill_texels(&gpu, &t, 0, 0, &texels);
        fill::quantise(
            &mut gpu,
            &PackedTarget {
                format,
                texture: t.clone(),
                mip: 0,
                z: 0,
            },
        );
        let (mut blocks, mut want) = (Vec::new(), Vec::new());
        packed::pack(format, &texels, &mut blocks);
        packed::widen(format, &blocks, &mut want);
        assert_eq!(read_level(&mut gpu, &t, 0, 0), want, "{format:?}");
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// Every converted format's held texels holding every byte value in every channel, at a region off
/// the origin reaching the right edge of a texture 7 texels wide (so a two-pixel format's last pair
/// is one texel), copied into a buffer (by a draw packing them) give the blocks the host packs them
/// to: each rounded to its bits as the host rounds (every 8-bit value, so every rounding).
#[test]
fn converted_formats_pack_every_held_value_as_the_host_does() {
    use crate::packed;
    let mut gpu = device();
    let (wide, high) = (7u32, 53u32);
    let (x, y, w, h) = (2u32, 1u32, 5u32, 52u32);
    for format in [
        Format::A8Unorm,
        Format::Bgrx8Unorm,
        Format::B5g6r5Unorm,
        Format::Bgr5a1Unorm,
        Format::Bgra4Unorm,
        Format::Rgbg8Unorm,
        Format::Grgb8Unorm,
    ] {
        let t = gpu
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: wide,
                height: high,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .expect("create_texture");
        // Byte k of the region's texel r (row after row) holds (r + 64 k) mod 256: the region's 260
        // texels hold every value in every channel; the rest of the texture holds zero.
        let held: Vec<u8> = (0..high)
            .flat_map(|ty| (0..wide).map(move |tx| (tx, ty)))
            .flat_map(|(tx, ty)| {
                let inside = (x..x + w).contains(&tx) && (y..y + h).contains(&ty);
                let r = if inside { (ty - y) * w + tx - x } else { 0 };
                (0..4).map(move |k| if inside { (r + 64 * k) as u8 } else { 0 })
            })
            .collect();
        let raw = crate::texture_of(&gpu, &t).expect("held").raw();
        gpu.queue.write_texture(
            raw.as_image_copy(),
            &held,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(wide * 4),
                rows_per_image: Some(high),
            },
            wgpu::Extent3d {
                width: wide,
                height: high,
                depth_or_array_layers: 1,
            },
        );
        let row = packed::packed_bytes(format, u64::from(w)) as u32;
        let mut want = Vec::new();
        for r in 0..h {
            let at = (((y + r) * wide + x) * 4) as usize;
            packed::pack(format, &held[at..at + (w * 4) as usize], &mut want);
        }
        let dst = upload(&mut gpu, &vec![0; want.len()]);
        gpu.copy_texture_to_buffer(
            &TextureRegion {
                origin: Origin { x, y, z: 0 },
                ..at(&t, 0, 0, false)
            },
            &CopyBufferTexture {
                buffer: &dst,
                offset: 0,
                row_pitch: row,
                rows_per_layer: h,
            },
            Extent {
                width: w,
                height: h,
                depth_or_layers: 1,
            },
        );
        assert_eq!(
            read_buffer(&mut gpu, &dst, want.len() as u64),
            want,
            "{format:?}"
        );
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A converted format's blocks (`b5g6r5unorm`'s, and the three-channel `rgb32uint`'s) copied from a
/// buffer into slice 1 of a 3D texture at a region off the origin (a column and a row that differ),
/// and copied from there into a buffer (by draws widening and packing them), come back as written.
#[test]
fn a_converted_3d_region_copies_in_and_out_at_its_place() {
    use crate::packed;
    let mut gpu = device();
    for format in [Format::B5g6r5Unorm, Format::Rgb32Uint] {
        let t = gpu
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D3,
                format,
                width: N,
                height: N,
                depth_or_layers: 2,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .expect("create_texture");
        let (w, h) = (4u32, 2u32);
        let row = packed::packed_bytes(format, u64::from(w)) as u32;
        let blocks: Vec<u8> = (0..row * h).map(|i| (i * 37 + 11) as u8).collect();
        let src = upload(&mut gpu, &blocks);
        let copy = |buffer| CopyBufferTexture {
            buffer,
            offset: 0,
            row_pitch: row,
            rows_per_layer: h,
        };
        let region = TextureRegion {
            origin: Origin { x: 3, y: 1, z: 1 },
            ..at(&t, 0, 0, false)
        };
        let size = Extent {
            width: w,
            height: h,
            depth_or_layers: 1,
        };
        gpu.copy_buffer_to_texture(&copy(&src), &region, size);
        let dst = upload(&mut gpu, &vec![0; blocks.len()]);
        gpu.copy_texture_to_buffer(&region, &copy(&dst), size);
        assert_eq!(
            read_buffer(&mut gpu, &dst, blocks.len() as u64),
            blocks,
            "{format:?}"
        );
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// Depth and stencil written from a buffer into layers 1 and 2 of a three-layer texture (by draws:
/// a part of a level) read back as written from those layers, by draws reading part of a level: each
/// layer from its own rows of the buffer.
#[test]
fn depth_and_stencil_by_draws_reach_each_layer() {
    let mut gpu = device();
    let format = Format::Depth32FloatStencil8;
    if !gpu.caps().formats.uses[format as usize]
        .contains(wcontainers_gpu::caps::FormatUse::DEPTH_STENCIL)
    {
        eprintln!("{format:?}: not held by this device");
        return;
    }
    let t = texture(
        &mut gpu,
        format,
        3,
        1,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST | TextureUsage::RENDER,
    );
    // A part of each layer, off the origin, so that each copy is a draw.
    let part = |layer, stencil| TextureRegion {
        origin: Origin { x: 1, y: 2, z: 0 },
        ..at(&t, 0, layer, stencil)
    };
    let (w, h) = (5, 3);
    let size = Extent {
        width: w,
        height: h,
        depth_or_layers: 2,
    };
    let stencil: Vec<u8> = (0..w * h * 2).map(|i| (i * 29 + 3) as u8).collect();
    let words: Vec<u32> = (0..w * h * 2)
        .map(|i| (i as f32 / 64.0).to_bits())
        .collect();
    let depth: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    for (bytes, stencil_aspect, texel) in [(&stencil, true, 1), (&depth, false, 4)] {
        let buffer = upload(&mut gpu, bytes);
        gpu.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: &buffer,
                offset: 0,
                row_pitch: w * texel,
                rows_per_layer: h,
            },
            &part(1, stencil_aspect),
            size,
        );
        assert_eq!(
            &read_back(&mut gpu, part(1, stencil_aspect), size, texel),
            bytes,
            "stencil {stencil_aspect}"
        );
    }
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// Each program of fill.rs is made at its first use and counted by its pipelines (one, nine for the
/// stencil draws: one a bit and the clear; the integer clear's once a target layout); the same use
/// again makes none.
#[test]
fn each_fill_program_is_made_once_and_counted() {
    let mut gpu = device();
    let made = |gpu: &mut WebGpu, step: &mut dyn FnMut(&mut WebGpu)| {
        let before = gpu.counters().pipelines;
        step(gpu);
        gpu.submit();
        gpu.counters().pipelines - before
    };
    let format = Format::Depth32FloatStencil8;
    let t = texture(
        &mut gpu,
        format,
        1,
        1,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST | TextureUsage::RENDER,
    );
    let part = |stencil| TextureRegion {
        origin: Origin { x: 1, y: 1, z: 0 },
        ..at(&t, 0, 0, stencil)
    };
    let words: Vec<u8> = (0..16u32)
        .flat_map(|i| (i as f32 / 16.0).to_bits().to_le_bytes())
        .collect();
    let buffer = upload(&mut gpu, &words);
    let copy = |texel: u32| CopyBufferTexture {
        buffer: &buffer,
        offset: 0,
        row_pitch: 4 * texel,
        rows_per_layer: 4,
    };
    let mut depth_write =
        |gpu: &mut WebGpu| gpu.copy_buffer_to_texture(&copy(4), &part(false), extent(4, 1));
    assert_eq!(made(&mut gpu, &mut depth_write), 1, "the depth draw");
    assert_eq!(made(&mut gpu, &mut depth_write), 0, "the depth draw again");
    let mut stencil_write =
        |gpu: &mut WebGpu| gpu.copy_buffer_to_texture(&copy(1), &part(true), extent(4, 1));
    assert_eq!(made(&mut gpu, &mut stencil_write), 9, "the stencil draws");
    assert_eq!(
        made(&mut gpu, &mut stencil_write),
        0,
        "the stencil draws again"
    );
    let mut depth_read = |gpu: &mut WebGpu| {
        read_back(gpu, part(false), extent(4, 1), 4);
    };
    assert_eq!(made(&mut gpu, &mut depth_read), 1, "the depth read");
    assert_eq!(made(&mut gpu, &mut depth_read), 0, "the depth read again");
    let packed = texture(
        &mut gpu,
        Format::B5g6r5Unorm,
        1,
        1,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    );
    let packed_buffer = upload(&mut gpu, &[7; 32]);
    let packed_copy = CopyBufferTexture {
        buffer: &packed_buffer,
        offset: 0,
        row_pitch: 8,
        rows_per_layer: 4,
    };
    let mut packed_write = |gpu: &mut WebGpu| {
        gpu.copy_buffer_to_texture(&packed_copy, &at(&packed, 0, 0, false), extent(4, 1));
    };
    assert_eq!(
        made(&mut gpu, &mut packed_write),
        1,
        "the packing draw into"
    );
    assert_eq!(
        made(&mut gpu, &mut packed_write),
        0,
        "the packing draw into again"
    );
    let mut packed_read = |gpu: &mut WebGpu| {
        gpu.copy_texture_to_buffer(&at(&packed, 0, 0, false), &packed_copy, extent(4, 1));
    };
    assert_eq!(made(&mut gpu, &mut packed_read), 1, "the packing draw from");
    assert_eq!(
        made(&mut gpu, &mut packed_read),
        0,
        "the packing draw from again"
    );
    gpu.float_clears = true;
    let target = texture(&mut gpu, Format::R32Uint, 1, 1, TextureUsage::RENDER);
    let view = gpu
        .create_view(
            &target,
            &TextureViewDesc {
                format: Format::R32Uint,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .expect("create_view");
    let mut clear = |gpu: &mut WebGpu| {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &view,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear([0xffff_ffff, 0, 0, 0]),
            store: StoreOp::Store,
        });
        gpu.begin_pass(&pass);
        gpu.end_pass();
    };
    assert_eq!(made(&mut gpu, &mut clear), 1, "the clear draw");
    assert_eq!(made(&mut gpu, &mut clear), 0, "the clear draw again");
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// Clears a back end rounding integer clears through a float makes by draws: 32-bit integers past
/// 2^24 and a float's NaN bits each clear to the exact value, counted as the slow path of their kind,
/// and an integer clear of a four-sample target is drawn at its sample count.
#[test]
fn clears_by_draws_write_the_exact_value_and_count_their_kind() {
    let mut gpu = device();
    gpu.float_clears = true;
    let cases: [(Format, [u32; 4], SlowPath); 3] = [
        (
            Format::Rgba32Uint,
            [0xffff_ffff, (1 << 25) + 1, 7, 0],
            SlowPath::IntegerClearByDraw,
        ),
        (
            Format::Rgba32Sint,
            [(-(1i32 << 25) - 3) as u32, 5, (1 << 25) + 1, u32::MAX],
            SlowPath::IntegerClearByDraw,
        ),
        (
            Format::R32Float,
            [0x7fc0_0001, 0, 0, 0],
            SlowPath::FloatBitsClearByDraw,
        ),
    ];
    for (format, bits, path) in cases {
        let (_, _, texel) = wcontainers_gpu::format::FORMAT_FACTS[format as usize].block;
        let t = texture(
            &mut gpu,
            format,
            1,
            1,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        );
        let v = gpu
            .create_view(
                &t,
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
            .expect("create_view");
        let before = gpu.slow_paths().counts;
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &v,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear(bits),
            store: StoreOp::Store,
        });
        gpu.begin_pass(&pass);
        gpu.end_pass();
        let after = gpu.slow_paths().counts;
        assert_eq!(
            [SlowPath::IntegerClearByDraw, SlowPath::FloatBitsClearByDraw]
                .map(|p| after[p as usize] - before[p as usize]),
            [
                u64::from(path == SlowPath::IntegerClearByDraw),
                u64::from(path == SlowPath::FloatBitsClearByDraw)
            ],
            "{format:?}"
        );
        let want: Vec<u8> = (0..N * N)
            .flat_map(|_| {
                bits.iter()
                    .flat_map(|b| b.to_le_bytes())
                    .take(texel as usize)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(
            read_back(&mut gpu, at(&t, 0, 0, false), extent(N, 1), texel),
            want,
            "{format:?}"
        );
    }
    // Four samples: the clear's pipeline is made at the target's sample count.
    let ms = gpu
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::R32Uint,
            width: N,
            height: N,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 4,
            usage: TextureUsage::RENDER,
            cube: false,
            reinterpret: false,
        })
        .expect("a four-sample r32uint target");
    let v = gpu
        .create_view(
            &ms,
            &TextureViewDesc {
                format: Format::R32Uint,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .expect("create_view");
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &v,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear([0xffff_ffff, 0, 0, 0]),
        store: StoreOp::Store,
    });
    let before = gpu.slow_paths().counts[SlowPath::IntegerClearByDraw as usize];
    gpu.begin_pass(&pass);
    gpu.end_pass();
    gpu.submit();
    assert_eq!(
        gpu.slow_paths().counts[SlowPath::IntegerClearByDraw as usize] - before,
        1
    );
    assert_eq!(gpu.status(), DeviceStatus::Ok);
}

/// A texture copied from is made bindable for a copy by a draw only where a depth copy reads its
/// format (`fill::DepthSource::readable`): an `r32uint` texture is, an `rgba8unorm` one is not.
#[test]
fn only_formats_a_depth_copy_reads_are_made_bindable_for_copies() {
    let mut gpu = device();
    for (format, bindable) in [(Format::R32Uint, true), (Format::Rgba8Unorm, false)] {
        let t = texture(&mut gpu, format, 1, 1, TextureUsage::COPY_SRC);
        let raw = crate::texture_of(&gpu, &t).expect("held").raw();
        assert_eq!(
            raw.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING),
            bindable,
            "{format:?}"
        );
    }
}
