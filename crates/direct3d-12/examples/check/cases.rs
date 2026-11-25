//! The pass bar's cases of G5, each a small Direct3D 12 program printing one line of results, run
//! with real Direct3D 12 and through ours and compared line by line: placed resources that alias
//! (buffers written by a copy, a storage view and the CPU; identical textures at one offset), copy
//! and compute queues waiting on each other and on a late CPU signal, binary occlusion resolved,
//! read back and used by `SetPredication`, `ExecuteIndirect`, pipeline and stream output
//! statistics, geometry shaders, layers a geometry shader names, timestamps, views a vertex
//! shader reads in the program, a depth bias on lines and points, debugging annotations, the video
//! formats stored as ordinary textures, and
//! alpha-antialiased lines (whose picture is compared within
//! [`PICTURES`]' tolerance, as their filter is the implementation's own).

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
    include_bytes!("../../../graphics-test/shaders/dxil/triangle.vs.dxil");
const PS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxil/triangle.ps.dxil");
/// Shades each corner by `SV_VertexID` and moves by a per-instance offset.
const DRAWS_VS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxil/draws.vs.dxil");
const DRAWS_PS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxil/draws.ps.dxil");
/// Reads items of `float2 a; uint b` at t0 and writes changed ones at u0.
const CS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxbc/compute.cs_structured.dxbc");
/// The shaders crate's geometry test shaders (`tests/own/geometry.hlsl`, shader model 5.0).
macro_rules! own {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../shaders/tests/own/",
            $name,
            ".dxbc"
        ))
    };
}
/// A full-screen triangle, and a sampling through `s0` of `t0` at a uv running past 0 and 1
/// with a growing scale, in the cell `b0` names (the test rig's `samplers.hlsl`).
const FULL_VS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxbc/common.vs_full.dxbc");
const SAMPLE_PS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxbc/samplers.ps_sample.dxbc");
const GEOMETRY_VS: &[u8] = own!("vs_geometry");
const GEOMETRY_PS: &[u8] = own!("ps_geometry");
const PAGE: u64 = D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as u64;
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
    "vertex-past-end",
    "vertex-pulled",
    "restart",
    "render-pass",
    "ms-array",
    "custom-heap",
    "geometry",
    "timestamps",
    "filter-depth",
    "vertex-views",
    "no-scissors",
    "early-per-primitive",
    "layers",
    "line-bias",
    "markers",
    "undefined-topology",
    "video-formats",
    "texture-kinds",
    ANTIALIASED_LINES,
];

/// The antialiased lines case's name.
const ANTIALIASED_LINES: &str = "antialiased-lines";

/// The cases writing a picture (`<name>.rgba` in the working directory), each compared with
/// real Direct3D 12's within its tolerance: Direct3D leaves the antialiased line's filter to the
/// implementation (the RTX 3080's and WARP's differ by up to 128 of 255), so a quarter of the
/// range.
pub const PICTURES: &[(&str, u8)] = &[(ANTIALIASED_LINES, u8::MAX / 4)];

/// The extension of a case's picture file.
pub const PICTURE_EXTENSION: &str = "rgba";

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
            "vertex-past-end" => vertex_past_end(&c),
            "vertex-pulled" => vertex_pulled(&c),
            "restart" => restart(&c),
            "render-pass" => render_pass(&c),
            "ms-array" => ms_array(&c),
            "custom-heap" => custom_heap(&c),
            "geometry" => geometry(&c),
            "timestamps" => timestamps(&c),
            "filter-depth" => filter_depth(&c),
            "vertex-views" => vertex_views(&c),
            "no-scissors" => no_scissors(&c),
            "early-per-primitive" => early_per_primitive(&c),
            "layers" => layers(&c),
            "line-bias" => line_bias(&c),
            "markers" => markers(&c),
            "undefined-topology" => undefined_topology(&c),
            "video-formats" => video_formats(&c),
            "texture-kinds" => texture_kinds(&c),
            ANTIALIASED_LINES => antialiased_lines(&c),
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
        unsafe { self.subresource_pixels(texture, state, 0) }
    }

    /// The same, of subresource `subresource`.
    unsafe fn subresource_pixels(
        &self,
        texture: &ID3D12Resource,
        state: D3D12_RESOURCE_STATES,
        subresource: u32,
    ) -> Result<Vec<u8>> {
        unsafe {
            let pitch = D3D12_TEXTURE_DATA_PITCH_ALIGNMENT;
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
                        SubresourceIndex: subresource,
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
            self.pso(draws, &layout[..if draws { 3 } else { 2 }])
        }
    }

    /// The triangle program, or with `draws` the draws program, reading the input `layout`.
    unsafe fn pso(
        &self,
        draws: bool,
        layout: &[D3D12_INPUT_ELEMENT_DESC],
    ) -> Result<(ID3D12RootSignature, ID3D12PipelineState)> {
        unsafe { self.pso_samples(draws, layout, 1) }
    }

    /// The same, drawing into targets of `samples` samples.
    unsafe fn pso_samples(
        &self,
        draws: bool,
        layout: &[D3D12_INPUT_ELEMENT_DESC],
        samples: u32,
    ) -> Result<(ID3D12RootSignature, ID3D12PipelineState)> {
        unsafe {
            self.pso_full(
                draws,
                layout,
                samples,
                D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
            )
        }
    }

    /// The triangle program with the strip cut value `cut`.
    unsafe fn pso_cut(
        &self,
        layout: &[D3D12_INPUT_ELEMENT_DESC],
        cut: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE,
    ) -> Result<(ID3D12RootSignature, ID3D12PipelineState)> {
        unsafe { self.pso_full(false, layout, 1, cut) }
    }

    unsafe fn pso_full(
        &self,
        draws: bool,
        layout: &[D3D12_INPUT_ELEMENT_DESC],
        samples: u32,
        cut: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE,
    ) -> Result<(ID3D12RootSignature, ID3D12PipelineState)> {
        unsafe {
            let root = self.root(
                &[],
                D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            )?;
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
                    NumElements: layout.len() as u32,
                },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                IBStripCutValue: cut,
                NumRenderTargets: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: samples,
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

/// Direct draws reading past the end of a vertex buffer view, which reads zeros: a strip whose
/// last vertex lies past it, and instances past a per-instance view's end.
unsafe fn vertex_past_end(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let (instanced_root, instanced) = c.triangle_pso(true)?;
        let vertices = c.upload(&triangles())?;
        let offsets = c.upload(&[0.25f32, 0.25, 0.5, 0.5].map(f32::to_le_bytes).concat())?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        let view = |size| D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: vertices.GetGPUVirtualAddress(),
            SizeInBytes: size,
            StrideInBytes: 20,
        };
        // Four vertices of five: the strip's third triangle ends at a zero vertex.
        list.IASetVertexBuffers(0, Some(&[view(80)]));
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        let half = SIZE / 2;
        viewport(&list, 0, 0, half);
        list.DrawInstanced(5, 1, 0, 0);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        // Two instances of one offset each, of which the view holds one.
        list.SetPipelineState(&instanced);
        list.SetGraphicsRootSignature(&instanced_root);
        list.IASetVertexBuffers(
            0,
            Some(&[
                view(240),
                D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: offsets.GetGPUVirtualAddress(),
                    SizeInBytes: 8,
                    StrideInBytes: 8,
                },
            ]),
        );
        viewport(&list, half, half, half);
        list.DrawInstanced(3, 2, 3, 0);
        c.run(&list)?;
        Ok(colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ))
    }
}

/// A list that sets every state but scissors and draws the triangles, after another list that
/// set them (Direct3D 12 leaves the scissors undefined; this compares what the GPU does).
unsafe fn no_scissors(c: &Ctx) -> Result<String> {
    unsafe {
        let (root, pso) = c.triangle_pso(false)?;
        let vertices = c.upload(&triangles())?;
        let (_a, first) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, rtv) = c.target(&first)?;
        c.run(&first)?;
        let (_b, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        list.DrawInstanced(12, 1, 0, 0);
        c.run(&list)?;
        Ok(colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ))
    }
}

/// One input element: semantic, format, slot, offset, and per-instance step rate (None: per
/// vertex).
fn element(
    semantic: windows::core::PCSTR,
    format: DXGI_FORMAT,
    slot: u32,
    offset: u32,
    rate: Option<u32>,
) -> D3D12_INPUT_ELEMENT_DESC {
    D3D12_INPUT_ELEMENT_DESC {
        SemanticName: semantic,
        Format: format,
        InputSlot: slot,
        AlignedByteOffset: offset,
        InputSlotClass: match rate {
            Some(_) => D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
            None => D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        },
        InstanceDataStepRate: rate.unwrap_or(0),
        ..Default::default()
    }
}

/// Vertex streams WebGPU cannot fetch as laid out: a stride of 6 with an element at offset 2,
/// per-instance offsets stepping every 2 instances from instance 1, a per-instance stream of
/// stride 1, and a stride of 2 in an `ExecuteIndirect` draw of a program reading
/// `SV_VertexID` from start vertex 1 and start instance 1.
unsafe fn vertex_pulled(c: &Ctx) -> Result<String> {
    unsafe {
        let vb = |b: &ID3D12Resource, size, stride| D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: b.GetGPUVirtualAddress(),
            SizeInBytes: size,
            StrideInBytes: stride,
        };
        let layouts = [
            (
                false,
                vec![
                    element(s!("POSITION"), DXGI_FORMAT_R8G8_SNORM, 0, 0, None),
                    element(s!("COLOR"), DXGI_FORMAT_R8G8B8A8_UNORM, 0, 2, None),
                ],
            ),
            (
                true,
                vec![
                    element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0, 0, None),
                    element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 0, 8, None),
                    element(s!("OFFSET"), DXGI_FORMAT_R32G32_FLOAT, 1, 0, Some(2)),
                ],
            ),
            (
                true,
                vec![
                    element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0, 0, None),
                    element(s!("COLOR"), DXGI_FORMAT_R8_UNORM, 1, 0, Some(1)),
                    element(s!("OFFSET"), DXGI_FORMAT_R32G32_FLOAT, 2, 0, Some(1)),
                ],
            ),
            (
                true,
                vec![
                    element(s!("POSITION"), DXGI_FORMAT_R8G8_SNORM, 0, 0, None),
                    element(s!("COLOR"), DXGI_FORMAT_R8G8B8A8_UNORM, 1, 0, Some(1)),
                    element(s!("OFFSET"), DXGI_FORMAT_R32G32_FLOAT, 2, 0, Some(1)),
                ],
            ),
            (
                false,
                vec![
                    element(s!("POSITION"), DXGI_FORMAT_R8G8_SNORM, 0, 0, None),
                    element(s!("COLOR"), DXGI_FORMAT_R8G8_UNORM, 0, 2, None),
                ],
            ),
        ];
        let mut line = String::new();
        let psos: Vec<_> = layouts
            .iter()
            .map(|(draws, layout)| match c.pso(*draws, layout) {
                Ok(p) => {
                    line.push_str("made ");
                    Some(p)
                }
                Err(e) => {
                    let _ = write!(line, "refused {:#x} ", e.code().0);
                    None
                }
            })
            .collect();
        // Three vertices of position and colour, six bytes each.
        let packed_vertices = c.upload(&[
            0x81, 0x81, 255, 0, 0, 255, //
            0, 0x7f, 0, 255, 0, 255, //
            0x7f, 0x81, 0, 0, 255, 255,
        ])?;
        let vertices = c.upload(&triangles())?;
        let offsets = c.upload(
            &[0.0f32, 0.0, 0.0, 0.0, 0.0, 0.4, -0.4, 0.0]
                .map(f32::to_le_bytes)
                .concat(),
        )?;
        let shades = c.upload(&[0x20, 0x40, 0x80, 0xff])?;
        let positions = c.upload(&[0, 0, 0x81, 0x81, 0, 0x7f, 0x7f, 0x81])?;
        let colors = c.upload(&[0, 0, 0, 0, 255, 128, 0, 255])?;
        let (signature, args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            16,
            &u32s(&[3, 1, 1, 1]),
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        let half = SIZE / 2;
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        let set = |i: usize| match &psos[i] {
            Some((root, pso)) => {
                list.SetPipelineState(pso);
                list.SetGraphicsRootSignature(root);
                true
            }
            None => false,
        };
        // Stride 6 with an element at offset 2: four bytes, then (if refused) two.
        for (i, x) in [(0, 0), (4, half / 2)] {
            if set(i) {
                list.IASetVertexBuffers(0, Some(&[vb(&packed_vertices, 18, 6)]));
                viewport(&list, x, 0, half / 2);
                list.DrawInstanced(3, 1, 0, 0);
            }
        }
        if set(1) {
            list.IASetVertexBuffers(0, Some(&[vb(&vertices, 240, 20), vb(&offsets, 32, 8)]));
            viewport(&list, half, half, half);
            list.DrawInstanced(3, 4, 3, 1);
        }
        if set(2) {
            list.IASetVertexBuffers(
                0,
                Some(&[
                    vb(&vertices, 240, 20),
                    vb(&shades, 4, 1),
                    vb(&offsets, 32, 8),
                ]),
            );
            viewport(&list, half, 0, half);
            list.DrawInstanced(3, 2, 6, 2);
        }
        if set(3) {
            list.IASetVertexBuffers(
                0,
                Some(&[vb(&positions, 8, 2), vb(&colors, 8, 4), vb(&offsets, 32, 8)]),
            );
            viewport(&list, 0, half, half);
            list.ExecuteIndirect(&signature, 1, &args, 0, None, 0);
        }
        c.run(&list)?;
        line.push_str(&colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ));
        Ok(line)
    }
}

/// Triangle strips of 16-bit and 32-bit indices holding 0xffff, drawn with each strip cut value:
/// directly, and the 16-bit ones also through `ExecuteIndirect`.
unsafe fn restart(c: &Ctx) -> Result<String> {
    unsafe {
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0, 0, None),
            element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 0, 8, None),
        ];
        // A quad's corners, with its last at element 0xffff; the rest zero.
        let mut bytes = vec![0u8; 0x10000 * 20];
        let corner = |x: f32, y: f32| [x, y, 1.0, 0.5, 0.0].map(f32::to_le_bytes).concat();
        for (i, (x, y)) in [
            (0, (-0.9, -0.9)),
            (1, (-0.9, 0.9)),
            (2, (0.9, -0.9)),
            (0xffff, (0.9, 0.9)),
        ] {
            bytes[i * 20..i * 20 + 20].copy_from_slice(&corner(x, y));
        }
        let vertices = c.upload(&bytes)?;
        let indices16 = c.upload(&[0u16, 1, 2, 0xffff, 1, 2].map(u16::to_le_bytes).concat())?;
        let indices32 = c.upload(&u32s(&[0, 1, 2, 0xffff, 1, 2]))?;
        let (signature, args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
            20,
            &u32s(&[4, 1, 0, 0, 0]),
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertices.GetGPUVirtualAddress(),
                SizeInBytes: bytes.len() as u32,
                StrideInBytes: 20,
            }]),
        );
        let quarter = SIZE / 4;
        let cuts = [
            D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
            D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFF,
            D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFFFFFF,
        ];
        let mut psos = Vec::new();
        for (row, cut) in cuts.into_iter().enumerate() {
            let (root, pso) = c.pso_cut(&layout, cut)?;
            list.SetPipelineState(&pso);
            list.SetGraphicsRootSignature(&root);
            for (column, (buffer, format)) in [
                (&indices16, DXGI_FORMAT_R16_UINT),
                (&indices32, DXGI_FORMAT_R32_UINT),
            ]
            .into_iter()
            .enumerate()
            {
                list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: buffer.GetGPUVirtualAddress(),
                    SizeInBytes: 24,
                    Format: format,
                }));
                viewport(
                    &list,
                    column as u32 * quarter,
                    row as u32 * quarter,
                    quarter,
                );
                list.DrawIndexedInstanced(6, 1, 0, 0, 0);
            }
            list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
                BufferLocation: indices16.GetGPUVirtualAddress(),
                SizeInBytes: 12,
                Format: DXGI_FORMAT_R16_UINT,
            }));
            viewport(&list, 2 * quarter, row as u32 * quarter, quarter);
            list.ExecuteIndirect(&signature, 1, &args, 0, None, 0);
            psos.push((root, pso));
        }
        c.run(&list)?;
        Ok(colours(
            &c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?,
        ))
    }
}

/// Render passes (`ID3D12GraphicsCommandList4`): one clearing the target and drawing, one keeping
/// it and drawing, and one clearing a 4-sample target, drawing and resolving it into a second
/// target at its end.
unsafe fn render_pass(c: &Ctx) -> Result<String> {
    unsafe {
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0, 0, None),
            element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 0, 8, None),
        ];
        let (root, pso) = c.pso(false, &layout)?;
        let (ms_root, ms_pso) = c.pso_samples(false, &layout, 4)?;
        let vertices = c.upload(&triangles())?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list4: ID3D12GraphicsCommandList4 = list.cast()?;
        let (target, _heap, rtv) = c.target(&list)?;
        let mut ms_desc = target_desc();
        ms_desc.SampleDesc.Count = 4;
        let ms = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &ms_desc,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        let resolved = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &target_desc(),
            D3D12_RESOURCE_STATE_RESOLVE_DEST,
        )?;
        let heap: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let ms_rtv = heap.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateRenderTargetView(&ms, None, ms_rtv);
        let clear = |color: [f32; 4]| D3D12_RENDER_PASS_BEGINNING_ACCESS {
            Type: D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR,
            Anonymous: D3D12_RENDER_PASS_BEGINNING_ACCESS_0 {
                Clear: D3D12_RENDER_PASS_BEGINNING_ACCESS_CLEAR_PARAMETERS {
                    ClearValue: D3D12_CLEAR_VALUE {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        Anonymous: D3D12_CLEAR_VALUE_0 { Color: color },
                    },
                },
            },
        };
        let keep = D3D12_RENDER_PASS_BEGINNING_ACCESS {
            Type: D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE,
            ..Default::default()
        };
        let kept = || D3D12_RENDER_PASS_ENDING_ACCESS {
            Type: D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE,
            ..Default::default()
        };
        let pass = |view, begin, end| D3D12_RENDER_PASS_RENDER_TARGET_DESC {
            cpuDescriptor: view,
            BeginningAccess: begin,
            EndingAccess: end,
        };
        let half = SIZE / 2;
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        list4.BeginRenderPass(
            Some(&[pass(rtv, clear([0.2, 0.4, 0.6, 1.0]), kept())]),
            None,
            D3D12_RENDER_PASS_FLAG_NONE,
        );
        viewport(&list, 0, 0, half);
        list.DrawInstanced(3, 1, 3, 0);
        list4.EndRenderPass();
        list4.BeginRenderPass(
            Some(&[pass(rtv, keep, kept())]),
            None,
            D3D12_RENDER_PASS_FLAG_NONE,
        );
        viewport(&list, half, half, half);
        list.DrawInstanced(3, 1, 6, 0);
        list4.EndRenderPass();
        let whole = D3D12_RENDER_PASS_ENDING_ACCESS_RESOLVE_SUBRESOURCE_PARAMETERS {
            SrcRect: RECT {
                left: 0,
                top: 0,
                right: SIZE as i32,
                bottom: SIZE as i32,
            },
            ..Default::default()
        };
        let resolve = D3D12_RENDER_PASS_ENDING_ACCESS {
            Type: D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_RESOLVE,
            Anonymous: D3D12_RENDER_PASS_ENDING_ACCESS_0 {
                Resolve: ManuallyDrop::new(D3D12_RENDER_PASS_ENDING_ACCESS_RESOLVE_PARAMETERS {
                    pSrcResource: transmute_copy(&ms),
                    pDstResource: transmute_copy(&resolved),
                    SubresourceCount: 1,
                    pSubresourceParameters: &whole,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    ResolveMode: D3D12_RESOLVE_MODE_AVERAGE,
                    PreserveResolveSource: false.into(),
                }),
            },
        };
        list.SetPipelineState(&ms_pso);
        list.SetGraphicsRootSignature(&ms_root);
        list4.BeginRenderPass(
            Some(&[pass(ms_rtv, clear([0.0, 0.0, 0.0, 1.0]), resolve)]),
            None,
            D3D12_RENDER_PASS_FLAG_NONE,
        );
        viewport(&list, 0, 0, SIZE);
        list.DrawInstanced(3, 1, 0, 0);
        list4.EndRenderPass();
        c.run(&list)?;
        Ok(format!(
            "{} | {}",
            colours(&c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?),
            colours(&c.pixels(&resolved, D3D12_RESOURCE_STATE_RESOLVE_DEST)?)
        ))
    }
}

/// A multisampled array of three layers: layer 0 cleared by a view of it alone, layers 1 and 2
/// by one view of every layer from 1, a triangle drawn into layer 2, the array copied whole into
/// another and each layer of the copy resolved.
unsafe fn ms_array(c: &Ctx) -> Result<String> {
    unsafe {
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32_FLOAT, 0, 0, None),
            element(s!("COLOR"), DXGI_FORMAT_R32G32B32_FLOAT, 0, 8, None),
        ];
        let (root, pso) = c.pso_samples(false, &layout, 4)?;
        let vertices = c.upload(&triangles())?;
        let mut desc = target_desc();
        desc.SampleDesc.Count = 4;
        desc.DepthOrArraySize = 3;
        let ms = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &desc,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        let copy = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let resolved = (0..3)
            .map(|_| {
                c.committed(
                    D3D12_HEAP_TYPE_DEFAULT,
                    &target_desc(),
                    D3D12_RESOURCE_STATE_RESOLVE_DEST,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let heap: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 3,
                ..Default::default()
            })?;
        let step = c
            .device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
            as usize;
        let start = heap.GetCPUDescriptorHandleForHeapStart();
        let rtv = |i: usize, first: u32, size: u32| {
            let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: start.ptr + i * step,
            };
            c.device.CreateRenderTargetView(
                &ms,
                Some(&D3D12_RENDER_TARGET_VIEW_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2DMSARRAY,
                    Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 {
                        Texture2DMSArray: D3D12_TEX2DMS_ARRAY_RTV {
                            FirstArraySlice: first,
                            ArraySize: size,
                        },
                    },
                }),
                handle,
            );
            handle
        };
        let (first, rest, last) = (rtv(0, 0, 1), rtv(1, 1, u32::MAX), rtv(2, 2, 1));
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.ClearRenderTargetView(rest, &[0.2, 0.4, 0.6, 1.0], None);
        list.ClearRenderTargetView(first, &[0.8, 0.1, 0.3, 1.0], None);
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        vertex_buffer(&list, &vertices);
        list.OMSetRenderTargets(1, Some(&last), false, None);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        viewport(&list, 0, 0, SIZE);
        list.DrawInstanced(3, 1, 0, 0);
        list.ResourceBarrier(&[transition(
            &ms,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        list.CopyResource(&copy, &ms);
        list.ResourceBarrier(&[transition(
            &copy,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_RESOLVE_SOURCE,
        )]);
        for (i, r) in resolved.iter().enumerate() {
            list.ResolveSubresource(r, 0, &copy, i as u32, DXGI_FORMAT_R8G8B8A8_UNORM);
        }
        c.run(&list)?;
        let mut out = Vec::new();
        for r in &resolved {
            out.push(colours(&c.pixels(r, D3D12_RESOURCE_STATE_RESOLVE_DEST)?));
        }
        Ok(out.join(" | "))
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
        let pitch = D3D12_TEXTURE_DATA_PITCH_ALIGNMENT;
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

/// Buffers in custom heaps the CPU sees (write-back, in system memory). A committed one: the CPU
/// writes it, copies read it and write part of it, the CPU reads it back, writes again, and a copy
/// executed right after one writing it reads it. Two placed ones overlapping in a custom heap: a
/// copy writes through one and the CPU reads through the other.
unsafe fn custom_heap(c: &Ctx) -> Result<String> {
    unsafe {
        let properties = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
            MemoryPoolPreference: D3D12_MEMORY_POOL_L0,
            ..Default::default()
        };
        let common = D3D12_RESOURCE_STATE_COMMON;
        let none = D3D12_RESOURCE_FLAG_NONE;
        let mut shared = None;
        c.device.CreateCommittedResource(
            &properties,
            D3D12_HEAP_FLAG_NONE,
            &buffer_desc(256, none),
            common,
            None,
            &mut shared,
        )?;
        let shared: ID3D12Resource = shared.unwrap();
        let other = c.committed(D3D12_HEAP_TYPE_DEFAULT, &buffer_desc(256, none), common)?;
        let pattern: Vec<u8> = (0..64u32).flat_map(|i| (i * 5 + 1).to_le_bytes()).collect();
        write(&shared, 0, &pattern)?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.CopyBufferRegion(&other, 0, &shared, 0, 64);
        list.ResourceBarrier(&[
            transition(
                &other,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
            transition(
                &shared,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_COPY_DEST,
            ),
        ]);
        list.CopyBufferRegion(&shared, 128, &other, 0, 32);
        c.run(&list)?;
        let mut out = String::new();
        write!(out, "mapped {} | ", words(&mapped(&shared, 256)?)).unwrap();
        write(&shared, 0, &[0xab; 16])?;
        let (_b, first) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        first.CopyBufferRegion(&shared, 200, &other, 0, 8);
        first.Close()?;
        let (_c, second) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        second.CopyBufferRegion(&other, 0, &shared, 0, 256);
        second.Close()?;
        c.queue.ExecuteCommandLists(&[Some(first.cast()?)]);
        c.queue.ExecuteCommandLists(&[Some(second.cast()?)]);
        c.finish()?;
        write!(out, "copied {} | ", words(&c.read(&other, 0, 256)?)).unwrap();
        write!(out, "mapped {} | ", words(&mapped(&shared, 256)?)).unwrap();
        let mut heap = None;
        c.device.CreateHeap(
            &D3D12_HEAP_DESC {
                SizeInBytes: 2 * PAGE,
                Properties: properties,
                Alignment: 0,
                Flags: D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS,
            },
            &mut heap,
        )?;
        let heap: ID3D12Heap = heap.unwrap();
        let x = c.placed(&heap, 0, &buffer_desc(2 * PAGE, none), common)?;
        let y = c.placed(&heap, PAGE, &buffer_desc(PAGE, none), common)?;
        let source = c.upload(&pattern)?;
        let (_d, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.CopyBufferRegion(&y, 64, &source, 0, 256);
        c.run(&list)?;
        let mut p = std::ptr::null_mut();
        x.Map(0, None, Some(&mut p))?;
        let through = std::slice::from_raw_parts(p.cast::<u8>().add(PAGE as usize), 512).to_vec();
        x.Unmap(0, None);
        write!(out, "overlapping {}", words(&through)).unwrap();
        Ok(out)
    }
}

/// Geometry shaders: points grown into quads (a root constant buffer seen by the geometry stage
/// alone, two instances from instance 1), a triangle strip cut into none, one or two triangles,
/// three geometry instances of indexed triangles with a base vertex, triangles with adjacency, and
/// points made into a varying number of points.
unsafe fn geometry(c: &Ctx) -> Result<String> {
    unsafe {
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, None),
            element(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, None),
        ];
        let root = c.root(
            &[D3D12_ROOT_PARAMETER {
                ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    Descriptor: D3D12_ROOT_DESCRIPTOR {
                        ShaderRegister: 0,
                        RegisterSpace: 0,
                    },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_GEOMETRY,
            }],
            D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        )?;
        let pso = |gs: &[u8], kind| -> Result<ID3D12PipelineState> {
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
                pShaderBytecode: b.as_ptr().cast(),
                BytecodeLength: b.len(),
            };
            let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: transmute_copy(&root),
                VS: code(GEOMETRY_VS),
                GS: code(gs),
                PS: code(GEOMETRY_PS),
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
                PrimitiveTopologyType: kind,
                NumRenderTargets: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            };
            desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
            c.device.CreateGraphicsPipelineState(&desc)
        };
        let point = D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT;
        let triangle = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
        let psos = [
            pso(own!("gs_quads"), point)?,
            pso(own!("gs_cut"), triangle)?,
            pso(own!("gs_instances"), triangle)?,
            pso(own!("gs_adjacent"), triangle)?,
            pso(own!("gs_points"), point)?,
        ];
        // Twelve vertices around a circle, red and green 0, 0.2 or 1 and blue 0 or 1, so that no
        // colour the pixel shader writes lies near a rounding boundary.
        let shades = [0.0f32, 0.2, 1.0];
        let vertices: Vec<u8> = (0..12)
            .flat_map(|i| {
                let turn = i as f32 * std::f32::consts::TAU / 12.0;
                let r = if i % 2 == 0 { 0.7 } else { 0.4 };
                [
                    r * turn.cos(),
                    r * turn.sin(),
                    0.5,
                    shades[i % 3],
                    shades[(i / 3) % 3],
                    (i % 2) as f32,
                    1.0,
                ]
            })
            .flat_map(f32::to_le_bytes)
            .collect();
        let vertices = c.upload(&vertices)?;
        let indices16 = c.upload(&[0u16, 2, 4, 5, 7, 9].map(u16::to_le_bytes).concat())?;
        let mut constants = [0.08f32, 0.08, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
            .map(f32::to_le_bytes)
            .concat();
        constants.resize(256, 0);
        let constants = c.upload(&constants)?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetGraphicsRootSignature(&root);
        list.SetGraphicsRootConstantBufferView(0, constants.GetGPUVirtualAddress());
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertices.GetGPUVirtualAddress(),
                SizeInBytes: 12 * 28,
                StrideInBytes: 28,
            }]),
        );
        list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: indices16.GetGPUVirtualAddress(),
            SizeInBytes: 12,
            Format: DXGI_FORMAT_R16_UINT,
        }));
        let third = SIZE / 3;
        let at = |i: u32| viewport(&list, (i % 3) * third, (i / 3) * third, third);
        list.SetPipelineState(&psos[0]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
        at(0);
        list.DrawInstanced(4, 2, 1, 1);
        list.SetPipelineState(&psos[1]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        at(1);
        list.DrawInstanced(7, 1, 2, 0);
        list.SetPipelineState(&psos[2]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        at(2);
        list.DrawIndexedInstanced(6, 1, 0, 1, 0);
        list.SetPipelineState(&psos[3]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ);
        at(3);
        list.DrawInstanced(12, 1, 0, 0);
        list.SetPipelineState(&psos[4]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
        at(4);
        list.DrawInstanced(12, 1, 0, 0);
        c.run(&list)?;
        // Each pixel in place, then how many of each colour.
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        Ok(format!("{} {}", words(&pixels), colours(&pixels)))
    }
}

/// The layers `gs_layers` names, one per instance; the target has half as many.
const NAMED_LAYERS: u32 = 4;
/// Viewport 1 of the layers case (left, top, width, height), inside the target.
const INNER_VIEWPORT: [f32; 4] = [8.0, 4.0, 40.0, 48.0];

/// A geometry shader naming one of four layers per instance, through viewport 0 or 1, drawing
/// into a target of two layers: primitives naming a layer past the target's are drawn into its
/// first layer, and the pixel shader reads the layer each names.
unsafe fn layers(c: &Ctx) -> Result<String> {
    unsafe {
        let root = c.root(&[], D3D12_ROOT_SIGNATURE_FLAG_NONE)?;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: code(own!("vs_fullscreen")),
            GS: code(own!("gs_layers")),
            PS: code(own!("ps_layer")),
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
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
        let pso: ID3D12PipelineState = c.device.CreateGraphicsPipelineState(&desc)?;
        let layers = NAMED_LAYERS / 2;
        let target = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                DepthOrArraySize: layers as u16,
                ..target_desc()
            },
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        let heap: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let rtv = heap.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateRenderTargetView(&target, None, rtv);
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.SetGraphicsRootSignature(&root);
        list.SetPipelineState(&pso);
        let size = SIZE as f32;
        list.RSSetViewports(&[
            D3D12_VIEWPORT {
                Width: size,
                Height: size,
                MaxDepth: 1.0,
                ..Default::default()
            },
            D3D12_VIEWPORT {
                TopLeftX: INNER_VIEWPORT[0],
                TopLeftY: INNER_VIEWPORT[1],
                Width: INNER_VIEWPORT[2],
                Height: INNER_VIEWPORT[3],
                MaxDepth: 1.0,
                ..Default::default()
            },
        ]);
        let all = RECT {
            left: 0,
            top: 0,
            right: SIZE as i32,
            bottom: SIZE as i32,
        };
        list.RSSetScissorRects(&[all, all]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.DrawInstanced(3, 1, 0, 0);
        c.run(&list)?;
        let mut out = String::new();
        for layer in 0..layers {
            let pixels =
                c.subresource_pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET, layer)?;
            write!(
                out,
                "layer {layer} {} {} | ",
                words(&pixels),
                colours(&pixels)
            )
            .unwrap();
        }
        Ok(out)
    }
}

/// The vertices of the antialiased lines case: position (x, y, z) and colour (r, g, b, a).
const LINE_VERTEX_FLOATS: usize = 7;

/// Alpha-antialiased lines (`AntialiasedLineEnable`, multisampling off) blended over the target:
/// a line list at many slopes and lengths, ends on and between pixel centres, then a line strip,
/// with back faces culled (lines are never culled). Writes the picture for [`PICTURES`].
unsafe fn antialiased_lines(c: &Ctx) -> Result<String> {
    unsafe {
        let layout = [
            element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, None),
            element(
                s!("COLOR"),
                DXGI_FORMAT_R32G32B32A32_FLOAT,
                0,
                D3D12_APPEND_ALIGNED_ELEMENT,
                None,
            ),
        ];
        let root = c.root(
            &[],
            D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        )?;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D12_BLEND_SRC_ALPHA,
            DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
            ..Default::default()
        };
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: code(own!("vs_line")),
            PS: code(own!("ps_line")),
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_BACK,
                DepthClipEnable: true.into(),
                AntialiasedLineEnable: true.into(),
                ..Default::default()
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: layout.as_ptr(),
                NumElements: layout.len() as u32,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        let pso: ID3D12PipelineState = c.device.CreateGraphicsPipelineState(&desc)?;
        // Lines from the centre out to a ring, every other end on a pixel centre, in colours of
        // 0, 0.5 and 1 per channel.
        let size = SIZE as f32;
        let pixel = 2.0 / size;
        let spokes = 12;
        let mut vertices = Vec::new();
        for i in 0..spokes {
            let turn = i as f32 * std::f32::consts::TAU / spokes as f32;
            let reach = if i % 2 == 0 { 0.8 } else { 0.55 };
            let half = if i % 2 == 0 { pixel / 2.0 } else { 0.0 };
            let colour = [(i % 3) as f32 / 2.0, ((i / 3) % 3) as f32 / 2.0, 1.0, 0.9];
            for (x, y) in [
                (half, half),
                (reach * turn.cos() + half, reach * turn.sin() + half),
            ] {
                vertices.extend([x, y, 0.5]);
                vertices.extend(colour);
            }
        }
        let strip_from = vertices.len() / LINE_VERTEX_FLOATS;
        for (x, y) in [(-0.9, -0.9), (-0.3, -0.75), (0.2, -0.95), (0.9, -0.6)] {
            vertices.extend([x, y, 0.5, 1.0, 0.5, 0.0, 1.0]);
        }
        let count = vertices.len() / LINE_VERTEX_FLOATS;
        let stride = (LINE_VERTEX_FLOATS * size_of::<f32>()) as u32;
        let bytes: Vec<u8> = vertices.iter().flat_map(|v| v.to_le_bytes()).collect();
        let buffer = c.upload(&bytes)?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetGraphicsRootSignature(&root);
        list.SetPipelineState(&pso);
        list.IASetVertexBuffers(
            0,
            Some(&[D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: buffer.GetGPUVirtualAddress(),
                SizeInBytes: bytes.len() as u32,
                StrideInBytes: stride,
            }]),
        );
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
        list.DrawInstanced(strip_from as u32, 1, 0, 0);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINESTRIP);
        list.DrawInstanced((count - strip_from) as u32, 1, strip_from as u32, 0);
        c.run(&list)?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        let file = format!("{ANTIALIASED_LINES}.{PICTURE_EXTENSION}");
        std::fs::write(&file, &pixels).expect("writing the picture");
        // The picture is compared within a tolerance; the line says only that it was drawn.
        let lit = pixels.chunks(4).filter(|p| p[..3] != [0, 0, 0]).count() > 0;
        Ok(format!("drawn {lit}, picture {file}"))
    }
}

/// Timestamps around a dispatch on the direct queue and around a copy on a copy queue (a copy
/// queue timestamp heap), each queue's frequency, and two clock calibrations 20 ms apart taken
/// before and after the work: the timestamps lie between the calibrations' GPU values, and the
/// GPU and CPU clocks advance alike between them. Values differ between runs, so only these
/// relations are printed.
unsafe fn timestamps(c: &Ctx) -> Result<String> {
    unsafe {
        let copy = Ctx::make_queue(&c.device, D3D12_COMMAND_LIST_TYPE_COPY)?;
        let heap = |kind| -> Result<ID3D12QueryHeap> {
            let mut h = None;
            c.device.CreateQueryHeap(
                &D3D12_QUERY_HEAP_DESC {
                    Type: kind,
                    Count: 2,
                    NodeMask: 0,
                },
                &mut h,
            )?;
            Ok(h.unwrap())
        };
        let (direct_heap, copy_heap) = (
            heap(D3D12_QUERY_HEAP_TYPE_TIMESTAMP)?,
            heap(D3D12_QUERY_HEAP_TYPE_COPY_QUEUE_TIMESTAMP)?,
        );
        let readback = |size| {
            c.committed(
                D3D12_HEAP_TYPE_READBACK,
                &buffer_desc(size, D3D12_RESOURCE_FLAG_NONE),
                D3D12_RESOURCE_STATE_COPY_DEST,
            )
        };
        let (direct_out, copy_out) = (readback(16)?, readback(16)?);
        let frequency = c.queue.GetTimestampFrequency()?;
        let copy_frequency = copy.GetTimestampFrequency()?;
        let (mut gpu0, mut cpu0, mut gpu1, mut cpu1) = (0, 0, 0, 0);
        c.queue.GetClockCalibration(&mut gpu0, &mut cpu0)?;

        let source = c.upload(&vec![7; 4096])?;
        let target = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(4096, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        let (root, pso) = c.compute_pso()?;
        let (_a1, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.EndQuery(&direct_heap, D3D12_QUERY_TYPE_TIMESTAMP, 0);
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pso);
        list.SetComputeRootShaderResourceView(0, source.GetGPUVirtualAddress());
        list.SetComputeRootUnorderedAccessView(1, target.GetGPUVirtualAddress());
        list.Dispatch(1, 1, 1);
        list.EndQuery(&direct_heap, D3D12_QUERY_TYPE_TIMESTAMP, 1);
        list.ResolveQueryData(
            &direct_heap,
            D3D12_QUERY_TYPE_TIMESTAMP,
            0,
            2,
            &direct_out,
            0,
        );
        c.run(&list)?;
        // The same queries resolved into a default buffer, then copied to a readback one.
        let resolved = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(16, D3D12_RESOURCE_FLAG_NONE),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let through = readback(16)?;
        let (_a3, again) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        again.ResolveQueryData(&direct_heap, D3D12_QUERY_TYPE_TIMESTAMP, 0, 2, &resolved, 0);
        again.ResourceBarrier(&[transition(
            &resolved,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        again.CopyBufferRegion(&through, 0, &resolved, 0, 16);
        c.run(&again)?;

        let fence: ID3D12Fence = c.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let (_a2, copying) = c.list(D3D12_COMMAND_LIST_TYPE_COPY)?;
        copying.EndQuery(&copy_heap, D3D12_QUERY_TYPE_TIMESTAMP, 0);
        copying.CopyBufferRegion(&target, 0, &source, 0, 4096);
        copying.EndQuery(&copy_heap, D3D12_QUERY_TYPE_TIMESTAMP, 1);
        copying.ResolveQueryData(&copy_heap, D3D12_QUERY_TYPE_TIMESTAMP, 0, 2, &copy_out, 0);
        copying.Close()?;
        copy.ExecuteCommandLists(&[Some(copying.cast()?)]);
        copy.Signal(&fence, 1)?;
        fence.SetEventOnCompletion(1, c.event)?;
        WaitForSingleObject(c.event, INFINITE);

        std::thread::sleep(std::time::Duration::from_millis(20));
        c.queue.GetClockCalibration(&mut gpu1, &mut cpu1)?;
        let mut qpc = 0;
        windows::Win32::System::Performance::QueryPerformanceFrequency(&mut qpc)?;
        let read = |r: &ID3D12Resource| -> Result<[u64; 2]> {
            let b = mapped(r, 16)?;
            Ok([
                u64::from_le_bytes(b[..8].try_into().unwrap()),
                u64::from_le_bytes(b[8..].try_into().unwrap()),
            ])
        };
        let (d, k) = (read(&direct_out)?, read(&copy_out)?);
        let copied = read(&through)? == d;
        let gpu_seconds = (gpu1 - gpu0) as f64 / frequency as f64;
        let cpu_seconds = (cpu1 - cpu0) as f64 / qpc as f64;
        let ratio = gpu_seconds / cpu_seconds;
        Ok(format!(
            "frequency set {} copy frequency equal {} direct in order {} copy in order {} \
             within calibrations {} clocks agree {} copied from a default buffer {}",
            frequency > 0,
            copy_frequency == frequency,
            d[0] <= d[1],
            k[0] <= k[1],
            gpu0 <= d[0] && d[1] <= gpu1 && gpu0 <= k[0] && k[1] <= gpu1,
            (0.9..1.1).contains(&ratio),
            copied,
        ))
    }
}

/// A 4 by 4 depth texture of three depths (cleared by rectangles) sampled as `R32_FLOAT` through
/// a linear clamping static sampler over a 64 by 64 target, against bilinear filtering worked
/// out here: the largest difference in steps of the target's 8 bits, whether it is within 2,
/// and how many shades the picture has. Real Direct3D filters in its texture units, ours in the
/// program where the back end cannot filter depth.
unsafe fn filter_depth(c: &Ctx) -> Result<String> {
    unsafe {
        let range = D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 1,
            ..Default::default()
        };
        let params = [
            D3D12_ROOT_PARAMETER {
                ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    Descriptor: D3D12_ROOT_DESCRIPTOR::default(),
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
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
        let sampler = D3D12_STATIC_SAMPLER_DESC {
            Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            MaxLOD: f32::MAX,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
            ..Default::default()
        };
        let mut blob = None;
        D3D12SerializeRootSignature(
            &D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: params.len() as u32,
                pParameters: params.as_ptr(),
                NumStaticSamplers: 1,
                pStaticSamplers: &sampler,
                Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE,
            },
            D3D_ROOT_SIGNATURE_VERSION_1_0,
            &mut blob,
            None,
        )?;
        let blob = blob.unwrap();
        let root: ID3D12RootSignature = c.device.CreateRootSignature(
            0,
            std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize()),
        )?;
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: code(FULL_VS),
            PS: code(SAMPLE_PS),
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
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
        let pso: ID3D12PipelineState = c.device.CreateGraphicsPipelineState(&desc)?;

        let depth = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: 4,
                Height: 4,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R32_TYPELESS,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
        )?;
        let dsv_heap: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let dsv = dsv_heap.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateDepthStencilView(
            &depth,
            Some(&D3D12_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D32_FLOAT,
                ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            }),
            dsv,
        );
        let views: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 1,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            })?;
        c.device.CreateShaderResourceView(
            &depth,
            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R32_FLOAT,
                ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                    Texture2D: D3D12_TEX2D_SRV {
                        MipLevels: 1,
                        ..Default::default()
                    },
                },
            }),
            views.GetCPUDescriptorHandleForHeapStart(),
        );
        let mut cell = [0.0f32, 0.0, SIZE as f32, SIZE as f32]
            .map(f32::to_le_bytes)
            .concat();
        cell.resize(256, 0);
        let cell = c.upload(&cell)?;

        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        // Texels: the top row 1.0, below it the left half 0.75 and the right half 0.25.
        let rect = |l, t, r, b| RECT {
            left: l,
            top: t,
            right: r,
            bottom: b,
        };
        list.ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH, 0.25, 0, None);
        list.ClearDepthStencilView(
            dsv,
            D3D12_CLEAR_FLAG_DEPTH,
            0.75,
            0,
            Some(&[rect(0, 1, 2, 4)]),
        );
        list.ClearDepthStencilView(
            dsv,
            D3D12_CLEAR_FLAG_DEPTH,
            1.0,
            0,
            Some(&[rect(0, 0, 4, 1)]),
        );
        list.ResourceBarrier(&[transition(
            &depth,
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )]);
        let (target, _heap, _rtv) = c.target(&list)?;
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetGraphicsRootSignature(&root);
        list.SetGraphicsRootConstantBufferView(0, cell.GetGPUVirtualAddress());
        list.SetGraphicsRootDescriptorTable(1, views.GetGPUDescriptorHandleForHeapStart());
        list.SetPipelineState(&pso);
        list.DrawInstanced(3, 1, 0, 0);
        c.run(&list)?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;

        let texel = |x: i32, y: i32| -> f32 {
            let (x, y) = (x.clamp(0, 3), y.clamp(0, 3));
            match (x, y) {
                (_, 0) => 1.0,
                (0 | 1, _) => 0.75,
                _ => 0.25,
            }
        };
        let mut largest = 0;
        let mut shades = std::collections::BTreeSet::new();
        for y in 0..SIZE {
            for x in 0..SIZE {
                let t = [
                    ((x as f32 + 0.5) / SIZE as f32).clamp(0.0, 1.0),
                    ((y as f32 + 0.5) / SIZE as f32).clamp(0.0, 1.0),
                ];
                let uv = [
                    (t[0] - 0.33) * 3.0,
                    (t[1] - 0.33) * (3.0 + 9.0 * t[1] * t[1]),
                ];
                let (u, v) = (
                    uv[0].clamp(0.0, 1.0) * 4.0 - 0.5,
                    uv[1].clamp(0.0, 1.0) * 4.0 - 0.5,
                );
                let (x0, y0) = (u.floor(), v.floor());
                let (fx, fy) = (u - x0, v - y0);
                let (x0, y0) = (x0 as i32, y0 as i32);
                let value = (texel(x0, y0) * (1.0 - fx) + texel(x0 + 1, y0) * fx) * (1.0 - fy)
                    + (texel(x0, y0 + 1) * (1.0 - fx) + texel(x0 + 1, y0 + 1) * fx) * fy;
                let expected = (value * 255.0).round() as i32;
                let red = pixels[((y * SIZE + x) * 4) as usize];
                largest = largest.max((i32::from(red) - expected).abs());
                shades.insert(red);
            }
        }
        Ok(format!(
            "within 2 steps of bilinear filtering {} more than 16 shades {}",
            largest <= 2,
            shades.len() > 16
        ))
    }
}

/// A pixel shader forcing early depth tests counts its fragments, and those facing front, in a
/// root unordered access view, with no render target, while the draw writes depth: a triangle
/// list of the same triangle twice under `LESS` (the second fails against the first's depth),
/// then a strip of two triangles under `ALWAYS`, the second of the strip's other winding.
unsafe fn early_per_primitive(c: &Ctx) -> Result<String> {
    unsafe {
        const DEPTH_SIDE: u32 = 4;
        const DEPTH_CLEAR: f32 = 0.5;
        let vs = compile(
            "static const float2 corners[10] = { float2(-1, -1), float2(-1, 3), float2(3, -1),
                 float2(-1, -1), float2(-1, 3), float2(3, -1),
                 float2(-1, -1), float2(-1, 3), float2(3, -1), float2(-1, -1) };
             float4 main(uint id : SV_VertexID) : SV_Position
             { return float4(corners[id], 0.25, 1); }",
            s!("vs_5_0"),
        )?;
        let ps = compile(
            "RWByteAddressBuffer counts : register(u0);
             [earlydepthstencil]
             void main(float4 p : SV_Position, bool front : SV_IsFrontFace)
             {
                 uint was;
                 counts.InterlockedAdd(0, 1, was);
                 if (front) counts.InterlockedAdd(4, 1, was);
             }",
            s!("ps_5_0"),
        )?;
        let param = D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_UAV,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Descriptor: D3D12_ROOT_DESCRIPTOR::default(),
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        };
        let root = c.root(&[param], D3D12_ROOT_SIGNATURE_FLAG_NONE)?;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let pso = |func| {
            c.device.CreateGraphicsPipelineState::<ID3D12PipelineState>(
                &D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                    pRootSignature: transmute_copy(&root),
                    VS: code(&vs),
                    PS: code(&ps),
                    SampleMask: u32::MAX,
                    RasterizerState: D3D12_RASTERIZER_DESC {
                        FillMode: D3D12_FILL_MODE_SOLID,
                        CullMode: D3D12_CULL_MODE_NONE,
                        DepthClipEnable: true.into(),
                        ..Default::default()
                    },
                    DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                        DepthEnable: true.into(),
                        DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
                        DepthFunc: func,
                        ..Default::default()
                    },
                    DSVFormat: DXGI_FORMAT_D32_FLOAT,
                    PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    ..Default::default()
                },
            )
        };
        let less = pso(D3D12_COMPARISON_FUNC_LESS)?;
        let always = pso(D3D12_COMPARISON_FUNC_ALWAYS)?;
        let depth = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: u64::from(DEPTH_SIDE),
                Height: DEPTH_SIDE,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_D32_FLOAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
        )?;
        let dsv_heap: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let dsv = dsv_heap.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateDepthStencilView(&depth, None, dsv);
        // Two counters of two words each, one per draw.
        let counts = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(16, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        list.OMSetRenderTargets(0, None, false, Some(&dsv));
        list.SetGraphicsRootSignature(&root);
        list.RSSetViewports(&[D3D12_VIEWPORT {
            Width: DEPTH_SIDE as f32,
            Height: DEPTH_SIDE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]);
        list.RSSetScissorRects(&[RECT {
            right: DEPTH_SIDE as i32,
            bottom: DEPTH_SIDE as i32,
            ..Default::default()
        }]);
        let at = counts.GetGPUVirtualAddress();
        list.ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH, DEPTH_CLEAR, 0, None);
        list.SetPipelineState(&less);
        list.SetGraphicsRootUnorderedAccessView(0, at);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.DrawInstanced(6, 1, 0, 0);
        list.ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH, DEPTH_CLEAR, 0, None);
        list.SetPipelineState(&always);
        list.SetGraphicsRootUnorderedAccessView(0, at + 8);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        list.DrawInstanced(4, 1, 6, 0);
        c.run(&list)?;
        Ok(format!("counts {}", words(&c.read(&counts, 0, 16)?)))
    }
}

/// Lines (rows 4 and 14), three points each (rows 24 and 34) and triangles (rows 40 to 50 and 52
/// to 62) at depth 0.5, each drawn by a pipeline of its topology type, the first of each pair
/// unbiased and the second with a `DepthBias` of 100000 (0.006 at that depth), into D32 depth
/// cleared to 0.503 and tested `LESS_EQUAL`. Prints whether each band has lit pixels: a biased
/// primitive fails the test. (How
/// many a line lights is the rasteriser's: a listed loss.)
unsafe fn line_bias(c: &Ctx) -> Result<String> {
    unsafe {
        const BIAS: i32 = 100_000;
        // Above the primitives' depth by less than the bias moves them.
        const DEPTH: f32 = 0.503;
        const BANDS: [(usize, usize); 6] =
            [(0, 10), (10, 20), (20, 30), (30, 38), (38, 51), (51, 64)];
        let vs = compile(
            "static const float2 at[16] = {
                 float2(-1, 0.859375), float2(1, 0.859375),
                 float2(-1, 0.546875), float2(1, 0.546875),
                 float2(-0.484375, 0.234375), float2(0.015625, 0.234375), float2(0.515625, 0.234375),
                 float2(-0.484375, -0.078125), float2(0.015625, -0.078125), float2(0.515625, -0.078125),
                 float2(-1, -0.25), float2(1, -0.25), float2(0, -0.5625),
                 float2(-1, -0.625), float2(1, -0.625), float2(0, -0.9375) };
             cbuffer draw : register(b0) { uint first; };
             float4 main(uint id : SV_VertexID) : SV_Position
             { return float4(at[first + id], 0.5, 1); }",
            s!("vs_5_0"),
        )?;
        let ps = compile(
            "float4 main() : SV_Target { return float4(1, 1, 1, 1); }",
            s!("ps_5_0"),
        )?;
        // `SV_VertexID` leaves out the draw's first vertex: the draw's first vertex at b0.
        let param = D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                    Num32BitValues: 1,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX,
        };
        let root = c.root(&[param], D3D12_ROOT_SIGNATURE_FLAG_NONE)?;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let pso = |kind, bias| {
            let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: transmute_copy(&root),
                VS: code(&vs),
                PS: code(&ps),
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: D3D12_CULL_MODE_NONE,
                    DepthClipEnable: true.into(),
                    DepthBias: bias,
                    ..Default::default()
                },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
                    DepthFunc: D3D12_COMPARISON_FUNC_LESS_EQUAL,
                    ..Default::default()
                },
                DSVFormat: DXGI_FORMAT_D32_FLOAT,
                PrimitiveTopologyType: kind,
                NumRenderTargets: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            };
            desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
            desc.BlendState.RenderTarget[0].RenderTargetWriteMask =
                D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            c.device
                .CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc)
        };
        let mut psos = Vec::new();
        for kind in [
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        ] {
            for bias in [0, BIAS] {
                psos.push(pso(kind, bias)?);
            }
        }
        let depth = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Format: DXGI_FORMAT_D32_FLOAT,
                Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
                ..target_desc()
            },
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
        )?;
        let dsv_heap: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let dsv = dsv_heap.GetCPUDescriptorHandleForHeapStart();
        c.device.CreateDepthStencilView(&depth, None, dsv);
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, rtv) = c.target(&list)?;
        list.OMSetRenderTargets(1, Some(&rtv), false, Some(&dsv));
        list.RSSetScissorRects(&[RECT {
            right: SIZE as i32,
            bottom: SIZE as i32,
            ..Default::default()
        }]);
        list.ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH, DEPTH, 0, None);
        list.SetGraphicsRootSignature(&root);
        let draws = [
            (D3D_PRIMITIVE_TOPOLOGY_LINELIST, 2),
            (D3D_PRIMITIVE_TOPOLOGY_POINTLIST, 3),
            (D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, 3),
        ];
        let mut first = 0;
        for (i, pso) in psos.iter().enumerate() {
            let (topology, count) = draws[i / 2];
            list.SetPipelineState(pso);
            list.IASetPrimitiveTopology(topology);
            list.SetGraphicsRoot32BitConstant(0, first, 0);
            list.DrawInstanced(count, 1, 0, 0);
            first += count;
        }
        c.run(&list)?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        let row = SIZE as usize * 4;
        let lit =
            |(from, to): (usize, usize)| pixels[from * row..to * row].chunks(4).any(|p| p[0] != 0);
        let [l, lb, p, pb, t, tb] = BANDS.map(lit);
        Ok(format!(
            "line {l} biased {lb} points {p} biased {pb} triangle {t} biased {tb}"
        ))
    }
}

/// Debugging annotations on a command list and a queue (`BeginEvent`, `SetMarker`, `EndEvent`),
/// then the list's `Close` and a clear it records: prints the close's result and the cleared
/// colour.
unsafe fn markers(c: &Ctx) -> Result<String> {
    unsafe {
        let text = b"frame\0";
        let (data, size) = (Some(text.as_ptr().cast()), text.len() as u32);
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let (target, _heap, rtv) = c.target(&list)?;
        list.BeginEvent(0, data, size);
        list.SetMarker(0, data, size);
        list.ClearRenderTargetView(rtv, &[0.0, 1.0, 0.0, 1.0], None);
        list.EndEvent();
        c.queue.BeginEvent(0, data, size);
        c.queue.SetMarker(0, data, size);
        c.queue.EndEvent();
        let closed = list.Close().map_err(|e| e.code());
        c.queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        c.finish()?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        Ok(format!("close {closed:?} colour {}", colours(&pixels)))
    }
}

/// Graphics pipelines of primitive topology type `UNDEFINED`, with a pixel shader and a target
/// without and with a depth bias, without a pixel shader, and without one or a colour target but
/// with a depth target: prints each creation's result.
unsafe fn undefined_topology(c: &Ctx) -> Result<String> {
    unsafe {
        let vs = compile(
            "float4 main(uint id : SV_VertexID) : SV_Position { return float4(id, 0, 0.5, 1); }",
            s!("vs_5_0"),
        )?;
        let ps = compile(
            "float4 main() : SV_Target { return float4(1, 1, 1, 1); }",
            s!("ps_5_0"),
        )?;
        let root = c.root(&[], D3D12_ROOT_SIGNATURE_FLAG_NONE)?;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let mut out = Vec::new();
        for (bias, pixel, target) in [
            (0, true, true),
            (1, true, true),
            (0, false, true),
            (0, false, false),
        ] {
            let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: transmute_copy(&root),
                VS: code(&vs),
                PS: if pixel {
                    code(&ps)
                } else {
                    D3D12_SHADER_BYTECODE::default()
                },
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: D3D12_CULL_MODE_NONE,
                    DepthClipEnable: true.into(),
                    DepthBias: bias,
                    ..Default::default()
                },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_UNDEFINED,
                NumRenderTargets: u32::from(target),
                DSVFormat: if target {
                    DXGI_FORMAT_UNKNOWN
                } else {
                    DXGI_FORMAT_D32_FLOAT
                },
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            };
            if target {
                desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
            }
            desc.BlendState.RenderTarget[0].RenderTargetWriteMask =
                D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let made = c
                .device
                .CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc)
                .map(|_| ())
                .map_err(|e| e.code());
            out.push(format!(
                "bias {bias} pixel {pixel} target {target} {made:?}"
            ));
        }
        Ok(out.join(" "))
    }
}

/// `source` compiled by the system's HLSL compiler for `target` (such as `vs_5_0`).
unsafe fn compile(source: &str, target: windows::core::PCSTR) -> Result<Vec<u8>> {
    unsafe {
        use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
        let mut code = None;
        let mut errors = None;
        let made = D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            None,
            None,
            None::<&ID3DInclude>,
            s!("main"),
            target,
            0,
            0,
            &mut code,
            Some(&mut errors),
        );
        if let Some(e) = &errors {
            let text =
                std::slice::from_raw_parts(e.GetBufferPointer().cast::<u8>(), e.GetBufferSize());
            eprintln!("{}", String::from_utf8_lossy(text));
        }
        made?;
        let code = code.unwrap();
        Ok(
            std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize())
                .to_vec(),
        )
    }
}

/// Points whose colours a vertex shader reads: typed buffers of `R32G32B32A32_FLOAT` and
/// `R8G8B8A8_UNORM`, a texture through a view with a component mapping, and a texture through a
/// sampler with a level-of-detail bias; drawn directly (rows 0 to 3), and the textures alone by
/// `ExecuteIndirect` of two instances (rows 4 to 11), both from a first vertex `SV_VertexID`
/// leaves out. Prints the colours of 4 by 13 pixels.
unsafe fn vertex_views(c: &Ctx) -> Result<String> {
    const VERTEX: &str = r"
        Buffer<float4> wide : register(t0);
        Buffer<float4> packed : register(t1);
        Texture2D<float4> mapped : register(t2);
        Texture2D<float4> plain : register(t3);
        SamplerState point_sampler : register(s0);
        SamplerState biased : register(s1);
        struct O { float4 pos : SV_Position; float4 color : COLOR; };
        O main(uint id : SV_VertexID, uint instance : SV_InstanceID)
        {
            O o;
            uint i = id % 4, row = id / 4 + instance * 4;
            o.pos = float4((i + 0.5) / 32.0 - 1.0, 1.0 - (row + 0.5) / 32.0, 0.0, 1.0);
            float2 uv = float2((i + 0.5) / 4.0, 0.5);
            switch (id / 4)
            {
                case 0: o.color = wide[i]; break;
                case 1: o.color = packed[i]; break;
                case 2: o.color = mapped.SampleLevel(point_sampler, uv, 0); break;
                default: o.color = plain.SampleLevel(biased, uv, 0); break;
            }
            return o;
        }";
    const PIXEL: &str = r"
        float4 main(float4 pos : SV_Position, float4 color : COLOR) : SV_Target
        {
            return color;
        }";
    unsafe {
        let vs = compile(VERTEX, s!("vs_5_0"))?;
        // Textures only, four rows lower: drawn by `ExecuteIndirect`.
        let textures = VERTEX
            .replace("wide[i]", "mapped.SampleLevel(point_sampler, uv, 1)")
            .replace("packed[i]", "plain.SampleLevel(point_sampler, uv, 1)")
            .replace("instance * 4", "instance * 4 + 4");
        let vs_textures = compile(&textures, s!("vs_5_0"))?;
        let ps = compile(PIXEL, s!("ps_5_0"))?;
        let range = D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 4,
            ..Default::default()
        };
        let params = [D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &range,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX,
        }];
        let sampler = |register, bias| D3D12_STATIC_SAMPLER_DESC {
            Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
            AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            MipLODBias: bias,
            MaxLOD: f32::MAX,
            ShaderRegister: register,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX,
            ..Default::default()
        };
        let samplers = [sampler(0, 0.0), sampler(1, 1.0)];
        let mut blob = None;
        D3D12SerializeRootSignature(
            &D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: params.len() as u32,
                pParameters: params.as_ptr(),
                NumStaticSamplers: samplers.len() as u32,
                pStaticSamplers: samplers.as_ptr(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE,
            },
            D3D_ROOT_SIGNATURE_VERSION_1_0,
            &mut blob,
            None,
        )?;
        let blob = blob.unwrap();
        let root: ID3D12RootSignature = c.device.CreateRootSignature(
            0,
            std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize()),
        )?;
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        let code = |b: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: b.as_ptr().cast(),
            BytecodeLength: b.len(),
        };
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: code(&vs),
            PS: code(&ps),
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        let pso: ID3D12PipelineState = c.device.CreateGraphicsPipelineState(&desc)?;
        desc.VS = code(&vs_textures);
        let pso_textures: ID3D12PipelineState = c.device.CreateGraphicsPipelineState(&desc)?;

        let floats: Vec<u8> = [
            [0.25f32, 0.5, 0.75, 1.0],
            [1.0, 0.0, 0.5, 0.25],
            [0.0, 1.0, 0.125, 0.5],
            [0.5, 0.25, 1.0, 0.0],
        ]
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
        let wide = c.upload(&floats)?;
        let packed = c.upload(&u32s(&[0x8040_20ff, 0x11223344, 0xff00ff00, 0x7f80_c0e0]))?;
        // Level 0: four texels; level 1: two, at the next 512-byte placement.
        let mut texels = u32s(&[0xff0000ff, 0xff00ff00, 0xffff0000, 0x80402010]);
        texels.resize(512, 0);
        texels.extend(u32s(&[0xff808080, 0xff123456]));
        let texels = c.upload(&texels)?;
        let texture = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: 4,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 2,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let views: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 4,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            })?;
        let step = c
            .device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
            as usize;
        let at = |i: usize| D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: views.GetCPUDescriptorHandleForHeapStart().ptr + i * step,
        };
        let buffer_view = |format| D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Buffer: D3D12_BUFFER_SRV {
                    NumElements: 4,
                    ..Default::default()
                },
            },
        };
        c.device.CreateShaderResourceView(
            &wide,
            Some(&buffer_view(DXGI_FORMAT_R32G32B32A32_FLOAT)),
            at(0),
        );
        c.device.CreateShaderResourceView(
            &packed,
            Some(&buffer_view(DXGI_FORMAT_R8G8B8A8_UNORM)),
            at(1),
        );
        // Blue, red, one, green: D3D12_ENCODE_SHADER_4_COMPONENT_MAPPING(2, 0, 5, 1).
        let mapping = 2 | (5 << 6) | (1 << 9) | (1 << 12);
        let texture_view = |mapping| D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: mapping,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV {
                    MipLevels: u32::MAX,
                    ..Default::default()
                },
            },
        };
        c.device
            .CreateShaderResourceView(&texture, Some(&texture_view(mapping)), at(2));
        c.device.CreateShaderResourceView(
            &texture,
            Some(&texture_view(D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING)),
            at(3),
        );
        let (signature, args) = arguments(
            c,
            D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            16,
            &u32s(&[16, 2, 5, 0]),
        )?;

        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        for level in 0..2u32 {
            list.CopyTextureRegion(
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: transmute_copy(&texture),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: level,
                    },
                },
                0,
                0,
                0,
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: transmute_copy(&texels),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                            Offset: u64::from(level) * 512,
                            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                                Width: 4 >> level,
                                Height: 1,
                                Depth: 1,
                                RowPitch: 256,
                            },
                        },
                    },
                },
                None,
            );
        }
        list.ResourceBarrier(&[
            transition(
                &texture,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            ),
            transition(
                &args,
                D3D12_RESOURCE_STATE_COMMON,
                D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT,
            ),
        ]);
        let (target, _heap, _rtv) = c.target(&list)?;
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetGraphicsRootSignature(&root);
        list.SetGraphicsRootDescriptorTable(0, views.GetGPUDescriptorHandleForHeapStart());
        list.SetPipelineState(&pso);
        list.DrawInstanced(16, 1, 7, 0);
        list.SetPipelineState(&pso_textures);
        list.ExecuteIndirect(&signature, 1, &args, 0, None, 0);
        c.run(&list)?;
        let pixels = c.pixels(&target, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
        let mut line = String::new();
        for row in 0..13usize {
            let at = row * SIZE as usize * 4;
            let colours: Vec<String> = pixels[at..at + 16]
                .chunks(4)
                .map(|p| format!("{:08x}", u32::from_le_bytes(p.try_into().unwrap())))
                .collect();
            write!(line, "row {row}: {} ", colours.join(" ")).unwrap();
        }
        Ok(line.trim_end().to_string())
    }
}

/// A copy location naming a placed footprint of buffer `r`.
unsafe fn footprint_location(
    r: &ID3D12Resource,
    layout: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        // SAFETY: the location borrows the resource for the call it is passed to.
        pResource: unsafe { transmute_copy(r) },
        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            PlacedFootprint: layout,
        },
    }
}

/// A copy location naming subresource `index` of texture `r`.
unsafe fn subresource_location(r: &ID3D12Resource, index: u32) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        // SAFETY: the location borrows the resource for the call it is passed to.
        pResource: unsafe { transmute_copy(r) },
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            SubresourceIndex: index,
        },
    }
}

/// A 64 by 32 texture of one level in `format` with `flags`.
fn video_desc(format: DXGI_FORMAT, flags: D3D12_RESOURCE_FLAGS) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Width: 64,
        Height: 32,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Flags: flags,
        ..Default::default()
    }
}

/// Where subresources lie in a buffer: each one's footprint, row count and bytes a row.
type Footprints = Vec<(D3D12_PLACED_SUBRESOURCE_FOOTPRINT, u32, u64)>;

/// The footprints of the first `count` subresources of `desc`, written into the line, and the
/// bytes they span.
unsafe fn video_footprints(
    c: &Ctx,
    desc: &D3D12_RESOURCE_DESC,
    count: usize,
    line: &mut String,
) -> (Footprints, u64) {
    let mut layouts = vec![D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default(); count];
    let (mut rows, mut sizes, mut total) = (vec![0u32; count], vec![0u64; count], 0u64);
    // SAFETY: arrays of `count` entries.
    unsafe {
        c.device.GetCopyableFootprints(
            desc,
            0,
            count as u32,
            0,
            Some(layouts.as_mut_ptr()),
            Some(rows.as_mut_ptr()),
            Some(sizes.as_mut_ptr()),
            Some(&mut total),
        )
    };
    write!(line, "total {total}").unwrap();
    for (i, l) in layouts.iter().enumerate() {
        let f = l.Footprint;
        write!(
            line,
            " [{} at {} {}x{} pitch {} rows {} size {}]",
            f.Format.0, l.Offset, f.Width, f.Height, f.RowPitch, rows[i], sizes[i]
        )
        .unwrap();
    }
    let placed = (0..count).map(|i| (layouts[i], rows[i], sizes[i]));
    (placed.collect(), total)
}

/// The video formats plan 4.12 stores as ordinary textures: what creating them answers, their
/// footprints, `NV12`'s two planes and `YUY2` written from a buffer, copied whole and by region,
/// cleared through target views of each `NV12` plane, read by a compute shader through views of
/// each plane (of two textures, as one is written) and of `YUY2` as `R8G8B8A8`, written through an
/// unordered access view of the chroma plane, and read back through their footprints.
unsafe fn video_formats(c: &Ctx) -> Result<String> {
    const SHADER: &str = "
        Texture2D<float> luma : register(t0);
        Texture2D<float2> chroma : register(t1);
        Texture2D<float4> pairs : register(t2);
        RWByteAddressBuffer result : register(u0);
        RWTexture2D<float2> chroma_out : register(u1);
        uint byte(float v) { return uint(v * 255.0 + 0.5); }
        [numthreads(1, 1, 1)]
        void main()
        {
            float2 c = chroma.Load(int3(9, 5, 0));
            float4 p = pairs.Load(int3(3, 2, 0));
            uint w, h, cw, ch, pw, ph;
            luma.GetDimensions(w, h);
            chroma.GetDimensions(cw, ch);
            pairs.GetDimensions(pw, ph);
            result.Store4(0, uint4(byte(luma.Load(int3(17, 9, 0))),
                byte(c.x) | byte(c.y) << 8,
                byte(p.x) | byte(p.y) << 8 | byte(p.z) << 16 | byte(p.w) << 24,
                w | h << 8 | cw << 16 | ch << 24));
            result.Store2(16, uint2(pw | ph << 8, 0));
            chroma_out[uint2(1, 1)] = float2(1.0, 0.25);
        }";
    /// The bytes the shader stores: six words.
    const RESULT_BYTES: u64 = 24;
    unsafe {
        let mut line = String::new();
        let nv12 = DXGI_FORMAT_NV12;
        let yuy2 = DXGI_FORMAT_YUY2;
        let none = D3D12_RESOURCE_FLAG_NONE;
        let shaped = |format, width, height, mips, layers| D3D12_RESOURCE_DESC {
            Width: width,
            Height: height,
            MipLevels: mips,
            DepthOrArraySize: layers,
            ..video_desc(format, none)
        };
        for (name, desc) in [
            ("nv12", video_desc(nv12, none)),
            ("nv12-levels", shaped(nv12, 64, 32, 2, 1)),
            ("nv12-odd-width", shaped(nv12, 63, 32, 1, 1)),
            ("nv12-odd-height", shaped(nv12, 64, 31, 1, 1)),
            ("nv12-array", shaped(nv12, 64, 32, 1, 2)),
            (
                "nv12-target",
                video_desc(nv12, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET),
            ),
            (
                "nv12-storage",
                video_desc(nv12, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            ),
            ("yuy2", video_desc(yuy2, none)),
            ("yuy2-odd-width", shaped(yuy2, 63, 32, 1, 1)),
            ("yuy2-odd-height", shaped(yuy2, 64, 31, 1, 1)),
            (
                "yuy2-storage",
                video_desc(yuy2, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            ),
            ("420-opaque", video_desc(DXGI_FORMAT_420_OPAQUE, none)),
        ] {
            let made = c
                .committed(D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_COMMON)
                .map_or_else(|e| format!("{:#x}", e.code().0), |_| "made".into());
            write!(line, "{name} {made}; ").unwrap();
        }
        let both =
            D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET | D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
        let planar = video_desc(nv12, both);
        write!(line, "nv12 footprints ").unwrap();
        let (layouts, total) = video_footprints(c, &planar, 2, &mut line);
        write!(line, "; yuy2 footprints ").unwrap();
        let pair_desc = video_desc(yuy2, none);
        let (pair_layouts, pair_total) = video_footprints(c, &pair_desc, 1, &mut line);
        write!(line, "; 420-opaque footprints ").unwrap();
        video_footprints(c, &video_desc(DXGI_FORMAT_420_OPAQUE, none), 1, &mut line);
        write!(line, "; ").unwrap();

        // Each plane's rows filled with a pattern of its own, then copied in by footprint.
        let pattern = |total: u64, seed: usize| -> Vec<u8> {
            (0..total as usize)
                .map(|i| (i * 7 + seed * 13 + i / 251) as u8)
                .collect()
        };
        let source = c.upload(&pattern(total, 1))?;
        let pair_source = c.upload(&pattern(pair_total, 2))?;
        let a = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &planar,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let b = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &video_desc(nv12, none),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let y = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &pair_desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let y2 = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &pair_desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        for (i, (l, ..)) in layouts.iter().enumerate() {
            list.CopyTextureRegion(
                &subresource_location(&a, i as u32),
                0,
                0,
                0,
                &footprint_location(&source, *l),
                None,
            );
        }
        list.CopyTextureRegion(
            &subresource_location(&y, 0),
            0,
            0,
            0,
            &footprint_location(&pair_source, pair_layouts[0].0),
            None,
        );
        list.ResourceBarrier(&[
            transition(
                &a,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
            transition(
                &y,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
        ]);
        list.CopyResource(&b, &a);
        list.CopyResource(&y2, &y);
        // Boxes of each plane, and of pixel pairs, at other places.
        let area = |left, top, right, bottom| D3D12_BOX {
            left,
            top,
            front: 0,
            right,
            bottom,
            back: 1,
        };
        list.CopyTextureRegion(
            &subresource_location(&b, 0),
            16,
            8,
            0,
            &subresource_location(&a, 0),
            Some(&area(0, 0, 8, 4)),
        );
        list.CopyTextureRegion(
            &subresource_location(&b, 1),
            8,
            4,
            0,
            &subresource_location(&a, 1),
            Some(&area(0, 0, 4, 2)),
        );
        list.CopyTextureRegion(
            &subresource_location(&y2, 0),
            20,
            4,
            0,
            &subresource_location(&y, 0),
            Some(&area(4, 0, 12, 2)),
        );
        list.ResourceBarrier(&[
            transition(
                &b,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
            transition(
                &y2,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
            transition(
                &a,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            ),
        ]);

        // Rectangles of each plane cleared through target views naming it.
        let targets: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 2,
                ..Default::default()
            })?;
        let rtv_size = c
            .device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
        let rtv = |i: u32| D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: targets.GetCPUDescriptorHandleForHeapStart().ptr + (i * rtv_size) as usize,
        };
        for (plane, format) in [(0, DXGI_FORMAT_R8_UNORM), (1, DXGI_FORMAT_R8G8_UNORM)] {
            c.device.CreateRenderTargetView(
                &a,
                Some(&D3D12_RENDER_TARGET_VIEW_DESC {
                    Format: format,
                    ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_RTV {
                            MipSlice: 0,
                            PlaneSlice: plane,
                        },
                    },
                }),
                rtv(plane),
            );
        }
        list.ClearRenderTargetView(
            rtv(0),
            &[0.5, 0.0, 0.0, 0.0],
            Some(&[RECT {
                left: 0,
                top: 0,
                right: 16,
                bottom: 8,
            }]),
        );
        list.ClearRenderTargetView(
            rtv(1),
            &[0.25, 0.75, 0.0, 0.0],
            Some(&[RECT {
                left: 0,
                top: 0,
                right: 8,
                bottom: 4,
            }]),
        );
        list.ResourceBarrier(&[
            transition(
                &a,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            ),
            transition(
                &y,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            ),
            transition(
                &b,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            ),
        ]);

        // A compute shader reading each plane and YUY2's pixel pairs, writing the chroma plane.
        let cs = compile(SHADER, s!("cs_5_0"))?;
        let ranges = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 3,
                ..Default::default()
            },
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
                NumDescriptors: 1,
                BaseShaderRegister: 1,
                OffsetInDescriptorsFromTableStart: 3,
                ..Default::default()
            },
        ];
        let root = c.root(
            &[
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: ranges.len() as u32,
                            pDescriptorRanges: ranges.as_ptr(),
                        },
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_UAV,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        Descriptor: D3D12_ROOT_DESCRIPTOR {
                            ShaderRegister: 0,
                            RegisterSpace: 0,
                        },
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
            ],
            D3D12_ROOT_SIGNATURE_FLAG_NONE,
        )?;
        let pso: ID3D12PipelineState =
            c.device
                .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                    pRootSignature: transmute_copy(&root),
                    CS: D3D12_SHADER_BYTECODE {
                        pShaderBytecode: cs.as_ptr().cast(),
                        BytecodeLength: cs.len(),
                    },
                    ..Default::default()
                })?;
        let views: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 4,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            })?;
        let view_size = c
            .device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
        let at = |i: u32| D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: views.GetCPUDescriptorHandleForHeapStart().ptr + (i * view_size) as usize,
        };
        for (i, (resource, format, plane)) in [
            (&a, DXGI_FORMAT_R8_UNORM, 0),
            (&b, DXGI_FORMAT_R8G8_UNORM, 1),
            (&y, DXGI_FORMAT_R8G8B8A8_UNORM, 0),
        ]
        .into_iter()
        .enumerate()
        {
            c.device.CreateShaderResourceView(
                resource,
                Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: format,
                    ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                    Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MipLevels: 1,
                            PlaneSlice: plane,
                            ..Default::default()
                        },
                    },
                }),
                at(i as u32),
            );
        }
        c.device.CreateUnorderedAccessView(
            &a,
            None,
            Some(&D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: DXGI_FORMAT_R8G8_UNORM,
                ViewDimension: D3D12_UAV_DIMENSION_TEXTURE2D,
                Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                    Texture2D: D3D12_TEX2D_UAV {
                        MipSlice: 0,
                        PlaneSlice: 1,
                    },
                },
            }),
            at(3),
        );
        let output = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(RESULT_BYTES, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )?;
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pso);
        list.SetComputeRootDescriptorTable(0, views.GetGPUDescriptorHandleForHeapStart());
        list.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress());
        list.Dispatch(1, 1, 1);
        list.ResourceBarrier(&[
            transition(
                &a,
                D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
            transition(
                &b,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            ),
        ]);

        // Every texture read back by its footprints.
        let readback = |total| {
            c.committed(
                D3D12_HEAP_TYPE_READBACK,
                &buffer_desc(total, D3D12_RESOURCE_FLAG_NONE),
                D3D12_RESOURCE_STATE_COPY_DEST,
            )
        };
        let (ra, rb, ry) = (readback(total)?, readback(total)?, readback(pair_total)?);
        for (r, t) in [(&ra, &a), (&rb, &b)] {
            for (i, (l, ..)) in layouts.iter().enumerate() {
                list.CopyTextureRegion(
                    &footprint_location(r, *l),
                    0,
                    0,
                    0,
                    &subresource_location(t, i as u32),
                    None,
                );
            }
        }
        list.CopyTextureRegion(
            &footprint_location(&ry, pair_layouts[0].0),
            0,
            0,
            0,
            &subresource_location(&y2, 0),
            None,
        );
        list.ResourceBarrier(&[transition(
            &output,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        c.run(&list)?;
        let results = c.read(&output, 0, RESULT_BYTES)?;
        write!(line, "shader read {}; ", words(&results)).unwrap();
        for (name, r, size, places) in [
            ("cleared and stored", &ra, total, &layouts),
            ("copied", &rb, total, &layouts),
            ("pairs copied", &ry, pair_total, &pair_layouts),
        ] {
            let bytes = mapped(r, size as usize)?;
            write!(line, "{name}:").unwrap();
            // Each plane's rows without the padding between them.
            for (i, (l, rows, row)) in places.iter().enumerate() {
                let texels: Vec<u8> = (0..*rows as usize)
                    .flat_map(|r| {
                        let at = l.Offset as usize + r * l.Footprint.RowPitch as usize;
                        bytes[at..at + *row as usize].to_vec()
                    })
                    .collect();
                write!(line, " plane {i} {}", words(&texels)).unwrap();
            }
            write!(line, "; ").unwrap();
        }
        Ok(line.trim_end().to_string())
    }
}

/// HLSL of the texture kinds case: textures loaded as another component kind than their views'
/// formats, the values stored to a raw buffer.
const TEXTURE_KINDS_CS: &str = "Texture2D<float4> unsigned_as_float : register(t0);
     Texture2D<uint4> float_as_unsigned : register(t1);
     Texture2D<uint4> signed_as_unsigned : register(t2);
     Texture2D<int4> unorm_as_signed : register(t3);
     RWByteAddressBuffer o : register(u0);
     [numthreads(1, 1, 1)] void main() {
         o.Store4(0, asuint(unsigned_as_float.Load(int3(0, 0, 0))));
         o.Store4(16, float_as_unsigned.Load(int3(0, 0, 0)));
         o.Store4(32, signed_as_unsigned.Load(int3(0, 0, 0)));
         o.Store4(48, asuint(unorm_as_signed.Load(int3(0, 0, 0)))); }";

/// One texel each of `R8G8B8A8_UINT`, `R32G32B32A32_FLOAT`, `R8G8B8A8_SINT` and
/// `R8G8B8A8_UNORM`, loaded by a compute shader declaring float, unsigned, unsigned and signed
/// textures: the words it stores (Direct3D gives the view's values, which the shader takes as the
/// bits of the type it declares).
unsafe fn texture_kinds(c: &Ctx) -> Result<String> {
    unsafe {
        let texels: [(DXGI_FORMAT, Vec<u8>); 4] = [
            (DXGI_FORMAT_R8G8B8A8_UINT, vec![1, 2, 3, 4]),
            (
                DXGI_FORMAT_R32G32B32A32_FLOAT,
                [1.0f32, -2.5, 0.375, f32::MAX]
                    .map(f32::to_le_bytes)
                    .concat(),
            ),
            (
                DXGI_FORMAT_R8G8B8A8_SINT,
                [-1i8, 2, -3, i8::MIN].map(|v| v as u8).to_vec(),
            ),
            (DXGI_FORMAT_R8G8B8A8_UNORM, vec![0, 64, 128, u8::MAX]),
        ];
        let range = D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: texels.len() as u32,
            ..Default::default()
        };
        let root = c.root(
            &[
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: 1,
                            pDescriptorRanges: &range,
                        },
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_UAV,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        Descriptor: D3D12_ROOT_DESCRIPTOR::default(),
                    },
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                },
            ],
            D3D12_ROOT_SIGNATURE_FLAG_NONE,
        )?;
        let cs = compile(TEXTURE_KINDS_CS, s!("cs_5_0"))?;
        let pso: ID3D12PipelineState =
            c.device
                .CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                    pRootSignature: transmute_copy(&root),
                    CS: D3D12_SHADER_BYTECODE {
                        pShaderBytecode: cs.as_ptr().cast(),
                        BytecodeLength: cs.len(),
                    },
                    ..Default::default()
                })?;
        let views: ID3D12DescriptorHeap =
            c.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: texels.len() as u32,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            })?;
        let step = c
            .device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
        let placement = D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as usize;
        let mut staged = vec![0u8; placement * texels.len()];
        for (i, (_, bytes)) in texels.iter().enumerate() {
            staged[i * placement..][..bytes.len()].copy_from_slice(bytes);
        }
        let staged = c.upload(&staged)?;
        let (_a, list) = c.list(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let mut textures = Vec::new();
        for (i, (format, _)) in texels.iter().enumerate() {
            let texture = c.committed(
                D3D12_HEAP_TYPE_DEFAULT,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: 1,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: *format,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COPY_DEST,
            )?;
            list.CopyTextureRegion(
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: transmute_copy(&texture),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: 0,
                    },
                },
                0,
                0,
                0,
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: transmute_copy(&staged),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                            Offset: (i * placement) as u64,
                            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                                Format: *format,
                                Width: 1,
                                Height: 1,
                                Depth: 1,
                                RowPitch: D3D12_TEXTURE_DATA_PITCH_ALIGNMENT,
                            },
                        },
                    },
                },
                None,
            );
            list.ResourceBarrier(&[transition(
                &texture,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            )]);
            c.device.CreateShaderResourceView(
                &texture,
                Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: *format,
                    ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                    Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MipLevels: 1,
                            ..Default::default()
                        },
                    },
                }),
                D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: views.GetCPUDescriptorHandleForHeapStart().ptr + i * step as usize,
                },
            );
            textures.push(texture);
        }
        let words = size_of::<[u32; 4]>() * texels.len();
        let out = c.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(words as u64, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS),
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        )?;
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pso);
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetComputeRootDescriptorTable(0, views.GetGPUDescriptorHandleForHeapStart());
        list.SetComputeRootUnorderedAccessView(1, out.GetGPUVirtualAddress());
        list.Dispatch(1, 1, 1);
        list.ResourceBarrier(&[transition(
            &out,
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            D3D12_RESOURCE_STATE_COMMON,
        )]);
        c.run(&list)?;
        let loaded: Vec<String> = c
            .read(&out, 0, words as u64)?
            .chunks(size_of::<u32>())
            .map(|w| {
                format!(
                    "{:08x}",
                    u32::from_le_bytes(w.try_into().unwrap_or_default())
                )
            })
            .collect();
        Ok(format!("loaded {}", loaded.join(" ")))
    }
}
