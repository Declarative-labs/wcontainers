//! A small Direct3D 12 program of this track's own, for checking the front against real Direct3D 12:
//! an indexed quad from two vertex streams copied from an upload buffer into a default buffer, one
//! triangle of it culled, a whole clear and a rectangle clear (beside the scissor, which clears
//! ignore), a scissor, and a read back of a box. Run it once beside our
//! d3d12.dll and dxgi.dll and once without, then compare:
//!   check_d3d12 draw <out.rgba>
//!   check_d3d12 compare <a.rgba> <b.rgba> <tolerance>
//! It draws with the rig's triangle shaders, the only DXIL the front takes until DXIL is converted.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("draw") => {
            let pixels = draw()?;
            std::fs::write(&args[2], pixels).expect("writing the picture");
        }
        Some("compare") => {
            let a = std::fs::read(&args[2]).expect("the first picture");
            let b = std::fs::read(&args[3]).expect("the second picture");
            let tolerance: u8 = args[4].parse().expect("a tolerance");
            assert_eq!(a.len(), b.len(), "pictures of different sizes");
            let largest = a
                .iter()
                .zip(&b)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0);
            let beyond = a
                .chunks(4)
                .zip(b.chunks(4))
                .filter(|(x, y)| {
                    x.iter()
                        .zip(y.iter())
                        .any(|(p, q)| p.abs_diff(*q) > tolerance)
                })
                .count();
            println!(
                "{beyond} of {} pixels beyond {tolerance}, largest difference {largest}",
                a.len() / 4
            );
            if beyond > 0 {
                // Where they differ, one character per 4 by 4 block; then the first differing pixel.
                let side = ((a.len() / 4) as f64).sqrt() as usize;
                for by in 0..side / 4 {
                    let line: String = (0..side / 4)
                        .map(|bx| {
                            let differs = (0..16).any(|i| {
                                let p = ((by * 4 + i / 4) * side + bx * 4 + i % 4) * 4;
                                (0..4).any(|c| a[p + c].abs_diff(b[p + c]) > tolerance)
                            });
                            if differs { '#' } else { '.' }
                        })
                        .collect();
                    println!("{line}");
                }
                let p = (0..a.len() / 4)
                    .find(|p| (0..4).any(|c| a[p * 4 + c].abs_diff(b[p * 4 + c]) > tolerance))
                    .unwrap();
                println!(
                    "pixel {} {}: {:?} and {:?}",
                    p % side,
                    p / side,
                    &a[p * 4..p * 4 + 4],
                    &b[p * 4..p * 4 + 4]
                );
            }
            std::process::exit(i32::from(beyond > 0));
        }
        _ => eprintln!("usage: check_d3d12 draw <out.rgba> | compare <a> <b> <tolerance>"),
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn draw() -> windows::core::Result<Vec<u8>> {
    use std::mem::{ManuallyDrop, transmute_copy};
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
    use windows::core::{Interface, s};

    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxil/triangle.vs.dxil");
    const PS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxil/triangle.ps.dxil");
    const SIZE: u32 = 64;
    let buffer = |size: u64| D3D12_RESOURCE_DESC {
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
    };
    unsafe {
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
        let adapter: IDXGIAdapter1 = factory.EnumAdapters1(0)?;
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
        let device = device.unwrap();
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;

        let mut blob = None;
        D3D12SerializeVersionedRootSignature(
            &D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
                Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
                Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                    Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                        ..Default::default()
                    },
                },
            },
            &mut blob,
            None,
        )?;
        let blob = blob.unwrap();
        let root: ID3D12RootSignature = device.CreateRootSignature(
            0,
            std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize()),
        )?;

        // Positions in stream 0, colours in stream 1.
        let layout = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"),
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("COLOR"),
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 1,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
        ];
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: VS.as_ptr().cast(),
                BytecodeLength: VS.len(),
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: PS.as_ptr().cast(),
                BytecodeLength: PS.len(),
            },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_BACK,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: layout.as_ptr(),
                NumElements: 2,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        let pso: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc)?;

        let committed =
            |heap, desc: &D3D12_RESOURCE_DESC, state| -> windows::core::Result<ID3D12Resource> {
                let mut r = None;
                device.CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: heap,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    desc,
                    state,
                    None,
                    &mut r,
                )?;
                Ok(r.unwrap())
            };
        let target = committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: SIZE.into(),
                Height: SIZE,
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
        )?;
        let heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let rtv = heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateRenderTargetView(&target, None, rtv);

        // A quad: positions at 0, colours at 256, 16-bit indices at 512; counter-clockwise ones are
        // back faces and culled.
        let mut bytes = vec![0u8; 1024];
        let positions: [f32; 8] = [-0.9, 0.9, 0.9, 0.9, 0.9, -0.9, -0.9, -0.9];
        let colors: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        let indices: [u16; 9] = [0, 1, 2, 0, 2, 3, 0, 3, 2];
        bytes[..32].copy_from_slice(&positions.map(f32::to_le_bytes).concat());
        bytes[256..304].copy_from_slice(&colors.map(f32::to_le_bytes).concat());
        bytes[512..530].copy_from_slice(&indices.map(u16::to_le_bytes).concat());
        let upload = committed(
            D3D12_HEAP_TYPE_UPLOAD,
            &buffer(1024),
            D3D12_RESOURCE_STATE_GENERIC_READ,
        )?;
        let mut mapped = std::ptr::null_mut();
        upload.Map(0, Some(&D3D12_RANGE::default()), Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.cast(), bytes.len());
        upload.Unmap(0, None);
        let vertices = committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer(1024),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let transition = |r: &ID3D12Resource, before, after| D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: transmute_copy(r),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        };
        list.CopyBufferRegion(&vertices, 0, &upload, 0, 1024);
        list.ResourceBarrier(&[transition(
            &vertices,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER | D3D12_RESOURCE_STATE_INDEX_BUFFER,
        )]);

        list.ClearRenderTargetView(rtv, &[0.1, 0.1, 0.15, 1.0], None);
        list.ClearRenderTargetView(
            rtv,
            &[0.0, 0.5, 0.5, 1.0],
            Some(&[RECT {
                left: 50,
                top: 4,
                right: 62,
                bottom: 30,
            }]),
        );
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]);
        list.RSSetScissorRects(&[RECT {
            left: 0,
            top: 0,
            right: 48,
            bottom: SIZE as i32,
        }]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        let base = vertices.GetGPUVirtualAddress();
        list.IASetVertexBuffers(
            0,
            Some(&[
                D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: base,
                    SizeInBytes: 32,
                    StrideInBytes: 8,
                },
                D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: base + 256,
                    SizeInBytes: 48,
                    StrideInBytes: 12,
                },
            ]),
        );
        list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: base + 512,
            SizeInBytes: 18,
            Format: DXGI_FORMAT_R16_UINT,
        }));
        list.DrawIndexedInstanced(9, 1, 0, 0, 0);

        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0;
        device.GetCopyableFootprints(
            &target.GetDesc(),
            0,
            1,
            0,
            Some(&mut footprint),
            None,
            None,
            Some(&mut total),
        );
        let readback = committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer(total),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        list.ResourceBarrier(&[transition(
            &target,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        list.CopyTextureRegion(
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: transmute_copy(&readback),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: footprint,
                },
            },
            0,
            0,
            0,
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: transmute_copy(&target),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            },
            Some(&D3D12_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: SIZE,
                bottom: SIZE,
                back: 1,
            }),
        );
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        queue.Signal(&fence, 1)?;
        let event = CreateEventW(None, false, false, None)?;
        fence.SetEventOnCompletion(1, event)?;
        WaitForSingleObject(event, INFINITE);
        let mut mapped = std::ptr::null_mut();
        readback.Map(
            0,
            Some(&D3D12_RANGE {
                Begin: 0,
                End: total as usize,
            }),
            Some(&mut mapped),
        )?;
        let rows = std::slice::from_raw_parts(mapped.cast::<u8>(), total as usize);
        let pitch = footprint.Footprint.RowPitch as usize;
        let pixels = (0..SIZE as usize)
            .flat_map(|y| rows[y * pitch..y * pitch + SIZE as usize * 4].to_vec())
            .collect();
        readback.Unmap(0, None);
        Ok(pixels)
    }
}
