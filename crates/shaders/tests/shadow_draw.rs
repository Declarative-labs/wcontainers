//! A texture both compared against and read plainly (`shadow.hlsl`'s `ps_shadow`, a shadow map) on the
//! WebGPU back end over a 16-bit depth texture, against real Direct3D 11 drawing the fxc build over an
//! R16_UNORM texture of the same bits on the same GPU: comparisons at two references (sampled and at
//! level zero), plain samples (implicit and at level zero), a load at the pixel and a gather. Both the
//! fxc and the dxc build are held to that picture with each plain and comparison filter (point and
//! linear): exactly, but for linear plain samples, which take Direct3D's filtering allowance. WebGPU
//! filters no depth texture through a sampler that does not compare, so a linear plain sampler takes
//! the variant filtering the texture in the program (`VariantKey::filter_in_shader`), as a front gives
//! it; comparisons stay native. Seeded texels: `WCONTAINERS_SHADOW_SEED` (default 1) and
//! `WCONTAINERS_SHADOW_CASES` seeds from it (default 4).

#![cfg(windows)]

mod common;

use common::*;
// The trait by its old name: graphics-test's Direct3D 11 device is named `Gpu` in this file.
use wcontainers_gpu::{
    AddressMode, BindingEntry, BindingResource, BlendDesc, BorderColor, BufferRange, BufferUsage,
    ColorAttachment, ColorWrites, CompareFn, CullMode, DepthStencilDesc, DrawCall, DrawRange,
    Extent, FORMAT_FACTS, FillMode, Filter, Format, FrontFace, Gpu as Backend, HELPER_SET,
    HelperValue, Interpolation, LoadOp, Origin, PassDesc, Program, ProvokingVertex, RasterDesc,
    ReadSource, RenderTargetBlend, SamplerDesc, ShaderStage, SlotKind, StageSet, StepMode, StoreOp,
    TextureRegion, TextureUsage, Topology, VariantKey, VertexAttribute, VertexFormat,
    VertexLayoutDesc, VertexStream, ViewComponent, Viewport, WriteMode, helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_graphics_test::d3d12;
use wcontainers_shaders::{ConvertError, Converted, Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

/// The shadow map's format on each side: its texels are the same 16-bit codes.
const SHADOW: Format = Format::Depth16Unorm;
const SHADOW_DXGI: DXGI_FORMAT = DXGI_FORMAT_R16_UNORM;
const TARGET: Format = Format::Rgba32Float;
/// A grid vertex: position (x, y, z), then the texture coordinate (u, v).
const POSITION_FLOATS: usize = 3;
const VERTEX_FLOATS: usize = POSITION_FLOATS + 2;
const FLOAT: usize = std::mem::size_of::<f32>();
const STRIDE: u32 = (VERTEX_FLOATS * FLOAT) as u32;
const TEXCOORD_OFFSET: u32 = (POSITION_FLOATS * FLOAT) as u32;
/// The plain sampler's slot and the comparison sampler's (`s0`, `s1`).
const PLAIN: u32 = 0;
const COMPARE: u32 = 1;

/// One scene: the plain and the comparison sampler's filters.
#[derive(Clone, Copy, Debug)]
struct Filters {
    plain: Filter,
    compare: Filter,
}

const SCENES: [Filters; 4] = [
    Filters {
        plain: Filter::Nearest,
        compare: Filter::Nearest,
    },
    Filters {
        plain: Filter::Nearest,
        compare: Filter::Linear,
    },
    Filters {
        plain: Filter::Linear,
        compare: Filter::Nearest,
    },
    Filters {
        plain: Filter::Linear,
        compare: Filter::Linear,
    },
];

/// The shadow map, as large as the target so the load at each pixel stays inside it: seeded 16-bit
/// depths over the whole range, so both references fall between texels.
fn shadow_texels(seed: u64) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed);
    (0..SIZE * SIZE)
        .flat_map(|_| rng.u16(..).to_le_bytes())
        .collect()
}

fn d3d11_filter(f: Filters) -> [D3D11_FILTER; 2] {
    match (f.plain, f.compare) {
        (Filter::Nearest, Filter::Nearest) => [
            D3D11_FILTER_MIN_MAG_MIP_POINT,
            D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
        ],
        (Filter::Nearest, Filter::Linear) => [
            D3D11_FILTER_MIN_MAG_MIP_POINT,
            D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
        ],
        (Filter::Linear, Filter::Nearest) => [
            D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
        ],
        (Filter::Linear, Filter::Linear) => [
            D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
        ],
    }
}

/// Real Direct3D 11 drawing the fxc builds over the grid; the target's texels.
fn direct3d(gpu: &Gpu, shader: &str, filters: Filters, shadow: &[u8]) -> Vec<u8> {
    let vs = bytecode("vs_transform.dxbc");
    let ps = bytecode(&format!("{shader}.dxbc"));
    let elements = [
        d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        d3d11::element(
            s!("TEXCOORD"),
            0,
            DXGI_FORMAT_R32G32_FLOAT,
            0,
            TEXCOORD_OFFSET,
            0,
        ),
    ];
    gpu.program(&vs, &ps, &elements).unwrap();
    let vertices = grid();
    let _vertex_buffer = gpu.vertices(0, bytes_of(&vertices), STRIDE).unwrap();
    let constants = gpu
        .immutable(&constants([1.0; 4]), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let texel = FORMAT_FACTS[SHADOW as usize].block.2;
    let texture = gpu
        .texture2d(
            &d3d11::tex2d(SIZE, SIZE, SHADOW_DXGI, D3D11_BIND_SHADER_RESOURCE),
            &[d3d11::data(shadow, SIZE * texel, 0)],
        )
        .unwrap();
    let view = gpu.srv(&texture.cast().unwrap(), None).unwrap();
    let samplers = d3d11_filter(filters).map(|filter| {
        Some(
            gpu.sampler(&D3D11_SAMPLER_DESC {
                Filter: filter,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_LESS,
                MaxLOD: f32::MAX,
                ..Default::default()
            })
            .unwrap(),
        )
    });
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, dxgi(TARGET)).unwrap();
    unsafe {
        let c = &gpu.context;
        c.VSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.PSSetShaderResources(0, Some(&[Some(view)]));
        c.PSSetSamplers(PLAIN, Some(&samplers));
        c.RSSetState(&raster);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.ClearRenderTargetView(&rtv, &CLEAR);
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        gpu.viewport(SIZE, SIZE);
        c.Draw((vertices.len() / VERTEX_FLOATS) as u32, 0);
    }
    read(gpu, &target, TARGET)
}

fn converted(name: &str, dxil: bool, key: &VariantKey) -> Converted {
    let ext = if dxil { "dxil" } else { "dxbc" };
    let bytes = bytecode(&format!("{name}.{ext}"));
    let source = if dxil {
        Source::Dxil(&bytes)
    } else {
        Source::Dxbc(&bytes)
    };
    convert(source, key, None).unwrap()
}

/// The shader comparing against the shadow map and reading its first component, and the one reading
/// it whole.
const SHADOW_SHADER: &str = "ps_shadow";
const WIDE_SHADER: &str = "ps_shadow_wide";

/// The texture slot of the shadow map (`t0`).
const SHADOW_SLOT: u32 = 0;

/// The pixel program's helper block for the sampler state `plain` and the shadow map's format.
fn helpers(c: &Converted, plain: Filter) -> Vec<u8> {
    let mut block = vec![0; c.reflection.helper_size as usize];
    for h in &c.reflection.helpers {
        let words: Vec<u32> = match h.value {
            HelperValue::SamplerState { sampler: PLAIN } => {
                let mut words = vec![plain as u32; 3];
                words.extend([AddressMode::Clamp as u32; 3]);
                words.extend([1, 0.0f32.to_bits(), f32::MAX.to_bits()]);
                words
            }
            HelperValue::BorderColor { sampler: PLAIN } => vec![0; 4],
            HelperValue::TextureComponents {
                texture: SHADOW_SLOT,
            } => vec![FORMAT_FACTS[SHADOW as usize].components],
            other => panic!("no value for {other:?}"),
        };
        let at = h.offset as usize;
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        block[at..at + bytes.len()].copy_from_slice(&bytes);
    }
    block
}

/// Direct3D 12's encoding of a view's component mapping (`D3D12_ENCODE_SHADER_4_COMPONENT_MAPPING`):
/// three bits a component, a component's index or 4 for zero and 5 for one, then the always-set bit.
fn d3d12_mapping(mapping: [ViewComponent; 4]) -> u32 {
    const SOURCE_BITS: u32 = 3;
    const ALWAYS_SET: u32 = 1 << (SOURCE_BITS * 4);
    mapping
        .iter()
        .enumerate()
        .fold(ALWAYS_SET, |bits, (i, &m)| {
            bits | ((m as u32) << (SOURCE_BITS * i as u32))
        })
}

fn d3d12_compare(compare: CompareFn) -> D3D12_COMPARISON_FUNC {
    match compare {
        CompareFn::Less => D3D12_COMPARISON_FUNC_LESS,
        CompareFn::Greater => D3D12_COMPARISON_FUNC_GREATER,
        other => panic!("no scene compares by {other:?}"),
    }
}

/// Real Direct3D 12 drawing the route's builds over the grid with point samplers, the shadow map a
/// 16-bit depth texture read through an R16_UNORM view mapped by `mapping` and compared by
/// `compare`; the target's texels.
fn direct3d12(
    gpu: &d3d12::Gpu,
    shader: &str,
    dxil: bool,
    compare: CompareFn,
    mapping: [ViewComponent; 4],
    shadow: &[u8],
) -> Vec<u8> {
    let ext = if dxil { "dxil" } else { "dxbc" };
    let vs = bytecode(&format!("vs_transform.{ext}"));
    let ps = bytecode(&format!("{shader}.{ext}"));
    let range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        ..Default::default()
    };
    let parameters = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Descriptor: D3D12_ROOT_DESCRIPTOR::default(),
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX,
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &range,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
    ];
    let sampler = |register: u32, filter, compare| D3D12_STATIC_SAMPLER_DESC {
        Filter: filter,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        MaxAnisotropy: 1,
        ComparisonFunc: compare,
        MaxLOD: f32::MAX,
        ShaderRegister: register,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        ..Default::default()
    };
    let samplers = [
        sampler(
            PLAIN,
            D3D12_FILTER_MIN_MAG_MIP_POINT,
            D3D12_COMPARISON_FUNC_NEVER,
        ),
        sampler(
            COMPARE,
            D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
            d3d12_compare(compare),
        ),
    ];
    let root = gpu
        .root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
            Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                    NumParameters: parameters.len() as u32,
                    pParameters: parameters.as_ptr(),
                    NumStaticSamplers: samplers.len() as u32,
                    pStaticSamplers: samplers.as_ptr(),
                    Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                },
            },
        })
        .unwrap();
    let layout = [
        d3d12::element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
        d3d12::element(s!("TEXCOORD"), DXGI_FORMAT_R32G32_FLOAT, TEXCOORD_OFFSET),
    ];
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::ManuallyDrop::new(Some(root.clone())),
        VS: d3d12::bytecode(&vs),
        PS: d3d12::bytecode(&ps),
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            DepthClipEnable: true.into(),
            ..Default::default()
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: layout.as_ptr(),
            NumElements: layout.len() as u32,
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    };
    desc.BlendState.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    desc.RTVFormats[0] = dxgi(TARGET);
    let pipeline: ID3D12PipelineState =
        unsafe { gpu.device.CreateGraphicsPipelineState(&desc) }.unwrap();
    drop(std::mem::ManuallyDrop::into_inner(desc.pRootSignature));
    let map = gpu
        .committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &d3d12::tex2d_desc(
                SIZE,
                SIZE,
                DXGI_FORMAT_R16_TYPELESS,
                D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
            ),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )
        .unwrap();
    gpu.fill_texture(&map, shadow, SIZE * FORMAT_FACTS[SHADOW as usize].block.2)
        .unwrap();
    let views = gpu
        .descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1, true)
        .unwrap();
    let mut view = D3D12_SHADER_RESOURCE_VIEW_DESC {
        Format: SHADOW_DXGI,
        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
        Shader4ComponentMapping: d3d12_mapping(mapping),
        ..Default::default()
    };
    view.Anonymous.Texture2D.MipLevels = 1;
    unsafe {
        gpu.device
            .CreateShaderResourceView(&map, Some(&view), gpu.cpu(&views, 0))
    };
    let (target, rtv_heap) = gpu.target(SIZE, SIZE, dxgi(TARGET)).unwrap();
    let vertices = grid();
    let vertex_buffer = gpu.upload(bytes_of(&vertices)).unwrap();
    let constant_buffer = gpu.upload(&constants([1.0; 4])).unwrap();
    let list = &gpu.list;
    unsafe {
        list.ResourceBarrier(&[d3d12::transition(
            &map,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )]);
        let rtv = gpu.cpu(&rtv_heap, 0);
        list.ClearRenderTargetView(rtv, &CLEAR, None);
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        d3d12::area(list, 0, 0, SIZE, SIZE);
        list.SetGraphicsRootSignature(&root);
        list.SetPipelineState(&pipeline);
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetGraphicsRootConstantBufferView(0, constant_buffer.GetGPUVirtualAddress());
        list.SetGraphicsRootDescriptorTable(1, gpu.gpu(&views, 0));
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertex_buffer.GetGPUVirtualAddress(),
                SizeInBytes: size_of_val(vertices.as_slice()) as u32,
                StrideInBytes: STRIDE,
            }]),
        );
        list.DrawInstanced((vertices.len() / VERTEX_FLOATS) as u32, 1, 0, 0);
    }
    for resource in [&map, &vertex_buffer, &constant_buffer] {
        gpu.keep(resource);
    }
    gpu.read(&target, 0, D3D12_RESOURCE_STATE_RENDER_TARGET)
        .unwrap()
        .into_rgba32f()
        .into_raw()
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// The same through the WebGPU back end, the shadow map a 16-bit depth texture written from the CPU
/// and read through a view with the component mapping `mapping` where there is one, compared by
/// `compare`.
fn ours<B: Backend>(
    b: &mut B,
    shader: &str,
    filters: Filters,
    shadow: &[u8],
    dxil: bool,
    mapping: Option<[ViewComponent; 4]>,
    compare: CompareFn,
) -> Vec<u8> {
    let in_program = filters.plain == Filter::Linear;
    let ps_key = VariantKey {
        filter_in_shader: u128::from(in_program) << SHADOW_SLOT,
        swizzled: mapping.map(|m| (SHADOW_SLOT, m)).into_iter().collect(),
        ..VariantKey::default()
    };
    let ps = converted(shader, dxil, &ps_key);
    // As a front links the stages: the vertex program writes each output as the pixel program reads it.
    let vs_key = VariantKey {
        output_interpolation: ps
            .reflection
            .inputs
            .iter()
            .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
            .map(|v| (v.location, v.interpolation))
            .collect(),
        ..VariantKey::default()
    };
    let vs = converted("vs_transform", dxil, &vs_key);
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let mut layout = VertexLayoutDesc {
        streams: [None; 32],
        attributes: [None; 32],
    };
    layout.streams[0] = Some(VertexStream {
        stride: STRIDE,
        step: StepMode::Vertex,
    });
    for input in vs.reflection.inputs.iter().filter(|i| i.system.is_none()) {
        let (offset, format) = match input.semantic.as_str() {
            "POSITION" => (0, VertexFormat::Float32x3),
            "TEXCOORD" => (TEXCOORD_OFFSET, VertexFormat::Float32x2),
            other => panic!("no vertex data for {other}"),
        };
        layout.attributes[input.location as usize] = Some(VertexAttribute {
            location: input.location,
            stream: 0,
            offset,
            format,
        });
    }
    let vertex_layout = b.create_vertex_layout(&layout).unwrap();
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
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [u8::MAX; 2],
            stencil_write_mask: [0; 2],
            depth_bounds: None,
        })
        .unwrap();
    let (target, target_view) = texture_levels(
        b,
        TARGET,
        SIZE,
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let (map, map_view) = texture_levels(
        b,
        SHADOW,
        SIZE,
        1,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    );
    let texel = FORMAT_FACTS[SHADOW as usize].block.2;
    b.write_texture(
        &TextureRegion {
            texture: &map,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: SIZE,
            height: SIZE,
            depth_or_layers: 1,
        },
        shadow,
        SIZE * texel,
        SIZE,
        WriteMode::Ordered,
    );
    let sampler = |b: &mut B, filter: Filter, compare: Option<CompareFn>| {
        b.create_sampler(&SamplerDesc {
            min: filter,
            mag: filter,
            mip: filter,
            address: [AddressMode::Clamp; 3],
            lod_range: [0.0f32.to_bits(), f32::MAX.to_bits()],
            anisotropy: 1,
            compare,
            border: BorderColor::TransparentBlack,
        })
        .unwrap()
    };
    // Filtered in the program from its state in the helper block, the sampler itself filters nothing.
    let plain = sampler(
        b,
        if in_program {
            Filter::Nearest
        } else {
            filters.plain
        },
        None,
    );
    let compare = sampler(b, filters.compare, Some(compare));
    let vertices = grid();
    let vertex_buffer = buffer(b, bytes_of(&vertices), BufferUsage::VERTEX);
    let constants = constants([1.0; 4]);
    let constants_buffer = buffer(b, &constants, BufferUsage::UNIFORM);
    let mut entries: [Vec<BindingEntry<'_, B>>; 4] = Default::default();
    for (c, stages) in [(&vs, StageSet::VERTEX), (&ps, StageSet::FRAGMENT)] {
        for r in &c.reflection.bindings {
            // By register: a DXIL program's slots rank the registers it declares.
            let resource = match (r.kind, r.register) {
                (SlotKind::Uniform, 0) => BindingResource::UniformBuffer(BufferRange {
                    buffer: &constants_buffer,
                    offset: 0,
                    size: constants.len() as u64,
                }),
                (SlotKind::Texture, SHADOW_SLOT) => BindingResource::Texture(&map_view),
                (SlotKind::Sampler, PLAIN) => BindingResource::Sampler(&plain),
                (SlotKind::Sampler, COMPARE) => BindingResource::Sampler(&compare),
                other => panic!("no binding for {other:?}"),
            };
            entries[r.set as usize].push(BindingEntry {
                binding: r.binding,
                stages,
                resource,
            });
        }
    }
    let helper_bytes = |c: &Converted| helpers(c, filters.plain);
    let helpers: Vec<_> = [(&vs, ShaderStage::Vertex), (&ps, ShaderStage::Fragment)]
        .into_iter()
        .filter(|(c, _)| c.reflection.helper_size > 0)
        .map(|(c, stage)| {
            (
                buffer(b, &helper_bytes(c), BufferUsage::UNIFORM),
                c.reflection.helper_size,
                stage,
            )
        })
        .collect();
    for (buffer, size, stage) in &helpers {
        entries[HELPER_SET as usize].push(BindingEntry {
            binding: helper_binding(*stage),
            stages: match stage {
                ShaderStage::Vertex => StageSet::VERTEX,
                _ => StageSet::FRAGMENT,
            },
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer,
                offset: 0,
                size: u64::from(*size),
            }),
        });
    }
    let sets: Vec<_> = entries
        .iter()
        .map(|e| (!e.is_empty()).then(|| b.create_binding_set(e).unwrap()))
        .collect();
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
        load: LoadOp::Clear(CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: std::array::from_fn(|i| sets[i].as_ref()),
        vertex_buffers: &[Some(wcontainers_gpu::VertexBinding {
            buffer: &vertex_buffer,
            offset: 0,
        })],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x: 0,
            y: 0,
            width: (SIZE as f32).to_bits(),
            height: (SIZE as f32).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: (vertices.len() / VERTEX_FLOATS) as u32,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let mut texels = vec![0; (SIZE * SIZE * FORMAT_FACTS[TARGET as usize].block.2) as usize];
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
        &mut texels,
    )
    .unwrap();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    texels
}

/// Texels with a float past its channel's allowance, and the largest difference of any float.
fn apart(a: &[u8], b: &[u8], allowed: [f32; 4]) -> (usize, f32) {
    let texel = FORMAT_FACTS[TARGET as usize].block.2 as usize;
    let float = |c: &[u8]| f32::from_le_bytes(c.try_into().unwrap());
    let mut count = 0;
    let mut largest = 0f32;
    for (x, y) in a.chunks(texel).zip(b.chunks(texel)) {
        let mut past = false;
        for ((p, q), allowed) in x.chunks(FLOAT).zip(y.chunks(FLOAT)).zip(allowed) {
            let d = (float(p) - float(q)).abs();
            largest = largest.max(d);
            past |= if allowed == 0.0 { p != q } else { d > allowed };
        }
        count += usize::from(past);
    }
    (count, largest)
}

/// What each output channel may differ by: exact, but for the plain samples (green, two summed)
/// filtered linearly in the program, which take Direct3D's filtering allowance each.
fn allowed(filters: Filters) -> [f32; 4] {
    let plain = match filters.plain {
        Filter::Linear => FILTER_ALLOWANCE * 2.0,
        Filter::Nearest => 0.0,
    };
    [0.0, plain, 0.0, 0.0]
}

#[test]
fn textures_compared_and_read_plainly_draw_as_direct3d_does() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_SHADOW_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_SHADOW_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let shadow = shadow_texels(seed);
        for filters in SCENES {
            let reference = direct3d(&gpu, SHADOW_SHADER, filters, &shadow);
            let name = format!("shadow-{seed}-{:?}-{:?}", filters.plain, filters.compare);
            keep(&format!("{name}.d3d11"), &reference, TARGET);
            for (route, dxil) in [("dxbc", false), ("dxil", true)] {
                let texels = ours(
                    &mut backend,
                    SHADOW_SHADER,
                    filters,
                    &shadow,
                    dxil,
                    None,
                    CompareFn::Less,
                );
                keep(&format!("{name}.webgpu-{route}"), &texels, TARGET);
                let (count, largest) = apart(&reference, &texels, allowed(filters));
                compared += 1;
                println!(
                    "seed {seed} {filters:?} {route}: {count} of {} texels past {:?} from Direct3D \
                     11, largest difference {largest}",
                    SIZE * SIZE,
                    allowed(filters)
                );
                if count != 0 {
                    failures.push(format!("seed {seed} {filters:?} {route}"));
                }
            }
        }
    }
    println!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The seeds a mapped-view test draws: `WCONTAINERS_SHADOW_SEED` (default 1) and
/// `WCONTAINERS_SHADOW_CASES` seeds from it (default 4).
fn seeds() -> std::ops::Range<u64> {
    let seed: u64 = std::env::var("WCONTAINERS_SHADOW_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_SHADOW_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    seed..seed + seeds
}

/// Component mappings of a depth view keeping red first, copying the depth and moving the defaults
/// into the other components.
const KEPT: [[ViewComponent; 4]; 3] = {
    use ViewComponent as C;
    [
        [C::R, C::R, C::R, C::One],
        [C::R, C::A, C::One, C::G],
        [C::R, C::Zero, C::R, C::B],
    ]
};

/// Component mappings of a depth view moving red off the first component, onto each kind of source
/// there: a default zero (green, blue), the default one (alpha), and each constant.
const MOVED: [[ViewComponent; 4]; 5] = {
    use ViewComponent as C;
    [
        [C::A, C::R, C::One, C::R],
        [C::G, C::R, C::R, C::A],
        [C::B, C::One, C::R, C::Zero],
        [C::Zero, C::R, C::B, C::One],
        [C::One, C::A, C::R, C::G],
    ]
};

/// `texel` of the target with the view's mapping applied, as Direct3D 12 maps the (depth, 0, 0, 1) a
/// depth view reads; a texel left at the clear colour is kept (a drawn one has green 0).
fn mapped(texel: &[u8], mapping: [ViewComponent; 4]) -> Vec<u8> {
    let read: Vec<f32> = texel
        .chunks(FLOAT)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    if read == CLEAR {
        return texel.to_vec();
    }
    mapping
        .iter()
        .flat_map(|&m| {
            match m {
                ViewComponent::Zero => 0.0,
                ViewComponent::One => 1.0,
                c => read[c as usize],
            }
            .to_le_bytes()
        })
        .collect()
}

fn floats(t: &[u8]) -> Vec<f32> {
    t.chunks(FLOAT)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Prints the first texel where `ours` differs from `reference`, beside the unmapped picture's.
fn first_apart(reference: &[u8], ours: &[u8], unmapped: &[u8]) {
    let texel = FORMAT_FACTS[TARGET as usize].block.2 as usize;
    if let Some((at, (want, got))) = reference
        .chunks(texel)
        .zip(ours.chunks(texel))
        .enumerate()
        .find(|(_, (a, b))| a != b)
    {
        println!(
            "  first at texel {at}: reference {:?}, unmapped {:?}, ours {:?}",
            floats(want),
            floats(&unmapped[at * texel..][..texel]),
            floats(got)
        );
    }
}

/// A depth view with a component mapping keeping red first (Direct3D 12's
/// `Shader4ComponentMapping`, which Direct3D 11 lacks), read whole by `ps_shadow_wide` and through
/// its first component by `ps_shadow`: the reference is real Direct3D 11's picture of the unmapped
/// view with the mapping applied to each texel on the CPU.
#[test]
fn depth_views_with_component_mappings_read_as_mapped() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let filters = SCENES[0];
    let texel = FORMAT_FACTS[TARGET as usize].block.2 as usize;
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seeds() {
        let shadow = shadow_texels(seed);
        for shader in [WIDE_SHADER, SHADOW_SHADER] {
            let plain = direct3d(&gpu, shader, filters, &shadow);
            for mapping in KEPT {
                let reference: Vec<u8> = match shader {
                    WIDE_SHADER => plain
                        .chunks(texel)
                        .flat_map(|t| mapped(t, mapping))
                        .collect(),
                    _ => plain.clone(),
                };
                for (route, dxil) in [("dxbc", false), ("dxil", true)] {
                    let texels = ours(
                        &mut backend,
                        shader,
                        filters,
                        &shadow,
                        dxil,
                        Some(mapping),
                        CompareFn::Less,
                    );
                    let (count, largest) = apart(&reference, &texels, [0.0; 4]);
                    compared += 1;
                    println!(
                        "seed {seed} {shader} {mapping:?} {route}: {count} of {} texels apart from \
                         Direct3D 11's mapped on the CPU, largest difference {largest}",
                        SIZE * SIZE,
                    );
                    if count != 0 {
                        first_apart(&reference, &texels, &plain);
                        failures.push(format!("seed {seed} {shader} {mapping:?} {route}"));
                    }
                }
            }
        }
    }
    println!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Mappings putting red in one component alone, the rest zero, as vkd3d's comparison gather tests
/// map views.
const ALONE: [[ViewComponent; 4]; 3] = {
    use ViewComponent as C;
    [
        [C::Zero, C::R, C::Zero, C::Zero],
        [C::Zero, C::Zero, C::R, C::Zero],
        [C::Zero, C::Zero, C::Zero, C::R],
    ]
};

/// How a program compares against the depth view: by plain comparisons, or by a comparison gather
/// of one component.
#[derive(Clone, Copy)]
enum Compared {
    Sample,
    Gather(usize),
}

/// The programs reading a depth view through a mapping, with how each compares: whole, through its
/// first component, gathers of each component, and comparison gathers of each.
const MAPPED_SHADERS: [(&str, Compared); 7] = [
    (WIDE_SHADER, Compared::Sample),
    (SHADOW_SHADER, Compared::Sample),
    ("ps_shadow_gathers", Compared::Sample),
    ("ps_shadow_compared_red", Compared::Gather(0)),
    ("ps_shadow_compared_green", Compared::Gather(1)),
    ("ps_shadow_compared_blue", Compared::Gather(2)),
    ("ps_shadow_compared_alpha", Compared::Gather(3)),
];

/// The refusal a program comparing as `compared` through `mapping` is to meet, where the RTX 3080,
/// WARP and vkd3d's tests disagree on the result: a plain comparison through green, blue or alpha,
/// and a comparison gather unless red is in the mapping's first and gathered components.
fn expected_refusal(compared: Compared, mapping: [ViewComponent; 4]) -> Option<&'static str> {
    use ViewComponent as C;
    let disagreed = match compared {
        Compared::Sample => !matches!(mapping[0], C::R | C::Zero | C::One),
        Compared::Gather(c) => mapping[c] != C::R || mapping[0] != C::R,
    };
    disagreed.then_some(match mapping == [C::R, C::G, C::B, C::A] {
        true => wcontainers_shaders::refusal::DEPTH_GATHER_COMPONENT,
        false => wcontainers_shaders::refusal::SWIZZLED_DEPTH,
    })
}

/// A depth view with any component mapping, red kept first or moved, against real Direct3D 12 on
/// the RTX 3080 drawing the same build through a view so mapped: comparisons by two functions
/// (which the program does not know), gathers and comparison gathers of each component, and reads
/// used whole or as the depth alone, both routes; where the devices disagree
/// ([`expected_refusal`]), the program is refused by name. Direct3D 12 on WARP draws each too; where it
/// differs from the RTX 3080 at texels both draw alike unmapped, the count is printed (a
/// measurement, not a failure).
#[test]
fn depth_views_with_component_mappings_read_as_direct3d_12_maps_them() {
    use ViewComponent as C;
    let mut backend = webgpu();
    let gpu = d3d12::create(wcontainers_graphics_test::Driver::Hardware, false).unwrap();
    let warp = d3d12::create(wcontainers_graphics_test::Driver::Warp, false).unwrap();
    let filters = SCENES[0];
    let identity = [C::R, C::G, C::B, C::A];
    let mut failures = Vec::new();
    let mut compared = 0;
    let mut warp_apart = Vec::new();
    let mut refused = 0;
    for seed in seeds() {
        let shadow = shadow_texels(seed);
        for (shader, compares) in MAPPED_SHADERS {
            for compare in [CompareFn::Less, CompareFn::Greater] {
                for (route, dxil) in [("dxbc", false), ("dxil", true)] {
                    let unmapped = direct3d12(&gpu, shader, dxil, compare, identity, &shadow);
                    let warp_unmapped = direct3d12(&warp, shader, dxil, compare, identity, &shadow);
                    let mappings = [identity].into_iter().chain(KEPT).chain(MOVED).chain(ALONE);
                    for mapping in mappings {
                        let reference = direct3d12(&gpu, shader, dxil, compare, mapping, &shadow);
                        let on_warp = direct3d12(&warp, shader, dxil, compare, mapping, &shadow);
                        let case = format!("seed {seed} {shader} {compare:?} {mapping:?} {route}");
                        // Texels the two draw apart unmapped too (which neighbour a sample on a
                        // texel's edge takes) say nothing of the mapping.
                        let texel = FORMAT_FACTS[TARGET as usize].block.2 as usize;
                        let otherwise: Vec<usize> = (0..reference.len() / texel)
                            .filter(|&t| {
                                let at = t * texel..(t + 1) * texel;
                                reference[at.clone()] != on_warp[at.clone()]
                                    && unmapped[at.clone()] == warp_unmapped[at]
                            })
                            .collect();
                        if let Some(&t) = otherwise.first() {
                            let at = t * texel..(t + 1) * texel;
                            println!(
                                "{case}: WARP draws {} texels otherwise; first at texel {t}: WARP \
                                 {:?}, RTX 3080 {:?}, unmapped {:?}",
                                otherwise.len(),
                                floats(&on_warp[at.clone()]),
                                floats(&reference[at.clone()]),
                                floats(&unmapped[at])
                            );
                            warp_apart.push(case.clone());
                        }
                        if let Some(refusal) = expected_refusal(compares, mapping) {
                            let ext = if dxil { "dxil" } else { "dxbc" };
                            let bytes = bytecode(&format!("{shader}.{ext}"));
                            let source = match dxil {
                                true => Source::Dxil(&bytes),
                                false => Source::Dxbc(&bytes),
                            };
                            let key = VariantKey {
                                swizzled: vec![(SHADOW_SLOT, mapping)],
                                ..VariantKey::default()
                            };
                            let got = convert(source, &key, None).err();
                            if got != Some(ConvertError::Unsupported(refusal)) {
                                println!("{case}: expected {refusal}, got {got:?}");
                                failures.push(case);
                            }
                            refused += 1;
                            continue;
                        }
                        let name = format!("mapped-{seed}-{shader}-{compare:?}-{mapping:?}");
                        keep(&format!("{name}.d3d12-{route}"), &reference, TARGET);
                        let texels = ours(
                            &mut backend,
                            shader,
                            filters,
                            &shadow,
                            dxil,
                            Some(mapping),
                            compare,
                        );
                        keep(&format!("{name}.webgpu-{route}"), &texels, TARGET);
                        let (count, largest) = apart(&reference, &texels, [0.0; 4]);
                        compared += 1;
                        println!(
                            "{case}: {count} of {} texels apart from Direct3D 12, largest \
                             difference {largest}",
                            SIZE * SIZE,
                        );
                        if count != 0 {
                            first_apart(&reference, &texels, &unmapped);
                            failures.push(case);
                        }
                    }
                }
            }
        }
    }
    println!(
        "{compared} comparisons and {refused} refusals by name, {} failed; WARP differs from the \
         RTX 3080 in {} cases",
        failures.len(),
        warp_apart.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
