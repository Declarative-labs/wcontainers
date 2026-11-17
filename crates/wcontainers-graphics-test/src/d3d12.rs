//! The triangle scene drawn with Direct3D 12 into an offscreen target, with the debug layer on.

use std::mem::{ManuallyDrop, transmute_copy};

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, RECT};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows::core::{Interface, s};

use crate::scene::{self, HEIGHT, WIDTH};
use crate::{Driver, Picture, debug, hlsl, picture_from_rows};

/// Bytes per row of the readback buffer: the picture's row, already a multiple of 256 as copies need.
const PITCH: u32 = WIDTH * 4;
const _: () = assert!(PITCH.is_multiple_of(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT));

pub fn render_triangle(driver: Driver) -> Result<Picture> {
    unsafe {
        let mut debug_interface: Option<ID3D12Debug> = None;
        D3D12GetDebugInterface(&mut debug_interface)
            .context("the debug layer needs the Graphics Tools feature")?;
        debug_interface.unwrap().EnableDebugLayer();

        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
        let adapter: IDXGIAdapter1 = match driver {
            Driver::Hardware => factory.EnumAdapters1(0)?,
            Driver::Warp => factory.EnumWarpAdapter()?,
        };
        let adapter_name = crate::adapter_name(driver, &adapter.GetDesc1()?)?;
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)
            .context("D3D12CreateDevice")?;
        let device = device.unwrap();

        let image = draw(&device);
        let messages = debug_messages(&device)?;
        debug::check(messages)?;
        Ok(Picture {
            adapter: adapter_name,
            image: image?.into(),
        })
    }
}

unsafe fn draw(device: &ID3D12Device) -> Result<image::RgbaImage> {
    unsafe {
        let root_desc = D3D12_ROOT_SIGNATURE_DESC {
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            ..Default::default()
        };
        let mut blob = None;
        let mut errors = None;
        D3D12SerializeRootSignature(
            &root_desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut blob,
            Some(&mut errors),
        )
        .with_context(|| {
            errors
                .map(|e| String::from_utf8_lossy(hlsl::blob_bytes(&e)).into_owned())
                .unwrap_or_default()
        })?;
        let root: ID3D12RootSignature =
            device.CreateRootSignature(0, hlsl::blob_bytes(&blob.unwrap()))?;

        let vs = hlsl::compile(scene::TRIANGLE_HLSL, "vs", "vs_5_0")?;
        let ps = hlsl::compile(scene::TRIANGLE_HLSL, "ps", "ps_5_0")?;
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0),
            element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 8),
        ];
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            SrcBlend: D3D12_BLEND_ONE,
            DestBlend: D3D12_BLEND_ZERO,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_ZERO,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
            ..Default::default()
        };
        let mut pipeline_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: bytecode(&vs),
            PS: bytecode(&ps),
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
        pipeline_desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        let pipeline: ID3D12PipelineState = device.CreateGraphicsPipelineState(&pipeline_desc)?;

        let target = committed(
            device,
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: WIDTH as u64,
                Height: HEIGHT,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            Some(&D3D12_CLEAR_VALUE {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                Anonymous: D3D12_CLEAR_VALUE_0 {
                    Color: scene::CLEAR,
                },
            }),
        )?;
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&scene::TRIANGLE);
        let vertices = committed(
            device,
            D3D12_HEAP_TYPE_UPLOAD,
            &buffer_desc(vertex_bytes.len() as u64),
            D3D12_RESOURCE_STATE_GENERIC_READ,
            None,
        )?;
        let mut mapped = std::ptr::null_mut();
        vertices.Map(0, Some(&D3D12_RANGE::default()), Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(vertex_bytes.as_ptr(), mapped.cast(), vertex_bytes.len());
        vertices.Unmap(0, None);
        let readback_size = (PITCH * HEIGHT) as u64;
        let readback = committed(
            device,
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(readback_size),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )?;

        let heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let rtv = heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateRenderTargetView(&target, None, rtv);

        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, &pipeline)?;

        list.SetGraphicsRootSignature(&root);
        list.RSSetViewports(&[D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: WIDTH as f32,
            Height: HEIGHT as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]);
        list.RSSetScissorRects(&[RECT {
            left: 0,
            top: 0,
            right: WIDTH as i32,
            bottom: HEIGHT as i32,
        }]);
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.ClearRenderTargetView(rtv, &scene::CLEAR, None);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertices.GetGPUVirtualAddress(),
                SizeInBytes: vertex_bytes.len() as u32,
                StrideInBytes: 20,
            }]),
        );
        list.DrawInstanced(3, 1, 0, 0);
        list.ResourceBarrier(&[transition(
            &target,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        let destination = D3D12_TEXTURE_COPY_LOCATION {
            pResource: transmute_copy(&readback),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: 0,
                    Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        Width: WIDTH,
                        Height: HEIGHT,
                        Depth: 1,
                        RowPitch: PITCH,
                    },
                },
            },
        };
        let source = D3D12_TEXTURE_COPY_LOCATION {
            pResource: transmute_copy(&target),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        };
        list.CopyTextureRegion(&destination, 0, 0, 0, &source, None);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);

        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        queue.Signal(&fence, 1)?;
        let event = CreateEventW(None, false, false, None)?;
        fence.SetEventOnCompletion(1, event)?;
        WaitForSingleObject(event, INFINITE);
        CloseHandle(event)?;

        let mut mapped = std::ptr::null_mut();
        readback.Map(
            0,
            Some(&D3D12_RANGE {
                Begin: 0,
                End: readback_size as usize,
            }),
            Some(&mut mapped),
        )?;
        let rows = std::slice::from_raw_parts(mapped.cast::<u8>(), readback_size as usize);
        let image = picture_from_rows(WIDTH, HEIGHT, rows, PITCH as usize);
        readback.Unmap(0, Some(&D3D12_RANGE::default()));
        Ok(image)
    }
}

unsafe fn committed(
    device: &ID3D12Device,
    heap: D3D12_HEAP_TYPE,
    desc: &D3D12_RESOURCE_DESC,
    state: D3D12_RESOURCE_STATES,
    clear: Option<&D3D12_CLEAR_VALUE>,
) -> Result<ID3D12Resource> {
    let properties = D3D12_HEAP_PROPERTIES {
        Type: heap,
        ..Default::default()
    };
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &properties,
            D3D12_HEAP_FLAG_NONE,
            desc,
            state,
            clear.map(|c| c as *const _),
            &mut resource,
        )?
    };
    Ok(resource.unwrap())
}

fn buffer_desc(size: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        ..Default::default()
    }
}

fn bytecode(code: &[u8]) -> D3D12_SHADER_BYTECODE {
    D3D12_SHADER_BYTECODE {
        pShaderBytecode: code.as_ptr().cast(),
        BytecodeLength: code.len(),
    }
}

/// A transition barrier that borrows `resource` without counting a reference.
unsafe fn transition(
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { transmute_copy(resource) },
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

fn element(
    name: windows::core::PCSTR,
    format: DXGI_FORMAT,
    offset: u32,
) -> D3D12_INPUT_ELEMENT_DESC {
    D3D12_INPUT_ELEMENT_DESC {
        SemanticName: name,
        SemanticIndex: 0,
        Format: format,
        InputSlot: 0,
        AlignedByteOffset: offset,
        InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    }
}

unsafe fn debug_messages(device: &ID3D12Device) -> Result<Vec<(i32, String)>> {
    let queue: ID3D12InfoQueue = device.cast()?;
    let mut messages = Vec::new();
    unsafe {
        for i in 0..queue.GetNumStoredMessages() {
            let mut length = 0;
            queue.GetMessage(i, None, &mut length)?;
            let mut storage = vec![0u64; length.div_ceil(8)];
            let message = storage.as_mut_ptr().cast::<D3D12_MESSAGE>();
            queue.GetMessage(i, Some(message), &mut length)?;
            let message = &*message;
            messages.push((
                message.Severity.0,
                debug::text(message.pDescription, message.DescriptionByteLength),
            ));
        }
    }
    Ok(messages)
}
