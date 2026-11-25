//! What the back end does where WebGPU has no native route, against real Direct3D 11 on the same GPU:
//! `generate_mips` of `r32float` on a device without `float32-filterable` and of `rgba16uint` holding
//! `rgba16unorm` values (both filtered in the program) against GenerateMips, and a multisampled
//! `rgba8unorm` texture made with `reinterpret` resolved as `rgba8unorm-srgb` against
//! ResolveSubresource with `R8G8B8A8_UNORM_SRGB`; BC textures loaded natively and decoded against
//! Direct3D 11 and WARP loading the same blocks; timestamps around the same work as Direct3D 11's;
//! formats held converted loaded, filtered and (8-bit alpha) drawn into as Direct3D 11 does; BGRA
//! vertices read as Direct3D 11 reads `B8G8R8A8_UNORM` and `B8G8R8X8_UNORM`.
//!
//! Windows only: needs a GPU and Direct3D 11 (the rig's device and shaders, test-only); `WEBGPU_API`
//! as in tests/scenes.rs.

#![cfg(windows)]

#[path = "../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

#[allow(dead_code)]
#[path = "../examples/scenes/timestamps.rs"]
mod timestamps;

#[allow(dead_code)]
#[path = "../examples/scenes/converted.rs"]
mod converted;

#[allow(dead_code)]
#[path = "../examples/scenes/vertices.rs"]
mod vertices;

#[allow(dead_code)]
#[path = "../examples/scenes/renames.rs"]
mod renames;

use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferRange, BufferUsage, ColorAttachment,
    DeviceStatus, DrawCall, DrawRange, Extent, FORMAT_FACTS, Features, Format, FormatUse, LoadOp,
    Origin, PassDesc, ReadSource, SlowPath, StageSet, StepMode, StoreOp, TextureDesc,
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
    assert!(b.caps().features.contains(Features::TEXTURE_VIEWS));
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
    hlsl_of(BC_LOAD_HLSL, entry, target)
}

fn hlsl_of(source: &str, entry: windows::core::PCSTR, target: windows::core::PCSTR) -> Vec<u8> {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    let mut code = None;
    unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
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
        BC_SIZE / 4 * FORMAT_FACTS[format as usize].block.2,
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
    assert!(native.caps().features.contains(Features::BLOCK_COMPRESSION));
    assert!(
        !decoded
            .caps()
            .features
            .contains(Features::BLOCK_COMPRESSION)
    );
    let hardware = direct3d();
    let warp = d3d11::create(Driver::Warp, false, &[D3D_FEATURE_LEVEL_11_0]).expect("WARP");
    let (vs, ps) = (
        hlsl(windows::core::s!("vs"), windows::core::s!("vs_5_0")),
        hlsl(windows::core::s!("ps"), windows::core::s!("ps_5_0")),
    );
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut beyond = Vec::new();
    for (format, dxgi) in BC_FORMATS {
        let block = FORMAT_FACTS[format as usize].block.2;
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

/// Milliseconds since the first call.
fn now() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
        * 1000.0
}

/// tests/timestamps.rs's work in HLSL: the same loop, rounds and target.
fn timestamp_hlsl() -> String {
    format!(
        r#"
float4 vs(uint i : SV_VertexID) : SV_Position {{
    float2 p = float2((i << 1) & 2, i & 2);
    return float4(p * 2.0 - 1.0, 0.0, 1.0);
}}
float4 ps(float4 p : SV_Position) : SV_Target {{
    float2 c = p.xy / {size}.0;
    [loop] for (int i = 0; i < {rounds}; i++) {{
        c = frac(c * 1.7 + c.yx * 0.3 + 0.1);
    }}
    return float4(c, 0.0, 1.0);
}}
"#,
        size = timestamps::SIZE,
        rounds = timestamps::ROUNDS
    )
}

/// Direct3D 11's two stretches around the same work, in nanoseconds, its timestamps in order.
fn d3d_stretches() -> (u64, u64) {
    let gpu = direct3d();
    let source = timestamp_hlsl();
    let vs = hlsl_of(
        &source,
        windows::core::s!("vs"),
        windows::core::s!("vs_5_0"),
    );
    let ps = hlsl_of(
        &source,
        windows::core::s!("ps"),
        windows::core::s!("ps_5_0"),
    );
    gpu.program(&vs, &ps, &[]).unwrap();
    // The covering triangle winds anticlockwise: no culling.
    let raster = gpu.rasterizer(&solid()).unwrap();
    unsafe { gpu.context.RSSetState(&raster) };
    let (_target, rtv) = gpu
        .target(
            timestamps::SIZE,
            timestamps::SIZE,
            DXGI_FORMAT_R8G8B8A8_UNORM,
        )
        .unwrap();
    let query = |kind| {
        d3d11::made(|o| unsafe {
            gpu.device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: kind,
                    MiscFlags: 0,
                },
                Some(o),
            )
        })
        .unwrap()
    };
    let wait = |q: &ID3D11Query, data: *mut std::ffi::c_void, size: usize| loop {
        // The wrapper takes S_FALSE (not ready) as success, so the call goes through the vtable.
        let hr = unsafe {
            (Interface::vtable(&gpu.context).GetData)(
                Interface::as_raw(&gpu.context),
                Interface::as_raw(q),
                data,
                size as u32,
                0,
            )
        };
        hr.ok().unwrap();
        if hr.0 == 0 {
            break;
        }
        std::thread::yield_now();
    };
    let stretch = |draws: u32| unsafe {
        gpu.context
            .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        gpu.viewport(timestamps::SIZE, timestamps::SIZE);
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        for _ in 0..draws {
            gpu.context.Draw(3, 0);
        }
    };
    stretch(1);
    let disjoint = query(D3D11_QUERY_TIMESTAMP_DISJOINT);
    let stamps: Vec<_> = (0..4).map(|_| query(D3D11_QUERY_TIMESTAMP)).collect();
    unsafe { gpu.context.Begin(&disjoint) };
    for (stamp, draws) in stamps
        .iter()
        .zip([timestamps::DRAWS, timestamps::DRAWS * 2, 0, 0])
    {
        unsafe { gpu.context.End(stamp) };
        stretch(draws);
    }
    unsafe {
        gpu.context.End(&disjoint);
        gpu.context.Flush();
    }
    let mut info = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
    wait(
        &disjoint,
        (&mut info as *mut D3D11_QUERY_DATA_TIMESTAMP_DISJOINT).cast(),
        std::mem::size_of_val(&info),
    );
    assert!(
        !info.Disjoint.as_bool(),
        "Direct3D's timestamps were disjoint"
    );
    let ticks: Vec<u64> = stamps
        .iter()
        .map(|q| {
            let mut t = 0u64;
            wait(q, (&mut t as *mut u64).cast(), 8);
            t
        })
        .collect();
    let ns: Vec<u64> = ticks
        .iter()
        .map(|t| (u128::from(*t) * 1_000_000_000 / u128::from(info.Frequency)) as u64)
        .collect();
    eprintln!(
        "Direct3D 11: frequency {} Hz, timestamps {ns:?} ns",
        info.Frequency
    );
    assert!(
        ns[0] < ns[1] && ns[1] < ns[2] && ns[2] <= ns[3],
        "Direct3D's own timestamps out of order: {ns:?}"
    );
    (ns[1] - ns[0], ns[2] - ns[1])
}

/// Our timestamps around the work are in order, and each stretch takes within a factor of two of
/// what Direct3D 11's timestamps say the same work takes on the same GPU.
#[test]
fn timestamps_match_direct3d_in_order_and_scale() {
    let ours = {
        let mut b = device_without(wgpu::Features::empty());
        let stamps = timestamps::measure(&mut b, now).unwrap_or_else(|e| panic!("{e}"));
        eprintln!("ours: {}", stamps.line());
        let n = stamps.ns;
        assert!(
            n[0] < n[1] && n[1] < n[2] && n[2] <= n[3],
            "out of order: {n:?}"
        );
        stamps.stretches()
    };
    let theirs = d3d_stretches();
    eprintln!(
        "stretches: ours {:.3} ms and {:.3} ms, Direct3D 11's {:.3} ms and {:.3} ms",
        ours.0 as f64 / 1e6,
        ours.1 as f64 / 1e6,
        theirs.0 as f64 / 1e6,
        theirs.1 as f64 / 1e6
    );
    for (o, t) in [(ours.0, theirs.0), (ours.1, theirs.1)] {
        let ratio = o as f64 / t as f64;
        assert!(
            (0.5..=2.0).contains(&ratio),
            "a stretch took {ratio:.2} times Direct3D's time"
        );
    }
}

/// examples/scenes/converted.rs's programs in Direct3D's language.
const CONVERTED_HLSL: &str = "
Texture2D<float4> t : register(t0);
SamplerState s : register(s0);
float4 vs(uint i : SV_VertexID) : SV_Position {
    float2 p = float2((i << 1) & 2, i & 2);
    return float4(p * 2.0 - 1.0, 0.0, 1.0);
}
float4 ps(float4 p : SV_Position) : SV_Target {
    float2 size;
    t.GetDimensions(size.x, size.y);
    if (p.x < size.x) return t.Load(int3(p.xy, 0));
    return t.SampleLevel(s, (p.xy - float2(size.x, 0.0) - 0.25) / size, 0.0);
}
float4 alpha(float4 p : SV_Position) : SV_Target {
    return float4(0.9, 0.6, 0.3, frac((p.x * 7.0 + p.y * 13.0) / 32.0));
}
";

/// The converted formats with Direct3D's.
const CONVERTED: [(Format, DXGI_FORMAT); 8] = [
    (Format::A8Unorm, DXGI_FORMAT_A8_UNORM),
    (Format::Bgrx8Unorm, DXGI_FORMAT_B8G8R8X8_UNORM),
    (Format::Bgrx8UnormSrgb, DXGI_FORMAT_B8G8R8X8_UNORM_SRGB),
    (Format::Rgbg8Unorm, DXGI_FORMAT_R8G8_B8G8_UNORM),
    (Format::Grgb8Unorm, DXGI_FORMAT_G8R8_G8B8_UNORM),
    (Format::B5g6r5Unorm, DXGI_FORMAT_B5G6R5_UNORM),
    (Format::Bgr5a1Unorm, DXGI_FORMAT_B5G5R5A1_UNORM),
    (Format::Bgra4Unorm, DXGI_FORMAT_B4G4R4A4_UNORM),
];

/// Direct3D 11's texels of `texels` in `format` loaded, then filtered, as converted.rs draws them.
fn d3d_sampled(gpu: &Gpu, vs: &[u8], format: DXGI_FORMAT, texels: &[u8]) -> (Vec<f32>, Vec<f32>) {
    let (w, h) = converted::SIZE;
    let texel = texels.len() as u32 / (w * h);
    let mut desc = tex2d(w, h, format, D3D11_BIND_SHADER_RESOURCE);
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let texture = gpu
        .texture2d(&desc, &[d3d11::data(texels, w * texel, 0)])
        .unwrap();
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    let sampler = gpu
        .sampler(&D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            ..Default::default()
        })
        .unwrap();
    let (target, rtv) = gpu
        .target(w * 2, h, DXGI_FORMAT_R32G32B32A32_FLOAT)
        .unwrap();
    gpu.begin(&rtv, None, w * 2, h);
    let ps = hlsl_of(
        CONVERTED_HLSL,
        windows::core::s!("ps"),
        windows::core::s!("ps_5_0"),
    );
    gpu.program(vs, &ps, &[]).unwrap();
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid()).unwrap());
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        gpu.context.PSSetShaderResources(0, Some(&[Some(view)]));
        gpu.context.PSSetSamplers(0, Some(&[Some(sampler)]));
        gpu.context.Draw(3, 0);
    }
    let floats = floats(&d3d_read(gpu, &target, 0, 16));
    let (mut loaded, mut filtered) = (Vec::new(), Vec::new());
    for row in floats.chunks_exact((w * 8) as usize) {
        let (l, f) = row.split_at((w * 4) as usize);
        loaded.extend_from_slice(l);
        filtered.extend_from_slice(f);
    }
    (loaded, filtered)
}

/// Direct3D 11's target of `format` drawn as converted.rs's `drawn` draws it: its bytes.
fn d3d_drawn(gpu: &Gpu, vs: &[u8], format: DXGI_FORMAT, texel: usize) -> Vec<u8> {
    let (w, h) = converted::SIZE;
    let (target, rtv) = gpu.target(w, h, format).unwrap();
    let ps = hlsl_of(
        CONVERTED_HLSL,
        windows::core::s!("alpha"),
        windows::core::s!("ps_5_0"),
    );
    gpu.program(vs, &ps, &[]).unwrap();
    let mut blend = D3D11_BLEND_DESC::default();
    blend.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        SrcBlend: D3D11_BLEND_SRC_ALPHA,
        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_SRC_ALPHA,
        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    unsafe {
        let c = &gpu.context;
        c.ClearRenderTargetView(&rtv, &[0.5, 0.75, 1.0, converted::CLEAR_ALPHA]);
        c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        gpu.viewport(w, h);
        c.RSSetState(&gpu.rasterizer(&solid()).unwrap());
        c.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        c.OMSetBlendState(&gpu.blend(&blend).unwrap(), None, u32::MAX);
        c.Draw(3, 0);
    }
    d3d_read(gpu, &target, 0, texel)
}

/// Direct3D 11's bytes of an immutable texture of `format` made from `texels`, read back.
fn d3d_kept(gpu: &Gpu, format: DXGI_FORMAT, texel: usize, texels: &[u8]) -> Vec<u8> {
    let (w, h) = converted::SIZE;
    let texture = gpu
        .texture2d(
            &tex2d(w, h, format, D3D11_BIND_SHADER_RESOURCE),
            &[d3d11::data(texels, w * texel as u32, 0)],
        )
        .unwrap();
    d3d_read(gpu, &texture, 0, texel)
}

/// Each converted format's seeded texels loaded and filtered by the back end, which holds them in
/// an 8-bit RGBA format, against Direct3D 11 on the same GPU: loads equal, filtered texels within
/// one 8-bit step; and 8-bit alpha and BGRX drawn with blending into a cleared target, byte for
/// byte but for BGRX's X (unused bits, which the back end reads back as one and the RTX 3080's
/// Direct3D keeps as written; printed). Prints each largest difference and how far Direct3D's own
/// loads are from `v / (2^n - 1)`.
#[test]
fn converted_formats_load_filter_and_draw_as_direct3d_does() {
    let mut b = device_without(wgpu::Features::empty());
    let gpu = direct3d();
    let vs = hlsl_of(
        CONVERTED_HLSL,
        windows::core::s!("vs"),
        windows::core::s!("vs_5_0"),
    );
    let mut beyond = Vec::new();
    let seed: u64 = std::env::var("WEBGPU_SEED").map_or(1, |s| s.parse().expect("a seed"));
    let cases: u64 = std::env::var("WEBGPU_CASES").map_or(4, |s| s.parse().expect("a count"));
    eprintln!("seed {seed}, {cases} textures per format");
    for (i, (format, dxgi)) in CONVERTED.into_iter().enumerate() {
        let (mut l, mut f, mut rule, mut filter_rule) = (0f32, 0f32, 0f32, 0f32);
        for c in 0..cases {
            let texels = converted::texels(format, seed + c * CONVERTED.len() as u64 + i as u64);
            let (d3d_loaded, d3d_filtered) = d3d_sampled(&gpu, &vs, dxgi, &texels);
            let (_, view) =
                converted::upload(&mut b, format, Some(&texels), TextureUsage::empty()).unwrap();
            let (loaded, filtered) = converted::sampled(&mut b, &view).unwrap();
            let want = converted::expected(format, &texels);
            rule = rule.max(converted::largest(&d3d_loaded, &want));
            filter_rule = filter_rule.max(converted::largest(
                &d3d_filtered,
                &converted::filtered(&want),
            ));
            l = l.max(converted::largest(&loaded, &d3d_loaded));
            f = f.max(converted::largest(&filtered, &d3d_filtered));
        }
        eprintln!(
            "{format:?}: loaded {:.3} and filtered {:.3} 8-bit steps from Direct3D's; Direct3D's loads {:e} and filtered texels {:.3} steps from v / (2^n - 1)",
            l * 255.0,
            f * 255.0,
            rule,
            filter_rule * 255.0
        );
        if l > 0.0 || f > 1.0 / 255.0 {
            beyond.push(format!("{format:?}: loaded {l:e}, filtered {f:e}"));
        }
    }
    for (format, dxgi, texel) in [
        (Format::A8Unorm, DXGI_FORMAT_A8_UNORM, 1),
        (Format::Bgrx8Unorm, DXGI_FORMAT_B8G8R8X8_UNORM, 4),
        (Format::Bgrx8UnormSrgb, DXGI_FORMAT_B8G8R8X8_UNORM_SRGB, 4),
    ] {
        let ours = converted::drawn(&mut b, format).unwrap().0;
        let theirs = d3d_drawn(&gpu, &vs, dxgi, texel);
        let unused = |i: usize| texel == 4 && i % 4 == 3;
        let differing = (0..ours.len())
            .filter(|&i| !unused(i) && ours[i] != theirs[i])
            .count();
        let x_kept = (0..ours.len())
            .filter(|&i| unused(i) && ours[i] == theirs[i])
            .count();
        eprintln!(
            "{format:?} drawn: {differing} of {} bytes differ from Direct3D's, X equal in {x_kept}",
            ours.len()
        );
        if differing > 0 {
            beyond.push(format!("{format:?} drawn: {differing} bytes"));
        }
    }
    // What Direct3D reads back of BGRX's X byte written from the host (the back end reads one).
    let texels = converted::texels(Format::Bgrx8Unorm, 9);
    let kept = d3d_kept(&gpu, DXGI_FORMAT_B8G8R8X8_UNORM, 4, &texels);
    let as_written = kept
        .iter()
        .zip(&texels)
        .skip(3)
        .step_by(4)
        .filter(|(a, b)| a == b)
        .count();
    let one = kept
        .iter()
        .skip(3)
        .step_by(4)
        .filter(|&&x| x == 255)
        .count();
    eprintln!(
        "Direct3D's BGRX read back: X as written in {as_written}, 255 in {one}, of {}",
        texels.len() / 4
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert!(b.slow_paths().count(SlowPath::FormatConverted) > 0);
    assert!(beyond.is_empty(), "{beyond:#?}");
}

/// A three-channel texture's texels loaded as their bits, as converted.rs's `loaded_bits` does;
/// `ty` is the texture's channel type.
fn bits_hlsl(ty: &str) -> String {
    format!(
        "Texture2D<{ty}4> t : register(t0);
uint4 ps(float4 p : SV_Position) : SV_Target {{ return asuint(t.Load(int3(p.xy, 0))); }}"
    )
}

/// Direct3D 11's texels of `texels` in the three-channel `format` loaded as their bits.
fn d3d_bits(gpu: &Gpu, vs: &[u8], format: DXGI_FORMAT, ty: &str, texels: &[u8]) -> Vec<u8> {
    let (w, h) = converted::SIZE;
    let mut desc = tex2d(w, h, format, D3D11_BIND_SHADER_RESOURCE);
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let texture = gpu
        .texture2d(&desc, &[d3d11::data(texels, w * 12, 0)])
        .unwrap();
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    let (target, rtv) = gpu.target(w, h, DXGI_FORMAT_R32G32B32A32_UINT).unwrap();
    gpu.begin(&rtv, None, w, h);
    let ps = hlsl_of(
        &bits_hlsl(ty),
        windows::core::s!("ps"),
        windows::core::s!("ps_5_0"),
    );
    gpu.program(vs, &ps, &[]).unwrap();
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid()).unwrap());
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        gpu.context.PSSetShaderResources(0, Some(&[Some(view)]));
        gpu.context.Draw(3, 0);
    }
    d3d_read(gpu, &target, 0, 16)
}

/// The three-channel 32-bit formats (held in their four-channel twins) written with seeded bits,
/// NaNs and denormals among them, then loaded in a program and read back, against Direct3D 11's
/// `R32G32B32_*` on the same GPU: every 32-bit channel equal. Prints how many differ.
#[test]
fn three_channel_formats_load_and_read_back_as_direct3d_does() {
    let mut b = device_without(wgpu::Features::empty());
    let gpu = direct3d();
    let vs = hlsl_of(
        CONVERTED_HLSL,
        windows::core::s!("vs"),
        windows::core::s!("vs_5_0"),
    );
    let seed: u64 = std::env::var("WEBGPU_SEED").map_or(1, |s| s.parse().expect("a seed"));
    let cases: u64 = std::env::var("WEBGPU_CASES").map_or(4, |s| s.parse().expect("a count"));
    eprintln!("seed {seed}, {cases} textures per format");
    let mut beyond = Vec::new();
    for (i, (format, dxgi, ty)) in [
        (Format::Rgb32Float, DXGI_FORMAT_R32G32B32_FLOAT, "float"),
        (Format::Rgb32Uint, DXGI_FORMAT_R32G32B32_UINT, "uint"),
        (Format::Rgb32Sint, DXGI_FORMAT_R32G32B32_SINT, "int"),
    ]
    .into_iter()
    .enumerate()
    {
        let differ = |a: &[u8], b: &[u8]| {
            a.as_chunks::<4>()
                .0
                .iter()
                .zip(b.as_chunks::<4>().0)
                .filter(|(x, y)| x != y)
                .count()
        };
        let (mut loads, mut kept, mut channels) = (0, 0, 0);
        for c in 0..cases {
            let texels = converted::texels(format, seed + c * 3 + i as u64);
            let theirs = d3d_bits(&gpu, &vs, dxgi, ty, &texels);
            let (t, view) =
                converted::upload(&mut b, format, Some(&texels), TextureUsage::empty()).unwrap();
            let ours = converted::loaded_bits(&mut b, format, &view).unwrap();
            loads += differ(&ours, &theirs);
            let ours = converted::read(
                &mut b,
                &converted::at(&t, 0, 0),
                converted::whole(),
                texels.len(),
            );
            kept += differ(&ours, &d3d_kept(&gpu, dxgi, 12, &texels));
            channels += texels.len() / 4;
        }
        eprintln!(
            "{format:?}: of {channels} channels, {loads} loaded and {kept} read back differ from Direct3D's"
        );
        if loads + kept > 0 {
            beyond.push(format!("{format:?}: {loads} loaded, {kept} read back"));
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert!(beyond.is_empty(), "{beyond:#?}");
}

/// Points carrying one vertex attribute into an `R32G32B32A32_FLOAT` target, as vertices.rs draws.
const ATTRIBUTE_HLSL: &str = r#"
struct O { float4 p : SV_Position; nointerpolation float4 v : V; };
O vs(uint i : SV_VertexID, float4 v : V) {
    O o;
    o.p = float4((i + 0.5) / N * 2.0 - 1.0, 0.0, 0.0, 1.0);
    o.v = v;
    return o;
}
float4 ps(O o) : SV_Target { return o.v; }
"#;

/// The attribute Direct3D 11 gives each 4-byte element of `bytes` read as `format`.
fn d3d_attributes(gpu: &Gpu, format: DXGI_FORMAT, bytes: &[u8]) -> Vec<[f32; 4]> {
    let n = (bytes.len() / 4) as u32;
    let source = ATTRIBUTE_HLSL.replace("/ N", &format!("/ {n}.0"));
    let vs = hlsl_of(
        &source,
        windows::core::s!("vs"),
        windows::core::s!("vs_5_0"),
    );
    let ps = hlsl_of(
        &source,
        windows::core::s!("ps"),
        windows::core::s!("ps_5_0"),
    );
    let (target, rtv) = gpu.target(n, 1, DXGI_FORMAT_R32G32B32A32_FLOAT).unwrap();
    gpu.begin(&rtv, None, n, 1);
    let element = d3d11::element(windows::core::s!("V"), 0, format, 0, 0, 0);
    gpu.program(&vs, &ps, &[element]).unwrap();
    let _vb = gpu.vertices(0, bytes, 4).unwrap();
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid()).unwrap());
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        );
        gpu.context.Draw(n, 0);
    }
    floats(&d3d_read(gpu, &target, 0, 16))
        .as_chunks::<4>()
        .0
        .to_vec()
}

/// Seeded elements read by the back end as `VertexFormat::Bgra8Unorm` and by Direct3D 11 as
/// `B8G8R8A8_UNORM` and `B8G8R8X8_UNORM`: every component equal, but BGRX's fourth, which Direct3D
/// leaves undefined and the back end reads as written (Direct3D's printed). `WEBGPU_SEED` (default
/// 188) and `WEBGPU_CASES` (vertices, default 64).
#[test]
fn bgra_vertices_read_as_direct3d_reads_bgra_and_bgrx() {
    let env = |name: &str, default: u64| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let (seed, n) = (env("WEBGPU_SEED", 188), env("WEBGPU_CASES", 64));
    let mut rng = fastrand::Rng::with_seed(seed);
    let bytes: Vec<u8> = (0..n * 4).map(|_| rng.u8(..)).collect();
    let mut b = device_without(wgpu::Features::empty());
    let ours = vertices::bgra_attributes(&mut b, &bytes);
    let gpu = direct3d();
    let bgra = d3d_attributes(&gpu, DXGI_FORMAT_B8G8R8A8_UNORM, &bytes);
    let bgrx = d3d_attributes(&gpu, DXGI_FORMAT_B8G8R8X8_UNORM, &bytes);
    let mut fourths = std::collections::BTreeSet::new();
    for i in 0..n as usize {
        assert_eq!(ours[i], bgra[i], "vertex {i} against B8G8R8A8_UNORM");
        assert_eq!(
            ours[i][..3],
            bgrx[i][..3],
            "vertex {i} against B8G8R8X8_UNORM"
        );
        fourths.insert(bgrx[i][3].to_bits());
    }
    let fourths: Vec<f32> = fourths.into_iter().map(f32::from_bits).collect();
    println!(
        "seed {seed}, {n} vertices: equal to Direct3D 11's B8G8R8A8_UNORM, and B8G8R8X8_UNORM but for its fourth component, which Direct3D 11 reads as {fourths:?}"
    );
}

/// Pairs divided by one draw, one a pixel.
const PAIRS: u32 = 64;

/// atg-simplecomputepc's division: its dividend, and the whole quotient Direct3D gives.
const SAMPLE_DIVIDEND: f32 = 1200.0;
const SAMPLE_QUOTIENT: f32 = 4.0;
/// The sample's divisor, which both programs also divide by as a literal.
const DIVISOR: f32 = SAMPLE_DIVIDEND / SAMPLE_QUOTIENT;
const SAMPLE_PAIR: [f32; 2] = [SAMPLE_DIVIDEND, DIVISOR];

/// Each pixel divides its pair: by the pair's run-time divisor and by [`DIVISOR`] written in the
/// program, each quotient with its fraction, as bits.
fn division_hlsl() -> String {
    format!(
        "cbuffer C : register(b0) {{ float4 v[{PAIRS}]; }};
float4 vs(uint i : SV_VertexID) : SV_Position {{
    float2 p = float2((i << 1) & 2, i & 2);
    return float4(p * 2.0 - 1.0, 0.0, 1.0);
}}
uint4 ps(float4 p : SV_Position) : SV_Target {{
    float4 a = v[(uint)p.x];
    float q = a.x / a.y;
    float k = a.x / {DIVISOR:?};
    return asuint(float4(q, frac(q), k, frac(k)));
}}"
    )
}

fn division_wgsl() -> String {
    format!(
        "struct C {{ v: array<vec4f, {PAIRS}> }}
@group(1) @binding(0) var<uniform> c: C;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4u {{
    let a = c.v[u32(p.x)];
    let q = a.x / a.y;
    let k = a.x / {DIVISOR:?};
    return bitcast<vec4u>(vec4f(q, fract(q), k, fract(k)));
}}"
    )
}

/// Seeded pairs, the first atg-simplecomputepc's: whole quotients of seeded integers and
/// seeded floats in a range.
fn division_pairs(seed: u64) -> Vec<[f32; 4]> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut pairs = vec![[SAMPLE_PAIR[0], SAMPLE_PAIR[1], 0.0, 0.0]];
    while pairs.len() < PAIRS as usize {
        let d = rng.u16(1..) as f32;
        let n = if rng.bool() {
            d * rng.u16(..) as f32
        } else {
            rng.f32() * f32::from(u16::MAX)
        };
        pairs.push([n, d, 0.0, 0.0]);
    }
    pairs
}

fn d3d_divided(gpu: &Gpu, pairs: &[[f32; 4]]) -> Vec<u32> {
    let source = division_hlsl();
    let vs = hlsl_of(
        &source,
        windows::core::s!("vs"),
        windows::core::s!("vs_5_0"),
    );
    let ps = hlsl_of(
        &source,
        windows::core::s!("ps"),
        windows::core::s!("ps_5_0"),
    );
    let bytes: &[u8] = bytemuck::cast_slice(pairs);
    let constants = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: bytes.len() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            },
            Some(bytes),
        )
        .unwrap();
    let (target, rtv) = gpu.target(PAIRS, 1, DXGI_FORMAT_R32G32B32A32_UINT).unwrap();
    gpu.begin(&rtv, None, PAIRS, 1);
    gpu.program(&vs, &ps, &[]).unwrap();
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid()).unwrap());
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        gpu.context
            .PSSetConstantBuffers(0, Some(&[Some(constants)]));
        gpu.context.Draw(3, 0);
    }
    bytemuck::pod_collect_to_vec(&d3d_read(gpu, &target, 0, size_of::<[u32; 4]>()))
}

fn our_divided(b: &mut B, pairs: &[[f32; 4]]) -> Vec<u32> {
    let (target, view) = draw::texture_2d(
        b,
        Format::Rgba32Uint,
        PAIRS,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let k = renames::Kit {
        program: draw::program(b, &division_wgsl()).unwrap(),
        target,
        view,
        ..renames::kit(b, Format::Rgba32Uint, PAIRS, &draw::opaque())
    };
    let bytes: &[u8] = bytemuck::cast_slice(pairs);
    let constants = draw::buffer(b, bytes, BufferUsage::UNIFORM).unwrap();
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &constants,
                offset: 0,
                size: bytes.len() as u64,
            }),
        }])
        .unwrap();
    renames::begin(b, &k, [0.0; 4]);
    renames::draw_at(b, &k, &set, None);
    bytemuck::pod_collect_to_vec(&renames::finish(b, &k, size_of::<[u32; 4]>() as u32))
}

/// Divisions in a converted program against Direct3D 11's on the same GPU: seeded pairs (the first
/// atg-simplecomputepc's 1200 by 300), each divided by its run-time divisor and by one written in
/// the program, with fractions. Prints every pair whose bits differ; `WEBGPU_SEED` and
/// `WEBGPU_CASES` (draws of 64 pairs, default 4) pick the pairs.
#[test]
fn divisions_match_direct3d() {
    let mut b = device_without(wgpu::Features::empty());
    let gpu = direct3d();
    let seed: u64 = std::env::var("WEBGPU_SEED").map_or(1, |s| s.parse().expect("a seed"));
    let cases: u64 = std::env::var("WEBGPU_CASES").map_or(4, |s| s.parse().expect("a count"));
    eprintln!("seed {seed}, {cases} draws of {PAIRS} pairs");
    let mut differ = Vec::new();
    for c in 0..cases {
        let pairs = division_pairs(seed + c);
        let theirs = d3d_divided(&gpu, &pairs);
        let ours = our_divided(&mut b, &pairs);
        for (i, (t, o)) in theirs.chunks(4).zip(ours.chunks(4)).enumerate() {
            if t != o {
                let f = |w: &[u32]| w.iter().map(|&x| f32::from_bits(x)).collect::<Vec<_>>();
                differ.push(format!(
                    "{} / {}: Direct3D {:?}, ours {:?}",
                    pairs[i][0],
                    pairs[i][1],
                    f(t),
                    f(o)
                ));
            }
        }
        if c == 0 {
            let f = |w: &[u32]| {
                w[..4]
                    .iter()
                    .map(|&x| f32::from_bits(x))
                    .collect::<Vec<_>>()
            };
            eprintln!(
                "1200 / 300 (quotient, fraction, by the literal, fraction): Direct3D {:?}, ours {:?}",
                f(&theirs),
                f(&ours)
            );
        }
    }
    eprintln!(
        "{} of {} pairs differ",
        differ.len(),
        cases * u64::from(PAIRS)
    );
    for d in &differ {
        eprintln!("  {d}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert!(differ.is_empty(), "{} pairs differ", differ.len());
}
