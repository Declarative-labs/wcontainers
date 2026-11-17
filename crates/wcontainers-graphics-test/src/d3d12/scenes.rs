//! Every Direct3D 12 scene by name: the drawing ground of the Direct3D 11 scenes through Direct3D
//! 12's own model (root signatures, descriptor heaps and tables, pipeline state objects, bundles,
//! barriers, placed and committed resources, upload and readback heaps, fences, several queues,
//! queries and indirect execution).

use std::fmt::Write;
use std::mem::transmute_copy;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

use super::{Gpu, area, buffer_desc, bytecode, element, tex2d_desc, transition};
use crate::d3d11::scenes::{Colored, hue, quad};
use crate::d3d11::{checker, hex};
use crate::dxil;
use crate::output::Output;

pub type Scene = fn(&Gpu) -> Result<Vec<Output>>;

pub const SCENES: &[(&str, Scene)] = &[
    ("triangle", triangle),
    ("roots", roots),
    ("heaps", heaps),
    ("bundles", bundles),
    ("compute", compute),
    ("queries", queries),
    ("queues", queues),
    ("copies", copies),
    ("swapchain", swapchain),
];

pub fn find(name: &str) -> Option<Scene> {
    SCENES.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

const RGBA: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;
const RT: D3D12_RESOURCE_STATES = D3D12_RESOURCE_STATE_RENDER_TARGET;

fn colored_layout() -> [D3D12_INPUT_ELEMENT_DESC; 2] {
    [
        element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
        element(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 12),
    ]
}

/// A pipeline drawing coloured vertices as triangles into one 8-bit target, solid, no blending.
fn pipeline(
    gpu: &Gpu,
    root: &ID3D12RootSignature,
    vs: &[u8],
    ps: &[u8],
    layout: &[D3D12_INPUT_ELEMENT_DESC],
) -> Result<ID3D12PipelineState> {
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: unsafe { transmute_copy(root) },
        VS: bytecode(vs),
        PS: bytecode(ps),
        BlendState: blend,
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
    desc.RTVFormats[0] = RGBA;
    Ok(unsafe { gpu.device.CreateGraphicsPipelineState(&desc)? })
}

/// Records a draw of coloured vertices from an upload buffer.
fn draw(gpu: &Gpu, list: &ID3D12GraphicsCommandList, vertices: &[Colored]) -> Result<()> {
    let buffer = gpu.upload(bytemuck::cast_slice(vertices))?;
    unsafe {
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: buffer.GetGPUVirtualAddress(),
                SizeInBytes: (vertices.len() * 28) as u32,
                StrideInBytes: 28,
            }]),
        );
        list.DrawInstanced(vertices.len() as u32, 1, 0, 0);
    }
    gpu.keep(&buffer);
    Ok(())
}

fn root_1_1(
    gpu: &Gpu,
    parameters: &[D3D12_ROOT_PARAMETER1],
    samplers: &[D3D12_STATIC_SAMPLER_DESC],
) -> Result<ID3D12RootSignature> {
    gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_1: D3D12_ROOT_SIGNATURE_DESC1 {
                NumParameters: parameters.len() as u32,
                pParameters: parameters.as_ptr(),
                NumStaticSamplers: samplers.len() as u32,
                pStaticSamplers: samplers.as_ptr(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            },
        },
    })
}

fn constants(register: u32, count: u32) -> D3D12_ROOT_PARAMETER1 {
    D3D12_ROOT_PARAMETER1 {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        Anonymous: D3D12_ROOT_PARAMETER1_0 {
            Constants: D3D12_ROOT_CONSTANTS {
                ShaderRegister: register,
                RegisterSpace: 0,
                Num32BitValues: count,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    }
}

fn descriptor(kind: D3D12_ROOT_PARAMETER_TYPE, register: u32) -> D3D12_ROOT_PARAMETER1 {
    D3D12_ROOT_PARAMETER1 {
        ParameterType: kind,
        Anonymous: D3D12_ROOT_PARAMETER1_0 {
            Descriptor: D3D12_ROOT_DESCRIPTOR1 {
                ShaderRegister: register,
                RegisterSpace: 0,
                Flags: D3D12_ROOT_DESCRIPTOR_FLAG_NONE,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    }
}

fn table(ranges: &[D3D12_DESCRIPTOR_RANGE1]) -> D3D12_ROOT_PARAMETER1 {
    D3D12_ROOT_PARAMETER1 {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        Anonymous: D3D12_ROOT_PARAMETER1_0 {
            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1 {
                NumDescriptorRanges: ranges.len() as u32,
                pDescriptorRanges: ranges.as_ptr(),
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    }
}

fn range(kind: D3D12_DESCRIPTOR_RANGE_TYPE, register: u32, offset: u32) -> D3D12_DESCRIPTOR_RANGE1 {
    D3D12_DESCRIPTOR_RANGE1 {
        RangeType: kind,
        NumDescriptors: 1,
        BaseShaderRegister: register,
        RegisterSpace: 0,
        Flags: D3D12_DESCRIPTOR_RANGE_FLAG_NONE,
        OffsetInDescriptorsFromTableStart: offset,
    }
}

fn sampler(filter: D3D12_FILTER, address: D3D12_TEXTURE_ADDRESS_MODE) -> D3D12_SAMPLER_DESC {
    D3D12_SAMPLER_DESC {
        Filter: filter,
        AddressU: address,
        AddressV: address,
        AddressW: address,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        MaxLOD: f32::MAX,
        ..Default::default()
    }
}

fn triangle(gpu: &Gpu) -> Result<Vec<Output>> {
    // An empty root signature of version 1.0.
    let root = gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                ..Default::default()
            },
        },
    })?;
    let layout = [
        element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0),
        element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 8),
    ];
    let pso = pipeline(
        gpu,
        &root,
        dxil!("triangle.vs"),
        dxil!("triangle.ps"),
        &layout,
    )?;
    let (w, h) = (crate::scene::WIDTH, crate::scene::HEIGHT);
    let (target, rtv) = gpu.target(w, h, RGBA)?;
    let vertices = gpu.upload(bytemuck::cast_slice(&crate::scene::TRIANGLE))?;
    let list = &gpu.list;
    unsafe {
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        gpu.begin(&rtv, w, h);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertices.GetGPUVirtualAddress(),
                SizeInBytes: 60,
                StrideInBytes: 20,
            }]),
        );
        list.DrawInstanced(3, 1, 0, 0);
    }
    gpu.keep(&vertices);
    Ok(vec![Output::picture("", gpu.read(&target, 0, RT)?)])
}

/// One root signature (version 1.1) with root constants, a root constant buffer, a root shader
/// resource, a table of a constant buffer and a texture, a table of a sampler and a static
/// sampler; one cell per kind.
fn roots(gpu: &Gpu) -> Result<Vec<Output>> {
    let cbv_srv = [
        range(D3D12_DESCRIPTOR_RANGE_TYPE_CBV, 2, 0),
        range(D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 1),
    ];
    let samplers = [range(D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER, 0, 0)];
    let parameters = [
        constants(0, 4),
        descriptor(D3D12_ROOT_PARAMETER_TYPE_CBV, 1),
        descriptor(D3D12_ROOT_PARAMETER_TYPE_SRV, 0),
        table(&cbv_srv),
        table(&samplers),
    ];
    let fixed = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        MaxLOD: f32::MAX,
        ShaderRegister: 1,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        ..Default::default()
    };
    let root = root_1_1(gpu, &parameters, &[fixed])?;

    // In one upload buffer: a constant buffer at byte 256, four float4s for the root shader
    // resource at 272, and the table's constant buffer at 512.
    let mut data = vec![0f32; 256];
    data[64..68].copy_from_slice(&[0.3, 0.9, 0.6, 1.0]);
    data[68..84].copy_from_slice(&[
        1.0, 0.2, 0.2, 1.0, 0.2, 1.0, 0.2, 1.0, 0.2, 0.2, 1.0, 1.0, 1.0, 1.0, 0.2, 1.0,
    ]);
    data[128..132].copy_from_slice(&[0.8, 0.7, 1.0, 1.0]);
    let upload = gpu.upload(bytemuck::cast_slice(&data))?;
    let address = unsafe { upload.GetGPUVirtualAddress() };

    let texture = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(8, 8, RGBA, D3D12_RESOURCE_FLAG_NONE),
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    gpu.fill_texture(&texture, &checker(8, 8, 3, 6), 32)?;
    let views = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 2, true)?;
    let sampler_heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, 1, true)?;
    unsafe {
        gpu.device.CreateConstantBufferView(
            Some(&D3D12_CONSTANT_BUFFER_VIEW_DESC {
                BufferLocation: address + 512,
                SizeInBytes: 256,
            }),
            gpu.cpu(&views, 0),
        );
        gpu.device
            .CreateShaderResourceView(&texture, None, gpu.cpu(&views, 1));
        gpu.device.CreateSampler(
            &sampler(
                D3D12_FILTER_MIN_MAG_MIP_LINEAR,
                D3D12_TEXTURE_ADDRESS_MODE_WRAP,
            ),
            gpu.cpu(&sampler_heap, 0),
        );
    }
    let (target, rtv) = gpu.target(192, 128, RGBA)?;
    let list = &gpu.list;
    unsafe {
        list.ResourceBarrier(&[transition(
            &texture,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE,
        )]);
        gpu.begin(&rtv, 192, 128);
        list.SetGraphicsRootSignature(&root);
        list.SetDescriptorHeaps(&[Some(views.clone()), Some(sampler_heap.clone())]);
        list.SetGraphicsRoot32BitConstants(0, 4, [0.9f32, 0.5, 0.2, 1.0].as_ptr().cast(), 0);
        list.SetGraphicsRootConstantBufferView(1, address + 256);
        list.SetGraphicsRootShaderResourceView(2, address + 272);
        list.SetGraphicsRootDescriptorTable(3, gpu.gpu(&views, 0));
        list.SetGraphicsRootDescriptorTable(4, gpu.gpu(&sampler_heap, 0));
    }
    let shaders = [
        dxil!("d3d12.ps_constants"),
        dxil!("d3d12.ps_root_cbv"),
        dxil!("d3d12.ps_root_srv"),
        dxil!("d3d12.ps_table"),
        dxil!("d3d12.ps_static"),
    ];
    // Pipelines must outlive the commands that use them, so they are kept until the read.
    let mut pipelines = Vec::new();
    for (i, ps) in shaders.into_iter().enumerate() {
        let pso = pipeline(gpu, &root, dxil!("common.vs_color"), ps, &colored_layout())?;
        unsafe { list.SetPipelineState(&pso) };
        pipelines.push(pso);
        area(list, 64 * (i as u32 % 3), 64 * (i as u32 / 3), 64, 64);
        draw(gpu, list, &quad(-0.8, -0.8, 0.8, 0.8, 0.5, 0.5, [1.0; 4]))?;
    }
    gpu.keep(&upload);
    gpu.keep(&texture);
    let picture = gpu.read(&target, 0, RT)?;
    drop(pipelines);
    Ok(vec![Output::picture("", picture)])
}

/// Two render targets placed at the same offset of one heap: the first drawn and copied out, an
/// aliasing barrier, the second cleared, drawn and copied out; and vertices in a buffer placed in
/// an upload heap.
fn heaps(gpu: &Gpu) -> Result<Vec<Output>> {
    let heap: ID3D12Heap = unsafe {
        let mut heap = None;
        gpu.device.CreateHeap(
            &D3D12_HEAP_DESC {
                SizeInBytes: 1 << 20,
                Properties: D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                Alignment: 0,
                Flags: D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES,
            },
            &mut heap,
        )?;
        heap.unwrap()
    };
    let upload_heap: ID3D12Heap = unsafe {
        let mut heap = None;
        gpu.device.CreateHeap(
            &D3D12_HEAP_DESC {
                SizeInBytes: 1 << 16,
                Properties: D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_UPLOAD,
                    ..Default::default()
                },
                Alignment: 0,
                Flags: D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
            },
            &mut heap,
        )?;
        heap.unwrap()
    };
    let placed = |heap: &ID3D12Heap,
                  offset: u64,
                  desc: &D3D12_RESOURCE_DESC,
                  state,
                  clear: Option<&D3D12_CLEAR_VALUE>|
     -> Result<ID3D12Resource> {
        let mut resource = None;
        unsafe {
            gpu.device.CreatePlacedResource(
                heap,
                offset,
                desc,
                state,
                clear.map(|c| c as *const _),
                &mut resource,
            )?
        };
        Ok(resource.unwrap())
    };
    let clear = D3D12_CLEAR_VALUE {
        Format: RGBA,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            Color: crate::scene::CLEAR,
        },
    };
    let desc = tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
    let first = placed(&heap, 0, &desc, RT, Some(&clear))?;
    let second = placed(&heap, 0, &desc, RT, Some(&clear))?;
    let shapes: Vec<Colored> = quad(-0.8, -0.6, 0.4, 0.7, 0.5, 0.5, [0.9, 0.4, 0.2, 1.0])
        .into_iter()
        .chain(quad(-0.3, -0.9, 0.9, 0.2, 0.5, 0.5, [0.2, 0.6, 0.9, 1.0]))
        .collect();
    let vertices = placed(
        &upload_heap,
        0,
        &buffer_desc(shapes.len() as u64 * 28),
        D3D12_RESOURCE_STATE_GENERIC_READ,
        None,
    )?;
    unsafe {
        let mut mapped = std::ptr::null_mut();
        vertices.Map(0, Some(&D3D12_RANGE::default()), Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(
            shapes.as_ptr().cast::<u8>(),
            mapped.cast(),
            shapes.len() * 28,
        );
        vertices.Unmap(0, None);
    }
    let root = root_1_1(gpu, &[], &[])?;
    let pso = pipeline(
        gpu,
        &root,
        dxil!("common.vs_color"),
        dxil!("common.ps_color"),
        &colored_layout(),
    )?;
    let copies = [
        gpu.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )?,
        gpu.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )?,
    ];
    let rtv_heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 2, false)?;
    let list = &gpu.list;
    for (k, (target, copy)) in [&first, &second].into_iter().zip(&copies).enumerate() {
        unsafe {
            gpu.device
                .CreateRenderTargetView(target, None, gpu.cpu(&rtv_heap, k as u32));
            if k == 1 {
                let aliasing = D3D12_RESOURCE_BARRIER {
                    Type: D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
                    Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                    Anonymous: D3D12_RESOURCE_BARRIER_0 {
                        Aliasing: std::mem::ManuallyDrop::new(D3D12_RESOURCE_ALIASING_BARRIER {
                            pResourceBefore: transmute_copy(&first),
                            pResourceAfter: transmute_copy(&second),
                        }),
                    },
                };
                list.ResourceBarrier(&[aliasing]);
            }
            let rtv = gpu.cpu(&rtv_heap, k as u32);
            list.ClearRenderTargetView(rtv, &crate::scene::CLEAR, None);
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            area(list, 0, 0, 64, 64);
            list.SetPipelineState(&pso);
            list.SetGraphicsRootSignature(&root);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.IASetVertexBuffers(
                0,
                Some(&[D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: vertices.GetGPUVirtualAddress(),
                    SizeInBytes: (shapes.len() * 28) as u32,
                    StrideInBytes: 28,
                }]),
            );
            list.DrawInstanced(6, 1, 6 * k as u32, 0);
            list.ResourceBarrier(&[transition(target, RT, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
            list.CopyResource(copy, target);
        }
    }
    gpu.submit()?;
    Ok(vec![
        Output::picture(
            "first",
            gpu.read(&copies[0], 0, D3D12_RESOURCE_STATE_COPY_DEST)?,
        ),
        Output::picture(
            "second",
            gpu.read(&copies[1], 0, D3D12_RESOURCE_STATE_COPY_DEST)?,
        ),
    ])
}

/// A bundle setting its pipeline and drawing, executed three times with different root constants,
/// which bundles inherit.
fn bundles(gpu: &Gpu) -> Result<Vec<Output>> {
    let root = root_1_1(gpu, &[constants(0, 4)], &[])?;
    let pso = pipeline(
        gpu,
        &root,
        dxil!("common.vs_color"),
        dxil!("d3d12.ps_bundle"),
        &colored_layout(),
    )?;
    let allocator: ID3D12CommandAllocator = unsafe {
        gpu.device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_BUNDLE)?
    };
    let bundle: ID3D12GraphicsCommandList = unsafe {
        gpu.device
            .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_BUNDLE, &allocator, &pso)?
    };
    let mut fan = Vec::new();
    for k in 0..5u32 {
        let (a, b) = (k as f32 * 1.2566, (k + 1) as f32 * 1.2566);
        let [r, g, bl] = hue(k, 5);
        let p = |x: f32, y: f32| [x, y, 0.5, r, g, bl, 1.0];
        fan.extend([
            p(0.0, 0.0),
            p(0.8 * a.cos(), 0.8 * a.sin()),
            p(0.8 * b.cos(), 0.8 * b.sin()),
        ]);
    }
    unsafe { bundle.SetGraphicsRootSignature(&root) };
    draw(gpu, &bundle, &fan)?;
    unsafe { bundle.Close()? };
    let (target, rtv) = gpu.target(192, 64, RGBA)?;
    let list = &gpu.list;
    unsafe {
        gpu.begin(&rtv, 192, 64);
        list.SetGraphicsRootSignature(&root);
        for k in 0..3u32 {
            area(list, 64 * k, 0, 64, 64);
            list.SetGraphicsRoot32BitConstant(0, (0.4 + 0.3 * k as f32).to_bits(), 0);
            list.ExecuteBundle(&bundle);
        }
    }
    let picture = gpu.read(&target, 0, RT)?;
    drop(allocator);
    Ok(vec![Output::picture("", picture)])
}

/// A compute pipeline writing a texture through a descriptor table and adding into a buffer
/// through a root unordered access view, dispatched by ExecuteIndirect, with a UAV barrier between
/// two dispatches; the texture then drawn by a pipeline reading it.
fn compute(gpu: &Gpu) -> Result<Vec<Output>> {
    // The texture's unordered access view and shader resource view sit in separate tables, as the
    // texture is in one state or the other.
    let uav_range = [range(D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 0, 0)];
    let srv_range = [range(D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 1, 0)];
    let root = root_1_1(
        gpu,
        &[
            constants(0, 4),
            descriptor(D3D12_ROOT_PARAMETER_TYPE_UAV, 1),
            table(&uav_range),
            table(&srv_range),
        ],
        &[],
    )?;
    let pso = |cs: &[u8]| -> Result<ID3D12PipelineState> {
        Ok(unsafe {
            gpu.device
                .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                    pRootSignature: transmute_copy(&root),
                    CS: bytecode(cs),
                    ..Default::default()
                })?
        })
    };
    let (image_pso, add_pso) = (pso(dxil!("d3d12.cs_image"))?, pso(dxil!("d3d12.cs_add"))?);
    let image = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(
            64,
            64,
            DXGI_FORMAT_R32G32B32A32_FLOAT,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        ),
        D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        None,
    )?;
    let mut sums_desc = buffer_desc(256);
    sums_desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    let sums = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &sums_desc,
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    let zeros = gpu.upload(&[0; 256])?;
    let views = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 2, true)?;
    unsafe {
        gpu.device
            .CreateUnorderedAccessView(&image, None, None, gpu.cpu(&views, 0));
        gpu.device
            .CreateShaderResourceView(&image, None, gpu.cpu(&views, 1));
    }
    let arguments = [D3D12_INDIRECT_ARGUMENT_DESC {
        Type: D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
        ..Default::default()
    }];
    let signature: ID3D12CommandSignature = unsafe {
        let mut signature = None;
        gpu.device.CreateCommandSignature(
            &D3D12_COMMAND_SIGNATURE_DESC {
                ByteStride: 12,
                NumArgumentDescs: 1,
                pArgumentDescs: arguments.as_ptr(),
                NodeMask: 0,
            },
            None,
            &mut signature,
        )?;
        signature.unwrap()
    };
    let indirect = gpu.upload(bytemuck::cast_slice(&[8u32, 8, 1, 3, 1, 1]))?;
    let list = &gpu.list;
    unsafe {
        list.CopyBufferRegion(&sums, 0, &zeros, 0, 256);
        list.ResourceBarrier(&[transition(
            &sums,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )]);
        list.SetComputeRootSignature(&root);
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetComputeRootUnorderedAccessView(1, sums.GetGPUVirtualAddress());
        list.SetComputeRootDescriptorTable(2, gpu.gpu(&views, 0));
        list.SetPipelineState(&image_pso);
        list.ExecuteIndirect(&signature, 1, &indirect, 0, None, 0);
        list.SetPipelineState(&add_pso);
        for k in 0..2u32 {
            list.SetComputeRoot32BitConstant(0, (100.0 * (k + 1) as f32).to_bits(), 0);
            list.ExecuteIndirect(&signature, 1, &indirect, 12, None, 0);
            let uav = D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
                Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                Anonymous: D3D12_RESOURCE_BARRIER_0 {
                    UAV: std::mem::ManuallyDrop::new(D3D12_RESOURCE_UAV_BARRIER {
                        pResource: transmute_copy(&sums),
                    }),
                },
            };
            list.ResourceBarrier(&[uav]);
        }
        list.ResourceBarrier(&[transition(
            &image,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE,
        )]);
    }
    gpu.keep(&zeros);
    gpu.keep(&indirect);
    let (target, rtv) = gpu.target(64, 64, RGBA)?;
    let draw_pso = pipeline(
        gpu,
        &root,
        dxil!("common.vs_color"),
        dxil!("d3d12.ps_image"),
        &colored_layout(),
    )?;
    unsafe {
        gpu.begin(&rtv, 64, 64);
        list.SetGraphicsRootSignature(&root);
        list.SetGraphicsRootDescriptorTable(3, gpu.gpu(&views, 1));
        list.SetPipelineState(&draw_pso);
    }
    draw(gpu, list, &quad(-0.9, -0.9, 0.9, 0.9, 0.5, 0.5, [1.0; 4]))?;
    let sums = gpu.read_buffer(&sums, 16, D3D12_RESOURCE_STATE_UNORDERED_ACCESS)?;
    let words: Vec<u32> = bytemuck::pod_collect_to_vec(&sums);
    Ok(vec![
        Output::picture("", gpu.read(&target, 0, RT)?),
        Output::picture(
            "image",
            gpu.read(&image, 0, D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE)?,
        ),
        Output::text("sums", format!("{words:?}\n")),
    ])
}

/// Occlusion, binary occlusion, pipeline statistics and timestamp queries resolved into a buffer,
/// and drawing predicated on a resolved binary occlusion result.
fn queries(gpu: &Gpu) -> Result<Vec<Output>> {
    let heap = |kind: D3D12_QUERY_HEAP_TYPE, count: u32| -> Result<ID3D12QueryHeap> {
        let mut heap = None;
        unsafe {
            gpu.device.CreateQueryHeap(
                &D3D12_QUERY_HEAP_DESC {
                    Type: kind,
                    Count: count,
                    NodeMask: 0,
                },
                &mut heap,
            )?
        };
        Ok(heap.unwrap())
    };
    let occlusion = heap(D3D12_QUERY_HEAP_TYPE_OCCLUSION, 4)?;
    let statistics = heap(D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS, 1)?;
    let timestamps = heap(D3D12_QUERY_HEAP_TYPE_TIMESTAMP, 2)?;
    let results = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &buffer_desc(256),
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    let root = root_1_1(gpu, &[], &[])?;
    let pso = pipeline(
        gpu,
        &root,
        dxil!("common.vs_color"),
        dxil!("common.ps_color"),
        &colored_layout(),
    )?;
    let (target, rtv) = gpu.target(64, 64, RGBA)?;
    let list = &gpu.list;
    let pixels = |x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]| {
        quad(
            x0 / 32.0 - 1.0,
            1.0 - y1 / 32.0,
            x1 / 32.0 - 1.0,
            1.0 - y0 / 32.0,
            0.5,
            0.5,
            color,
        )
    };
    unsafe {
        gpu.begin(&rtv, 64, 64);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.EndQuery(&timestamps, D3D12_QUERY_TYPE_TIMESTAMP, 0);
        list.BeginQuery(&occlusion, D3D12_QUERY_TYPE_OCCLUSION, 0);
    }
    draw(
        gpu,
        list,
        &pixels(4.0, 4.0, 14.0, 11.0, [0.9, 0.3, 0.2, 1.0]),
    )?;
    unsafe {
        list.EndQuery(&occlusion, D3D12_QUERY_TYPE_OCCLUSION, 0);
        list.BeginQuery(&occlusion, D3D12_QUERY_TYPE_BINARY_OCCLUSION, 1);
    }
    // Entirely outside the scissor rectangle: nothing passes.
    let left = windows::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: 32,
        bottom: 64,
    };
    unsafe { list.RSSetScissorRects(&[left]) };
    draw(
        gpu,
        list,
        &pixels(40.0, 4.0, 60.0, 20.0, [0.2, 0.9, 0.3, 1.0]),
    )?;
    unsafe {
        list.EndQuery(&occlusion, D3D12_QUERY_TYPE_BINARY_OCCLUSION, 1);
        list.BeginQuery(&occlusion, D3D12_QUERY_TYPE_BINARY_OCCLUSION, 2);
    }
    draw(
        gpu,
        list,
        &pixels(4.0, 20.0, 8.0, 24.0, [0.2, 0.3, 0.9, 1.0]),
    )?;
    area(list, 0, 0, 64, 64);
    unsafe {
        list.EndQuery(&occlusion, D3D12_QUERY_TYPE_BINARY_OCCLUSION, 2);
        list.BeginQuery(&statistics, D3D12_QUERY_TYPE_PIPELINE_STATISTICS, 0);
    }
    draw(
        gpu,
        list,
        &pixels(36.0, 30.0, 60.0, 46.0, [0.9, 0.9, 0.2, 1.0]),
    )?;
    unsafe {
        list.EndQuery(&statistics, D3D12_QUERY_TYPE_PIPELINE_STATISTICS, 0);
        list.EndQuery(&timestamps, D3D12_QUERY_TYPE_TIMESTAMP, 1);
        list.ResolveQueryData(&occlusion, D3D12_QUERY_TYPE_OCCLUSION, 0, 1, &results, 0);
        list.ResolveQueryData(
            &occlusion,
            D3D12_QUERY_TYPE_BINARY_OCCLUSION,
            1,
            2,
            &results,
            8,
        );
        list.ResolveQueryData(
            &statistics,
            D3D12_QUERY_TYPE_PIPELINE_STATISTICS,
            0,
            1,
            &results,
            32,
        );
        list.ResolveQueryData(&timestamps, D3D12_QUERY_TYPE_TIMESTAMP, 0, 2, &results, 128);
        list.ResourceBarrier(&[transition(
            &results,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PREDICATION,
        )]);
        // Predicated on the hidden draw's result (0): skipped. On the visible one's: drawn.
        list.SetPredication(&results, 8, D3D12_PREDICATION_OP_EQUAL_ZERO);
    }
    draw(
        gpu,
        list,
        &pixels(20.0, 50.0, 28.0, 58.0, [1.0, 0.0, 0.0, 1.0]),
    )?;
    unsafe { list.SetPredication(&results, 16, D3D12_PREDICATION_OP_EQUAL_ZERO) };
    draw(
        gpu,
        list,
        &pixels(36.0, 50.0, 44.0, 58.0, [0.0, 1.0, 0.0, 1.0]),
    )?;
    unsafe { list.SetPredication(None, 0, D3D12_PREDICATION_OP_EQUAL_ZERO) };
    let picture = gpu.read(&target, 0, RT)?;
    let r: Vec<u64> = bytemuck::pod_collect_to_vec(&gpu.read_buffer(
        &results,
        144,
        D3D12_RESOURCE_STATE_PREDICATION,
    )?);
    let mut text = String::new();
    writeln!(
        text,
        "occlusion: {} samples; binary: {} and {}",
        r[0], r[1], r[2]
    )?;
    // Pipeline statistics from offset 32: vertices, primitives, vertex shader, geometry shader,
    // its primitives, clipper, its primitives, pixel shader (at least the 384 covered pixels).
    writeln!(
        text,
        "statistics: {} {} {} {} {} {} {}, pixel shader at least 384: {}",
        r[4],
        r[5],
        r[6],
        r[7],
        r[8],
        r[9],
        r[10],
        r[11] >= 384
    )?;
    writeln!(text, "timestamps in order: {}", r[17] >= r[16])?;
    Ok(vec![
        Output::text("results", text),
        Output::picture("", picture),
    ])
}

/// Three queues: a copy queue uploads vertices into a default buffer, a compute queue adds into a
/// buffer, and the direct queue waits on both through fences, then draws.
fn queues(gpu: &Gpu) -> Result<Vec<Output>> {
    let queue = |kind: D3D12_COMMAND_LIST_TYPE| -> Result<(
        ID3D12CommandQueue,
        ID3D12CommandAllocator,
        ID3D12GraphicsCommandList,
    )> {
        unsafe {
            let queue: ID3D12CommandQueue =
                gpu.device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                    Type: kind,
                    ..Default::default()
                })?;
            let allocator: ID3D12CommandAllocator = gpu.device.CreateCommandAllocator(kind)?;
            let list: ID3D12GraphicsCommandList =
                gpu.device.CreateCommandList(0, kind, &allocator, None)?;
            Ok((queue, allocator, list))
        }
    };
    let fence: ID3D12Fence = unsafe { gpu.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
    let mut shapes = Vec::new();
    for k in 0..4u32 {
        let [r, g, b] = hue(k, 4);
        let x = -0.9 + 0.45 * k as f32;
        shapes.extend(quad(x, -0.7, x + 0.35, 0.7, 0.5, 0.5, [r, g, b, 1.0]));
    }
    let bytes: &[u8] = bytemuck::cast_slice(&shapes);
    let staged = gpu.upload(bytes)?;
    let vertices = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &buffer_desc(bytes.len() as u64),
        D3D12_RESOURCE_STATE_COMMON,
        None,
    )?;
    let (copy_queue, _copy_allocator, copy_list) = queue(D3D12_COMMAND_LIST_TYPE_COPY)?;
    unsafe {
        copy_list.CopyBufferRegion(&vertices, 0, &staged, 0, bytes.len() as u64);
        copy_list.Close()?;
        copy_queue.ExecuteCommandLists(&[Some(copy_list.cast()?)]);
        copy_queue.Signal(&fence, 1)?;
    }
    let root = root_1_1(
        gpu,
        &[
            constants(0, 4),
            descriptor(D3D12_ROOT_PARAMETER_TYPE_UAV, 1),
        ],
        &[],
    )?;
    let mut sums_desc = buffer_desc(256);
    sums_desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    let sums = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &sums_desc,
        D3D12_RESOURCE_STATE_COMMON,
        None,
    )?;
    let (compute_queue, _compute_allocator, compute_list) = queue(D3D12_COMMAND_LIST_TYPE_COMPUTE)?;
    let add: ID3D12PipelineState = unsafe {
        gpu.device
            .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                pRootSignature: transmute_copy(&root),
                CS: bytecode(dxil!("d3d12.cs_add")),
                ..Default::default()
            })?
    };
    unsafe {
        compute_list.SetPipelineState(&add);
        compute_list.SetComputeRootSignature(&root);
        compute_list.SetComputeRoot32BitConstant(0, 7.0f32.to_bits(), 0);
        compute_list.SetComputeRootUnorderedAccessView(1, sums.GetGPUVirtualAddress());
        compute_list.Dispatch(2, 1, 1);
        compute_list.Close()?;
        compute_queue.Wait(&fence, 1)?;
        compute_queue.ExecuteCommandLists(&[Some(compute_list.cast()?)]);
        compute_queue.Signal(&fence, 2)?;
        gpu.queue.Wait(&fence, 2)?;
    }
    let pso = pipeline(
        gpu,
        &root,
        dxil!("common.vs_color"),
        dxil!("common.ps_color"),
        &colored_layout(),
    )?;
    let (target, rtv) = gpu.target(128, 64, RGBA)?;
    let list = &gpu.list;
    unsafe {
        list.ResourceBarrier(&[transition(
            &vertices,
            D3D12_RESOURCE_STATE_COMMON,
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
        )]);
        gpu.begin(&rtv, 128, 64);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertices.GetGPUVirtualAddress(),
                SizeInBytes: bytes.len() as u32,
                StrideInBytes: 28,
            }]),
        );
        list.DrawInstanced(shapes.len() as u32, 1, 0, 0);
        list.ResourceBarrier(&[transition(
            &vertices,
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
            D3D12_RESOURCE_STATE_COMMON,
        )]);
    }
    gpu.keep(&staged);
    let picture = gpu.read(&target, 0, RT)?;
    let sums: Vec<u32> =
        bytemuck::pod_collect_to_vec(&gpu.read_buffer(&sums, 16, D3D12_RESOURCE_STATE_COMMON)?);
    gpu.wait(&compute_queue)?;
    gpu.wait(&copy_queue)?;
    Ok(vec![
        Output::picture("", picture),
        Output::text("sums", format!("{sums:?}\n")),
    ])
}

/// Copies: buffer ranges, a texture region at an offset, a whole resource, and a 4-sample target
/// resolved.
fn copies(gpu: &Gpu) -> Result<Vec<Output>> {
    let list = &gpu.list;
    let pattern: Vec<u8> = (0..=255u8).collect();
    let from = gpu.upload(&pattern)?;
    let to = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &buffer_desc(256),
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    let fill = gpu.upload(&[0xee; 256])?;
    unsafe {
        list.CopyBufferRegion(&to, 0, &fill, 0, 256);
        list.CopyBufferRegion(&to, 16, &from, 64, 64);
    }
    gpu.keep(&from);
    gpu.keep(&fill);
    let buffer = hex(&gpu.read_buffer(&to, 256, D3D12_RESOURCE_STATE_COPY_DEST)?);

    let source = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(32, 32, RGBA, D3D12_RESOURCE_FLAG_NONE),
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    gpu.fill_texture(&source, &checker(32, 32, 1, 3), 128)?;
    let grey = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_NONE),
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    gpu.fill_texture(&grey, &vec![90u8; 64 * 64 * 4], 256)?;
    let whole = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(32, 32, RGBA, D3D12_RESOURCE_FLAG_NONE),
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
    )?;
    unsafe {
        list.ResourceBarrier(&[transition(
            &source,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        let region = D3D12_BOX {
            left: 4,
            top: 2,
            front: 0,
            right: 28,
            bottom: 18,
            back: 1,
        };
        list.CopyTextureRegion(
            &super::texture_location(&grey, 0),
            10,
            20,
            0,
            &super::texture_location(&source, 0),
            Some(&region),
        );
        list.CopyResource(&whole, &source);
    }

    // Four samples, resolved.
    let mut desc = tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
    // Quality u32::MAX is the standard sample pattern.
    desc.SampleDesc = DXGI_SAMPLE_DESC {
        Count: 4,
        Quality: u32::MAX,
    };
    let clear = D3D12_CLEAR_VALUE {
        Format: RGBA,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            Color: crate::scene::CLEAR,
        },
    };
    let samples = gpu.committed(D3D12_HEAP_TYPE_DEFAULT, &desc, RT, Some(&clear))?;
    let rtv_heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)?;
    unsafe {
        gpu.device
            .CreateRenderTargetView(&samples, None, gpu.cpu(&rtv_heap, 0))
    };
    let root = root_1_1(gpu, &[], &[])?;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let layout = colored_layout();
    let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: unsafe { transmute_copy(&root) },
        VS: bytecode(dxil!("common.vs_color")),
        PS: bytecode(dxil!("common.ps_color")),
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            DepthClipEnable: true.into(),
            MultisampleEnable: true.into(),
            ..Default::default()
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: layout.as_ptr(),
            NumElements: layout.len() as u32,
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        SampleDesc: desc.SampleDesc,
        ..Default::default()
    };
    pso_desc.RTVFormats[0] = RGBA;
    let pso: ID3D12PipelineState = unsafe { gpu.device.CreateGraphicsPipelineState(&pso_desc)? };
    let resolved = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_NONE),
        D3D12_RESOURCE_STATE_RESOLVE_DEST,
        None,
    )?;
    unsafe {
        gpu.begin(&rtv_heap, 64, 64);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
    }
    let mut wedges = Vec::new();
    for k in 0..4u32 {
        let [r, g, b] = hue(k, 4);
        let (a0, a1) = (k as f32 * 0.4 + 0.05, k as f32 * 0.4 + 0.2);
        let p = |a: f32, d: f32| [d * a.cos() - 0.9, d * a.sin() - 0.9, 0.5, r, g, b, 1.0];
        wedges.extend([p(a0, 0.1), p(a1, 1.9), p(a0, 1.9)]);
    }
    draw(gpu, list, &wedges)?;
    unsafe {
        list.ResourceBarrier(&[transition(
            &samples,
            RT,
            D3D12_RESOURCE_STATE_RESOLVE_SOURCE,
        )]);
        list.ResolveSubresource(&resolved, 0, &samples, 0, RGBA);
    }
    Ok(vec![
        Output::text("buffer", buffer),
        Output::picture(
            "region",
            gpu.read(&grey, 0, D3D12_RESOURCE_STATE_COPY_DEST)?,
        ),
        Output::picture(
            "resource",
            gpu.read(&whole, 0, D3D12_RESOURCE_STATE_COPY_DEST)?,
        ),
        Output::picture(
            "resolve",
            gpu.read(&resolved, 0, D3D12_RESOURCE_STATE_RESOLVE_DEST)?,
        ),
    ])
}

/// A flip-model swap chain of three buffers on a window that is never shown, on the direct queue:
/// three frames, each drawn into the current back buffer between barriers from and to the present
/// state and read before presenting, then ResizeBuffers and one more frame. Records the back
/// buffer index of each frame, whether each present succeeded and the sizes after resizing; needs
/// a session with a desktop (`gpu-job --desktop`).
fn swapchain(gpu: &Gpu) -> Result<Vec<Output>> {
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows::core::w;
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("swap chain"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            200,
            200,
            None,
            None,
            None,
            None,
        )?
    };
    let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))? };
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: 128,
        Height: 96,
        Format: RGBA,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 3,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        Scaling: DXGI_SCALING_STRETCH,
        ..Default::default()
    };
    let result = (|| -> Result<Vec<Output>> {
        let chain: IDXGISwapChain3 = unsafe {
            factory
                .CreateSwapChainForHwnd(&gpu.queue, window, &desc, None, None)?
                .cast()?
        };
        let root = root_1_1(gpu, &[], &[])?;
        let pso = pipeline(
            gpu,
            &root,
            dxil!("common.vs_color"),
            dxil!("common.ps_color"),
            &colored_layout(),
        )?;
        let heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)?;
        let mut outputs = Vec::new();
        let mut text = String::new();
        let frames = [(128, 96), (128, 96), (128, 96), (64, 80)];
        for (frame, (width, height)) in frames.into_iter().enumerate() {
            if frame == 3 {
                unsafe {
                    chain.ResizeBuffers(
                        0,
                        width,
                        height,
                        DXGI_FORMAT_UNKNOWN,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )?
                };
                let after = unsafe { chain.GetDesc1()? };
                writeln!(
                    text,
                    "resized: {} by {}, {} buffers",
                    after.Width, after.Height, after.BufferCount
                )?;
            }
            let index = unsafe { chain.GetCurrentBackBufferIndex() };
            let back: ID3D12Resource = unsafe { chain.GetBuffer(index)? };
            let list = &gpu.list;
            let [r, g, b] = hue(frame as u32, 4);
            unsafe {
                gpu.device
                    .CreateRenderTargetView(&back, None, gpu.cpu(&heap, 0));
                list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, RT)]);
                list.SetPipelineState(&pso);
                list.SetGraphicsRootSignature(&root);
                gpu.begin(&heap, width, height);
            }
            draw(
                gpu,
                list,
                &quad(-0.7, -0.6, 0.5, 0.8, 0.5, 0.5, [r, g, b, 1.0]),
            )?;
            draw(
                gpu,
                list,
                &quad(-0.2, -0.9, 0.9, 0.1, 0.5, 0.5, [0.1, 0.4, 0.8, 0.5]),
            )?;
            let picture = gpu.read(&back, 0, RT)?;
            unsafe { list.ResourceBarrier(&[transition(&back, RT, D3D12_RESOURCE_STATE_PRESENT)]) };
            gpu.submit()?;
            outputs.push(Output::picture(&format!("frame{frame}"), picture));
            let presented = unsafe { chain.Present(0, DXGI_PRESENT(0)) };
            gpu.wait(&gpu.queue)?;
            writeln!(
                text,
                "frame {frame}: back buffer {index}, present {}",
                if presented.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                }
            )?;
        }
        outputs.push(Output::text("results", text));
        Ok(outputs)
    })();
    unsafe { DestroyWindow(window)? };
    result
}
