//! What the back end does where WebGPU has no native route, against real Direct3D 11 on the same GPU:
//! `generate_mips` of `r32float` on a device without `float32-filterable` and of `rgba16uint` holding
//! `rgba16unorm` values (both filtered in the program) against GenerateMips, and a multisampled
//! `rgba8unorm` texture made with `reinterpret` resolved as `rgba8unorm-srgb` against
//! ResolveSubresource with `R8G8B8A8_UNORM_SRGB`.
//!
//! Windows only: needs a GPU and Direct3D 11 (the rig's device and shaders, test-only); `WEBGPU_API`
//! as in tests/scenes.rs.

#![cfg(windows)]

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use wcontainers_gpu::{
    Backend, ColorAttachment, DeviceStatus, DrawCall, DrawRange, Extent, Features, Format,
    FormatUse, LoadOp, Origin, PassDesc, ReadSource, SlowPath, StepMode, StoreOp, TextureDesc,
    TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, Topology, VertexBinding,
    VertexFormat, ViewDimension, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;
use wcontainers_graphics_test::Driver;
use wcontainers_graphics_test::d3d11::scenes::{Colored, colored_program, draw_colored, hue};
use wcontainers_graphics_test::d3d11::{self, Gpu, solid, tex2d};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

type B = WebGpu;

/// The level 0 the mip chains start from: the rig's mips scene's size, odd further down.
const MIP_SIZE: (u32, u32) = (64, 48);
const MIP_LEVELS: u32 = 7;

fn backends() -> wgpu::Backends {
    match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        _ => wgpu::Backends::PRIMARY,
    }
}

fn device_without(features: wgpu::Features) -> B {
    pollster::block_on(draw::device_without(&draw::instance(backends()), features))
        .expect("a GPU adapter")
        .0
}

fn direct3d() -> Gpu {
    d3d11::create(Driver::Hardware, false, &[D3D_FEATURE_LEVEL_11_0]).expect("a Direct3D 11 device")
}

/// The texels of one subresource of a Direct3D texture, rows packed.
fn d3d_read(gpu: &Gpu, texture: &ID3D11Texture2D, mip: u32, texel: usize) -> Vec<u8> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    let (width, height) = ((desc.Width >> mip).max(1), (desc.Height >> mip).max(1));
    let staging = gpu
        .texture2d(
            &D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                ..desc
            },
            &[],
        )
        .unwrap();
    let row = width as usize * texel;
    let mut out = Vec::with_capacity(row * height as usize);
    unsafe {
        gpu.context
            .CopySubresourceRegion(&staging, 0, 0, 0, 0, texture, mip, None);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        for y in 0..height as usize {
            let at = mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize);
            out.extend_from_slice(std::slice::from_raw_parts(at, row));
        }
        gpu.context.Unmap(&staging, 0);
    }
    out
}

/// Direct3D 11's GenerateMips of `level0` in `format`: every level, rows packed.
fn d3d_mips(format: DXGI_FORMAT, texel: usize, level0: &[u8]) -> Vec<Vec<u8>> {
    let gpu = direct3d();
    let mut desc = tex2d(
        MIP_SIZE.0,
        MIP_SIZE.1,
        format,
        D3D11_BIND_FLAG(D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0),
    );
    desc.MipLevels = MIP_LEVELS;
    desc.MiscFlags = D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32;
    let texture = gpu.texture2d(&desc, &[]).unwrap();
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    unsafe {
        gpu.context.UpdateSubresource(
            &texture,
            0,
            None,
            level0.as_ptr().cast(),
            MIP_SIZE.0 * texel as u32,
            0,
        );
        gpu.context.GenerateMips(&view);
    }
    (0..MIP_LEVELS)
        .map(|mip| d3d_read(&gpu, &texture, mip, texel))
        .collect()
}

/// Reads one level of a texture back through the back end.
fn read(b: &mut B, texture: &<B as Backend>::Texture, mip: u32, len: usize) -> Vec<u8> {
    let size = |n: u32| (n >> mip).max(1);
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture,
                mip,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: size(MIP_SIZE.0),
                height: size(MIP_SIZE.1),
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; len];
    b.finish_read(rb, &mut out);
    out
}

/// The back end's `generate_mips` in a texture of `format` holding `values`, each level made from
/// Direct3D's level above it (`reference`), so a difference does not carry down the chain: levels 1
/// and below.
fn our_mips(
    b: &mut B,
    format: Format,
    values: Format,
    texel: usize,
    reference: &[Vec<u8>],
) -> Vec<Vec<u8>> {
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: MIP_SIZE.0,
        height: MIP_SIZE.1,
        depth_or_layers: 1,
        mip_levels: MIP_LEVELS,
        samples: 1,
        usage: TextureUsage::RENDER
            | TextureUsage::SAMPLE
            | TextureUsage::COPY_SRC
            | TextureUsage::COPY_DST,
        cube: false,
        reinterpret: false,
    };
    let texture = b.create_texture(&desc).unwrap();
    let size = |n: u32, mip: u32| (n >> mip).max(1);
    (1..MIP_LEVELS)
        .map(|mip| {
            let (w, h) = (size(MIP_SIZE.0, mip - 1), size(MIP_SIZE.1, mip - 1));
            b.write_texture(
                &TextureRegion {
                    texture: &texture,
                    mip: mip - 1,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                Extent {
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                },
                &reference[mip as usize - 1],
                w * texel as u32,
                h,
                WriteMode::Ordered,
            );
            b.generate_mips(&texture, &desc, values, mip..mip + 1, 0..1)
                .unwrap();
            let len = (size(MIP_SIZE.0, mip) * size(MIP_SIZE.1, mip)) as usize * texel;
            read(b, &texture, mip, len)
        })
        .collect()
}

/// Seeded texels: `count` values from `next`.
fn seeded<T>(count: usize, mut next: impl FnMut(&mut fastrand::Rng) -> T) -> Vec<T> {
    let mut rng = fastrand::Rng::with_seed(0x6d1b5);
    (0..count).map(|_| next(&mut rng)).collect()
}

/// The largest difference of each level below the first, by `diff`, on a device made without
/// `without` (the program's filter, `values` held in `format`) and on one with every feature (the
/// sampler's, in `values` itself). Prints both and returns the first device's.
fn mips_against_direct3d(
    without: wgpu::Features,
    format: Format,
    values: Format,
    texel: usize,
    reference: &[Vec<u8>],
    diff: impl Fn(&[u8], &[u8]) -> f64,
) -> Vec<f64> {
    let mut worst = Vec::new();
    for (route, features) in [
        ("in the program", without),
        ("by the sampler", wgpu::Features::empty()),
    ] {
        let mut b = device_without(features);
        let filtered = b.caps().formats.get(values).contains(FormatUse::FILTER);
        // With the feature, in the format itself, as Direct3D holds it.
        let held = if filtered { values } else { format };
        let ours = our_mips(&mut b, held, values, texel, reference);
        let levels: Vec<f64> = ours
            .iter()
            .zip(&reference[1..])
            .map(|(got, d3d)| diff(d3d, got))
            .collect();
        eprintln!(
            "{values:?} in {held:?}, filtered {route}: largest difference per level {levels:?}"
        );
        assert_eq!(b.status(), DeviceStatus::Ok);
        if worst.is_empty() {
            assert!(!filtered);
            assert!(b.slow_paths().count(SlowPath::FilterInShader) > 0);
            worst = levels;
        }
    }
    worst
}

#[test]
fn generate_mips_of_r32float_without_filtering_match_direct3d() {
    let texels = (MIP_SIZE.0 * MIP_SIZE.1) as usize;
    let values = seeded(texels, |r| r.f32() * 8.0 - 2.0);
    let reference = d3d_mips(DXGI_FORMAT_R32_FLOAT, 4, bytemuck::cast_slice(&values));
    let worst = mips_against_direct3d(
        wgpu::Features::FLOAT32_FILTERABLE
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        Format::R32Float,
        Format::R32Float,
        4,
        &reference,
        |d3d, got| {
            let (d3d, got): (&[f32], &[f32]) =
                (bytemuck::cast_slice(d3d), bytemuck::cast_slice(got));
            d3d.iter()
                .zip(got)
                .map(|(a, b)| f64::from((a - b).abs() / a.abs().max(1.0)))
                .fold(0.0, f64::max)
        },
    );
    // Relative to the value (at least 1): the program averages in 32-bit floats; the sampler's
    // filter may use fewer bits for its weights and sums.
    for (level, w) in worst.iter().enumerate() {
        assert!(*w <= 1.0 / 65536.0, "level {}: {w:e}", level + 1);
    }
}

#[test]
fn generate_mips_of_unorm_values_in_rgba16uint_match_direct3d() {
    let texels = (MIP_SIZE.0 * MIP_SIZE.1 * 4) as usize;
    let values = seeded(texels, |r| r.u16(..));
    let reference = d3d_mips(
        DXGI_FORMAT_R16G16B16A16_UNORM,
        8,
        bytemuck::cast_slice(&values),
    );
    let worst = mips_against_direct3d(
        wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        Format::Rgba16Uint,
        Format::Rgba16Unorm,
        8,
        &reference,
        |d3d, got| {
            let (d3d, got): (&[u16], &[u16]) =
                (bytemuck::cast_slice(d3d), bytemuck::cast_slice(got));
            d3d.iter()
                .zip(got)
                .map(|(a, b)| f64::from(a.abs_diff(*b)))
                .fold(0.0, f64::max)
        },
    );
    // In steps of the format: the average of four values rounds to the nearest step.
    for (level, w) in worst.iter().enumerate() {
        assert!(*w <= 1.0, "level {}: {w}", level + 1);
    }
}

const RESOLVE_SIZE: u32 = 64;

/// Wedges of five colours with half-transparent alpha, whose edges cross pixels at many angles.
fn wedges() -> Vec<Colored> {
    let mut v = Vec::new();
    for k in 0..5u32 {
        let a0 = k as f32 * 0.31 + 0.05;
        let a1 = a0 + 0.12;
        let [r, g, b] = hue(k, 5);
        let p = |a: f32, d: f32| [d * a.cos() - 0.9, d * a.sin() - 0.9, 0.5, r, g, b, 0.5];
        v.extend([p(a0, 0.1), p(a1, 1.9), p(a0, 1.9)]);
    }
    v
}

/// Direct3D 11: the wedges drawn into a four-sample `R8G8B8A8_TYPELESS` texture through a unorm
/// view, resolved with `R8G8B8A8_UNORM_SRGB`.
fn d3d_srgb_resolve() -> Vec<u8> {
    let gpu = direct3d();
    let mut desc = tex2d(
        RESOLVE_SIZE,
        RESOLVE_SIZE,
        DXGI_FORMAT_R8G8B8A8_TYPELESS,
        D3D11_BIND_RENDER_TARGET,
    );
    desc.SampleDesc = DXGI_SAMPLE_DESC {
        Count: 4,
        Quality: D3D11_STANDARD_MULTISAMPLE_PATTERN.0 as u32,
    };
    let samples = gpu.texture2d(&desc, &[]).unwrap();
    let rtv = gpu
        .rtv(
            &samples.cast().unwrap(),
            Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2DMS,
                ..Default::default()
            }),
        )
        .unwrap();
    gpu.begin(&rtv, None, RESOLVE_SIZE, RESOLVE_SIZE);
    colored_program(&gpu).unwrap();
    let raster = gpu
        .rasterizer(&D3D11_RASTERIZER_DESC {
            MultisampleEnable: true.into(),
            ..solid()
        })
        .unwrap();
    unsafe { gpu.context.RSSetState(&raster) };
    draw_colored(&gpu, &wedges()).unwrap();
    let single = gpu
        .texture2d(
            &tex2d(
                RESOLVE_SIZE,
                RESOLVE_SIZE,
                DXGI_FORMAT_R8G8B8A8_TYPELESS,
                D3D11_BIND_FLAG(0),
            ),
            &[],
        )
        .unwrap();
    unsafe {
        gpu.context
            .ResolveSubresource(&single, 0, &samples, 0, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB)
    };
    d3d_read(&gpu, &single, 0, 4)
}

const COLORED_WGSL: &str = "
struct VsOut { @builtin(position) pos: vec4f, @location(0) color: vec4f }
@vertex fn vs(@location(0) p: vec3f, @location(1) c: vec4f) -> VsOut { return VsOut(vec4f(p, 1.0), c); }
@fragment fn ps(i: VsOut) -> @location(0) vec4f { return i.color; }
";

/// The back end: the same wedges drawn into a four-sample `rgba8unorm` texture made with
/// `reinterpret`, resolved as `format` into another.
fn our_resolve(b: &mut B, format: Format) -> Vec<u8> {
    let texture = |b: &mut B, samples, usage| {
        b.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: RESOLVE_SIZE,
            height: RESOLVE_SIZE,
            depth_or_layers: 1,
            mip_levels: 1,
            samples,
            usage,
            cube: false,
            reinterpret: true,
        })
        .unwrap()
    };
    let samples = texture(b, 4, TextureUsage::RENDER);
    let single = texture(b, 1, TextureUsage::RENDER | TextureUsage::COPY_SRC);
    let view = b
        .create_view(
            &samples,
            &TextureViewDesc {
                format: Format::Rgba8Unorm,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .unwrap();
    let program = draw::program(b, COLORED_WGSL).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let layout = b
        .create_vertex_layout(&draw::layout(
            &[(28, StepMode::Vertex)],
            &[
                (0, 0, VertexFormat::Float32x3),
                (0, 12, VertexFormat::Float32x4),
            ],
        ))
        .unwrap();
    let wedges = wedges();
    let vertices = draw::buffer(
        b,
        bytemuck::cast_slice(&wedges),
        wcontainers_gpu::BufferUsage::VERTEX,
    )
    .unwrap();
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
        load: LoadOp::Clear(scene::CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &layout,
        bindings: [None; 4],
        vertex_buffers: &[Some(VertexBinding {
            buffer: &vertices,
            offset: 0,
        })],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: wedges.len() as u32,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let region = |texture| TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    b.resolve(&region(&samples), &region(&single), format);
    let rb = b
        .read_back(&ReadSource::Texture {
            region: region(&single),
            extent: Extent {
                width: RESOLVE_SIZE,
                height: RESOLVE_SIZE,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (RESOLVE_SIZE * RESOLVE_SIZE * 4) as usize];
    b.finish_read(rb, &mut out);
    out
}

#[test]
fn srgb_resolve_of_a_reinterpreted_texture_matches_direct3d() {
    let mut b = device_without(wgpu::Features::empty());
    assert!(b.caps().has(Features::TEXTURE_VIEWS));
    let reference = d3d_srgb_resolve();
    let differ = |ours: &[u8]| {
        let off = reference.iter().zip(ours).filter(|(a, b)| a != b).count();
        let worst = reference
            .iter()
            .zip(ours)
            .map(|(a, b)| a.abs_diff(*b))
            .max();
        (off, worst.unwrap())
    };
    let (off, worst) = differ(&our_resolve(&mut b, Format::Rgba8UnormSrgb));
    eprintln!("resolved as rgba8unorm-srgb: {off} bytes differ, by up to {worst}");
    // Resolved as stored, partly covered pixels average in gamma space and differ: the check sees
    // the format.
    let (unorm_off, unorm_worst) = differ(&our_resolve(&mut b, Format::Rgba8Unorm));
    eprintln!("resolved as rgba8unorm: {unorm_off} bytes differ, by up to {unorm_worst}");
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(off, 0, "{off} bytes differ, by up to {worst}");
    assert!(unorm_off > 0);
}
