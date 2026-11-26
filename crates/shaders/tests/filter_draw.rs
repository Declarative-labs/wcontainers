//! Filtering and border colours in the program (`VariantKey::filter_in_shader`, `border_in_shader`)
//! drawn through the WebGPU and WebGL 2 back ends against real Direct3D 11 sampling the same texels
//! through the same sampler, and against the back ends' own samplers where they have the sampler's
//! state natively. `ps_filter` samples a 16 by 16 texture of five levels over a quad in perspective
//! (magnified at the bottom, minified at the top, the coordinate from -1 to 2) six ways: implicit
//! level, bias, explicit level, gradients, a texel offset and a gather, or loads the texel under it
//! after asking the texture's size. An R32_FLOAT texture (which WebGPU cannot filter, the reason for
//! the variant), an RGBA8 one, and normalised ones the back ends hold as the integer format of the
//! same bits, read through `VariantKey::decoded` (`NormAsInteger`).
//!
//! `ps_clamp` samples with Direct3D's per-sample clamp (`Sample` with its clamp argument, as
//! `sample_cl`, `sample_b_cl` and `sample_d_cl`) at an implicit level, with a bias, and with
//! gradients.
//!
//! `ps_lod` asks the level of detail an implicit sample takes (`CalculateLevelOfDetail` and its
//! unclamped form, which the program computes from its derivatives and the sampler's state) over the
//! same plane, at its coordinate and at the coordinate scaled.
//!
//! Each case is a seeded sampler state: filters, address modes, border colour, bias, level range and
//! anisotropy (`WCONTAINERS_FILTER_SEED`, default 1; `WCONTAINERS_FILTER_CASES` draws that many seeds
//! from it, default 8). Windows only, and needs a GPU: run as a GPU job (docs/status-graphics-shaders.md).

#![cfg(windows)]

mod common;

use common::*;
// The trait by its old name: graphics-test's Direct3D 11 device is named `Gpu` in this file.
use wcontainers_gpu::{
    Gpu as Backend, draw::ColorAttachment, draw::DrawCall, draw::DrawRange, draw::PassDesc,
    draw::StoreOp, draw::Viewport, format::FORMAT_FACTS, format::Format, reflect::HelperValue,
    reflect::SlotKind, reflect::VariantKey, resource::AddressMode, resource::BindingEntry,
    resource::BindingResource, resource::BorderColor, resource::BufferRange, resource::BufferUsage,
    resource::Extent, resource::Filter, resource::Origin, resource::ReadSource,
    resource::SamplerDesc, resource::TextureRegion, resource::TextureUsage, resource::WriteMode,
    shader::Program, shader::StageSet, state::BlendDesc, state::ColorWrites, state::CullMode,
    state::DepthStencilDesc, state::FillMode, state::FrontFace, state::ProvokingVertex,
    state::RasterDesc, state::RenderTargetBlend, state::Topology, state::VertexLayoutDesc,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

const TEX: u32 = 16;
const FILTER: &str = "ps_filter";
const LOD: &str = "ps_lod";
const CLAMP: &str = "ps_clamp";
/// `ps_clamp`'s ways of sampling, each with the clamp.
const CLAMP_MODES: [&str; 3] = ["implicit", "bias", "gradients"];
const LEVELS: u32 = 5;
const LEVEL_MODE: &str = "level";
const GATHER_MODE: &str = "gather";
const LOAD_MODE: &str = "load";
const MODES: [&str; 7] = [
    "implicit",
    "bias",
    LEVEL_MODE,
    "gradients",
    "offset",
    GATHER_MODE,
    LOAD_MODE,
];

/// The normalised formats drawn, each with the integer format of the same bits a front stores it as
/// where the back end lacks it.
const DECODED: [(Format, Format); 4] = [
    (Format::R16Unorm, Format::R16Uint),
    (Format::Rg16Snorm, Format::Rg16Sint),
    (Format::Rgba16Unorm, Format::Rgba16Uint),
    (Format::Rgba8Snorm, Format::Rgba8Sint),
];

/// One sampler state and how `ps_filter` is asked to sample.
#[derive(Clone, Debug)]
struct Case {
    /// Min, mag and mip filter: linear when set.
    linear: [bool; 3],
    anisotropy: u32,
    /// Address modes as `AddressMode`'s codes.
    address: [u32; 3],
    border: [f32; 4],
    bias: f32,
    lod: [f32; 2],
    /// The bias or level scale of modes 1 and 2, and the gradients' scales of mode 3.
    value: f32,
    scale: [f32; 2],
}

impl Case {
    fn new(seed: u64) -> Case {
        let mut rng = fastrand::Rng::with_seed(seed);
        let anisotropic = rng.u32(..4) == 0;
        let pick = |rng: &mut fastrand::Rng, v: &[f32]| v[rng.usize(..v.len())];
        Case {
            linear: if anisotropic {
                [true; 3]
            } else {
                [rng.bool(), rng.bool(), rng.bool()]
            },
            anisotropy: if anisotropic { 1 << rng.u32(1..5) } else { 1 },
            address: [rng.u32(..5), rng.u32(..5), rng.u32(..5)],
            border: [rng.f32(), rng.f32(), rng.f32(), rng.f32()],
            bias: pick(&mut rng, &[0.0, 0.0, 0.75, -1.25]),
            lod: [
                pick(&mut rng, &[0.0, 0.0, 0.5, 1.25]),
                pick(&mut rng, &[f32::MAX, f32::MAX, 2.5]),
            ],
            value: pick(&mut rng, &[0.5, 1.0, -0.5, 1.5]),
            scale: [
                pick(&mut rng, &[1.0, 3.0, 0.25]),
                pick(&mut rng, &[1.0, 2.0, 0.5]),
            ],
        }
    }

    /// Whether sampling in mode `name` filters over an anisotropic footprint: the modes that take
    /// the level of detail from derivatives, with anisotropy above 1.
    fn anisotropic_footprint(&self, name: &str) -> bool {
        self.anisotropy > 1 && !matches!(name, LEVEL_MODE | GATHER_MODE | LOAD_MODE)
    }

    fn params(&self, mode: u32) -> Vec<u8> {
        [
            mode,
            self.value.to_bits(),
            self.scale[0].to_bits(),
            self.scale[1].to_bits(),
        ]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect()
    }

    fn direct3d(&self) -> D3D11_SAMPLER_DESC {
        let [min, mag, mip] = self.linear.map(|l| l as i32);
        let address = self.address.map(|a| match a {
            0 => D3D11_TEXTURE_ADDRESS_WRAP,
            1 => D3D11_TEXTURE_ADDRESS_MIRROR,
            2 => D3D11_TEXTURE_ADDRESS_CLAMP,
            3 => D3D11_TEXTURE_ADDRESS_BORDER,
            _ => D3D11_TEXTURE_ADDRESS_MIRROR_ONCE,
        });
        D3D11_SAMPLER_DESC {
            Filter: if self.anisotropy > 1 {
                D3D11_FILTER_ANISOTROPIC
            } else {
                D3D11_FILTER(min << 4 | mag << 2 | mip)
            },
            AddressU: address[0],
            AddressV: address[1],
            AddressW: address[2],
            MipLODBias: self.bias,
            MaxAnisotropy: self.anisotropy,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: self.border,
            MinLOD: self.lod[0],
            MaxLOD: self.lod[1],
        }
    }

    /// The back ends' own sampler, where they have this state natively.
    fn native(&self) -> Option<SamplerDesc> {
        if self.bias != 0.0 || self.address.iter().any(|&a| a > 2) {
            return None;
        }
        let filter = |l: bool| if l { Filter::Linear } else { Filter::Nearest };
        Some(SamplerDesc {
            min: filter(self.linear[0]),
            mag: filter(self.linear[1]),
            mip: filter(self.linear[2]),
            address: self.address.map(|a| match a {
                0 => AddressMode::Repeat,
                1 => AddressMode::Mirror,
                _ => AddressMode::Clamp,
            }),
            lod_range: self.lod.map(f32::to_bits),
            anisotropy: self.anisotropy as u16,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
    }

    /// The helper values the variant reads, for a texture of `format`.
    fn helper(&self, value: HelperValue, format: Format) -> Vec<u32> {
        match value {
            HelperValue::SamplerState { .. } => {
                let mut words = self.linear.map(u32::from).to_vec();
                words.extend(self.address);
                words.extend([
                    self.anisotropy,
                    self.lod[0].to_bits(),
                    self.lod[1].to_bits(),
                ]);
                words
            }
            HelperValue::BorderColor { .. } => self.border.map(f32::to_bits).to_vec(),
            HelperValue::LodBias { .. } => vec![self.bias.to_bits()],
            HelperValue::TextureLevels { .. } => vec![LEVELS],
            HelperValue::TextureComponents { .. } => vec![FORMAT_FACTS[format as usize].components],
            other => panic!("no value for {other:?}"),
        }
    }
}

/// The texture's levels: seeded values in [0, 1] for R32_FLOAT, seeded bytes for the others.
fn levels(format: Format, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5eed);
    (0..LEVELS)
        .map(|n| {
            let texels = (TEX >> n) * (TEX >> n);
            match format {
                Format::R32Float => (0..texels).flat_map(|_| rng.f32().to_le_bytes()).collect(),
                _ => (0..texels * FORMAT_FACTS[format as usize].block.2)
                    .map(|_| rng.u8(..))
                    .collect(),
            }
        })
        .collect()
}

/// Real Direct3D 11 drawing pixel program `pixel` (fxc's build) over the plane.
fn direct3d_11(
    d3d: &Gpu,
    pixel: &str,
    format: Format,
    texels: &[Vec<u8>],
    case: &Case,
    mode: u32,
) -> Vec<u8> {
    d3d.program(
        &bytecode("vs_filter.dxbc"),
        &bytecode(&format!("{pixel}.dxbc")),
        &[],
    )
    .unwrap();
    let constants = d3d
        .immutable(&case.params(mode), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let mut desc = d3d11::tex2d(TEX, TEX, dxgi(format), D3D11_BIND_SHADER_RESOURCE);
    desc.MipLevels = LEVELS;
    let size = FORMAT_FACTS[format as usize].block.2;
    let data: Vec<_> = texels
        .iter()
        .enumerate()
        .map(|(n, t)| d3d11::data(t, (TEX >> n) * size, 0))
        .collect();
    let texture = d3d.texture2d(&desc, &data).unwrap();
    let view = d3d.srv(&texture.cast().unwrap(), None).unwrap();
    let sampler = d3d.sampler(&case.direct3d()).unwrap();
    let (target, rtv) = d3d
        .target(SIZE, SIZE, DXGI_FORMAT_R32G32B32A32_FLOAT)
        .unwrap();
    unsafe {
        let c = &d3d.context;
        c.PSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.PSSetShaderResources(0, Some(&[Some(view)]));
        c.PSSetSamplers(0, Some(&[Some(sampler)]));
        c.RSSetState(&d3d.rasterizer(&d3d11::solid()).unwrap());
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        d3d.viewport(SIZE, SIZE);
        c.Draw(6, 0);
    }
    read(d3d, &target, Format::Rgba32Float)
}

fn source(c: &[u8], dxil: bool) -> Source<'_> {
    if dxil {
        Source::Dxil(c)
    } else {
        Source::Dxbc(c)
    }
}

/// Draws pixel program `pixel` through a back end: with `key` the variant (its helper values from `case`), without it the
/// plain program through the back end's own sampler.
#[allow(clippy::too_many_arguments)]
fn ours<B: Backend>(
    b: &mut B,
    pixel: &str,
    route: &str,
    format: Format,
    texels: &[Vec<u8>],
    case: &Case,
    mode: u32,
    key: &VariantKey,
    native: Option<SamplerDesc>,
) -> Vec<u8> {
    let dxil = route == "dxil";
    let ps_code = bytecode(&format!("{pixel}.{route}"));
    let vs_code = bytecode(&format!("vs_filter.{route}"));
    let ps = convert(source(&ps_code, dxil), key, None).unwrap();
    let vs = convert(source(&vs_code, dxil), &VariantKey::default(), None).unwrap();
    assert!(vs.reflection.bindings.is_empty());
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let params = case.params(mode);
    let constants = buffer(b, &params, BufferUsage::UNIFORM);
    let (sampled, view) = texture_levels(
        b,
        format,
        TEX,
        LEVELS,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    );
    let size = FORMAT_FACTS[format as usize].block.2;
    for (level, data) in texels.iter().enumerate() {
        let side = TEX >> level;
        b.write_texture(
            &TextureRegion {
                texture: &sampled,
                mip: level as u32,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: side,
                height: side,
                depth_or_layers: 1,
            },
            data,
            side * size,
            side,
            WriteMode::Ordered,
        );
    }
    let sampler = b
        .create_sampler(&native.unwrap_or(SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [AddressMode::Clamp; 3],
            lod_range: [0, f32::MAX.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        }))
        .unwrap();
    let mut entries = Vec::new();
    for r in &ps.reflection.bindings {
        assert_eq!(r.register, 0, "{r:?}");
        let resource = match r.kind {
            SlotKind::Uniform => BindingResource::UniformBuffer(BufferRange {
                buffer: &constants,
                offset: 0,
                size: params.len() as u64,
            }),
            SlotKind::Texture => BindingResource::Texture(&view),
            SlotKind::Sampler => BindingResource::Sampler(&sampler),
            k => panic!("no case binds {k:?}"),
        };
        entries.push(BindingEntry {
            binding: r.binding,
            stages: StageSet::FRAGMENT,
            resource,
        });
    }
    let set = b.create_binding_set(&entries).unwrap();
    let mut helpers = vec![0u8; ps.reflection.helper_size as usize];
    for slot in &ps.reflection.helpers {
        for (i, w) in case.helper(slot.value, format).iter().enumerate() {
            let at = slot.offset as usize + i * 4;
            helpers[at..at + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    let helper_buffer = (!helpers.is_empty()).then(|| buffer(b, &helpers, BufferUsage::UNIFORM));
    let helper_set = helper_buffer.as_ref().map(|buffer| {
        b.create_binding_set(&[BindingEntry {
            binding: wcontainers_gpu::shader::ShaderStage::helper_binding(
                wcontainers_gpu::shader::ShaderStage::Fragment,
            ),
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap()
    });
    let vertex_layout = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams: [None; 32],
            attributes: [None; 32],
        })
        .unwrap();
    let blend = b
        .create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; 8],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .unwrap();
    let raster = b
        .create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::Clockwise,
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
        .unwrap();
    let (target, target_view) = texture(
        b,
        Format::Rgba32Float,
        SIZE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
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
        load: wcontainers_gpu::draw::LoadOp::Clear([0; 4]),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    let side = (SIZE as f32).to_bits();
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [None, Some(&set), None, helper_set.as_ref()],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x: 0,
            y: 0,
            width: side,
            height: side,
            min_depth: 0.0f32.to_bits(),
            max_depth: 1.0f32.to_bits(),
        }],
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
    let mut out = vec![0; (SIZE * SIZE * 16) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: SIZE,
                height: SIZE,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(
        b.status(),
        wcontainers_gpu::error::DeviceStatus::Ok,
        "{key:?}"
    );
    out
}

/// Each texel's largest channel difference (infinite where a NaN differs).
fn differences(a: &[u8], b: &[u8]) -> Vec<f32> {
    let floats = |t: &[u8]| -> Vec<f32> {
        t.chunks(4)
            .map(|w| f32::from_le_bytes(w.try_into().unwrap()))
            .collect()
    };
    let (a, b) = (floats(a), floats(b));
    a.chunks(4)
        .zip(b.chunks(4))
        .map(|(x, y)| {
            let d = x
                .iter()
                .zip(y)
                .map(|(p, q)| {
                    if p.to_bits() == q.to_bits() {
                        0.0
                    } else {
                        (p - q).abs()
                    }
                })
                .fold(0.0, f32::max);
            if d.is_nan() { f32::INFINITY } else { d }
        })
        .collect()
}

/// How many texels are past `allowance`, and the largest difference.
fn count(d: &[f32], allowance: f32) -> (usize, f32) {
    (
        d.iter().filter(|&&d| d > allowance).count(),
        d.iter().copied().fold(0.0, f32::max),
    )
}

/// How far the level of detail is moved to find a texel's other side of a decision.
const SHIFT: f32 = 1.0 / 256.0;

#[test]
fn filtering_in_the_program_matches_direct3d() {
    let decoded = DECODED.iter().map(|&(format, _)| format);
    let formats: Vec<_> = [Format::R32Float, Format::Rgba8Unorm]
        .into_iter()
        .chain(decoded)
        .collect();
    samples_match_direct3d(FILTER, &MODES, &formats);
}

/// Samples with Direct3D's per-sample clamp (`ps_clamp`), filtered by the back ends' samplers and in
/// the program.
#[test]
fn clamped_samples_match_direct3d() {
    samples_match_direct3d(CLAMP, &CLAMP_MODES, &[Format::Rgba8Unorm]);
}

/// Pixel program `pixel` sampling in each of `modes` (its constant's values in order) textures of
/// each of `formats`, drawn by the back ends against real Direct3D 11 on the GPU and on warp.
fn samples_match_direct3d(pixel: &str, modes: &[&str], formats: &[Format]) {
    let seed: u64 = std::env::var("WCONTAINERS_FILTER_SEED").map_or(1, |s| s.parse().unwrap());
    let cases: u64 = std::env::var("WCONTAINERS_FILTER_CASES").map_or(8, |s| s.parse().unwrap());
    let mut gpu = webgpu();
    let mut gl = webgl();
    let device = |driver| {
        d3d11::create(
            driver,
            false,
            &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
        )
        .unwrap()
    };
    let d3d = device(wcontainers_graphics_test::Driver::Hardware);
    let warp = device(wcontainers_graphics_test::Driver::Warp);
    // The pictures kept are named by the program without its stage.
    let prefix = pixel.trim_start_matches("ps_");
    let mut failed = Vec::new();
    let mut known = Vec::new();
    let mut compared = 0;
    for case_seed in seed..seed + cases {
        let case = Case::new(case_seed);
        println!("case {case_seed}: {case:?}");
        for &format in formats {
            let texels = levels(format, case_seed);
            // R32_FLOAT is filtered by its texture's bit, RGBA8 by the sampler's border bit, and the
            // normalised formats are decoded from their integer formats and filtered in the program.
            let stored = DECODED.iter().find(|&&(f, _)| f == format).map(|&(_, s)| s);
            let key = match (format, stored) {
                (_, Some(_)) => VariantKey {
                    decoded: vec![(0, format)],
                    lod_bias: 1,
                    ..Default::default()
                },
                (Format::R32Float, None) => VariantKey {
                    filter_in_shader: 1,
                    lod_bias: 1,
                    ..Default::default()
                },
                _ => VariantKey {
                    border_in_shader: 1,
                    lod_bias: 1,
                    ..Default::default()
                },
            };
            let stored = stored.unwrap_or(format);
            for (mode, name) in modes.iter().enumerate() {
                let mode = mode as u32;
                let reference = direct3d_11(&d3d, pixel, format, &texels, &case, mode);
                let software = direct3d_11(&warp, pixel, format, &texels, &case, mode);
                let between = differences(&reference, &software);
                let (apart, largest) = count(&between, FILTER_ALLOWANCE);
                // Where an anisotropic footprint matters: texels whose anisotropic sample on the GPU
                // or warp lies past the allowance from the same device's isotropic one.
                let footprint: Vec<bool> = if case.anisotropic_footprint(name) {
                    let isotropic = Case {
                        anisotropy: 1,
                        ..case.clone()
                    };
                    let gpu = differences(
                        &reference,
                        &direct3d_11(&d3d, pixel, format, &texels, &isotropic, mode),
                    );
                    let soft = differences(
                        &software,
                        &direct3d_11(&warp, pixel, format, &texels, &isotropic, mode),
                    );
                    gpu.iter()
                        .zip(&soft)
                        .map(|(g, w)| *g > FILTER_ALLOWANCE || *w > FILTER_ALLOWANCE)
                        .collect()
                } else {
                    vec![false; between.len()]
                };
                println!(
                    "  {format:?} {name} direct3d on the gpu against warp: {apart} of {} texels past {FILTER_ALLOWANCE}, largest {largest}",
                    SIZE * SIZE
                );
                for route in ["dxbc", "dxil"] {
                    // WebGL 2's fronts read level counts from the helpers.
                    let gl_key = VariantKey {
                        flags: wcontainers_gpu::reflect::VariantFlags::TEXTURE_INFO,
                        ..key.clone()
                    };
                    let mut webgpu_texels = Vec::new();
                    for back_end in ["webgpu", "webgl"] {
                        // The variant with the sampler's bias moved by `shift`.
                        let mut draw = |shift: f32| {
                            let moved = Case {
                                bias: case.bias + shift,
                                ..case.clone()
                            };
                            if back_end == "webgpu" {
                                ours(
                                    &mut gpu, pixel, route, stored, &texels, &moved, mode, &key,
                                    None,
                                )
                            } else {
                                ours(
                                    &mut gl, pixel, route, stored, &texels, &moved, mode, &gl_key,
                                    None,
                                )
                            }
                        };
                        let picture = draw(0.0);
                        let gpu_apart = differences(&reference, &picture);
                        let soft_apart = differences(&software, &picture);
                        let (gpu_count, gpu_largest) = count(&gpu_apart, FILTER_ALLOWANCE);
                        let (soft_count, soft_largest) = count(&soft_apart, FILTER_ALLOWANCE);
                        // Texels equal to neither Direct3D, then those still so with the level of
                        // detail moved by 1/256 either way: a texel whose level of detail lies
                        // that close to a decision (magnification, the nearest level) may fall
                        // either side, as the GPU and warp themselves do.
                        let mut neither: Vec<usize> = (0..gpu_apart.len())
                            .filter(|&n| {
                                gpu_apart[n] > FILTER_ALLOWANCE && soft_apart[n] > FILTER_ALLOWANCE
                            })
                            .collect();
                        let first = neither.len();
                        if !neither.is_empty() {
                            let moved = [draw(-SHIFT), draw(SHIFT)];
                            neither.retain(|&n| {
                                moved.iter().all(|m| {
                                    differences(
                                        &reference[n * 16..n * 16 + 16],
                                        &m[n * 16..n * 16 + 16],
                                    )[0] > FILTER_ALLOWANCE
                                        && differences(
                                            &software[n * 16..n * 16 + 16],
                                            &m[n * 16..n * 16 + 16],
                                        )[0] > FILTER_ALLOWANCE
                                })
                            });
                        }
                        // Where the GPU and warp give different answers the specification leaves the
                        // texel to the implementation (a point sample on a texel's edge, a level
                        // blended otherwise): a third answer there passes when it lies no further
                        // from one of them than they lie from each other.
                        let before = neither.len();
                        neither.retain(|&n| {
                            between[n] <= FILTER_ALLOWANCE
                                || gpu_apart[n].min(soft_apart[n]) > between[n]
                        });
                        let third = before - neither.len();
                        println!(
                            "  {format:?} {name} {route} {back_end}: {} of {} texels past {FILTER_ALLOWANCE} from both ({first} before moving the level of detail, {third} more where they differ by more); from the gpu {gpu_count} (largest {gpu_largest}), from warp {soft_count} (largest {soft_largest})",
                            neither.len(),
                            SIZE * SIZE
                        );
                        let floats = |t: &[u8], n: usize| -> Vec<f32> {
                            t[n * 16..n * 16 + 16]
                                .chunks(4)
                                .map(|w| f32::from_le_bytes(w.try_into().unwrap()))
                                .collect()
                        };
                        for &n in neither.iter().take(3) {
                            println!(
                                "    ({}, {}) {:?}, gpu {:?}, warp {:?}",
                                n as u32 % SIZE,
                                n as u32 / SIZE,
                                floats(&picture, n),
                                floats(&reference, n),
                                floats(&software, n)
                            );
                        }
                        keep(
                            &format!("{prefix}_{case_seed}_{format:?}_{name}_{route}_{back_end}"),
                            &picture,
                            Format::Rgba32Float,
                        );
                        compared += 1;
                        if !neither.is_empty() {
                            let what = format!("{case_seed} {format:?} {name} {route} {back_end}");
                            // Known failure: the anisotropic footprint is the implementation's own
                            // (the GPU's and warp's differ from each other), and the program's
                            // taps along the major axis are a third (docs/status-graphics-shaders.md).
                            // It is the cause only where the footprint matters; an explicit level
                            // or a gather takes no footprint and must match.
                            let elsewhere = neither.iter().filter(|&&n| !footprint[n]).count();
                            if elsewhere == 0 {
                                known.push(what);
                            } else {
                                failed.push(format!(
                                    "{what} ({elsewhere} texels where no anisotropic footprint matters)"
                                ));
                            }
                        }
                        if back_end == "webgpu" {
                            webgpu_texels = picture;
                        }
                    }
                    // The back ends' own samplers, for what they filter natively: the variant's
                    // texels against WebGPU's own sampler, or warp where the hardware blends levels
                    // otherwise; and the own sampler against Direct3D on the same GPU.
                    if let (Some(native), Format::Rgba8Unorm) = (case.native(), format) {
                        let plain = VariantKey::default();
                        let own = ours(
                            &mut gpu,
                            pixel,
                            route,
                            format,
                            &texels,
                            &case,
                            mode,
                            &plain,
                            Some(native),
                        );
                        keep(
                            &format!("{prefix}_{case_seed}_{format:?}_{name}_{route}_own"),
                            &own,
                            Format::Rgba32Float,
                        );
                        let native = differences(&webgpu_texels, &own);
                        let soft = differences(&software, &webgpu_texels);
                        let (apart, largest) = count(&native, FILTER_ALLOWANCE);
                        let neither: Vec<usize> = (0..native.len())
                            .filter(|&n| native[n] > FILTER_ALLOWANCE && soft[n] > FILTER_ALLOWANCE)
                            .collect();
                        let (own_apart, own_largest) =
                            count(&differences(&reference, &own), FILTER_ALLOWANCE);
                        // Of those, the texels the main comparison allows and this one does not
                        // (A18): where the GPU and warp differ, and at a level decision (within the
                        // allowance of the own sampler or warp with the level of detail moved by
                        // 1/256).
                        let where_they_differ = neither
                            .iter()
                            .filter(|&&n| between[n] > FILTER_ALLOWANCE)
                            .count();
                        let at_decision = if neither.is_empty() {
                            0
                        } else {
                            let moved = [-SHIFT, SHIFT].map(|shift| {
                                let moved = Case {
                                    bias: case.bias + shift,
                                    ..case.clone()
                                };
                                ours(
                                    &mut gpu, pixel, route, stored, &texels, &moved, mode, &key,
                                    None,
                                )
                            });
                            let near = |a: &[u8], m: &[u8], n: usize| {
                                differences(&a[n * 16..n * 16 + 16], &m[n * 16..n * 16 + 16])[0]
                                    <= FILTER_ALLOWANCE
                            };
                            neither
                                .iter()
                                .filter(|&&n| {
                                    moved
                                        .iter()
                                        .any(|m| near(&own, m, n) || near(&software, m, n))
                                })
                                .count()
                        };
                        println!(
                            "  {format:?} {name} {route} webgpu against its own sampler: {} of {} texels past {FILTER_ALLOWANCE} from it and warp ({at_decision} of them at a level decision, {where_they_differ} where the gpu and warp differ); from it {apart} (largest {largest}); its own sampler from direct3d on the gpu {own_apart} (largest {own_largest})",
                            neither.len(),
                            SIZE * SIZE
                        );
                        compared += 1;
                        if !neither.is_empty() {
                            let what = format!(
                                "{case_seed} {format:?} {name} {route} webgpu's own sampler"
                            );
                            let elsewhere = neither.iter().filter(|&&n| !footprint[n]).count();
                            if elsewhere == 0 {
                                known.push(what);
                            } else {
                                failed.push(format!(
                                    "{what} ({elsewhere} texels where no anisotropic footprint matters)"
                                ));
                            }
                        }
                    }
                }
                keep(
                    &format!("{prefix}_{case_seed}_{format:?}_{name}_direct3d"),
                    &reference,
                    Format::Rgba32Float,
                );
                keep(
                    &format!("{prefix}_{case_seed}_{format:?}_{name}_warp"),
                    &software,
                    Format::Rgba32Float,
                );
            }
        }
    }
    println!(
        "{compared} comparisons, {} failed, {} known failures (anisotropic): {known:?}",
        failed.len(),
        known.len()
    );
    assert!(failed.is_empty(), "differ: {failed:?}");
}

/// How far a level of detail may lie from Direct3D's: one and a half steps of the 8 fraction bits
/// the GPU gives levels of detail in (its values lie 1/256 apart), half a step for its rounding and
/// one for derivatives rounded apart.
const LOD_ALLOWANCE: f32 = 1.5 / (1u32 << LOD_FRACTION_BITS) as f32;
const LOD_FRACTION_BITS: u32 = 8;

#[test]
fn levels_of_detail_match_direct3d() {
    let seed: u64 = std::env::var("WCONTAINERS_FILTER_SEED").map_or(1, |s| s.parse().unwrap());
    let cases: u64 = std::env::var("WCONTAINERS_FILTER_CASES").map_or(8, |s| s.parse().unwrap());
    let mut gpu = webgpu();
    let mut gl = webgl();
    let device = |driver| {
        d3d11::create(
            driver,
            false,
            &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
        )
        .unwrap()
    };
    let d3d = device(wcontainers_graphics_test::Driver::Hardware);
    let warp = device(wcontainers_graphics_test::Driver::Warp);
    let format = Format::Rgba8Unorm;
    let mut failed = Vec::new();
    let mut known = Vec::new();
    let mut compared = 0;
    for case_seed in seed..seed + cases {
        let case = Case::new(case_seed);
        println!("case {case_seed}: {case:?}");
        let texels = levels(format, case_seed);
        let reference = direct3d_11(&d3d, LOD, format, &texels, &case, 0);
        let software = direct3d_11(&warp, LOD, format, &texels, &case, 0);
        let (apart, largest) = count(&differences(&reference, &software), LOD_ALLOWANCE);
        println!(
            "  direct3d on the gpu against warp: {apart} of {} texels past {LOD_ALLOWANCE}, largest {largest}",
            SIZE * SIZE
        );
        // Where an anisotropic footprint matters: texels whose level of detail on the GPU or warp
        // lies past the allowance from the same device's without anisotropy.
        let footprint: Vec<bool> = if case.anisotropy > 1 {
            let isotropic = Case {
                anisotropy: 1,
                ..case.clone()
            };
            let gpu = differences(
                &reference,
                &direct3d_11(&d3d, LOD, format, &texels, &isotropic, 0),
            );
            let soft = differences(
                &software,
                &direct3d_11(&warp, LOD, format, &texels, &isotropic, 0),
            );
            gpu.iter()
                .zip(&soft)
                .map(|(g, w)| *g > LOD_ALLOWANCE || *w > LOD_ALLOWANCE)
                .collect()
        } else {
            vec![false; (SIZE * SIZE) as usize]
        };
        for (build, back_end) in ["dxbc", "dxil"]
            .into_iter()
            .flat_map(|b| [(b, "webgpu"), (b, "webgl")])
        {
            let key = VariantKey {
                flags: if back_end == "webgl" {
                    wcontainers_gpu::reflect::VariantFlags::TEXTURE_INFO
                } else {
                    wcontainers_gpu::reflect::VariantFlags::empty()
                },
                ..Default::default()
            };
            let picture = if back_end == "webgpu" {
                ours(&mut gpu, LOD, build, format, &texels, &case, 0, &key, None)
            } else {
                ours(&mut gl, LOD, build, format, &texels, &case, 0, &key, None)
            };
            let gpu_apart = differences(&reference, &picture);
            let soft_apart = differences(&software, &picture);
            // A texel with a value past the allowance from both devices' (each value apart: the
            // devices round a level a point mip filter picks halfway differently, and the GPU's
            // level of detail is an approximation of its own).
            let value =
                |t: &[u8], n: usize| f32::from_le_bytes(t[n * 4..n * 4 + 4].try_into().unwrap());
            let mut neither: Vec<usize> = (0..gpu_apart.len())
                .filter(|&n| {
                    (n * 4..n * 4 + 4).any(|v| {
                        let ours = value(&picture, v);
                        (ours - value(&reference, v)).abs() > LOD_ALLOWANCE
                            && (ours - value(&software, v)).abs() > LOD_ALLOWANCE
                    })
                })
                .collect();
            // The farthest first.
            neither.sort_by(|&m, &n| soft_apart[n].total_cmp(&soft_apart[m]));
            let (gpu_count, gpu_largest) = count(&gpu_apart, LOD_ALLOWANCE);
            let (soft_count, soft_largest) = count(&soft_apart, LOD_ALLOWANCE);
            println!(
                "  {build} {back_end}: {} of {} texels past {LOD_ALLOWANCE} from both; from the gpu {gpu_count} (largest {gpu_largest}), from warp {soft_count} (largest {soft_largest})",
                neither.len(),
                SIZE * SIZE
            );
            let floats = |t: &[u8], n: usize| -> Vec<f32> {
                t[n * 16..n * 16 + 16]
                    .chunks(4)
                    .map(|w| f32::from_le_bytes(w.try_into().unwrap()))
                    .collect()
            };
            for &n in neither.iter().take(3) {
                println!(
                    "    ({}, {}) {:?}, gpu {:?}, warp {:?}",
                    n as u32 % SIZE,
                    n as u32 / SIZE,
                    floats(&picture, n),
                    floats(&reference, n),
                    floats(&software, n)
                );
            }
            keep(
                &format!("lod_{case_seed}_{build}_{back_end}"),
                &picture,
                Format::Rgba32Float,
            );
            compared += 1;
            if !neither.is_empty() {
                let what = format!("{case_seed} {build} {back_end}");
                // Known failure: an anisotropic footprint's level of detail is the implementation's
                // own (the GPU's and warp's differ from each other), as for sampling above; it is
                // the cause only where the footprint matters.
                let elsewhere = neither.iter().filter(|&&n| !footprint[n]).count();
                if elsewhere == 0 {
                    known.push(what);
                } else {
                    failed.push(format!(
                        "{what} ({elsewhere} texels where no anisotropic footprint matters)"
                    ));
                }
            }
        }
        keep(
            &format!("lod_{case_seed}_direct3d"),
            &reference,
            Format::Rgba32Float,
        );
        keep(
            &format!("lod_{case_seed}_warp"),
            &software,
            Format::Rgba32Float,
        );
    }
    println!(
        "{compared} comparisons, {} failed, {} known failures (anisotropic): {known:?}",
        failed.len(),
        known.len()
    );
    assert!(failed.is_empty(), "differ: {failed:?}");
}
