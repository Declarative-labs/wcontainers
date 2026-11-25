//! Direct3D 11's draws and constants scenes drawn through Direct3D 12: every kind of draw with
//! start vertices, base vertices and start instances, both indirect draws by ExecuteIndirect, and
//! vertex and index buffer views starting inside their buffers; and fourteen constant buffer views
//! in one table, and views of windows into one large buffer, past whose end reads give zero.

use anyhow::{Context, Result};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::D3D11_VIEWPORT;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

use super::scenes::pipeline;
use super::states::{Immediate, RT};
use super::{Gpu, area, element, viewport12};
use crate::d3d11::cell;
use crate::d3d11::scenes::{constants as constants11, draws as draws11};
use crate::dxil;
use crate::output::Output;

const RGBA: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

fn per_instance(
    name: windows::core::PCSTR,
    format: DXGI_FORMAT,
    slot: u32,
    step: u32,
) -> D3D12_INPUT_ELEMENT_DESC {
    D3D12_INPUT_ELEMENT_DESC {
        InputSlot: slot,
        InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
        InstanceDataStepRate: step,
        ..element(name, format, 0)
    }
}

fn vertex_view(
    buffer: &ID3D12Resource,
    offset: u64,
    size: usize,
    stride: u32,
) -> D3D12_VERTEX_BUFFER_VIEW {
    D3D12_VERTEX_BUFFER_VIEW {
        BufferLocation: unsafe { buffer.GetGPUVirtualAddress() } + offset,
        SizeInBytes: size as u32 - offset as u32,
        StrideInBytes: stride,
    }
}

fn index_view(
    buffer: &ID3D12Resource,
    offset: u64,
    size: usize,
    format: DXGI_FORMAT,
) -> D3D12_INDEX_BUFFER_VIEW {
    D3D12_INDEX_BUFFER_VIEW {
        BufferLocation: unsafe { buffer.GetGPUVirtualAddress() } + offset,
        SizeInBytes: size as u32 - offset as u32,
        Format: format,
    }
}

/// Direct3D 11's draws scene, call for call.
pub fn draws(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = draws11::SIZE;
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
        per_instance(s!("OFFSET"), DXGI_FORMAT_R32G32_FLOAT, 1, 1),
        per_instance(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 2, 2),
    ];
    let pso = pipeline(gpu, &root, dxil!("draws.vs"), dxil!("draws.ps"), &layout)?;
    let signature = |kind: D3D12_INDIRECT_ARGUMENT_TYPE, stride: u32| {
        let argument = D3D12_INDIRECT_ARGUMENT_DESC {
            Type: kind,
            ..Default::default()
        };
        let mut signature: Option<ID3D12CommandSignature> = None;
        unsafe {
            gpu.device.CreateCommandSignature(
                &D3D12_COMMAND_SIGNATURE_DESC {
                    ByteStride: stride,
                    NumArgumentDescs: 1,
                    pArgumentDescs: &argument,
                    NodeMask: 0,
                },
                None,
                &mut signature,
            )?
        };
        signature.context("no command signature")
    };
    let draw = signature(D3D12_INDIRECT_ARGUMENT_TYPE_DRAW, 16)?;
    let draw_indexed = signature(D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED, 20)?;

    let (positions, offsets, colors) = draws11::vertices();
    let wide: Vec<u32> = draws11::INDICES.iter().map(|&i| i as u32).collect();
    let bytes = |b: &[u8]| (gpu.upload(b), b.len());
    let (positions, positions_size) = bytes(bytemuck::cast_slice(&positions));
    let (offsets, offsets_size) = bytes(bytemuck::cast_slice(&offsets));
    let (colors, colors_size) = bytes(bytemuck::cast_slice(&colors));
    let (i16, i16_size) = bytes(bytemuck::cast_slice(&draws11::INDICES));
    let (i32, i32_size) = bytes(bytemuck::cast_slice(&wide));
    let (args, _) = bytes(bytemuck::cast_slice(&draws11::ARGS));
    let (positions, offsets, colors, i16, i32, args) =
        (positions?, offsets?, colors?, i16?, i32?, args?);

    let (target, rtv) = gpu.target(SIZE, SIZE, RGBA)?;
    let (indirect, indirect_rtv) = gpu.target(SIZE, SIZE, RGBA)?;
    gpu.begin(&rtv, SIZE, SIZE);
    let (start, base) = (draws11::LIST_START, draws11::INDEXED_BASE);
    unsafe {
        let list = &gpu.list;
        area(list, 0, 0, SIZE, SIZE);
        list.SetGraphicsRootSignature(&root);
        list.SetPipelineState(&pso);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(
            0,
            Some(&[
                vertex_view(&positions, 0, positions_size, 8),
                vertex_view(&offsets, 0, offsets_size, 8),
                vertex_view(&colors, 0, colors_size, 16),
            ]),
        );
        let at = |i: u32| list.RSSetViewports(&[viewport12(&cell(i, 3, 64.0))]);
        at(0);
        list.DrawInstanced(6, 1, start, 0);
        at(1);
        list.DrawInstanced(6, 4, start, 0);
        at(2);
        list.DrawInstanced(6, 4, start, 3);
        list.IASetIndexBuffer(Some(&index_view(&i16, 0, i16_size, DXGI_FORMAT_R16_UINT)));
        at(3);
        list.DrawIndexedInstanced(6, 1, 2, base, 0);
        at(4);
        list.DrawIndexedInstanced(6, 4, 8, base - 5, 1);
        list.IASetIndexBuffer(Some(&index_view(&i32, 0, i32_size, DXGI_FORMAT_R32_UINT)));
        at(5);
        list.DrawIndexedInstanced(6, 4, 2, base, 2);
        // The indirect draws in a picture of their own, as in Direct3D 11.
        gpu.begin(&indirect_rtv, SIZE, SIZE);
        at(6);
        list.ExecuteIndirect(&draw, 1, &args, 0, None, 0);
        list.IASetIndexBuffer(Some(&index_view(&i32, 4, i32_size, DXGI_FORMAT_R32_UINT)));
        at(7);
        // Starts at index 2 of the view 4 bytes in: index 3 of the whole buffer.
        list.ExecuteIndirect(&draw_indexed, 1, &args, 16, None, 0);
        list.OMSetRenderTargets(1, Some(&gpu.cpu(&rtv, 0)), false, None);
        at(8);
        list.IASetVertexBuffers(
            0,
            Some(&[vertex_view(&positions, 8 * start as u64, positions_size, 8)]),
        );
        list.DrawInstanced(6, 2, 0, 1);
    }
    let picture = gpu.read(&target, 0, RT)?;
    Ok(vec![
        Output::picture("", picture),
        Output::picture("indirect", gpu.read(&indirect, 0, RT)?),
    ])
}

/// A root signature of one table of `count` constant buffer views from b0.
fn table_root(gpu: &Gpu, count: u32) -> Result<ID3D12RootSignature> {
    let range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
        NumDescriptors: count,
        ..Default::default()
    };
    let table = D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                NumDescriptorRanges: 1,
                pDescriptorRanges: &range,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
    };
    gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 1,
                pParameters: &table,
                ..Default::default()
            },
        },
    })
}

/// Direct3D 11's constants scene. A constant buffer view's size is a multiple of 256 bytes, so
/// each small slot's view covers 256 bytes of its own buffer.
pub fn constants(gpu: &Gpu) -> Result<Vec<Output>> {
    let (slots, large) = constants11::values();
    let windows = constants11::WINDOWS;
    let heap = gpu.descriptor_heap(
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        (slots.len() + windows.len()) as u32,
        true,
    )?;
    let mut buffers = Vec::new();
    for (i, values) in slots.iter().enumerate() {
        let buffer = gpu.upload(bytemuck::cast_slice(values))?;
        unsafe {
            gpu.device.CreateConstantBufferView(
                Some(&D3D12_CONSTANT_BUFFER_VIEW_DESC {
                    BufferLocation: buffer.GetGPUVirtualAddress(),
                    SizeInBytes: (values.len() as u32 * 4).next_multiple_of(256),
                }),
                gpu.cpu(&heap, i as u32),
            )
        };
        buffers.push(buffer);
    }
    let big = gpu.upload(bytemuck::cast_slice(&large))?;
    for (i, &(first, count)) in windows.iter().enumerate() {
        unsafe {
            gpu.device.CreateConstantBufferView(
                Some(&D3D12_CONSTANT_BUFFER_VIEW_DESC {
                    BufferLocation: big.GetGPUVirtualAddress() + 16 * first as u64,
                    SizeInBytes: 16 * count,
                }),
                gpu.cpu(&heap, (slots.len() + i) as u32),
            )
        };
    }

    let mut im = Immediate::new(gpu)?;
    im.vs = dxil!("common.vs_full");
    im.colored = false;
    let mut outputs = Vec::new();
    let (target, rtv) = gpu.target(128, 64, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 128, 64);
    im.root = table_root(gpu, slots.len() as u32)?;
    im.ps = dxil!("constants.ps_slots");
    im.tables = vec![(0, gpu.gpu(&heap, 0))];
    unsafe { gpu.list.SetDescriptorHeaps(&[Some(heap.clone())]) };
    im.draw_generated(3)?;
    outputs.push(Output::picture("slots", gpu.read(&target, 0, RT)?));

    let (target, rtv) = gpu.target(64, 64, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 64, 64);
    im.root = table_root(gpu, 1)?;
    im.ps = dxil!("constants.ps_window");
    unsafe { gpu.list.SetDescriptorHeaps(&[Some(heap.clone())]) };
    for i in 0..windows.len() {
        im.viewport(D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 16.0 * i as f32,
            Width: 64.0,
            Height: 16.0,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        });
        im.tables = vec![(0, gpu.gpu(&heap, (slots.len() + i) as u32))];
        im.draw_generated(3)?;
    }
    outputs.push(Output::picture("window", gpu.read(&target, 0, RT)?));
    Ok(outputs)
}
