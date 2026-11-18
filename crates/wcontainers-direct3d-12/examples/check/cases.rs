//! The pass bar's cases of G5, each a small Direct3D 12 program printing one line of results, run
//! with real Direct3D 12 and through ours and compared line by line: placed resources that alias
//! (buffers written by a copy, a storage view and the CPU; identical textures at one offset), copy
//! and compute queues waiting on each other and on a late CPU signal, binary occlusion resolved,
//! read back and used by `SetPredication`, `ExecuteIndirect`, and pipeline and stream output
//! statistics.

use std::cell::Cell;
use std::fmt::Write;
use std::mem::{ManuallyDrop, transmute_copy};

use windows::Win32::Foundation::{HANDLE, RECT};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows::core::{Interface, Result, s};

const VS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxil/triangle.vs.dxil");
const PS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxil/triangle.ps.dxil");
/// Shades each corner by `SV_VertexID` and moves by a per-instance offset.
const DRAWS_VS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxil/draws.vs.dxil");
const DRAWS_PS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxil/draws.ps.dxil");
/// Reads items of `float2 a; uint b` at t0 and writes changed ones at u0.
const CS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxbc/compute.cs_structured.dxbc");
const PAGE: u64 = 64 << 10;
const SIZE: u32 = 64;

pub const CASES: &[&str] = &[
    "alias-copy",
    "alias-storage",
    "alias-cpu",
    "alias-texture",
    "queues",
    "predication",
    "indirect-draw",
    "indirect-count",
    "indirect-dispatch",
    "indirect-short",
    "depth-planes",
    "indirect-vertex-buffer",
    "indirect-vertex-id",
    "predicated-resolve",
    "occlusion-count",
    "statistics",
];

/// Runs case `name` and returns its line of results.
pub fn run(name: &str) -> Result<String> {
    let c = Ctx::new()?;
    // SAFETY: Direct3D 12 calls on objects this program made, as the API documents them.
    unsafe {
        match name {
            "alias-copy" => alias_copy(&c),
            "alias-storage" => alias_storage(&c),
            "alias-cpu" => alias_cpu(&c),
            "alias-texture" => alias_texture(&c),
            "queues" => queues(&c),
            "predication" => predication(&c),
            "indirect-draw" => indirect_draw(&c, false),
            "indirect-count" => indirect_draw(&c, true),
            "indirect-dispatch" => indirect_dispatch(&c),
            "indirect-short" => indirect_short(&c),
            "depth-planes" => depth_planes(&c),
            "indirect-vertex-buffer" => indirect_vertex_buffer(&c),
            "indirect-vertex-id" => indirect_vertex_id(&c),
            "predicated-resolve" => predicated_resolve(&c),
            "occlusion-count" => occlusion_count(&c),
            "statistics" => statistics(&c),
            _ => panic!("no case {name}"),
        }
    }
}

struct Ctx {
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    value: Cell<u64>,
    event: HANDLE,
}

fn buffer_desc(size: u64, flags: D3D12_RESOURCE_FLAGS) -> D3D12_RESOURCE_DESC {
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
        Flags: flags,
        ..Default::default()
    }
}

fn target_desc() -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
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
    }
}

fn transition(
    r: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            // SAFETY: the barrier borrows the resource for the call it is passed to.
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { transmute_copy(r) },
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

/// An aliasing barrier with no resources named: every placed resource may change.
fn aliasing() -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_ALIASING,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Aliasing: ManuallyDrop::new(D3D12_RESOURCE_ALIASING_BARRIER::default()),
        },
    }
}

fn uav_barrier() -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            UAV: ManuallyDrop::new(D3D12_RESOURCE_UAV_BARRIER::default()),
        },
    }
}

/// The first words and an FNV-1a hash of some bytes.
fn words(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let first: Vec<String> = bytes
        .chunks(4)
        .take(4)
        .map(|w| format!("{:08x}", u32::from_le_bytes(w.try_into().unwrap_or([0; 4]))))
        .collect();
    format!("{} hash {hash:016x}", first.join(" "))
}

/// How many pixels of each colour a picture has, most common first.
fn colours(pixels: &[u8]) -> String {
    let mut counts: std::collections::BTreeMap<[u8; 4], u32> = Default::default();
    for p in pixels.chunks(4) {
        *counts.entry(p.try_into().unwrap()).or_default() += 1;
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    counts
        .iter()
        .map(|(c, n)| format!("{:02x}{:02x}{:02x}{:02x}:{n}", c[0], c[1], c[2], c[3]))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Input items for `CS`: 64 of `float2 a; uint b`.
fn items() -> Vec<u8> {
    (0..64u32)
        .flat_map(|i| {
            [
                (i as f32 * 0.5).to_le_bytes(),
                (100.0 - i as f32).to_le_bytes(),
                (i.wrapping_mul(0x9e37_79b9)).to_le_bytes(),
            ]
            .concat()
        })
        .collect()
}

impl Ctx {
    fn new() -> Result<Self> {
        // SAFETY: plain object creation.
        unsafe {
            let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let adapter: IDXGIAdapter1 = factory.EnumAdapters1(0)?;
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
            let device = device.unwrap();
            let queue = Self::make_queue(&device, D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            Ok(Ctx {
                device,
                queue,
                fence,
                value: Cell::new(0),
                event: CreateEventW(None, false, false, None)?,
            })
        }
    }

    unsafe fn make_queue(
        device: &ID3D12Device,
        kind: D3D12_COMMAND_LIST_TYPE,
    ) -> Result<ID3D12CommandQueue> {
        unsafe {
            device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: kind,
                ..Default::default()
            })
        }
    }

    unsafe fn list(
        &self,
        kind: D3D12_COMMAND_LIST_TYPE,
    ) -> Result<(ID3D12CommandAllocator, ID3D12GraphicsCommandList)> {
        unsafe {
            let allocator: ID3D12CommandAllocator = self.device.CreateCommandAllocator(kind)?;
            let list = self.device.CreateCommandList(0, kind, &allocator, None)?;
            Ok((allocator, list))
        }
    }

    /// Closes `list`, runs it on the direct queue and waits for it.
    unsafe fn run(&self, list: &ID3D12GraphicsCommandList) -> Result<()> {
        unsafe {
            list.Close()?;
            self.queue.ExecuteCommandLists(&[Some(list.cast()?)]);
            self.finish()
        }
    }

    unsafe fn finish(&self) -> Result<()> {
        unsafe {
            let v = self.value.get() + 1;
            self.value.set(v);
            self.queue.Signal(&self.fence, v)?;
            self.fence.SetEventOnCompletion(v, self.event)?;
            WaitForSingleObject(self.event, INFINITE);
            Ok(())
        }
    }

    unsafe fn committed(
        &self,
        heap: D3D12_HEAP_TYPE,
        desc: &D3D12_RESOURCE_DESC,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<ID3D12Resource> {
        unsafe {
            let mut r = None;
            self.device.CreateCommittedResource(
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
        }
    }

    unsafe fn heap(
        &self,
        kind: D3D12_HEAP_TYPE,
        size: u64,
        flags: D3D12_HEAP_FLAGS,
    ) -> Result<ID3D12Heap> {
        unsafe {
            let mut h = None;
            self.device.CreateHeap(
                &D3D12_HEAP_DESC {
                    SizeInBytes: size,
                    Properties: D3D12_HEAP_PROPERTIES {
                        Type: kind,
                        ..Default::default()
                    },
                    Alignment: 0,
                    Flags: flags,
                },
                &mut h,
            )?;
            Ok(h.unwrap())
        }
    }

    unsafe fn placed(
        &self,
        heap: &ID3D12Heap,
        offset: u64,
        desc: &D3D12_RESOURCE_DESC,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<ID3D12Resource> {
        unsafe {
            let mut r = None;
            self.device
                .CreatePlacedResource(heap, offset, desc, state, None, &mut r)?;
            Ok(r.unwrap())
        }
    }

    /// An upload buffer holding `bytes`.
    unsafe fn upload(&self, bytes: &[u8]) -> Result<ID3D12Resource> {
        unsafe {
            let r = self.committed(
                D3D12_HEAP_TYPE_UPLOAD,
                &buffer_desc(bytes.len() as u64, D3D12_RESOURCE_FLAG_NONE),
                D3D12_RESOURCE_STATE_GENERIC_READ,
            )?;
            write(&r, 0, bytes)?;
            Ok(r)
        }
    }

    /// `size` bytes at `offset` of buffer `src`, which the copy promotes from the common state.
    unsafe fn read(&self, src: &ID3D12Resource, offset: u64, size: u64) -> Result<Vec<u8>> {
        unsafe {
            let readback = self.committed(
                D3D12_HEAP_TYPE_READBACK,
                &buffer_desc(size, D3D12_RESOURCE_FLAG_NONE),
                D3D12_RESOURCE_STATE_COPY_DEST,
            )?;
            let (_a, list) = self.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            list.CopyBufferRegion(&readback, 0, src, offset, size);
            self.run(&list)?;
            mapped(&readback, size as usize)
        }
    }

    /// The pixels of a 64 by 64 RGBA8 texture in `state`, as rows without padding.
    unsafe fn pixels(
        &self,
        texture: &ID3D12Resource,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<Vec<u8>> {
        unsafe {
            let pitch = 256u32;
            let readback = self.committed(
                D3D12_HEAP_TYPE_READBACK,
                &buffer_desc(u64::from(pitch * SIZE), D3D12_RESOURCE_FLAG_NONE),
                D3D12_RESOURCE_STATE_COPY_DEST,
            )?;
            let (_a, list) = self.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            list.ResourceBarrier(&[transition(texture, state, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
            list.CopyTextureRegion(
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: transmute_copy(&readback),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                            Offset: 0,
                            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                                Width: SIZE,
                                Height: SIZE,
                                Depth: 1,
                                RowPitch: pitch,
                            },
                        },
                    },
                },
                0,
                0,
                0,
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: transmute_copy(texture),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: 0,
                    },
                },
                None,
            );
            list.ResourceBarrier(&[transition(texture, D3D12_RESOURCE_STATE_COPY_SOURCE, state)]);
            self.run(&list)?;
            let rows = mapped(&readback, (pitch * SIZE) as usize)?;
            Ok(rows
                .chunks(pitch as usize)
                .flat_map(|r| r[..SIZE as usize * 4].to_vec())
                .collect())
        }
    }

    unsafe fn root(
        &self,
        params: &[D3D12_ROOT_PARAMETER],
        flags: D3D12_ROOT_SIGNATURE_FLAGS,
    ) -> Result<ID3D12RootSignature> {
        unsafe {
            let mut blob = None;
            D3D12SerializeRootSignature(
                &D3D12_ROOT_SIGNATURE_DESC {
                    NumParameters: params.len() as u32,
                    pParameters: params.as_ptr(),
                    Flags: flags,
                    ..Default::default()
                },
                D3D_ROOT_SIGNATURE_VERSION_1_0,
                &mut blob,
                None,
            )?;
            let blob = blob.unwrap();
            self.device.CreateRootSignature(
                0,
                std::slice::from_raw_parts(
                    blob.GetBufferPointer().cast::<u8>(),
                    blob.GetBufferSize(),
                ),
            )
        }
    }

    /// The triangle program drawing interleaved `float2` positions and `float3` colours into an
    /// RGBA8 target; with `draws`, the draws program, which also reads an offset per instance from
    /// slot 1 and `SV_VertexID`.
    unsafe fn triangle_pso(
        &self,
        draws: bool,
    ) -> Result<(ID3D12RootSignature, ID3D12PipelineState)> {
        unsafe {
            let root = self.root(
                &[],
                D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            )?;
            let layout = [
                D3D12_INPUT_ELEMENT_DESC {
                    SemanticName: s!("POSITION"),
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                    ..Default::default()
                },
                D3D12_INPUT_ELEMENT_DESC {
                    SemanticName: s!("COLOR"),
                    Format: DXGI_FORMAT_R32G32B32_FLOAT,
                    AlignedByteOffset: 8,
                    InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                    ..Default::default()
                },
                D3D12_INPUT_ELEMENT_DESC {
                    SemanticName: s!("OFFSET"),
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlot: 1,
                    InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
                    InstanceDataStepRate: 1,
                    ..Default::default()
                },
            ];
            let (vs, ps) = if draws {
                (DRAWS_VS, DRAWS_PS)
            } else {
                (VS, PS)
            };
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: vs.as_ptr().cast(),
                    BytecodeLength: vs.len(),
                },
                PS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: ps.as_ptr().cast(),
                    BytecodeLength: ps.len(),
                },
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
                    NumElements: if draws { 3 } else { 2 },
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
            let pso = self.device.CreateGraphicsPipelineState(&desc)?;
            Ok((root, pso))
        }
    }

    /// The compute program `CS` with a root SRV at t0 (parameter 0) and a root UAV at u0
    /// (parameter 1).
    unsafe fn compute_pso(&self) -> Result<(ID3D12RootSignature, ID3D12PipelineState)> {
        unsafe {
            let view = |kind, register| D3D12_ROOT_PARAMETER {
                ParameterType: kind,
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    Descriptor: D3D12_ROOT_DESCRIPTOR {
                        ShaderRegister: register,
                        RegisterSpace: 0,
                    },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
            };
            let root = self.root(
                &[
                    view(D3D12_ROOT_PARAMETER_TYPE_SRV, 0),
                    view(D3D12_ROOT_PARAMETER_TYPE_UAV, 0),
                ],
                D3D12_ROOT_SIGNATURE_FLAG_NONE,
            )?;
            let pso =
                self.device
                    .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                        pRootSignature: transmute_copy(&root),
                        CS: D3D12_SHADER_BYTECODE {
                            pShaderBytecode: CS.as_ptr().cast(),
                            BytecodeLength: CS.len(),
                        },
                        ..Default::default()
                    })?;
            Ok((root, pso))
        }
    }

    /// A 64 by 64 render target cleared to black, its view, and the heap holding the view.
    unsafe fn target(
        &self,
        list: &ID3D12GraphicsCommandList,
    ) -> Result<(
        ID3D12Resource,
        ID3D12DescriptorHeap,
        D3D12_CPU_DESCRIPTOR_HANDLE,
    )> {
        unsafe {
            let target = self.committed(
                D3D12_HEAP_TYPE_DEFAULT,
                &target_desc(),
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            )?;
            let heap: ID3D12DescriptorHeap =
                self.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                        NumDescriptors: 1,
                        ..Default::default()
                    })?;
            let rtv = heap.GetCPUDescriptorHandleForHeapStart();
            self.device.CreateRenderTargetView(&target, None, rtv);
            list.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            viewport(list, 0, 0, SIZE);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            Ok((target, heap, rtv))
        }
    }
}

unsafe fn viewport(list: &ID3D12GraphicsCommandList, x: u32, y: u32, size: u32) {
    unsafe {
        list.RSSetViewports(&[D3D12_VIEWPORT {
            TopLeftX: x as f32,
            TopLeftY: y as f32,
            Width: size as f32,
            Height: size as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]);
        list.RSSetScissorRects(&[RECT {
            left: 0,
            top: 0,
            right: SIZE as i32,
            bottom: SIZE as i32,
        }]);
    }
}

unsafe fn write(r: &ID3D12Resource, offset: usize, bytes: &[u8]) -> Result<()> {
    unsafe {
        let mut p = std::ptr::null_mut();
        r.Map(0, Some(&D3D12_RANGE::default()), Some(&mut p))?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.cast::<u8>().add(offset), bytes.len());
        r.Unmap(0, None);
        Ok(())
    }
}

unsafe fn mapped(r: &ID3D12Resource, size: usize) -> Result<Vec<u8>> {
    unsafe {
        let mut p = std::ptr::null_mut();
        r.Map(
            0,
            Some(&D3D12_RANGE {
                Begin: 0,
                End: size,
            }),
            Some(&mut p),
        )?;
        let bytes = std::slice::from_raw_parts(p.cast::<u8>(), size).to_vec();
        r.Unmap(0, None);
        Ok(bytes)
    }
}

/// Triangles of one colour each, as `float2` positions and `float3` colours: a large one, one in
/// the left half and one in the right half, then a degenerate one.
fn triangles() -> Vec<u8> {
    let tri = |p: [f32; 6], c: [f32; 3]| -> Vec<u8> {
        (0..3)
            .flat_map(|i| [p[i * 2], p[i * 2 + 1], c[0], c[1], c[2]])
            .flat_map(f32::to_le_bytes)
            .collect()
    };
    [
        tri([-0.8, -0.8, 0.0, 0.8, 0.8, -0.8], [1.0, 0.0, 0.0]),
        tri([-0.9, -0.5, -0.5, 0.5, -0.1, -0.5], [0.0, 1.0, 0.0]),
        tri([0.1, -0.5, 0.5, 0.5, 0.9, -0.5], [0.0, 0.0, 1.0]),
        tri([0.2, 0.2, 0.2, 0.2, 0.2, 0.2], [1.0, 1.0, 1.0]),
    ]
    .concat()
}

unsafe fn vertex_buffer(list: &ID3D12GraphicsCommandList, upload: &ID3D12Resource) {
    unsafe {
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: upload.GetGPUVirtualAddress(),
                SizeInBytes: 12 * 20,
                StrideInBytes: 20,
            }]),
        );
    }
}

/// Buffers A (0 to 2 pages) and B (1 to 3 pages) of one default heap; a copy writes A, B's
/// first page is read.
unsafe fn alias_copy(c: &Ctx) -> Result<String> {
    unsafe {
        let heap = c.heap(
            D3D12_HEAP_TYPE_DEFAULT,
            3 * PAGE,
            D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
        )?;
        let common = D3D12_RESOURCE_STATE_COMMON;
        let a = c.placed(
            &heap,
            0,
            &buffer_desc(2 * PAGE, D3D12_RESOURCE_FLAG_NONE),
            common,
        )?;
        let b = c.placed(
            &heap,
            PAGE,
            &buffer_desc(2 * PAGE, D3D12_RESOURCE_FLAG_NONE),
            common,
        )?;
        let pattern: Vec<u8> = (0..2 * PAGE as u32 / 4)
            .flat_map(|i| i.wrapping_mul(2_654_435_761).to_le_bytes())
            .collect();
        let upload = c.upload(&pattern)?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.CopyBufferRegion(&a, 0, &upload, 0, 2 * PAGE);
        list.ResourceBarrier(&[aliasing()]);
        c.run(&list)?;
        let bytes = c.read(&b, 0, PAGE)?;
        let expected = bytes == pattern[PAGE as usize..];
        Ok(format!("inherited {expected} {}", words(&bytes)))
    }
}

/// A storage view writes A's second page; B, placed there, is read.
unsafe fn alias_storage(c: &Ctx) -> Result<String> {
    unsafe {
        let heap = c.heap(
            D3D12_HEAP_TYPE_DEFAULT,
            2 * PAGE,
            D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
        )?;
        let common = D3D12_RESOURCE_STATE_COMMON;
        let uav = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
        let a = c.placed(&heap, 0, &buffer_desc(2 * PAGE, uav), common)?;
        let b = c.placed(
            &heap,
            PAGE,
            &buffer_desc(PAGE, D3D12_RESOURCE_FLAG_NONE),
            common,
        )?;
        let input = c.upload(&items())?;
        let (root, pso) = c.compute_pso()?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pso);
        list.SetComputeRootShaderResourceView(0, input.GetGPUVirtualAddress());
        list.SetComputeRootUnorderedAccessView(1, a.GetGPUVirtualAddress() + PAGE);
        list.Dispatch(1, 1, 1);
        list.ResourceBarrier(&[uav_barrier(), aliasing()]);
        c.run(&list)?;
        Ok(words(&c.read(&b, 0, 768)?))
    }
}

/// Buffers X (0 to 2 pages) and Y (1 to 2 pages) of one upload heap: the CPU writes through X,
/// a copy reads Y.
unsafe fn alias_cpu(c: &Ctx) -> Result<String> {
    unsafe {
        let heap = c.heap(
            D3D12_HEAP_TYPE_UPLOAD,
            2 * PAGE,
            D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
        )?;
        let read = D3D12_RESOURCE_STATE_GENERIC_READ;
        let x = c.placed(
            &heap,
            0,
            &buffer_desc(2 * PAGE, D3D12_RESOURCE_FLAG_NONE),
            read,
        )?;
        let y = c.placed(
            &heap,
            PAGE,
            &buffer_desc(PAGE, D3D12_RESOURCE_FLAG_NONE),
            read,
        )?;
        let pattern: Vec<u8> = (0..64u32).flat_map(|i| (i * 7 + 3).to_le_bytes()).collect();
        write(&x, PAGE as usize, &pattern)?;
        let bytes = c.read(&y, 0, 256)?;
        Ok(format!("inherited {} {}", bytes == pattern, words(&bytes)))
    }
}

/// Two identically described render targets at one offset: the first is cleared, the second read.
unsafe fn alias_texture(c: &Ctx) -> Result<String> {
    unsafe {
        let heap = c.heap(
            D3D12_HEAP_TYPE_DEFAULT,
            PAGE,
            D3D12_HEAP_FLAG_ALLOW_ONLY_RT_DS_TEXTURES,
        )?;
        let state = D3D12_RESOURCE_STATE_RENDER_TARGET;
        let first = c.placed(&heap, 0, &target_desc(), state)?;
        let second = c.placed(&heap, 0, &target_desc(), state)?;
        let views: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let rtv = views.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateRenderTargetView(&first, None, rtv);
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.ClearRenderTargetView(rtv, &[0.25, 0.5, 0.75, 1.0], None);
        list.ClearRenderTargetView(
            rtv,
            &[1.0, 0.0, 0.0, 1.0],
            Some(&[RECT {
                left: 8,
                top: 8,
                right: 24,
                bottom: 40,
            }]),
        );
        list.ResourceBarrier(&[aliasing()]);
        c.run(&list)?;
        Ok(colours(&c.pixels(&second, state)?))
    }
}

/// A compute queue waits for a copy queue, which waits for the CPU; the copy queue then waits for
/// the compute queue's result and copies it out. The CPU writes the upload memory and signals only
/// after all of it is submitted.
unsafe fn queues(c: &Ctx) -> Result<String> {
    unsafe {
        let copy = Ctx::make_queue(&c.device, D3D12_COMMAND_LIST_TYPE_COPY)?;
        let compute = Ctx::make_queue(&c.device, D3D12_COMMAND_LIST_TYPE_COMPUTE)?;
        let fence = |v| c.device.CreateFence(v, D3D12_FENCE_FLAG_NONE);
        let (cpu, copied, computed): (ID3D12Fence, ID3D12Fence, ID3D12Fence) =
            (fence(0)?, fence(0)?, fence(0)?);
        let common = D3D12_RESOURCE_STATE_COMMON;
        let upload = c.upload(&vec![0; 768])?;
        let source = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(768, D3D12_RESOURCE_FLAG_NONE),
            common,
        )?;
        let output = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(768, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            common,
        )?;
        let readback = c.committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(768, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let (root, pso) = c.compute_pso()?;
        let (_a1, work) = c.list(D3D12_COMMAND_LIST_TYPE_COMPUTE)?;
        work.SetComputeRootSignature(&root);
        work.SetPipelineState(&pso);
        work.SetComputeRootShaderResourceView(0, source.GetGPUVirtualAddress());
        work.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress());
        work.Dispatch(1, 1, 1);
        work.Close()?;
        let (_a2, fill) = c.list(D3D12_COMMAND_LIST_TYPE_COPY)?;
        fill.CopyBufferRegion(&source, 0, &upload, 0, 768);
        fill.Close()?;
        let (_a3, out) = c.list(D3D12_COMMAND_LIST_TYPE_COPY)?;
        out.CopyBufferRegion(&readback, 0, &output, 0, 768);
        out.Close()?;

        compute.Wait(&copied, 1)?;
        compute.ExecuteCommandLists(&[Some(work.cast()?)]);
        compute.Signal(&computed, 1)?;
        copy.Wait(&cpu, 1)?;
        copy.ExecuteCommandLists(&[Some(fill.cast()?)]);
        copy.Signal(&copied, 1)?;
        copy.Wait(&computed, 1)?;
        copy.ExecuteCommandLists(&[Some(out.cast()?)]);
        copy.Signal(&copied, 2)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        let before = (copied.GetCompletedValue(), computed.GetCompletedValue());
        write(&upload, 0, &items())?;
        cpu.Signal(1)?;
        copied.SetEventOnCompletion(2, c.event)?;
        WaitForSingleObject(c.event, INFINITE);
        let after = (copied.GetCompletedValue(), computed.GetCompletedValue());
        Ok(format!(
            "before {before:?} after {after:?} {}",
            words(&mapped(&readback, 768)?)
        ))
    }
}

/// Two binary occlusion queries (a visible triangle, a degenerate one) resolved into a default
/// buffer, copied to a readback buffer, and used by `SetPredication` for draws and clears.
unsafe fn predication(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let vertices = c.upload(&triangles())?;
        let mut queries = None;
        c.device.CreateQueryHeap(
            &D3D12_QUERY_HEAP_DESC {
                Type: D3D12_QUERY_HEAP_TYPE_OCCLUSION,
                Count: 2,
                NodeMask: 0,
            },
            &mut queries,
        )?;
        let queries: ID3D12QueryHeap = queries.unwrap();
        let answers = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(16, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let readback = c.committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(16, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, rtv) = c.target(&list)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        let binary = D3D12_QUERY_TYPE_BINARY_OCCLUSION;
        list.BeginQuery(&queries, binary, 0);
        list.DrawInstanced(3, 1, 0, 0);
        list.EndQuery(&queries, binary, 0);
        list.BeginQuery(&queries, binary, 1);
        list.DrawInstanced(3, 1, 9, 0);
        list.EndQuery(&queries, binary, 1);
        list.ResolveQueryData(&queries, binary, 0, 2, &answers, 0);
        list.ResourceBarrier(&[transition(
            &answers,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_COPY_SOURCE | D3D12_RESOURCE_STATE_PREDICATION,
        )]);
        list.CopyBufferRegion(&readback, 0, &answers, 0, 16);
        // Each quadrant: drawn where the predicate lets it through.
        let half = SIZE / 2;
        let cases = [
            (0, D3D12_PREDICATION_OP_EQUAL_ZERO, 0, 0),
            (8, D3D12_PREDICATION_OP_EQUAL_ZERO, half, 0),
            (0, D3D12_PREDICATION_OP_NOT_EQUAL_ZERO, 0, half),
            (8, D3D12_PREDICATION_OP_NOT_EQUAL_ZERO, half, half),
        ];
        for (offset, op, x, y) in cases {
            list.SetPredication(&answers, offset, op);
            viewport(&list, x, y, half);
            list.DrawInstanced(3, 1, 3, 0);
        }
        list.SetPredication(&answers, 8, D3D12_PREDICATION_OP_EQUAL_ZERO);
        let rect = |x: i32| RECT {
            left: x,
            top: 28,
            right: x + 8,
            bottom: 36,
        };
        list.ClearRenderTargetView(rtv, &[1.0, 1.0, 0.0, 1.0], Some(&[rect(4)]));
        list.SetPredication(&answers, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
        list.ClearRenderTargetView(rtv, &[0.0, 1.0, 1.0, 1.0], Some(&[rect(52)]));
        list.SetPredication(None, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
        c.run(&list)?;
        let resolved = mapped(&readback, 16)?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        let at = |x: usize, y: usize| {
            let p = (y * SIZE as usize + x) * 4;
            format!(
                "{:02x}{:02x}{:02x}",
                pixels[p],
                pixels[p + 1],
                pixels[p + 2]
            )
        };
        let mut out = format!("resolved {} pixels", words(&resolved));
        for (x, y) in [(16, 20), (48, 20), (16, 52), (48, 52), (8, 32), (56, 32)] {
            write!(out, " {}", at(x, y)).unwrap();
        }
        write!(out, " {}", colours(&pixels)).unwrap();
        Ok(out)
    }
}

/// Counted occlusion queries: one draw, two draws with a clear between them, none, a degenerate
/// triangle and two instances; resolved in a second list, and one count used as a predicate.
unsafe fn occlusion_count(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let vertices = c.upload(&triangles())?;
        let mut queries = None;
        c.device.CreateQueryHeap(
            &D3D12_QUERY_HEAP_DESC {
                Type: D3D12_QUERY_HEAP_TYPE_OCCLUSION,
                Count: 5,
                NodeMask: 0,
            },
            &mut queries,
        )?;
        let queries: ID3D12QueryHeap = queries.unwrap();
        let answers = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(40, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let readback = c.committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(40, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, rtv) = c.target(&list)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        let counted = D3D12_QUERY_TYPE_OCCLUSION;
        list.BeginQuery(&queries, counted, 0);
        list.DrawInstanced(3, 1, 0, 0);
        list.EndQuery(&queries, counted, 0);
        list.BeginQuery(&queries, counted, 1);
        list.DrawInstanced(3, 1, 3, 0);
        let rect = RECT {
            left: 0,
            top: 0,
            right: 4,
            bottom: 4,
        };
        list.ClearRenderTargetView(rtv, &[1.0, 1.0, 0.0, 1.0], Some(&[rect]));
        list.DrawInstanced(3, 1, 6, 0);
        list.EndQuery(&queries, counted, 1);
        list.BeginQuery(&queries, counted, 2);
        list.EndQuery(&queries, counted, 2);
        list.BeginQuery(&queries, counted, 3);
        list.DrawInstanced(3, 1, 9, 0);
        list.EndQuery(&queries, counted, 3);
        list.BeginQuery(&queries, counted, 4);
        list.DrawInstanced(3, 2, 3, 0);
        list.EndQuery(&queries, counted, 4);
        c.run(&list)?;
        let (_b, second) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        second.ResolveQueryData(&queries, counted, 0, 5, &answers, 0);
        second.ResourceBarrier(&[transition(
            &answers,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_COPY_SOURCE | D3D12_RESOURCE_STATE_PREDICATION,
        )]);
        second.CopyBufferRegion(&readback, 0, &answers, 0, 40);
        // The empty query's zero skips the first clear; the first query's count lets the second
        // through.
        let rect = |x: i32| RECT {
            left: x,
            top: 28,
            right: x + 8,
            bottom: 36,
        };
        second.SetPredication(&answers, 16, D3D12_PREDICATION_OP_EQUAL_ZERO);
        second.ClearRenderTargetView(rtv, &[0.0, 1.0, 1.0, 1.0], Some(&[rect(4)]));
        second.SetPredication(&answers, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
        second.ClearRenderTargetView(rtv, &[1.0, 0.0, 1.0, 1.0], Some(&[rect(52)]));
        second.SetPredication(None, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
        c.run(&second)?;
        let resolved = mapped(&readback, 40)?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        Ok(format!(
            "counted {} pixels {}",
            words(&resolved),
            colours(&pixels)
        ))
    }
}

/// Pipeline statistics queries around nothing, around direct draws, an indexed draw and a dispatch
/// (with a counted occlusion query inside), and around an indirect draw and a skipped predicated
/// one; a stream output statistics query. Counts Direct3D leaves to the implementation are printed
/// as bounds: vertex shader invocations at most the vertices read, primitives out of the clipper
/// above zero, pixel shader invocations at least the samples the occlusion query counted.
unsafe fn statistics(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let (compute_root, compute) = c.compute_pso()?;
        let vertices = c.upload(&triangles())?;
        let indices = c.upload(&u32s(&[0, 1, 2, 3, 4, 5]))?;
        let input = c.upload(&items())?;
        let output = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(1024, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        let (draws, draw_args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            16,
            &u32s(&[3, 2, 3, 0]),
        )?;
        let heap = |kind, count| -> Result<ID3D12QueryHeap> {
            let mut q = None;
            c.device.CreateQueryHeap(
                &D3D12_QUERY_HEAP_DESC {
                    Type: kind,
                    Count: count,
                    NodeMask: 0,
                },
                &mut q,
            )?;
            Ok(q.unwrap())
        };
        let stats = heap(D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS, 3)?;
        let occlusion = heap(D3D12_QUERY_HEAP_TYPE_OCCLUSION, 1)?;
        let streams = heap(D3D12_QUERY_HEAP_TYPE_SO_STATISTICS, 2)?;
        let size = 3 * 88 + 8 + 16;
        let readback = c.committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(size, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let zero = filled(c, &[0; 8])?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (_target, _heap, _rtv) = c.target(&list)?;
        let kind = D3D12_QUERY_TYPE_PIPELINE_STATISTICS;
        list.BeginQuery(&stats, kind, 0);
        list.EndQuery(&stats, kind, 0);
        list.BeginQuery(&streams, D3D12_QUERY_TYPE_SO_STATISTICS_STREAM1, 1);
        list.BeginQuery(&stats, kind, 1);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        list.BeginQuery(&occlusion, D3D12_QUERY_TYPE_OCCLUSION, 0);
        list.DrawInstanced(3, 1, 0, 0);
        list.DrawInstanced(3, 2, 3, 0);
        list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: indices.GetGPUVirtualAddress(),
            SizeInBytes: 24,
            Format: DXGI_FORMAT_R32_UINT,
        }));
        list.DrawIndexedInstanced(5, 1, 1, 3, 0);
        list.EndQuery(&occlusion, D3D12_QUERY_TYPE_OCCLUSION, 0);
        list.SetComputeRootSignature(&compute_root);
        list.SetPipelineState(&compute);
        list.SetComputeRootShaderResourceView(0, input.GetGPUVirtualAddress());
        list.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress());
        list.Dispatch(1, 1, 1);
        list.EndQuery(&stats, kind, 1);
        list.BeginQuery(&stats, kind, 2);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.ExecuteIndirect(&draws, 1, &draw_args, 0, None, 0);
        list.SetPredication(&zero, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
        list.DrawInstanced(3, 1, 0, 0);
        list.SetPredication(None, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
        list.EndQuery(&stats, kind, 2);
        list.EndQuery(&streams, D3D12_QUERY_TYPE_SO_STATISTICS_STREAM1, 1);
        list.ResolveQueryData(&stats, kind, 0, 3, &readback, 0);
        list.ResolveQueryData(
            &occlusion,
            D3D12_QUERY_TYPE_OCCLUSION,
            0,
            1,
            &readback,
            3 * 88,
        );
        list.ResolveQueryData(
            &streams,
            D3D12_QUERY_TYPE_SO_STATISTICS_STREAM1,
            1,
            1,
            &readback,
            3 * 88 + 8,
        );
        c.run(&list)?;
        let bytes = mapped(&readback, size as usize)?;
        let u64s: Vec<u64> = bytes
            .chunks(8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let samples = u64s[33];
        let mut out = format!("occlusion {samples}");
        for (i, q) in u64s[..33].chunks(11).enumerate() {
            let s = D3D12_QUERY_DATA_PIPELINE_STATISTICS {
                IAVertices: q[0],
                IAPrimitives: q[1],
                VSInvocations: q[2],
                GSInvocations: q[3],
                GSPrimitives: q[4],
                CInvocations: q[5],
                CPrimitives: q[6],
                PSInvocations: q[7],
                HSInvocations: q[8],
                DSInvocations: q[9],
                CSInvocations: q[10],
            };
            write!(
                out,
                " | ia {} {} vs<=ia {} gs {} c {} c>0 {} ps {} hs {} ds {} cs {}",
                s.IAVertices,
                s.IAPrimitives,
                s.VSInvocations <= s.IAVertices,
                s.GSPrimitives,
                s.CInvocations,
                s.CPrimitives > 0,
                // At least the occlusion query's samples where it ran; some where anything drew.
                (i != 1 || s.PSInvocations >= samples)
                    && (s.PSInvocations == 0) == (s.IAPrimitives == 0),
                s.HSInvocations,
                s.DSInvocations,
                s.CSInvocations,
            )
            .unwrap();
        }
        write!(out, " | streams {:?}", &u64s[34..36]).unwrap();
        Ok(out)
    }
}

/// A command signature of one argument, and a default buffer holding `bytes` for its arguments.
unsafe fn arguments(
    c: &Ctx,
    kind: D3D12_INDIRECT_ARGUMENT_TYPE,
    stride: u32,
    bytes: &[u8],
) -> Result<(ID3D12CommandSignature, ID3D12Resource)> {
    unsafe {
        let argument = D3D12_INDIRECT_ARGUMENT_DESC {
            Type: kind,
            ..Default::default()
        };
        let mut signature = None;
        c.device.CreateCommandSignature(
            &D3D12_COMMAND_SIGNATURE_DESC {
                ByteStride: stride,
                NumArgumentDescs: 1,
                pArgumentDescs: &argument,
                NodeMask: 0,
            },
            None,
            &mut signature,
        )?;
        Ok((signature.unwrap(), filled(c, bytes)?))
    }
}

/// A default buffer holding `bytes`, copied from an upload buffer.
unsafe fn filled(c: &Ctx, bytes: &[u8]) -> Result<ID3D12Resource> {
    unsafe {
        let upload = c.upload(bytes)?;
        let buffer = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(bytes.len() as u64, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.CopyBufferRegion(&buffer, 0, &upload, 0, bytes.len() as u64);
        c.run(&list)?;
        Ok(buffer)
    }
}

fn u32s(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Draws and indexed draws from argument buffers, at strides past their size; with `count`, a
/// count buffer allows fewer than the most.
unsafe fn indirect_draw(c: &Ctx, count: bool) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let vertices = c.upload(&triangles())?;
        let indices = c.upload(&u32s(&[2, 1, 0, 0, 1, 2]))?;
        // Draw: the left triangle, then the right one; indexed: the large one from base vertex 0
        // and index 3, then the left one from base vertex 3.
        let (draws, draw_args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            20,
            &u32s(&[3, 1, 3, 0, 99, 3, 1, 6, 0, 99]),
        )?;
        let (indexed, indexed_args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
            24,
            &u32s(&[3, 1, 3, 0, 0, 99, 3, 1, 0, 3, 0, 99]),
        )?;
        let counts = filled(c, &u32s(&[1, 0, 2, 7]))?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: indices.GetGPUVirtualAddress(),
            SizeInBytes: 24,
            Format: DXGI_FORMAT_R32_UINT,
        }));
        let half = SIZE / 2;
        viewport(&list, 0, 0, half);
        if count {
            // One draw of the two, and the most (2) of a count of 7.
            list.ExecuteIndirect(&draws, 2, &draw_args, 0, &counts, 0);
            viewport(&list, half, half, half);
            list.ExecuteIndirect(&indexed, 2, &indexed_args, 0, &counts, 12);
            // A count of zero draws nothing.
            viewport(&list, half, 0, half);
            list.ExecuteIndirect(&draws, 2, &draw_args, 0, &counts, 4);
        } else {
            list.ExecuteIndirect(&draws, 2, &draw_args, 0, None, 0);
            viewport(&list, half, half, half);
            list.ExecuteIndirect(&indexed, 2, &indexed_args, 0, None, 0);
        }
        c.run(&list)?;
        Ok(colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ))
    }
}

/// Dispatches from an argument buffer, with and without a count.
unsafe fn indirect_dispatch(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.compute_pso()?;
        let input = c.upload(&items())?;
        let (dispatches, args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
            12,
            &u32s(&[1, 1, 1, 1, 1, 1]),
        )?;
        let counts = filled(c, &u32s(&[0, 1]))?;
        let output = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(3 * 1024, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pso);
        list.SetComputeRootShaderResourceView(0, input.GetGPUVirtualAddress());
        // 64 items of 12 bytes into each of three ranges of 1024 bytes.
        list.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress());
        list.ExecuteIndirect(&dispatches, 1, &args, 0, None, 0);
        list.ResourceBarrier(&[uav_barrier()]);
        list.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress() + 1024);
        // A count of zero: nothing.
        list.ExecuteIndirect(&dispatches, 2, &args, 0, &counts, 0);
        list.ResourceBarrier(&[uav_barrier()]);
        list.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress() + 2048);
        // A count of one.
        list.ExecuteIndirect(&dispatches, 2, &args, 0, &counts, 4);
        list.ResourceBarrier(&[uav_barrier()]);
        c.run(&list)?;
        let bytes = c.read(&output, 0, 3 * 1024)?;
        let mut out = String::new();
        for part in bytes.chunks(1024) {
            write!(out, "{} | ", words(&part[..768])).unwrap();
        }
        Ok(out)
    }
}

/// An argument buffer that holds the last command's arguments but not its whole stride.
unsafe fn indirect_short(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let (draws, args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            20,
            &u32s(&[3, 1, 3, 0, 99, 3, 1, 6, 0]),
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.ExecuteIndirect(&draws, 2, &args, 0, None, 0);
        Ok(match list.Close() {
            Ok(()) => "closed".to_string(),
            Err(e) => format!("close refused {:?}", e.code()),
        })
    }
}

/// A signature that sets the vertex buffer before each draw: each command draws the triangle at
/// its own place in the buffer.
unsafe fn indirect_vertex_buffer(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let vertices = c.upload(&triangles())?;
        let arguments = [
            D3D12_INDIRECT_ARGUMENT_DESC {
                Type: D3D12_INDIRECT_ARGUMENT_TYPE_VERTEX_BUFFER_VIEW,
                Anonymous: D3D12_INDIRECT_ARGUMENT_DESC_0 {
                    VertexBuffer: D3D12_INDIRECT_ARGUMENT_DESC_0_0 { Slot: 0 },
                },
            },
            D3D12_INDIRECT_ARGUMENT_DESC {
                Type: D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
                ..Default::default()
            },
        ];
        let mut signature = None;
        c.device.CreateCommandSignature(
            &D3D12_COMMAND_SIGNATURE_DESC {
                ByteStride: 32,
                NumArgumentDescs: 2,
                pArgumentDescs: arguments.as_ptr(),
                NodeMask: 0,
            },
            None,
            &mut signature,
        )?;
        let signature: ID3D12CommandSignature = signature.unwrap();
        let base = vertices.GetGPUVirtualAddress();
        let command = |triangle: u64| {
            let address = base + triangle * 60;
            u32s(&[address as u32, (address >> 32) as u32, 60, 20, 3, 1, 0, 0])
        };
        let args = filled(c, &[command(1), command(2)].concat())?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.ExecuteIndirect(&signature, 2, &args, 0, None, 0);
        c.run(&list)?;
        Ok(colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ))
    }
}

/// Indexed indirect draws of a vertex shader shading by `SV_VertexID`, from base vertex 0 and from
/// base vertex 3 (also through a count buffer): Direct3D's vertex IDs are the indices, without the
/// base vertex; and a draw from first vertex 3 and first instance 1, whose IDs leave both out.
unsafe fn indirect_vertex_id(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(true)?;
        let vertices = c.upload(&triangles())?;
        let offsets = c.upload(&[0; 16])?;
        let indices = c.upload(&u32s(&[0, 1, 2]))?;
        let (plain, plain_args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            16,
            &u32s(&[3, 1, 3, 1]),
        )?;
        let counts = filled(c, &u32s(&[2]))?;
        let (draws, args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
            20,
            &u32s(&[3, 1, 0, 0, 0, 3, 1, 0, 3, 0, 3, 1, 0, 6, 0]),
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        list.IASetVertexBuffers(
            1,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: offsets.GetGPUVirtualAddress(),
                SizeInBytes: 16,
                StrideInBytes: 8,
            }]),
        );
        list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: indices.GetGPUVirtualAddress(),
            SizeInBytes: 12,
            Format: DXGI_FORMAT_R32_UINT,
        }));
        let half = SIZE / 2;
        viewport(&list, 0, 0, half);
        list.ExecuteIndirect(&draws, 1, &args, 0, None, 0);
        viewport(&list, half, half, half);
        list.ExecuteIndirect(&draws, 1, &args, 20, None, 0);
        viewport(&list, half, 0, half);
        list.ExecuteIndirect(&plain, 1, &plain_args, 0, None, 0);
        viewport(&list, 0, half, half);
        list.ExecuteIndirect(&draws, 3, &args, 0, &counts, 0);
        c.run(&list)?;
        Ok(colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ))
    }
}

/// The copyable footprints of both planes of the depth-stencil formats, and the planes of a
/// cleared texture copied out.
unsafe fn depth_planes(c: &Ctx) -> Result<String> {
    unsafe {
        let mut out = String::new();
        for (name, format, planes) in [
            ("d24s8", DXGI_FORMAT_D24_UNORM_S8_UINT, 2),
            ("d32s8", DXGI_FORMAT_D32_FLOAT_S8X24_UINT, 2),
            ("d32", DXGI_FORMAT_D32_FLOAT, 1),
            ("d16", DXGI_FORMAT_D16_UNORM, 1),
        ] {
            let n = 2 * planes;
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: 64,
                Height: 32,
                DepthOrArraySize: 1,
                MipLevels: 2,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
                ..Default::default()
            };
            let mut layouts = [D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default(); 4];
            let (mut rows, mut sizes, mut total) = ([0u32; 4], [0u64; 4], 0u64);
            c.device.GetCopyableFootprints(
                &desc,
                0,
                n as u32,
                0,
                Some(layouts.as_mut_ptr()),
                Some(rows.as_mut_ptr()),
                Some(sizes.as_mut_ptr()),
                Some(&mut total),
            );
            write!(out, "{name} total {total}").unwrap();
            for i in 0..n {
                let f = layouts[i].Footprint;
                write!(
                    out,
                    " [{} at {} {}x{} pitch {} rows {} size {}]",
                    f.Format.0, layouts[i].Offset, f.Width, f.Height, f.RowPitch, rows[i], sizes[i]
                )
                .unwrap();
            }
            if total > 1 << 20 {
                write!(out, "; ").unwrap();
                continue;
            }
            let texture = c.committed(
                D3D12_HEAP_TYPE_DEFAULT,
                &desc,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
            )?;
            let views: ID3D12DescriptorHeap =
                c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                    NumDescriptors: 1,
                    ..Default::default()
                })?;
            let dsv = views.GetCPUDescriptorHandleForHeapStart();
            c.device.CreateDepthStencilView(&texture, None, dsv);
            let readback = c.committed(
                D3D12_HEAP_TYPE_READBACK,
                &buffer_desc(total, D3D12_RESOURCE_FLAG_NONE),
                D3D12_RESOURCE_STATE_COPY_DEST,
            )?;
            let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let flags = if planes == 2 {
                D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL
            } else {
                D3D12_CLEAR_FLAG_DEPTH
            };
            list.ClearDepthStencilView(dsv, flags, 0.5, 0x5a, None);
            list.ResourceBarrier(&[transition(
                &texture,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
            let copied = if planes == 2 { vec![0, 2] } else { vec![0] };
            for &i in &copied {
                list.CopyTextureRegion(
                    &D3D12_TEXTURE_COPY_LOCATION {
                        pResource: transmute_copy(&readback),
                        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                            PlacedFootprint: layouts[i],
                        },
                    },
                    0,
                    0,
                    0,
                    &D3D12_TEXTURE_COPY_LOCATION {
                        pResource: transmute_copy(&texture),
                        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                            SubresourceIndex: i as u32,
                        },
                    },
                    None,
                );
            }
            c.run(&list)?;
            let bytes = mapped(&readback, total as usize)?;
            for &i in &copied {
                let at = layouts[i].Offset as usize;
                write!(out, " plane {i}: {}", words(&bytes[at..at + 16])).unwrap();
            }
            write!(out, "; ").unwrap();
        }
        Ok(out)
    }
}

/// A multisampled target cleared in one list, beside a texture written by a copy in it, then
/// resolved into that texture under a predicate that skips and one that lets it through.
unsafe fn predicated_resolve(c: &Ctx) -> Result<String> {
    unsafe {
        let ms = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 4,
                    Quality: 0,
                },
                ..target_desc()
            },
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        let copy = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Flags: D3D12_RESOURCE_FLAG_NONE,
                ..target_desc()
            },
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let pitch = 256u32;
        let texels: Vec<u8> = (0..pitch * SIZE / 4)
            .flat_map(|_| 0x3949_5969u32.to_le_bytes())
            .collect();
        let upload = c.upload(&texels)?;
        let condition = filled(c, &[0; 8])?;
        let views: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let rtv = views.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateRenderTargetView(
            &ms,
            Some(&D3D12_RENDER_TARGET_VIEW_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2DMS,
                ..Default::default()
            }),
            rtv,
        );
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.ClearRenderTargetView(rtv, &[0.345, 0.282, 0.219, 0.156], None);
        list.CopyTextureRegion(
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: transmute_copy(&copy),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            },
            0,
            0,
            0,
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: transmute_copy(&upload),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                            Width: SIZE,
                            Height: SIZE,
                            Depth: 1,
                            RowPitch: pitch,
                        },
                    },
                },
            },
            None,
        );
        c.run(&list)?;
        // A transition into the state the resource is in.
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.ResourceBarrier(&[transition(
            &copy,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )]);
        let mut out = format!(
            "same-state transition {:?}; ",
            list.Close().map_err(|e| e.code())
        );
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.ResourceBarrier(&[
            transition(
                &copy,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_RESOLVE_DEST,
            ),
            transition(
                &ms,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_RESOLVE_SOURCE,
            ),
        ]);
        c.run(&list)?;
        for (name, op) in [
            ("skipped", D3D12_PREDICATION_OP_EQUAL_ZERO),
            ("resolved", D3D12_PREDICATION_OP_NOT_EQUAL_ZERO),
        ] {
            let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            list.ResourceBarrier(&[transition(
                &condition,
                D3D12_RESOURCE_STATE_COMMON,
                D3D12_RESOURCE_STATE_PREDICATION,
            )]);
            list.SetPredication(&condition, 0, op);
            list.ResolveSubresource(&copy, 0, &ms, 0, DXGI_FORMAT_R8G8B8A8_UNORM);
            list.SetPredication(None, 0, D3D12_PREDICATION_OP_EQUAL_ZERO);
            list.ResourceBarrier(&[transition(
                &condition,
                D3D12_RESOURCE_STATE_PREDICATION,
                D3D12_RESOURCE_STATE_COMMON,
            )]);
            c.run(&list)?;
            let pixels = c.pixels(&copy, D3D12_RESOURCE_STATE_RESOLVE_DEST)?;
            write!(out, "{name} {}; ", colours(&pixels)).unwrap();
        }
        Ok(out)
    }
}
