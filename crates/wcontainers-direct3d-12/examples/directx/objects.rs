//! The COM objects' own methods: each turns its call into the front's and back. Every other method
//! of their interfaces is in com.rs and reports itself as not implemented.

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use wcontainers_direct3d_12::address::DESCRIPTOR_SIZE;
use wcontainers_direct3d_12::list::{
    Command, CommandList, CopyLocation, Descriptor, IndexBufferView, ResourceView, RootArgument,
    SamplerView, TargetView, VertexBufferView, ViewportF,
};
use wcontainers_direct3d_12::resource::{Footprint, HeapType, ResourceDesc};
use wcontainers_direct3d_12::state::{
    Blend, DepthStencil, InputElement, Raster, StencilOps, TargetBlend,
};
use wcontainers_direct3d_12::{
    Error, FenceId, GraphicsPipelineDesc, HeapId, PipelineId, ResourceId, RootId, features,
};
use wcontainers_directx_graphics_infrastructure::{AdapterDesc, format_support, plane_count};
use wcontainers_gpu::Backend;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{AsImpl, BOOL, GUID, HRESULT, Interface, Ref, Result, implement};

use crate::base::{Base, HasBase};
use crate::gpu;

/// The HRESULT for a front error.
pub fn hr(e: Error) -> windows::core::Error {
    match e {
        Error::Invalid(s) => windows::core::Error::new(E_INVALIDARG, s),
        Error::NotImplemented(s) => {
            eprintln!("wcontainers: {s}: not implemented");
            windows::core::Error::new(E_NOTIMPL, s)
        }
        Error::OutOfMemory => E_OUTOFMEMORY.into(),
        Error::DeviceRemoved => DXGI_ERROR_DEVICE_REMOVED.into(),
    }
}

/// Hands out `object` as the interface `riid` names.
pub fn give<I: Interface>(object: I, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
    if riid.is_null() || out.is_null() {
        return Err(E_POINTER.into());
    }
    // SAFETY: both pointers were checked; COM passes them from the caller.
    unsafe { object.query(riid, out).ok() }
}

macro_rules! not_implemented {
    ($($t:ty),*) => {$(
        impl $t {
            /// A method the front does not do yet: reported, never answered as success.
            pub fn unsupported(&self, name: &str) -> windows::core::Error {
                eprintln!("wcontainers: {}::{name} is not implemented", stringify!($t));
                windows::core::Error::new(E_NOTIMPL, name)
            }
        }
    )*};
}
not_implemented!(
    Factory,
    Adapter,
    crate::present::Output,
    crate::present::SwapChain,
    Device,
    Queue,
    Allocator,
    Fence,
    Resource,
    RootSignature,
    PipelineState,
    DescriptorHeap,
    Blob
);

// ---- DXGI ----

/// A DXGI factory, with the flags it was made with.
#[implement(IDXGIFactory4)]
pub struct Factory(pub u32);

impl Factory {
    pub fn EnumAdapters1(&self, adapter: u32) -> Result<IDXGIAdapter1> {
        if adapter != 0 {
            return Err(DXGI_ERROR_NOT_FOUND.into());
        }
        let desc = gpu::call(|d| AdapterDesc::from_caps(d.backend().caps()));
        Ok(Adapter(desc).into())
    }

    pub fn EnumAdapters(&self, adapter: u32) -> Result<IDXGIAdapter> {
        self.EnumAdapters1(adapter)?.cast()
    }

    pub fn EnumAdapterByLuid(
        &self,
        luid: &LUID,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        let ours = adapter_luid();
        if (luid.LowPart, luid.HighPart) != (ours.LowPart, ours.HighPart) {
            return Err(E_INVALIDARG.into());
        }
        give(self.EnumAdapters1(0)?, riid, out)
    }

    /// The adapter list never changes.
    pub fn IsCurrent(&self) -> BOOL {
        TRUE
    }

    pub fn GetCreationFlags(&self) -> DXGI_CREATE_FACTORY_FLAGS {
        DXGI_CREATE_FACTORY_FLAGS(self.0)
    }

    /// No stereo output.
    pub fn IsWindowedStereoEnabled(&self) -> BOOL {
        FALSE
    }

    pub fn CreateSwapChainForHwnd(
        &self,
        device: Ref<windows::core::IUnknown>,
        hwnd: HWND,
        desc: *const DXGI_SWAP_CHAIN_DESC1,
        fullscreen: *const DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        _output: Ref<IDXGIOutput>,
    ) -> Result<IDXGISwapChain1> {
        // SAFETY: the caller's descriptions.
        let (desc, fullscreen) = unsafe { (desc.as_ref(), fullscreen.as_ref()) };
        crate::present::create(
            device,
            hwnd,
            desc.ok_or(DXGI_ERROR_INVALID_CALL)?,
            fullscreen,
        )
    }

    pub fn CreateSwapChain(
        &self,
        device: Ref<windows::core::IUnknown>,
        desc: *const DXGI_SWAP_CHAIN_DESC,
        out: windows::core::OutRef<IDXGISwapChain>,
    ) -> HRESULT {
        // SAFETY: the caller's description.
        let Some(d) = (unsafe { desc.as_ref() }) else {
            return DXGI_ERROR_INVALID_CALL;
        };
        if !d.Windowed.as_bool() {
            return DXGI_ERROR_NOT_CURRENTLY_AVAILABLE;
        }
        let desc1 = DXGI_SWAP_CHAIN_DESC1 {
            Width: d.BufferDesc.Width,
            Height: d.BufferDesc.Height,
            Format: d.BufferDesc.Format,
            Stereo: FALSE,
            SampleDesc: d.SampleDesc,
            BufferUsage: d.BufferUsage,
            BufferCount: d.BufferCount,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: d.SwapEffect,
            AlphaMode: DXGI_ALPHA_MODE_UNSPECIFIED,
            Flags: d.Flags,
        };
        match crate::present::create(device, d.OutputWindow, &desc1, None)
            .and_then(|c| c.cast::<IDXGISwapChain>())
        {
            Ok(c) => {
                let _ = out.write(Some(c));
                S_OK
            }
            Err(e) => e.code(),
        }
    }
}

/// The adapter's LUID, which the device reports too.
pub fn adapter_luid() -> LUID {
    luid(wcontainers_directx_graphics_infrastructure::ADAPTER_LUID)
}

#[implement(IDXGIAdapter1)]
pub struct Adapter(AdapterDesc);

impl Adapter {
    pub fn GetDesc1(&self) -> Result<DXGI_ADAPTER_DESC1> {
        let d = &self.0;
        Ok(DXGI_ADAPTER_DESC1 {
            Description: d.description_utf16(),
            VendorId: d.vendor_id,
            DeviceId: d.device_id,
            SubSysId: d.sub_sys_id,
            Revision: d.revision,
            DedicatedVideoMemory: d.dedicated_video_memory as usize,
            DedicatedSystemMemory: d.dedicated_system_memory as usize,
            SharedSystemMemory: d.shared_system_memory as usize,
            AdapterLuid: luid(d.luid),
            Flags: if d.software {
                DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32
            } else {
                0
            },
        })
    }

    pub fn GetDesc(&self) -> Result<DXGI_ADAPTER_DESC> {
        let d = self.GetDesc1()?;
        Ok(DXGI_ADAPTER_DESC {
            Description: d.Description,
            VendorId: d.VendorId,
            DeviceId: d.DeviceId,
            SubSysId: d.SubSysId,
            Revision: d.Revision,
            DedicatedVideoMemory: d.DedicatedVideoMemory,
            DedicatedSystemMemory: d.DedicatedSystemMemory,
            SharedSystemMemory: d.SharedSystemMemory,
            AdapterLuid: d.AdapterLuid,
        })
    }

    /// The host's monitors, the primary first.
    /// A new factory: adapters do not keep the one that enumerated them.
    pub fn GetParent(&self, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        give::<IDXGIFactory4>(Factory(0).into(), riid, out)
    }

    pub fn EnumOutputs(&self, output: u32) -> Result<IDXGIOutput> {
        let (desc, monitor) = crate::present::outputs()
            .into_iter()
            .nth(output as usize)
            .ok_or(DXGI_ERROR_NOT_FOUND)?;
        Ok(IDXGIOutput6::from(crate::present::Output { desc, monitor }).into())
    }
}

fn luid(value: u64) -> LUID {
    LUID {
        LowPart: value as u32,
        HighPart: (value >> 32) as i32,
    }
}

// ---- The device ----

#[implement(ID3D12Device)]
#[derive(Default)]
pub struct Device {
    base: Base,
}

macro_rules! based {
    ($($t:ty => $f:tt),*) => {$(
        impl HasBase for $t {
            fn base(&self) -> &Base {
                &self.$f
            }
        }
    )*};
}
based!(
    Device => base, Queue => 1, Allocator => 1, Fence => base, Resource => base,
    RootSignature => 1, PipelineState => 1, DescriptorHeap => base, List => base
);

/// The front's description of a resource.
fn resource_desc(d: &D3D12_RESOURCE_DESC) -> ResourceDesc {
    ResourceDesc {
        dimension: d.Dimension.0 as u32,
        alignment: d.Alignment,
        width: d.Width,
        height: d.Height,
        depth_or_array_size: d.DepthOrArraySize,
        mip_levels: d.MipLevels,
        format: d.Format.0 as u32,
        sample_count: d.SampleDesc.Count,
        sample_quality: d.SampleDesc.Quality,
        layout: d.Layout.0 as u32,
        flags: d.Flags.0 as u32,
    }
}

/// The bytes of a shader the pipeline names.
///
/// # Safety
/// The bytecode points to `BytecodeLength` readable bytes, as the API requires.
unsafe fn code(b: &D3D12_SHADER_BYTECODE) -> Vec<u8> {
    if b.pShaderBytecode.is_null() {
        return Vec::new();
    }
    // SAFETY: as the caller promises.
    unsafe { std::slice::from_raw_parts(b.pShaderBytecode.cast::<u8>(), b.BytecodeLength) }.to_vec()
}

impl Device {
    pub fn GetNodeCount(&self) -> u32 {
        1
    }

    pub fn GetAdapterLuid(&self) -> LUID {
        adapter_luid()
    }

    /// Sizes and alignments of the resources together, as the front would place them; on an
    /// invalid description, a size of `u64::MAX` as Direct3D 12 answers.
    pub fn GetResourceAllocationInfo(
        &self,
        _mask: u32,
        count: u32,
        descs: *const D3D12_RESOURCE_DESC,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        let failed = D3D12_RESOURCE_ALLOCATION_INFO {
            SizeInBytes: u64::MAX,
            Alignment: 0,
        };
        if descs.is_null() {
            return failed;
        }
        // SAFETY: `count` descriptions, as the API requires.
        let descs = unsafe { std::slice::from_raw_parts(descs, count as usize) };
        let (mut size, mut alignment) = (0u64, 0u64);
        for d in descs {
            let Ok((bytes, align)) = resource_desc(d).allocation() else {
                return failed;
            };
            size = size.next_multiple_of(align) + bytes;
            alignment = alignment.max(align);
        }
        D3D12_RESOURCE_ALLOCATION_INFO {
            SizeInBytes: size,
            Alignment: alignment,
        }
    }

    /// The heap types as a discrete adapter has them, as `ARCHITECTURE` answers (no UMA): default
    /// heaps in the adapter's own memory, upload and readback heaps in system memory.
    pub fn GetCustomHeapProperties(
        &self,
        _mask: u32,
        kind: D3D12_HEAP_TYPE,
    ) -> D3D12_HEAP_PROPERTIES {
        let (page, pool) = match kind {
            D3D12_HEAP_TYPE_DEFAULT => {
                (D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE, D3D12_MEMORY_POOL_L1)
            }
            D3D12_HEAP_TYPE_UPLOAD => (D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE, D3D12_MEMORY_POOL_L0),
            D3D12_HEAP_TYPE_READBACK => (D3D12_CPU_PAGE_PROPERTY_WRITE_BACK, D3D12_MEMORY_POOL_L0),
            _ => (D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_MEMORY_POOL_UNKNOWN),
        };
        D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: page,
            MemoryPoolPreference: pool,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        }
    }

    pub fn CreateCommandQueue(
        &self,
        pdesc: *const D3D12_COMMAND_QUEUE_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let desc = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        give::<ID3D12CommandQueue>(
            crate::abi::fixed(Queue(*desc, Base::child()).into()),
            riid,
            out,
        )
    }

    pub fn CreateCommandAllocator(
        &self,
        r#type: D3D12_COMMAND_LIST_TYPE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        give::<ID3D12CommandAllocator>(Allocator(r#type, Base::child()).into(), riid, out)
    }

    pub fn CreateCommandList(
        &self,
        _nodemask: u32,
        r#type: D3D12_COMMAND_LIST_TYPE,
        _allocator: Ref<ID3D12CommandAllocator>,
        initial: Ref<ID3D12PipelineState>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        let list = List {
            kind: r#type,
            rec: Mutex::new(CommandList::new()),
            base: Base::child(),
        };
        list.start(initial);
        give::<ID3D12GraphicsCommandList>(list.into(), riid, out)
    }

    pub fn CreateDescriptorHeap(
        &self,
        pdesc: *const D3D12_DESCRIPTOR_HEAP_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let desc = *unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        let count = desc.NumDescriptors;
        let (id, cpu, gpu) = gpu::call(move |d| {
            let (id, cpu, gpu) = d.create_descriptor_heap(count);
            (id, cpu as usize, gpu)
        });
        let visible = desc.Flags.0 & D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE.0 != 0;
        give::<ID3D12DescriptorHeap>(
            crate::abi::fixed(
                DescriptorHeap {
                    desc,
                    id,
                    cpu,
                    gpu: if visible { gpu } else { 0 },
                    base: Base::child(),
                }
                .into(),
            ),
            riid,
            out,
        )
    }

    pub fn GetDescriptorHandleIncrementSize(&self, _type: D3D12_DESCRIPTOR_HEAP_TYPE) -> u32 {
        DESCRIPTOR_SIZE as u32
    }

    pub fn CreateRootSignature(
        &self,
        _nodemask: u32,
        blob: *const c_void,
        length: usize,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if blob.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: the caller's blob of `length` bytes.
        let bytes = unsafe { std::slice::from_raw_parts(blob.cast::<u8>(), length) }.to_vec();
        let id = gpu::call(move |d| d.create_root_signature(&bytes)).map_err(hr)?;
        give::<ID3D12RootSignature>(RootSignature(id, Base::child()).into(), riid, out)
    }

    pub fn CreateGraphicsPipelineState(
        &self,
        pdesc: *const D3D12_GRAPHICS_PIPELINE_STATE_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let d = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        if d.StreamOutput.NumEntries != 0
            || !d.GS.pShaderBytecode.is_null()
            || !d.HS.pShaderBytecode.is_null()
            || !d.DS.pShaderBytecode.is_null()
        {
            return Err(hr(Error::NotImplemented(
                "geometry, hull and domain stages and stream output",
            )));
        }
        let root = d.pRootSignature.as_ref().map(|r| {
            // SAFETY: root signatures come from this library.
            unsafe { r.as_impl() }.0
        });
        // SAFETY: the API requires valid bytecode and input layout pointers.
        let (vs, ps) = unsafe { (code(&d.VS), code(&d.PS)) };
        let elements = if d.InputLayout.NumElements == 0 {
            &[][..]
        } else {
            // SAFETY: as the API requires.
            unsafe {
                std::slice::from_raw_parts(
                    d.InputLayout.pInputElementDescs,
                    d.InputLayout.NumElements as usize,
                )
            }
        };
        let inputs = elements
            .iter()
            .map(|e| InputElement {
                // SAFETY: semantic names are C strings, as the API requires.
                semantic: unsafe { e.SemanticName.to_string() }.unwrap_or_default(),
                semantic_index: e.SemanticIndex,
                format: e.Format.0 as u32,
                slot: e.InputSlot,
                offset: e.AlignedByteOffset,
                per_instance: e.InputSlotClass == D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA,
                step_rate: e.InstanceDataStepRate,
            })
            .collect();
        let b = &d.BlendState;
        let mut blend = Blend {
            alpha_to_coverage: b.AlphaToCoverageEnable.as_bool(),
            independent: b.IndependentBlendEnable.as_bool(),
            targets: Default::default(),
        };
        for (t, s) in blend.targets.iter_mut().zip(&b.RenderTarget) {
            *t = TargetBlend {
                blend_enable: s.BlendEnable.as_bool(),
                logic_op_enable: s.LogicOpEnable.as_bool(),
                src: s.SrcBlend.0 as u32,
                dst: s.DestBlend.0 as u32,
                op: s.BlendOp.0 as u32,
                src_alpha: s.SrcBlendAlpha.0 as u32,
                dst_alpha: s.DestBlendAlpha.0 as u32,
                op_alpha: s.BlendOpAlpha.0 as u32,
                logic_op: s.LogicOp.0 as u32,
                write_mask: s.RenderTargetWriteMask,
            };
        }
        let r = &d.RasterizerState;
        let raster = Raster {
            fill: r.FillMode.0 as u32,
            cull: r.CullMode.0 as u32,
            front_counter_clockwise: r.FrontCounterClockwise.as_bool(),
            depth_bias: r.DepthBias,
            depth_bias_clamp: r.DepthBiasClamp,
            slope_scaled_depth_bias: r.SlopeScaledDepthBias,
            depth_clip: r.DepthClipEnable.as_bool(),
            multisample: r.MultisampleEnable.as_bool(),
            antialiased_line: r.AntialiasedLineEnable.as_bool(),
            forced_sample_count: r.ForcedSampleCount,
            conservative: r.ConservativeRaster.0 as u32,
        };
        let s = &d.DepthStencilState;
        let ops = |o: &D3D12_DEPTH_STENCILOP_DESC| StencilOps {
            fail: o.StencilFailOp.0 as u32,
            depth_fail: o.StencilDepthFailOp.0 as u32,
            pass: o.StencilPassOp.0 as u32,
            func: o.StencilFunc.0 as u32,
        };
        let depth_stencil = DepthStencil {
            depth_enable: s.DepthEnable.as_bool(),
            depth_write_all: s.DepthWriteMask == D3D12_DEPTH_WRITE_MASK_ALL,
            depth_func: s.DepthFunc.0 as u32,
            stencil_enable: s.StencilEnable.as_bool(),
            stencil_read_mask: s.StencilReadMask,
            stencil_write_mask: s.StencilWriteMask,
            front: ops(&s.FrontFace),
            back: ops(&s.BackFace),
        };
        let targets = d.NumRenderTargets.min(8) as usize;
        let rtv_formats: Vec<u32> = d.RTVFormats[..targets].iter().map(|f| f.0 as u32).collect();
        let (sample_mask, strip_cut, topology_type, dsv_format, samples) = (
            d.SampleMask,
            d.IBStripCutValue.0 as u32,
            d.PrimitiveTopologyType.0 as u32,
            d.DSVFormat.0 as u32,
            d.SampleDesc.Count,
        );
        let id = gpu::call(move |dev| {
            dev.create_graphics_pipeline(&GraphicsPipelineDesc {
                root,
                vs: &vs,
                ps: &ps,
                blend,
                sample_mask,
                raster,
                depth_stencil,
                inputs,
                strip_cut,
                topology_type,
                rtv_formats,
                dsv_format,
                samples,
            })
        })
        .map_err(hr)?;
        give::<ID3D12PipelineState>(PipelineState(id, Base::child()).into(), riid, out)
    }

    pub fn CreateRenderTargetView(
        &self,
        resource: Ref<ID3D12Resource>,
        pdesc: *const D3D12_RENDER_TARGET_VIEW_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        let descriptor = match resource.as_ref() {
            Some(r) => {
                // SAFETY: resources come from this library.
                let r = unsafe { r.as_impl() };
                // SAFETY: an optional description, as the API allows.
                match unsafe { pdesc.as_ref() } {
                    None => Some(TargetView {
                        resource: r.id,
                        format: r.desc.Format.0 as u32,
                        mip: 0,
                        first_layer: 0,
                        layers: 1,
                    }),
                    Some(v) if v.ViewDimension == D3D12_RTV_DIMENSION_TEXTURE2D => {
                        // SAFETY: the member the dimension names.
                        let t = unsafe { v.Anonymous.Texture2D };
                        Some(TargetView {
                            resource: r.id,
                            format: v.Format.0 as u32,
                            mip: t.MipSlice,
                            first_layer: 0,
                            layers: 1,
                        })
                    }
                    Some(v) if v.ViewDimension == D3D12_RTV_DIMENSION_TEXTURE2DARRAY => {
                        // SAFETY: the member the dimension names.
                        let t = unsafe { v.Anonymous.Texture2DArray };
                        Some(TargetView {
                            resource: r.id,
                            format: v.Format.0 as u32,
                            mip: t.MipSlice,
                            first_layer: t.FirstArraySlice,
                            layers: t.ArraySize,
                        })
                    }
                    Some(_) => {
                        self.unsupported("CreateRenderTargetView of this dimension");
                        None
                    }
                }
            }
            None => None,
        };
        let own = resource
            .as_ref()
            .map_or(0, |r| unsafe { r.as_impl() }.desc.Format.0 as u32);
        let descriptor = descriptor.map_or(Descriptor::Empty, |v| {
            // A view of format UNKNOWN reads the resource's own format.
            let format = if v.format == 0 { own } else { v.format };
            Descriptor::RenderTarget(TargetView { format, ..v })
        });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor };
    }

    pub fn CreateDepthStencilView(
        &self,
        resource: Ref<ID3D12Resource>,
        pdesc: *const D3D12_DEPTH_STENCIL_VIEW_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        let descriptor = resource.as_ref().and_then(|r| {
            // SAFETY: resources come from this library.
            let r = unsafe { r.as_impl() };
            let own = r.desc.Format.0 as u32;
            // SAFETY: an optional description, as the API allows.
            let (format, mip, first_layer, layers, read_only) = match unsafe { pdesc.as_ref() } {
                None => (own, 0, 0, 1, 0),
                Some(v) => {
                    let format = if v.Format.0 == 0 {
                        own
                    } else {
                        v.Format.0 as u32
                    };
                    // SAFETY: the member the dimension names.
                    let (mip, first, count) = unsafe {
                        match v.ViewDimension {
                            D3D12_DSV_DIMENSION_TEXTURE2D => (v.Anonymous.Texture2D.MipSlice, 0, 1),
                            D3D12_DSV_DIMENSION_TEXTURE2DARRAY => {
                                let t = v.Anonymous.Texture2DArray;
                                (t.MipSlice, t.FirstArraySlice, t.ArraySize)
                            }
                            D3D12_DSV_DIMENSION_TEXTURE2DMS => (0, 0, 1),
                            _ => {
                                self.unsupported("CreateDepthStencilView of this dimension");
                                return None;
                            }
                        }
                    };
                    (format, mip, first, count, v.Flags.0 as u32)
                }
            };
            Some(Descriptor::DepthStencil {
                view: TargetView {
                    resource: r.id,
                    format,
                    mip,
                    first_layer,
                    layers,
                },
                read_only,
            })
        });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor.unwrap_or_default() };
    }

    pub fn CreateConstantBufferView(
        &self,
        pdesc: *const D3D12_CONSTANT_BUFFER_VIEW_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        // SAFETY: an optional description, as the API allows.
        let descriptor =
            unsafe { pdesc.as_ref() }.map_or(Descriptor::Empty, |d| Descriptor::ConstantBuffer {
                address: d.BufferLocation,
                size: d.SizeInBytes,
            });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor };
    }

    pub fn CreateShaderResourceView(
        &self,
        resource: Ref<ID3D12Resource>,
        pdesc: *const D3D12_SHADER_RESOURCE_VIEW_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        let descriptor = resource.as_ref().and_then(|r| {
            // SAFETY: resources come from this library; an optional description.
            let (r, d) = unsafe { (r.as_impl(), pdesc.as_ref()) };
            shader_resource_view(r, d).map(Descriptor::ShaderResource)
        });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor.unwrap_or_default() };
    }

    pub fn CreateUnorderedAccessView(
        &self,
        resource: Ref<ID3D12Resource>,
        _counter: Ref<ID3D12Resource>,
        pdesc: *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        let descriptor = resource.as_ref().and_then(|r| {
            // SAFETY: resources come from this library; an optional description.
            let (r, d) = unsafe { (r.as_impl(), pdesc.as_ref()) };
            unordered_access_view(r, d).map(Descriptor::UnorderedAccess)
        });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor.unwrap_or_default() };
    }

    pub fn CreateSampler(
        &self,
        pdesc: *const D3D12_SAMPLER_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        // SAFETY: the API requires a description.
        let Some(d) = (unsafe { pdesc.as_ref() }) else {
            return;
        };
        let descriptor = Descriptor::Sampler(SamplerView {
            filter: d.Filter.0 as u32,
            address: [
                d.AddressU.0 as u32,
                d.AddressV.0 as u32,
                d.AddressW.0 as u32,
            ],
            lod_bias: d.MipLODBias.to_bits(),
            anisotropy: d.MaxAnisotropy,
            compare: d.ComparisonFunc.0 as u32,
            border: d.BorderColor.map(f32::to_bits),
            min_lod: d.MinLOD.to_bits(),
            max_lod: d.MaxLOD.to_bits(),
        });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor };
    }

    pub fn CopyDescriptorsSimple(
        &self,
        count: u32,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
        src: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) {
        // SAFETY: handles of this library's heaps with `count` slots after each.
        unsafe {
            std::ptr::copy(
                src.ptr as *const Descriptor,
                dest.ptr as *mut Descriptor,
                count as usize,
            )
        };
    }

    #[allow(clippy::too_many_arguments)]
    pub fn CopyDescriptors(
        &self,
        dest_ranges: u32,
        dest_starts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        dest_sizes: *const u32,
        src_ranges: u32,
        src_starts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        src_sizes: *const u32,
        _kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) {
        // Every source descriptor in order, then written into the destination ranges in order. A
        // missing size list means ranges of one.
        // SAFETY: the caller's arrays of the lengths given, and handles of this library's heaps.
        unsafe {
            let size = |sizes: *const u32, i: usize| {
                if sizes.is_null() {
                    1
                } else {
                    *sizes.add(i) as usize
                }
            };
            let mut all = Vec::new();
            for (i, s) in many(src_starts, src_ranges).iter().enumerate() {
                let n = size(src_sizes, i);
                all.extend_from_slice(std::slice::from_raw_parts(s.ptr as *const Descriptor, n));
            }
            let mut from = all.iter();
            for (i, d) in many(dest_starts, dest_ranges).iter().enumerate() {
                for k in 0..size(dest_sizes, i) {
                    if let Some(v) = from.next() {
                        *(d.ptr as *mut Descriptor).add(k) = *v;
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn CreateCommittedResource(
        &self,
        heap: *const D3D12_HEAP_PROPERTIES,
        flags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC,
        // The initial state and the optimised clear value change nothing this front does.
        _state: D3D12_RESOURCE_STATES,
        _clear: *const D3D12_CLEAR_VALUE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        wcontainers_direct3d_12::resource::check_heap_flags(flags.0 as u32, true).map_err(hr)?;
        // SAFETY: the API requires both.
        let (heap, desc) = unsafe {
            (
                heap.as_ref().ok_or(E_INVALIDARG)?,
                *pdesc.as_ref().ok_or(E_INVALIDARG)?,
            )
        };
        let kind = HeapType::from_d3d12(heap.Type.0 as u32).map_err(hr)?;
        let heap_type = heap.Type;
        let front = resource_desc(&desc);
        let (id, address, host) = gpu::call(move |d| {
            let id = d.create_committed(kind, &front)?;
            let host = if kind == HeapType::Default {
                0
            } else {
                d.map(id)? as usize
            };
            Ok::<_, Error>((id, d.address(id), host))
        })
        .map_err(hr)?;
        let mut desc = desc;
        if desc.MipLevels == 0 {
            desc.MipLevels = front.mips() as u16;
        }
        give::<ID3D12Resource>(
            crate::abi::fixed(
                Resource {
                    id,
                    desc,
                    address,
                    host,
                    back_buffer: false,
                    heap: heap_type,
                    heap_flags: flags,
                    base: Base::child(),
                }
                .into(),
            ),
            riid,
            out,
        )
    }

    pub fn CreateFence(
        &self,
        initial: u64,
        _flags: D3D12_FENCE_FLAGS,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        let id = gpu::call(move |d| d.create_fence(initial));
        give::<ID3D12Fence>(
            Fence {
                id,
                base: Base::child(),
            }
            .into(),
            riid,
            out,
        )
    }

    pub fn GetDeviceRemovedReason(&self) -> Result<()> {
        match gpu::call(|d| d.removed()) {
            None => Ok(()),
            Some(why) => Err(windows::core::Error::new(
                DXGI_ERROR_DRIVER_INTERNAL_ERROR,
                why,
            )),
        }
    }

    pub fn CheckFeatureSupport(
        &self,
        feature: D3D12_FEATURE,
        data: *mut c_void,
        size: u32,
    ) -> Result<()> {
        /// The program's structure for `feature`, when it passed one of the right size.
        ///
        /// # Safety
        /// `data` points to `size` writable bytes, as the API requires.
        unsafe fn fill<'a, T>(data: *mut c_void, size: u32) -> Result<&'a mut T> {
            if data.is_null() || size as usize != size_of::<T>() {
                return Err(E_INVALIDARG.into());
            }
            // SAFETY: as the caller promises; the size is the structure's.
            Ok(unsafe { &mut *data.cast::<T>() })
        }
        let caps = gpu::call(|d| d.backend().caps().clone());
        let b = |v: bool| BOOL::from(v);
        // SAFETY (each `fill` call): the program's pointer and size, as the API defines them.
        match feature {
            D3D12_FEATURE_D3D12_OPTIONS => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS>(data, size)? };
                let o = features::options(&caps);
                *d = D3D12_FEATURE_DATA_D3D12_OPTIONS {
                    DoublePrecisionFloatShaderOps: b(o.double_precision_float_shader_ops),
                    OutputMergerLogicOp: b(o.output_merger_logic_op),
                    MinPrecisionSupport: D3D12_SHADER_MIN_PRECISION_SUPPORT(
                        o.min_precision_support as i32,
                    ),
                    TiledResourcesTier: D3D12_TILED_RESOURCES_TIER(o.tiled_resources_tier as i32),
                    ResourceBindingTier: D3D12_RESOURCE_BINDING_TIER(
                        o.resource_binding_tier as i32,
                    ),
                    PSSpecifiedStencilRefSupported: b(o.ps_specified_stencil_ref_supported),
                    TypedUAVLoadAdditionalFormats: b(o.typed_uav_load_additional_formats),
                    ROVsSupported: b(o.rovs_supported),
                    ConservativeRasterizationTier: D3D12_CONSERVATIVE_RASTERIZATION_TIER(
                        o.conservative_rasterization_tier as i32,
                    ),
                    MaxGPUVirtualAddressBitsPerResource: o.max_gpu_virtual_address_bits_per_resource,
                    StandardSwizzle64KBSupported: b(o.standard_swizzle_64kb_supported),
                    CrossNodeSharingTier: D3D12_CROSS_NODE_SHARING_TIER(
                        o.cross_node_sharing_tier as i32,
                    ),
                    CrossAdapterRowMajorTextureSupported: b(
                        o.cross_adapter_row_major_texture_supported,
                    ),
                    VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation: b(
                        o.vp_and_rt_array_index_from_any_shader_feeding_rasterizer_supported_without_gs_emulation,
                    ),
                    ResourceHeapTier: D3D12_RESOURCE_HEAP_TIER(o.resource_heap_tier as i32),
                };
            }
            D3D12_FEATURE_D3D12_OPTIONS1 => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS1>(data, size)? };
                let o = features::options1(&caps);
                *d = D3D12_FEATURE_DATA_D3D12_OPTIONS1 {
                    WaveOps: b(o.wave_ops),
                    WaveLaneCountMin: o.wave_lane_count_min,
                    WaveLaneCountMax: o.wave_lane_count_max,
                    TotalLaneCount: o.total_lane_count,
                    ExpandedComputeResourceStates: b(o.expanded_compute_resource_states),
                    Int64ShaderOps: b(o.int64_shader_ops),
                };
            }
            D3D12_FEATURE_D3D12_OPTIONS2 => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS2>(data, size)? };
                *d = D3D12_FEATURE_DATA_D3D12_OPTIONS2 {
                    DepthBoundsTestSupported: b(features::depth_bounds(&caps)),
                    ProgrammableSamplePositionsTier: D3D12_PROGRAMMABLE_SAMPLE_POSITIONS_TIER(
                        features::ABSENT_TIER as i32,
                    ),
                };
            }
            // Every field of these blocks is an ability the front does not have (plan section
            // 9: view instancing, barycentrics, 16-bit shader operations, ray tracing, variable
            // rate shading, mesh shaders, sampler feedback), and zero says absent.
            D3D12_FEATURE_D3D12_OPTIONS3 => {
                *unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS3>(data, size)? } =
                    Default::default();
            }
            D3D12_FEATURE_D3D12_OPTIONS4 => {
                *unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS4>(data, size)? } =
                    Default::default();
            }
            D3D12_FEATURE_D3D12_OPTIONS5 => {
                *unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS5>(data, size)? } =
                    Default::default();
            }
            D3D12_FEATURE_D3D12_OPTIONS6 => {
                *unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS6>(data, size)? } =
                    Default::default();
            }
            D3D12_FEATURE_D3D12_OPTIONS7 => {
                *unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS7>(data, size)? } =
                    Default::default();
            }
            D3D12_FEATURE_ARCHITECTURE => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_ARCHITECTURE>(data, size)? };
                if d.NodeIndex != 0 {
                    return Err(E_INVALIDARG.into());
                }
                let a = features::Architecture::default();
                d.TileBasedRenderer = b(a.tile_based_renderer);
                d.UMA = b(a.uma);
                d.CacheCoherentUMA = b(a.cache_coherent_uma);
            }
            D3D12_FEATURE_ARCHITECTURE1 => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_ARCHITECTURE1>(data, size)? };
                if d.NodeIndex != 0 {
                    return Err(E_INVALIDARG.into());
                }
                let a = features::Architecture::default();
                d.TileBasedRenderer = b(a.tile_based_renderer);
                d.UMA = b(a.uma);
                d.CacheCoherentUMA = b(a.cache_coherent_uma);
                d.IsolatedMMU = b(a.isolated_mmu);
            }
            D3D12_FEATURE_FEATURE_LEVELS => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_FEATURE_LEVELS>(data, size)? };
                // SAFETY: the program's array of `NumFeatureLevels` levels.
                let asked: Vec<u32> =
                    unsafe { many(d.pFeatureLevelsRequested, d.NumFeatureLevels) }
                        .iter()
                        .map(|l| l.0 as u32)
                        .collect();
                if asked.is_empty() {
                    return Err(E_INVALIDARG.into());
                }
                let level =
                    features::max_feature_level(&caps, &asked).ok_or(DXGI_ERROR_UNSUPPORTED)?;
                d.MaxSupportedFeatureLevel = D3D_FEATURE_LEVEL(level as i32);
            }
            D3D12_FEATURE_SHADER_MODEL => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_SHADER_MODEL>(data, size)? };
                d.HighestShaderModel =
                    D3D_SHADER_MODEL(features::shader_model(d.HighestShaderModel.0 as u32) as i32);
            }
            D3D12_FEATURE_ROOT_SIGNATURE => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_ROOT_SIGNATURE>(data, size)? };
                d.HighestVersion = D3D_ROOT_SIGNATURE_VERSION(features::root_signature(
                    d.HighestVersion.0 as u32,
                ) as i32);
            }
            D3D12_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT => {
                let d =
                    unsafe { fill::<D3D12_FEATURE_DATA_GPU_VIRTUAL_ADDRESS_SUPPORT>(data, size)? };
                d.MaxGPUVirtualAddressBitsPerResource = features::VIRTUAL_ADDRESS_BITS;
                d.MaxGPUVirtualAddressBitsPerProcess = features::VIRTUAL_ADDRESS_BITS;
            }
            D3D12_FEATURE_FORMAT_SUPPORT => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_FORMAT_SUPPORT>(data, size)? };
                let s = format_support(&caps, d.Format.0 as u32);
                d.Support1 = D3D12_FORMAT_SUPPORT1(s.support1 as i32);
                d.Support2 = D3D12_FORMAT_SUPPORT2(s.support2 as i32);
                if s.is_empty() {
                    return Err(E_FAIL.into());
                }
            }
            D3D12_FEATURE_FORMAT_INFO => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_FORMAT_INFO>(data, size)? };
                d.PlaneCount = plane_count(d.Format.0 as u32).ok_or(E_INVALIDARG)?;
            }
            D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS => {
                let d =
                    unsafe { fill::<D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS>(data, size)? };
                d.NumQualityLevels = 0;
                d.NumQualityLevels =
                    features::quality_levels(&caps, d.Format.0 as u32, d.SampleCount)
                        .ok_or(E_FAIL)?;
            }
            _ => return Err(self.unsupported(&format!("CheckFeatureSupport({})", feature.0))),
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn GetCopyableFootprints(
        &self,
        pdesc: *const D3D12_RESOURCE_DESC,
        first: u32,
        count: u32,
        base: u64,
        layouts: *mut D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
        rows: *mut u32,
        row_sizes: *mut u64,
        total: *mut u64,
    ) {
        // SAFETY: the API requires a description.
        let Some(desc) = (unsafe { pdesc.as_ref() }) else {
            return;
        };
        let result = resource_desc(desc).footprints(first, count, base);
        // SAFETY: each output, where given, has room for `count` entries, as the API requires.
        unsafe {
            match result {
                Ok((infos, bytes)) => {
                    for (i, f) in infos.iter().enumerate() {
                        let p = f.footprint;
                        if !layouts.is_null() {
                            *layouts.add(i) = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                                Offset: p.offset,
                                Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                                    Format: DXGI_FORMAT(p.format as i32),
                                    Width: p.width,
                                    Height: p.height,
                                    Depth: p.depth,
                                    RowPitch: p.row_pitch,
                                },
                            };
                        }
                        if !rows.is_null() {
                            *rows.add(i) = f.rows;
                        }
                        if !row_sizes.is_null() {
                            *row_sizes.add(i) = f.row_size;
                        }
                    }
                    if !total.is_null() {
                        *total = bytes;
                    }
                }
                Err(_) => {
                    if !total.is_null() {
                        *total = u64::MAX;
                    }
                }
            }
        }
    }
}

// ---- Queues, allocators, fences ----

#[implement(ID3D12CommandQueue)]
pub struct Queue(D3D12_COMMAND_QUEUE_DESC, Base);

impl Queue {
    pub fn GetDesc(&self) -> D3D12_COMMAND_QUEUE_DESC {
        self.0
    }

    pub fn ExecuteCommandLists(&self, count: u32, lists: *const Option<ID3D12CommandList>) {
        if lists.is_null() {
            return;
        }
        // SAFETY: `count` lists, as the API requires.
        let lists = unsafe { std::slice::from_raw_parts(lists, count as usize) };
        let mut commands = Vec::with_capacity(lists.len());
        for l in lists.iter().flatten() {
            let Ok(l) = l.cast::<ID3D12GraphicsCommandList>() else {
                self.unsupported("ExecuteCommandLists of a list of another kind");
                return;
            };
            // SAFETY: command lists come from this library.
            let list = unsafe { l.as_impl() };
            match list.rec.lock().unwrap().commands() {
                Some(c) => commands.push(Arc::clone(c)),
                None => {
                    eprintln!("wcontainers: ExecuteCommandLists of a list that is not closed");
                    return;
                }
            }
        }
        if let Err(e) = gpu::call(move |d| d.execute(&commands)) {
            eprintln!(
                "wcontainers: the device was removed: {:?}",
                gpu::call(|d| d.removed()).unwrap_or("?")
            );
            let _ = e;
        }
    }

    pub fn Signal(&self, fence: Ref<ID3D12Fence>, value: u64) -> Result<()> {
        // SAFETY: fences come from this library.
        let fence = unsafe { fence.ok()?.as_impl() };
        let id = fence.id;
        gpu::call(move |d| d.queue_signal(id, value)).map_err(hr)
    }
}

#[implement(ID3D12CommandAllocator)]
pub struct Allocator(#[allow(dead_code)] D3D12_COMMAND_LIST_TYPE, Base);

impl Allocator {
    /// Lists keep their own commands, so there is nothing to free here.
    pub fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

#[implement(ID3D12Fence)]
pub struct Fence {
    id: FenceId,
    base: Base,
}

impl Fence {
    pub fn GetCompletedValue(&self) -> u64 {
        let id = self.id;
        gpu::call(move |d| d.completed(id))
    }

    /// With an event, the device thread sets it once the fence reaches `value`, and nothing
    /// waits here. With none, the call itself waits, as the API defines.
    pub fn SetEventOnCompletion(&self, value: u64, event: HANDLE) -> Result<()> {
        let id = self.id;
        if event.is_invalid() || event.0.is_null() {
            // No event: the call returns when the fence reaches the value. The calling thread waits
            // on an event of its own, so the device thread stays free for the signal (which may be a
            // CPU signal from another thread).
            use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
            // SAFETY: an unnamed auto-reset event, closed below.
            let own = unsafe { CreateEventW(None, false, false, None)? };
            let token = own.0 as usize as u64;
            gpu::call(move |d| d.add_waiter(id, value, token));
            // SAFETY: `own` is open until closed here.
            unsafe {
                WaitForSingleObject(own, INFINITE);
                let _ = CloseHandle(own);
            }
            return Ok(());
        }
        let token = event.0 as usize as u64;
        gpu::call(move |d| d.add_waiter(id, value, token));
        Ok(())
    }

    pub fn Signal(&self, value: u64) -> Result<()> {
        let id = self.id;
        gpu::call(move |d| d.cpu_signal(id, value));
        Ok(())
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        let id = self.id;
        gpu::call(move |d| d.destroy_fence(id));
    }
}

// ---- Resources and descriptors ----

#[implement(ID3D12Resource)]
pub struct Resource {
    id: ResourceId,
    desc: D3D12_RESOURCE_DESC,
    address: u64,
    /// The memory of an upload or readback buffer, as a number.
    host: usize,
    /// A swap chain's back buffer, which the swap chain releases.
    back_buffer: bool,
    /// The heap's type and the flags the resource was made with.
    heap: D3D12_HEAP_TYPE,
    heap_flags: D3D12_HEAP_FLAGS,
    base: Base,
}

impl Resource {
    pub fn back_buffer(id: ResourceId, desc: D3D12_RESOURCE_DESC) -> ID3D12Resource {
        crate::abi::fixed(
            Resource {
                id,
                desc,
                address: 0,
                host: 0,
                back_buffer: true,
                heap: D3D12_HEAP_TYPE_DEFAULT,
                heap_flags: D3D12_HEAP_FLAG_NONE,
                base: Base::child(),
            }
            .into(),
        )
    }

    pub fn GetDesc(&self) -> D3D12_RESOURCE_DESC {
        self.desc
    }

    pub fn GetHeapProperties(
        &self,
        properties: *mut D3D12_HEAP_PROPERTIES,
        flags: *mut D3D12_HEAP_FLAGS,
    ) -> Result<()> {
        // SAFETY: the caller's optional out pointers.
        unsafe {
            if let Some(p) = properties.as_mut() {
                *p = D3D12_HEAP_PROPERTIES {
                    Type: self.heap,
                    CreationNodeMask: 1,
                    VisibleNodeMask: 1,
                    ..Default::default()
                };
            }
            if let Some(f) = flags.as_mut() {
                *f = self.heap_flags;
            }
        }
        Ok(())
    }

    pub fn GetGPUVirtualAddress(&self) -> u64 {
        self.address
    }

    /// Upload and readback memory stays mapped; the pointer is the same every time. Readback
    /// memory holds the bytes of every copy whose fence the program has seen.
    pub fn Map(
        &self,
        _subresource: u32,
        _range: *const D3D12_RANGE,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if self.host == 0 {
            return Err(windows::core::Error::new(
                E_INVALIDARG,
                "only upload and readback buffers map",
            ));
        }
        let id = self.id;
        gpu::call(move |d| d.map(id).map(|_| ())).map_err(hr)?;
        if !out.is_null() {
            // SAFETY: the caller's out pointer.
            unsafe { *out = self.host as *mut c_void };
        }
        Ok(())
    }

    /// Written bytes reach the GPU when a list reading them is executed.
    pub fn Unmap(&self, _subresource: u32, _range: *const D3D12_RANGE) {}
}

impl Drop for Resource {
    fn drop(&mut self) {
        if !self.back_buffer {
            let id = self.id;
            gpu::call(move |d| d.destroy_resource(id));
        }
    }
}

#[implement(ID3D12RootSignature)]
pub struct RootSignature(pub RootId, Base);

impl Drop for RootSignature {
    fn drop(&mut self) {
        let id = self.0;
        gpu::call(move |d| d.destroy_root_signature(id));
    }
}

#[implement(ID3D12PipelineState)]
pub struct PipelineState(pub PipelineId, Base);

impl Drop for PipelineState {
    fn drop(&mut self) {
        let id = self.0;
        gpu::call(move |d| d.destroy_pipeline(id));
    }
}

#[implement(ID3D12DescriptorHeap)]
pub struct DescriptorHeap {
    desc: D3D12_DESCRIPTOR_HEAP_DESC,
    id: HeapId,
    /// The front's slots, one of `DESCRIPTOR_SIZE` bytes per descriptor, which CPU handles point
    /// into.
    cpu: usize,
    /// The first GPU handle; zero for heaps shaders do not see.
    gpu: u64,
    base: Base,
}

impl Drop for DescriptorHeap {
    fn drop(&mut self) {
        let id = self.id;
        gpu::call(move |d| d.destroy_descriptor_heap(id));
    }
}

impl DescriptorHeap {
    pub fn GetDesc(&self) -> D3D12_DESCRIPTOR_HEAP_DESC {
        self.desc
    }

    pub fn GetCPUDescriptorHandleForHeapStart(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.cpu }
    }

    pub fn GetGPUDescriptorHandleForHeapStart(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        D3D12_GPU_DESCRIPTOR_HANDLE { ptr: self.gpu }
    }
}

#[implement(ID3DBlob)]
pub struct Blob(pub Vec<u8>);

impl Blob {
    pub fn GetBufferPointer(&self) -> *mut c_void {
        self.0.as_ptr() as *mut c_void
    }

    pub fn GetBufferSize(&self) -> usize {
        self.0.len()
    }
}

// ---- Command lists ----

#[implement(ID3D12GraphicsCommandList)]
pub struct List {
    kind: D3D12_COMMAND_LIST_TYPE,
    rec: Mutex<CommandList>,
    base: Base,
}

/// The descriptor a CPU handle points at.
///
/// # Safety
/// The handle points into a descriptor heap of this library.
unsafe fn descriptor(h: D3D12_CPU_DESCRIPTOR_HANDLE) -> Descriptor {
    // SAFETY: as the caller promises.
    unsafe { *(h.ptr as *const Descriptor) }
}

/// The resource behind an interface pointer of this library.
fn id(r: &Ref<ID3D12Resource>) -> Option<ResourceId> {
    // SAFETY: resources come from this library.
    r.as_ref().map(|r| unsafe { r.as_impl() }.id)
}

/// # Safety
/// `p` points to `n` values, or `n` is zero.
unsafe fn many<'a, T>(p: *const T, n: u32) -> &'a [T] {
    if n == 0 || p.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, n as usize) }
    }
}

impl List {
    /// A method the front does not do yet: the list's `Close` fails with it.
    pub fn unsupported(&self, name: &str) -> windows::core::Error {
        eprintln!("wcontainers: ID3D12GraphicsCommandList::{name} is not implemented");
        self.rec.lock().unwrap().fail(Error::NotImplemented(
            "a command list method the front does not do yet",
        ));
        windows::core::Error::new(E_NOTIMPL, name)
    }

    fn record(&self, c: Command) {
        self.rec.lock().unwrap().record(c);
    }

    fn start(&self, initial: Ref<ID3D12PipelineState>) {
        if let Some(p) = initial.as_ref() {
            // SAFETY: pipeline states come from this library.
            self.record(Command::Pipeline(unsafe { p.as_impl() }.0));
        }
    }

    pub fn GetType(&self) -> D3D12_COMMAND_LIST_TYPE {
        self.kind
    }

    pub fn Close(&self) -> Result<()> {
        self.rec.lock().unwrap().close().map_err(hr)
    }

    pub fn Reset(
        &self,
        _allocator: Ref<ID3D12CommandAllocator>,
        initial: Ref<ID3D12PipelineState>,
    ) -> Result<()> {
        self.rec.lock().unwrap().reset();
        self.start(initial);
        Ok(())
    }

    pub fn SetPipelineState(&self, pipeline: Ref<ID3D12PipelineState>) {
        match pipeline.as_ref() {
            // SAFETY: pipeline states come from this library.
            Some(p) => self.record(Command::Pipeline(unsafe { p.as_impl() }.0)),
            None => self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("SetPipelineState with no pipeline")),
        }
    }

    /// Bindings of an empty root signature need nothing; root parameters are set by methods the
    /// front does not do yet, which fail the list.
    fn root_signature(&self, compute: bool, root: Ref<ID3D12RootSignature>) {
        // SAFETY: root signatures come from this library.
        let root = root.as_ref().map(|r| unsafe { r.as_impl() }.0);
        self.record(Command::RootSignature { compute, root });
    }

    pub fn SetGraphicsRootSignature(&self, root: Ref<ID3D12RootSignature>) {
        self.root_signature(false, root);
    }

    pub fn SetComputeRootSignature(&self, root: Ref<ID3D12RootSignature>) {
        self.root_signature(true, root);
    }

    /// The heaps tables name are found from the handles themselves.
    pub fn SetDescriptorHeaps(&self, _count: u32, _heaps: *const Option<ID3D12DescriptorHeap>) {}

    fn argument(&self, compute: bool, index: u32, argument: RootArgument) {
        self.record(Command::RootArgument {
            compute,
            index,
            argument,
        });
    }

    pub fn SetGraphicsRootDescriptorTable(&self, index: u32, base: &D3D12_GPU_DESCRIPTOR_HANDLE) {
        self.argument(false, index, RootArgument::Table(base.ptr));
    }

    pub fn SetComputeRootDescriptorTable(&self, index: u32, base: &D3D12_GPU_DESCRIPTOR_HANDLE) {
        self.argument(true, index, RootArgument::Table(base.ptr));
    }

    pub fn SetGraphicsRootConstantBufferView(&self, index: u32, address: u64) {
        self.argument(false, index, RootArgument::Descriptor(address));
    }

    pub fn SetComputeRootConstantBufferView(&self, index: u32, address: u64) {
        self.argument(true, index, RootArgument::Descriptor(address));
    }

    pub fn SetGraphicsRootShaderResourceView(&self, index: u32, address: u64) {
        self.argument(false, index, RootArgument::Descriptor(address));
    }

    pub fn SetComputeRootShaderResourceView(&self, index: u32, address: u64) {
        self.argument(true, index, RootArgument::Descriptor(address));
    }

    pub fn SetGraphicsRootUnorderedAccessView(&self, index: u32, address: u64) {
        self.argument(false, index, RootArgument::Descriptor(address));
    }

    pub fn SetComputeRootUnorderedAccessView(&self, index: u32, address: u64) {
        self.argument(true, index, RootArgument::Descriptor(address));
    }

    fn constants(&self, compute: bool, index: u32, offset: u32, values: &[u32]) {
        self.record(Command::RootConstants {
            compute,
            index,
            offset,
            values: values.to_vec(),
        });
    }

    pub fn SetGraphicsRoot32BitConstant(&self, index: u32, value: u32, offset: u32) {
        self.constants(false, index, offset, &[value]);
    }

    pub fn SetComputeRoot32BitConstant(&self, index: u32, value: u32, offset: u32) {
        self.constants(true, index, offset, &[value]);
    }

    pub fn SetGraphicsRoot32BitConstants(
        &self,
        index: u32,
        count: u32,
        values: *const c_void,
        offset: u32,
    ) {
        // SAFETY: the caller's `count` values.
        let values = unsafe { many(values.cast::<u32>(), count) };
        self.constants(false, index, offset, values);
    }

    pub fn SetComputeRoot32BitConstants(
        &self,
        index: u32,
        count: u32,
        values: *const c_void,
        offset: u32,
    ) {
        // SAFETY: the caller's `count` values.
        let values = unsafe { many(values.cast::<u32>(), count) };
        self.constants(true, index, offset, values);
    }

    /// Both back ends order their own work; plan section 5.2.
    pub fn ResourceBarrier(&self, _count: u32, _barriers: *const D3D12_RESOURCE_BARRIER) {}

    pub fn IASetPrimitiveTopology(&self, topology: D3D_PRIMITIVE_TOPOLOGY) {
        self.record(Command::Topology(topology.0 as u32));
    }

    pub fn IASetVertexBuffers(
        &self,
        start: u32,
        count: u32,
        views: *const D3D12_VERTEX_BUFFER_VIEW,
    ) {
        // SAFETY: `count` views, as the API requires; none unbinds.
        let given = unsafe { many(views, count) };
        let views = if given.is_empty() {
            vec![VertexBufferView::default(); count as usize]
        } else {
            given
                .iter()
                .map(|v| VertexBufferView {
                    address: v.BufferLocation,
                    size: v.SizeInBytes,
                    stride: v.StrideInBytes,
                })
                .collect()
        };
        self.record(Command::VertexBuffers { start, views });
    }

    pub fn IASetIndexBuffer(&self, view: *const D3D12_INDEX_BUFFER_VIEW) {
        // SAFETY: an optional view, as the API allows.
        let view = unsafe { view.as_ref() }.map(|v| IndexBufferView {
            address: v.BufferLocation,
            size: v.SizeInBytes,
            format: v.Format.0 as u32,
        });
        self.record(Command::IndexBuffer(view));
    }

    pub fn RSSetViewports(&self, count: u32, viewports: *const D3D12_VIEWPORT) {
        // SAFETY: `count` viewports, as the API requires.
        let v = unsafe { many(viewports, count) };
        self.record(Command::Viewports(
            v.iter()
                .map(|v| ViewportF {
                    x: v.TopLeftX,
                    y: v.TopLeftY,
                    width: v.Width,
                    height: v.Height,
                    min_depth: v.MinDepth,
                    max_depth: v.MaxDepth,
                })
                .collect(),
        ));
    }

    pub fn RSSetScissorRects(&self, count: u32, rects: *const RECT) {
        // SAFETY: `count` rectangles, as the API requires.
        let r = unsafe { many(rects, count) };
        self.record(Command::Scissors(
            r.iter()
                .map(|r| [r.left, r.top, r.right, r.bottom])
                .collect(),
        ));
    }

    pub fn OMSetBlendFactor(&self, factor: *const f32) {
        // SAFETY: four floats or none, as the API allows.
        let f = if factor.is_null() {
            [1.0; 4]
        } else {
            unsafe { *(factor as *const [f32; 4]) }
        };
        self.record(Command::BlendFactor(f));
    }

    pub fn OMSetStencilRef(&self, value: u32) {
        self.record(Command::StencilRef(value));
    }

    pub fn OMSetRenderTargets(
        &self,
        count: u32,
        targets: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        single_range: BOOL,
        depth: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        // SAFETY: handles of this library's heaps, as many as the API says.
        let colors = unsafe {
            if targets.is_null() || count == 0 {
                Vec::new()
            } else if single_range.as_bool() {
                let first = (*targets).ptr;
                (0..count as usize)
                    .map(|i| {
                        descriptor(D3D12_CPU_DESCRIPTOR_HANDLE {
                            ptr: first + i * DESCRIPTOR_SIZE as usize,
                        })
                    })
                    .collect()
            } else {
                many(targets, count)
                    .iter()
                    .map(|h| descriptor(*h))
                    .collect()
            }
        };
        // SAFETY: an optional handle of this library's heaps.
        let depth = unsafe { depth.as_ref() }.map(|h| unsafe { descriptor(*h) });
        self.record(Command::Targets { colors, depth });
    }

    pub fn ClearRenderTargetView(
        &self,
        view: &D3D12_CPU_DESCRIPTOR_HANDLE,
        color: *const f32,
        count: u32,
        rects: *const RECT,
    ) {
        // SAFETY: a handle of this library's heaps, four floats and `count` rectangles.
        let (view, color, rects) = unsafe {
            (
                descriptor(*view),
                *(color as *const [f32; 4]),
                many(rects, count),
            )
        };
        self.record(Command::ClearTarget {
            view,
            color,
            rects: rects
                .iter()
                .map(|r| [r.left, r.top, r.right, r.bottom])
                .collect(),
        });
    }

    pub fn ClearDepthStencilView(
        &self,
        view: &D3D12_CPU_DESCRIPTOR_HANDLE,
        flags: D3D12_CLEAR_FLAGS,
        depth: f32,
        stencil: u8,
        count: u32,
        rects: *const RECT,
    ) {
        // SAFETY: a handle of this library's heaps and `count` rectangles.
        let (view, rects) = unsafe { (descriptor(*view), many(rects, count)) };
        self.record(Command::ClearDepth {
            view,
            depth: (flags.0 & D3D12_CLEAR_FLAG_DEPTH.0 != 0).then_some(depth),
            stencil: (flags.0 & D3D12_CLEAR_FLAG_STENCIL.0 != 0).then_some(stencil),
            rects: rects
                .iter()
                .map(|r| [r.left, r.top, r.right, r.bottom])
                .collect(),
        });
    }

    pub fn DrawInstanced(
        &self,
        vertices: u32,
        instances: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.record(Command::Draw {
            vertices,
            instances,
            first_vertex,
            first_instance,
        });
    }

    pub fn DrawIndexedInstanced(
        &self,
        indices: u32,
        instances: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) {
        self.record(Command::DrawIndexed {
            indices,
            instances,
            first_index,
            base_vertex,
            first_instance,
        });
    }

    /// The whole resource: a buffer's bytes, or every subresource of a texture.
    pub fn CopyResource(&self, dst: Ref<ID3D12Resource>, src: Ref<ID3D12Resource>) {
        let (Some(d), Some(s)) = (dst.as_ref(), src.as_ref()) else {
            return self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("CopyResource with no resource"));
        };
        // SAFETY: resources come from this library.
        let (d, s) = unsafe { (d.as_impl(), s.as_impl()) };
        if d.desc.Dimension == D3D12_RESOURCE_DIMENSION_BUFFER {
            return self.record(Command::CopyBuffer {
                dst: d.id,
                dst_offset: 0,
                src: s.id,
                src_offset: 0,
                size: d.desc.Width.min(s.desc.Width),
            });
        }
        let layers = if d.desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE3D {
            1
        } else {
            u32::from(d.desc.DepthOrArraySize)
        };
        for index in 0..u32::from(d.desc.MipLevels) * layers {
            self.record(Command::CopyTexture {
                dst: CopyLocation::Subresource {
                    resource: d.id,
                    index,
                },
                at: [0; 3],
                src: CopyLocation::Subresource {
                    resource: s.id,
                    index,
                },
                area: None,
            });
        }
    }

    pub fn CopyBufferRegion(
        &self,
        dst: Ref<ID3D12Resource>,
        dst_offset: u64,
        src: Ref<ID3D12Resource>,
        src_offset: u64,
        size: u64,
    ) {
        match (id(&dst), id(&src)) {
            (Some(dst), Some(src)) => self.record(Command::CopyBuffer {
                dst,
                dst_offset,
                src,
                src_offset,
                size,
            }),
            _ => self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("CopyBufferRegion with no buffer")),
        }
    }

    pub fn CopyTextureRegion(
        &self,
        dst: *const D3D12_TEXTURE_COPY_LOCATION,
        x: u32,
        y: u32,
        z: u32,
        src: *const D3D12_TEXTURE_COPY_LOCATION,
        area: *const D3D12_BOX,
    ) {
        // SAFETY: the API requires both locations; the box is optional.
        let (dst, src, area) = unsafe { (dst.as_ref(), src.as_ref(), area.as_ref()) };
        let location = |l: &D3D12_TEXTURE_COPY_LOCATION| {
            // SAFETY: resources come from this library; the union member is the one `Type` names.
            let resource = unsafe { l.pResource.as_ref()?.as_impl() }.id;
            Some(if l.Type == D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX {
                CopyLocation::Subresource {
                    resource,
                    index: unsafe { l.Anonymous.SubresourceIndex },
                }
            } else {
                let p = unsafe { l.Anonymous.PlacedFootprint };
                let f = p.Footprint;
                CopyLocation::Footprint {
                    resource,
                    footprint: Footprint {
                        offset: p.Offset,
                        format: f.Format.0 as u32,
                        width: f.Width,
                        height: f.Height,
                        depth: f.Depth,
                        row_pitch: f.RowPitch,
                    },
                }
            })
        };
        match (dst.and_then(location), src.and_then(location)) {
            (Some(dst), Some(src)) => self.record(Command::CopyTexture {
                dst,
                at: [x, y, z],
                src,
                area: area.map(|b| [b.left, b.top, b.front, b.right, b.bottom, b.back]),
            }),
            _ => self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("CopyTextureRegion with no resource")),
        }
    }
}

/// A shader resource view of `r`; None for a description this front cannot read.
fn shader_resource_view(
    r: &Resource,
    d: Option<&D3D12_SHADER_RESOURCE_VIEW_DESC>,
) -> Option<ResourceView> {
    let own = r.desc.Format.0 as u32;
    let layers = u32::from(r.desc.DepthOrArraySize);
    let mut v = ResourceView {
        resource: r.id,
        format: own,
        dimension: 0,
        mapping: 0x1688,
        first_element: 0,
        count: u32::MAX,
        stride: 0,
        mip: 0,
        first_layer: 0,
        layers: 1,
        flags: 0,
        min_lod: 0,
    };
    let Some(d) = d else {
        // The resource's own dimension, every mip and layer.
        v.dimension = match r.desc.Dimension {
            D3D12_RESOURCE_DIMENSION_TEXTURE2D if r.desc.SampleDesc.Count > 1 => {
                if layers > 1 {
                    7
                } else {
                    6
                }
            }
            D3D12_RESOURCE_DIMENSION_TEXTURE2D => {
                if layers > 1 {
                    5
                } else {
                    4
                }
            }
            D3D12_RESOURCE_DIMENSION_TEXTURE3D => 8,
            D3D12_RESOURCE_DIMENSION_TEXTURE1D => {
                if layers > 1 {
                    3
                } else {
                    2
                }
            }
            _ => return None,
        };
        v.layers = layers;
        return Some(v);
    };
    if d.Format.0 != 0 {
        v.format = d.Format.0 as u32;
    }
    v.dimension = d.ViewDimension.0 as u32;
    v.mapping = d.Shader4ComponentMapping;
    // SAFETY: the member the dimension names.
    unsafe {
        match d.ViewDimension {
            D3D12_SRV_DIMENSION_BUFFER => {
                let b = d.Anonymous.Buffer;
                v.first_element = b.FirstElement;
                v.count = b.NumElements;
                v.stride = b.StructureByteStride;
                v.flags = b.Flags.0 as u32;
            }
            D3D12_SRV_DIMENSION_TEXTURE2D => {
                let t = d.Anonymous.Texture2D;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
            }
            D3D12_SRV_DIMENSION_TEXTURE2DARRAY => {
                let t = d.Anonymous.Texture2DArray;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
                (v.first_layer, v.layers) = (t.FirstArraySlice, t.ArraySize);
            }
            D3D12_SRV_DIMENSION_TEXTURE2DMS => {}
            D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY => {
                let t = d.Anonymous.Texture2DMSArray;
                (v.first_layer, v.layers) = (t.FirstArraySlice, t.ArraySize);
            }
            D3D12_SRV_DIMENSION_TEXTURE3D => {
                let t = d.Anonymous.Texture3D;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
            }
            D3D12_SRV_DIMENSION_TEXTURECUBE => {
                let t = d.Anonymous.TextureCube;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
            }
            D3D12_SRV_DIMENSION_TEXTURECUBEARRAY => {
                let t = d.Anonymous.TextureCubeArray;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
                (v.first_layer, v.layers) = (t.First2DArrayFace, t.NumCubes);
            }
            D3D12_SRV_DIMENSION_TEXTURE1D => {
                let t = d.Anonymous.Texture1D;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
            }
            D3D12_SRV_DIMENSION_TEXTURE1DARRAY => {
                let t = d.Anonymous.Texture1DArray;
                (v.mip, v.count, v.min_lod) = (
                    t.MostDetailedMip,
                    t.MipLevels,
                    t.ResourceMinLODClamp.to_bits(),
                );
                (v.first_layer, v.layers) = (t.FirstArraySlice, t.ArraySize);
            }
            _ => return None,
        }
    }
    Some(v)
}

/// An unordered access view of `r`; None for a description this front cannot read.
fn unordered_access_view(
    r: &Resource,
    d: Option<&D3D12_UNORDERED_ACCESS_VIEW_DESC>,
) -> Option<ResourceView> {
    let mut v = ResourceView {
        resource: r.id,
        format: r.desc.Format.0 as u32,
        dimension: 0,
        mapping: 0x1688,
        first_element: 0,
        count: 1,
        stride: 0,
        mip: 0,
        first_layer: 0,
        layers: 1,
        flags: 0,
        min_lod: 0,
    };
    let Some(d) = d else {
        v.dimension = match r.desc.Dimension {
            D3D12_RESOURCE_DIMENSION_TEXTURE2D if r.desc.DepthOrArraySize > 1 => 5,
            D3D12_RESOURCE_DIMENSION_TEXTURE2D => 4,
            D3D12_RESOURCE_DIMENSION_TEXTURE3D => 8,
            _ => return None,
        };
        v.layers = u32::from(r.desc.DepthOrArraySize);
        return Some(v);
    };
    if d.Format.0 != 0 {
        v.format = d.Format.0 as u32;
    }
    v.dimension = d.ViewDimension.0 as u32;
    // SAFETY: the member the dimension names.
    unsafe {
        match d.ViewDimension {
            D3D12_UAV_DIMENSION_BUFFER => {
                let b = d.Anonymous.Buffer;
                v.first_element = b.FirstElement;
                v.count = b.NumElements;
                v.stride = b.StructureByteStride;
                v.flags = b.Flags.0 as u32;
            }
            D3D12_UAV_DIMENSION_TEXTURE2D => v.mip = d.Anonymous.Texture2D.MipSlice,
            D3D12_UAV_DIMENSION_TEXTURE2DARRAY => {
                let t = d.Anonymous.Texture2DArray;
                (v.mip, v.first_layer, v.layers) = (t.MipSlice, t.FirstArraySlice, t.ArraySize);
            }
            D3D12_UAV_DIMENSION_TEXTURE3D => {
                let t = d.Anonymous.Texture3D;
                (v.mip, v.first_layer, v.layers) = (t.MipSlice, t.FirstWSlice, t.WSize);
            }
            _ => return None,
        }
    }
    Some(v)
}
