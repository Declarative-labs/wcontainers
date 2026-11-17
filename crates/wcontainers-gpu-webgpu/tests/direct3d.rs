//! What the back end does where WebGPU has no native route, against real Direct3D 11 on the same GPU:
//! `generate_mips` of `r32float` on a device without `float32-filterable` and of `rgba16uint` holding
//! `rgba16unorm` values (both filtered in the program) against GenerateMips, and a multisampled
//! `rgba8unorm` texture made with `reinterpret` resolved as `rgba8unorm-srgb` against
//! ResolveSubresource with `R8G8B8A8_UNORM_SRGB`; BC textures loaded natively and decoded against
//! Direct3D 11 and WARP loading the same blocks.
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
    Backend, BindingEntry, BindingResource, ColorAttachment, DeviceStatus, DrawCall, DrawRange,
    Extent, Features, Format, FormatUse, LoadOp, Origin, PassDesc, ReadSource, SlowPath, StageSet,
    StepMode, StoreOp, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
    Topology, VertexBinding, VertexFormat, ViewDimension, WriteMode,
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

/// Texels of one block-compressed texture: 8 by 8 blocks.
const BC_SIZE: u32 = 32;

/// Every BC format with Direct3D's.
const BC_FORMATS: [(Format, DXGI_FORMAT); 14] = [
    (Format::Bc1RgbaUnorm, DXGI_FORMAT_BC1_UNORM),
    (Format::Bc1RgbaUnormSrgb, DXGI_FORMAT_BC1_UNORM_SRGB),
    (Format::Bc2RgbaUnorm, DXGI_FORMAT_BC2_UNORM),
    (Format::Bc2RgbaUnormSrgb, DXGI_FORMAT_BC2_UNORM_SRGB),
    (Format::Bc3RgbaUnorm, DXGI_FORMAT_BC3_UNORM),
    (Format::Bc3RgbaUnormSrgb, DXGI_FORMAT_BC3_UNORM_SRGB),
    (Format::Bc4RUnorm, DXGI_FORMAT_BC4_UNORM),
    (Format::Bc4RSnorm, DXGI_FORMAT_BC4_SNORM),
    (Format::Bc5RgUnorm, DXGI_FORMAT_BC5_UNORM),
    (Format::Bc5RgSnorm, DXGI_FORMAT_BC5_SNORM),
    (Format::Bc6hRgbUfloat, DXGI_FORMAT_BC6H_UF16),
    (Format::Bc6hRgbFloat, DXGI_FORMAT_BC6H_SF16),
    (Format::Bc7RgbaUnorm, DXGI_FORMAT_BC7_UNORM),
    (Format::Bc7RgbaUnormSrgb, DXGI_FORMAT_BC7_UNORM_SRGB),
];

/// Each texel of `t` loaded into a float target, in Direct3D's language.
const BC_LOAD_HLSL: &str = "
Texture2D<float4> t : register(t0);
float4 vs(uint i : SV_VertexID) : SV_Position {
    float2 p = float2((i << 1) & 2, i & 2);
    return float4(p * 2.0 - 1.0, 0.0, 1.0);
}
float4 ps(float4 p : SV_Position) : SV_Target { return t.Load(int3(p.xy, 0)); }
";

const BC_LOAD_WGSL: &str = "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@group(1) @binding(32) var t: texture_2d<f32>;
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return textureLoad(t, vec2i(p.xy), 0);
}
";

fn hlsl(entry: windows::core::PCSTR, target: windows::core::PCSTR) -> Vec<u8> {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    let mut code = None;
    unsafe {
        D3DCompile(
            BC_LOAD_HLSL.as_ptr().cast(),
            BC_LOAD_HLSL.len(),
            windows::core::PCSTR::null(),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            None,
        )
        .expect("the HLSL compiles");
        let code = code.expect("compiled code");
        std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize())
            .to_vec()
    }
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Direct3D 11's texels of `blocks` in `format`, loaded by a pixel shader.
fn d3d_bc_texels(
    gpu: &Gpu,
    program: (&[u8], &[u8]),
    format: DXGI_FORMAT,
    block: u32,
    blocks: &[u8],
) -> Vec<f32> {
    let mut desc = tex2d(BC_SIZE, BC_SIZE, format, D3D11_BIND_SHADER_RESOURCE);
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let texture = gpu
        .texture2d(&desc, &[d3d11::data(blocks, BC_SIZE / 4 * block, 0)])
        .unwrap();
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    let (target, rtv) = gpu
        .target(BC_SIZE, BC_SIZE, DXGI_FORMAT_R32G32B32A32_FLOAT)
        .unwrap();
    gpu.begin(&rtv, None, BC_SIZE, BC_SIZE);
    gpu.program(program.0, program.1, &[]).unwrap();
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid()).unwrap());
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        gpu.context.PSSetShaderResources(0, Some(&[Some(view)]));
        gpu.context.Draw(3, 0);
    }
    floats(&d3d_read(gpu, &target, 0, 16))
}

/// The back end's texels of `blocks` in `format`, loaded by a program.
fn our_bc_texels(b: &mut B, format: Format, blocks: &[u8]) -> Vec<f32> {
    let texture = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: BC_SIZE,
            height: BC_SIZE,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let whole = Extent {
        width: BC_SIZE,
        height: BC_SIZE,
        depth_or_layers: 1,
    };
    let region = |texture| TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    b.write_texture(
        &region(&texture),
        whole,
        blocks,
        BC_SIZE / 4 * format.block().2,
        BC_SIZE / 4,
        WriteMode::Ordered,
    );
    let view = b
        .create_view(
            &texture,
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
        .unwrap();
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&view),
        }])
        .unwrap();
    let (target, target_view) = draw::texture_2d(
        b,
        Format::Rgba32Float,
        BC_SIZE,
        BC_SIZE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let program = draw::program(b, BC_LOAD_WGSL).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let layout = b.create_vertex_layout(&draw::layout(&[], &[])).unwrap();
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
        vertex_layout: &layout,
        bindings: [None, Some(&set), None, None],
        vertex_buffers: &[],
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
            count: 3,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let rb = b
        .read_back(&ReadSource::Texture {
            region: region(&target),
            extent: whole,
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (BC_SIZE * BC_SIZE * 16) as usize];
    b.finish_read(rb, &mut out);
    floats(&out)
}

/// One step of the format the back end stores `format` in when it decodes it: 8-bit unorm (in
/// linear light for sRGB, largest at the top: 1 - 254 / 255 decoded is 2.27 / 255), 16-bit float
/// below one for BC4 and BC5; BC6H and BC7 are stored exactly.
fn decoded_step(format: Format) -> f64 {
    use Format::*;
    match format {
        Bc1RgbaUnorm | Bc2RgbaUnorm | Bc3RgbaUnorm => 1.0 / 255.0,
        Bc1RgbaUnormSrgb | Bc2RgbaUnormSrgb | Bc3RgbaUnormSrgb => 2.3 / 255.0,
        Bc4RUnorm | Bc4RSnorm | Bc5RgUnorm | Bc5RgSnorm => 1.0 / 2048.0,
        _ => 0.0,
    }
}

fn picture(texels: Vec<f32>) -> image::DynamicImage {
    image::DynamicImage::ImageRgba32F(
        image::Rgba32FImage::from_raw(BC_SIZE, BC_SIZE, texels).expect("a whole picture"),
    )
}

/// Every BC format's seeded blocks loaded texel by texel: natively (the GPU's decoder) equal to
/// Direct3D 11 on the same GPU, and decoded by the back end on a device made without
/// `texture-compression-bc` (`BlockDecoded`) no further from the GPU's texels than Direct3D's own
/// software rasteriser, WARP, is, plus one step of the format the decoded texels are stored in
/// ([`decoded_step`]): every channel within WARP's largest difference from the GPU and that step.
/// BC1 to BC5 interpolate their endpoints with a tolerance hardware decoders use (the RTX 3080's
/// differ from WARP's by up to 13 / 255), so this is the bar; the decoded texels' distance from
/// WARP's is printed. `WEBGPU_SEED` (default 188) picks the blocks, `WEBGPU_CASES` (default 1) how many
/// textures of each format.
#[test]
fn bc_textures_decoded_are_as_close_to_direct3d_as_warp() {
    use wcontainers_graphics_test::compare::compare;
    let seed: u64 = std::env::var("WEBGPU_SEED").map_or(188, |s| s.parse().expect("a seed"));
    let cases: u64 = std::env::var("WEBGPU_CASES").map_or(1, |s| s.parse().expect("a count"));
    eprintln!("seed {seed}, {cases} textures per format");
    let mut native = device_without(wgpu::Features::empty());
    let mut decoded = device_without(wgpu::Features::TEXTURE_COMPRESSION_BC);
    assert!(native.caps().has(Features::BLOCK_COMPRESSION));
    assert!(!decoded.caps().has(Features::BLOCK_COMPRESSION));
    let hardware = direct3d();
    let warp = d3d11::create(Driver::Warp, false, &[D3D_FEATURE_LEVEL_11_0]).expect("WARP");
    let (vs, ps) = (
        hlsl(windows::core::s!("vs"), windows::core::s!("vs_5_0")),
        hlsl(windows::core::s!("ps"), windows::core::s!("ps_5_0")),
    );
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut beyond = Vec::new();
    for (format, dxgi) in BC_FORMATS {
        let block = format.block().2;
        for case in 0..cases {
            let blocks: Vec<u8> = (0..(BC_SIZE / 4).pow(2) * block)
                .map(|_| rng.u8(..))
                .collect();
            let gpu = picture(d3d_bc_texels(&hardware, (&vs, &ps), dxgi, block, &blocks));
            let soft = picture(d3d_bc_texels(&warp, (&vs, &ps), dxgi, block, &blocks));
            let ours_native = picture(our_bc_texels(&mut native, format, &blocks));
            let ours_decoded = picture(our_bc_texels(&mut decoded, format, &blocks));
            let warp_diff = compare(&soft, &gpu, 0.0).unwrap();
            let n = compare(&ours_native, &gpu, 0.0).unwrap();
            let allowed = warp_diff.largest + decoded_step(format);
            let d = compare(&ours_decoded, &gpu, allowed).unwrap();
            let d_exact = compare(&ours_decoded, &gpu, 0.0).unwrap();
            let dw = compare(&ours_decoded, &soft, 0.0).unwrap();
            eprintln!(
                "{format:?} {case}: from the GPU's texels WARP's differ in {} by up to {:e}, the decoded in {} by up to {:e}; natively {} differ; the decoded from WARP's in {} by up to {:e}",
                warp_diff.differing,
                warp_diff.largest,
                d_exact.differing,
                d_exact.largest,
                n.differing,
                dw.differing,
                dw.largest
            );
            if n.differing > 0 {
                beyond.push(format!(
                    "{format:?} {case} natively: {} texels",
                    n.differing
                ));
            }
            if d.differing > 0 {
                beyond.push(format!(
                    "{format:?} {case} decoded: {} texels beyond {allowed:e}",
                    d.differing
                ));
            }
        }
    }
    assert_eq!(native.status(), DeviceStatus::Ok);
    assert_eq!(decoded.status(), DeviceStatus::Ok);
    assert!(decoded.slow_paths().count(SlowPath::BlockDecoded) > 0);
    assert!(beyond.is_empty(), "{beyond:#?}");
}
