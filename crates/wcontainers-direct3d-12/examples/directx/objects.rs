//! The COM objects' own methods: each turns its call into the front's and back. Every other method
//! of their interfaces is in com.rs and reports itself as not implemented.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use wcontainers_direct3d_12::address::DESCRIPTOR_SIZE;
use wcontainers_direct3d_12::list::{
    Command, CommandList, CopyLocation, Descriptor, IndexBufferView, IndirectArgument,
    ResourceView, RootArgument, SamplerView, StreamView, TargetView, VertexBufferView, ViewportF,
};
use wcontainers_direct3d_12::resource::{
    DIMENSION_BUFFER, Footprint, HeapType, ResourceDesc, check_creation, check_state,
};
use wcontainers_direct3d_12::state::{
    Blend, DepthStencil, InputElement, Raster, StencilOps, TargetBlend,
};
use wcontainers_direct3d_12::{
    Error, FenceId, GraphicsPipelineDesc, HeapId, MemoryId, PipelineId, QueryHeapId, QueueId,
    ResourceId, features,
};
use wcontainers_directx_graphics_infrastructure::{
    AdapterDesc, dxgi_format, format_support, plane_count,
};
use wcontainers_gpu::{Backend, Extent, Origin};
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
    QueryHeap,
    CommandSignature,
    Heap,
    DescriptorHeap,
    Blob
);

// ---- DXGI ----

/// A DXGI factory, with the flags it was made with.
#[implement(IDXGIFactory7)]
pub struct Factory {
    pub flags: u32,
    /// The window `MakeWindowAssociation` named, as a number.
    window: std::sync::atomic::AtomicIsize,
    /// The cookies `RegisterAdaptersChangedEvent` gave and not yet taken back, and the next one.
    adapter_events: Mutex<(Vec<u32>, u32)>,
}

impl Factory {
    pub fn EnumAdapters1(&self, adapter: u32) -> Result<IDXGIAdapter1> {
        if adapter != 0 {
            return Err(DXGI_ERROR_NOT_FOUND.into());
        }
        let desc = gpu::call(|d| AdapterDesc::from_caps(d.backend().caps()));
        Ok(IDXGIAdapter4::from(Adapter(desc)).into())
    }

    pub fn EnumAdapters(&self, adapter: u32) -> Result<IDXGIAdapter> {
        self.EnumAdapters1(adapter)?.cast()
    }

    /// One adapter, first for every preference.
    pub fn EnumAdapterByGpuPreference(
        &self,
        adapter: u32,
        preference: DXGI_GPU_PREFERENCE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if !(0..=2).contains(&preference.0) {
            return Err(E_INVALIDARG.into());
        }
        give(self.EnumAdapters1(adapter)?, riid, out)
    }

    /// Presents never tear: the host shows whole frames.
    pub fn CheckFeatureSupport(
        &self,
        feature: DXGI_FEATURE,
        data: *mut c_void,
        size: u32,
    ) -> Result<()> {
        if feature != DXGI_FEATURE_PRESENT_ALLOW_TEARING
            || data.is_null()
            || size as usize != size_of::<BOOL>()
        {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        // SAFETY: the caller's BOOL, checked above.
        unsafe { data.cast::<BOOL>().write_unaligned(FALSE) };
        Ok(())
    }

    /// The adapter list never changes, so the event is never set.
    pub fn RegisterAdaptersChangedEvent(&self, event: HANDLE) -> Result<u32> {
        if event.is_invalid() {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        let mut g = self
            .adapter_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        g.1 += 1;
        let cookie = g.1;
        g.0.push(cookie);
        Ok(cookie)
    }

    pub fn UnregisterAdaptersChangedEvent(&self, cookie: u32) -> Result<()> {
        let mut g = self
            .adapter_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let at = g.0.iter().position(|&c| c == cookie);
        g.0.swap_remove(at.ok_or(DXGI_ERROR_INVALID_CALL)?);
        Ok(())
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

    pub fn new(flags: u32) -> Self {
        Self {
            flags,
            window: Default::default(),
            adapter_events: Default::default(),
        }
    }

    /// The front never changes modes on Alt+Enter nor reacts to Print Screen, the keys this
    /// association governs, so only the window is kept.
    pub fn MakeWindowAssociation(&self, window: HWND, flags: DXGI_MWA_FLAGS) -> Result<()> {
        if flags.0
            & !(DXGI_MWA_NO_WINDOW_CHANGES.0 | DXGI_MWA_NO_ALT_ENTER.0 | DXGI_MWA_NO_PRINT_SCREEN.0)
            != 0
        {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        self.window.store(window.0 as isize, Ordering::Relaxed);
        Ok(())
    }

    pub fn GetWindowAssociation(&self) -> Result<HWND> {
        Ok(HWND(self.window.load(Ordering::Relaxed) as *mut c_void))
    }

    /// The adapter list never changes.
    pub fn IsCurrent(&self) -> BOOL {
        TRUE
    }

    pub fn GetCreationFlags(&self) -> DXGI_CREATE_FACTORY_FLAGS {
        DXGI_CREATE_FACTORY_FLAGS(self.flags)
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

/// DXGI 1.6's adapter, as Windows 10 gives every Direct3D 12 program: programs call its newer
/// methods through the adapter `EnumAdapters` gives without asking for them. Video memory
/// budgets and reservations answer E_NOTIMPL (no browser reports memory).
#[implement(IDXGIAdapter4)]
pub struct Adapter(AdapterDesc);

impl Adapter {
    /// The flags say only what is known: no preemption finer than a submission is promised, and
    /// fences are Direct3D 12's (monitored) ones.
    pub fn GetDesc3(&self) -> Result<DXGI_ADAPTER_DESC3> {
        let d = self.GetDesc1()?;
        Ok(DXGI_ADAPTER_DESC3 {
            Description: d.Description,
            VendorId: d.VendorId,
            DeviceId: d.DeviceId,
            SubSysId: d.SubSysId,
            Revision: d.Revision,
            DedicatedVideoMemory: d.DedicatedVideoMemory,
            DedicatedSystemMemory: d.DedicatedSystemMemory,
            SharedSystemMemory: d.SharedSystemMemory,
            AdapterLuid: d.AdapterLuid,
            Flags: DXGI_ADAPTER_FLAG3(d.Flags as i32) | DXGI_ADAPTER_FLAG3_SUPPORT_MONITORED_FENCES,
            GraphicsPreemptionGranularity: DXGI_GRAPHICS_PREEMPTION_DMA_BUFFER_BOUNDARY,
            ComputePreemptionGranularity: DXGI_COMPUTE_PREEMPTION_DMA_BUFFER_BOUNDARY,
        })
    }

    pub fn GetDesc2(&self) -> Result<DXGI_ADAPTER_DESC2> {
        let d = self.GetDesc3()?;
        Ok(DXGI_ADAPTER_DESC2 {
            Description: d.Description,
            VendorId: d.VendorId,
            DeviceId: d.DeviceId,
            SubSysId: d.SubSysId,
            Revision: d.Revision,
            DedicatedVideoMemory: d.DedicatedVideoMemory,
            DedicatedSystemMemory: d.DedicatedSystemMemory,
            SharedSystemMemory: d.SharedSystemMemory,
            AdapterLuid: d.AdapterLuid,
            Flags: (d.Flags & DXGI_ADAPTER_FLAG3_SOFTWARE).0 as u32,
            GraphicsPreemptionGranularity: d.GraphicsPreemptionGranularity,
            ComputePreemptionGranularity: d.ComputePreemptionGranularity,
        })
    }

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
        give::<IDXGIFactory7>(Factory::new(0).into(), riid, out)
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

#[implement(ID3D12Device8)]
pub struct Device {
    base: Base,
    /// The LUID of the adapter it was made on.
    luid: LUID,
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
    Device => base, Queue => 1, Allocator => base, Fence => base, Resource => base,
    RootSignature => 1, PipelineState => 1, QueryHeap => 1, Heap => 2, DescriptorHeap => base, List => base,
    CommandSignature => 1
);

/// Checks a `D3D12_COMMAND_LIST_TYPE`: the video types are not done, others do not exist.
fn list_type(kind: D3D12_COMMAND_LIST_TYPE) -> Result<()> {
    match kind {
        D3D12_COMMAND_LIST_TYPE_DIRECT
        | D3D12_COMMAND_LIST_TYPE_BUNDLE
        | D3D12_COMMAND_LIST_TYPE_COMPUTE
        | D3D12_COMMAND_LIST_TYPE_COPY => Ok(()),
        D3D12_COMMAND_LIST_TYPE_VIDEO_DECODE
        | D3D12_COMMAND_LIST_TYPE_VIDEO_PROCESS
        | D3D12_COMMAND_LIST_TYPE_VIDEO_ENCODE => {
            Err(hr(Error::NotImplemented("video command lists")))
        }
        _ => Err(E_INVALIDARG.into()),
    }
}

/// What `GetResourceAllocationInfo` answers for an invalid description.
fn allocation_failed() -> D3D12_RESOURCE_ALLOCATION_INFO {
    D3D12_RESOURCE_ALLOCATION_INFO {
        SizeInBytes: u64::MAX,
        Alignment: D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as u64,
    }
}

/// The size and alignment of resources placed one after another, and each one's place in
/// `infos` when given.
fn allocation(
    descs: impl Iterator<Item = ResourceDesc>,
    mut infos: Option<&mut [D3D12_RESOURCE_ALLOCATION_INFO1]>,
) -> D3D12_RESOURCE_ALLOCATION_INFO {
    let (mut size, mut alignment) = (0u64, 0u64);
    for (i, d) in descs.enumerate() {
        let Ok((bytes, align)) = d.allocation() else {
            return allocation_failed();
        };
        let offset = size.next_multiple_of(align);
        if let Some(info) = infos.as_deref_mut().and_then(|infos| infos.get_mut(i)) {
            *info = D3D12_RESOURCE_ALLOCATION_INFO1 {
                Offset: offset,
                Alignment: align,
                SizeInBytes: bytes,
            };
        }
        size = offset + bytes;
        alignment = alignment.max(align);
    }
    D3D12_RESOURCE_ALLOCATION_INFO {
        SizeInBytes: size,
        Alignment: alignment,
    }
}

/// A `D3D12_RESOURCE_DESC1` as the older description: its sampler feedback region must be empty,
/// as `D3D12_OPTIONS7` reports no sampler feedback.
fn desc_from_desc1(d: &D3D12_RESOURCE_DESC1) -> Result<D3D12_RESOURCE_DESC> {
    let r = d.SamplerFeedbackMipRegion;
    if r.Width != 0 || r.Height != 0 || r.Depth != 0 {
        return Err(E_INVALIDARG.into());
    }
    Ok(D3D12_RESOURCE_DESC {
        Dimension: d.Dimension,
        Alignment: d.Alignment,
        Width: d.Width,
        Height: d.Height,
        DepthOrArraySize: d.DepthOrArraySize,
        MipLevels: d.MipLevels,
        Format: d.Format,
        SampleDesc: d.SampleDesc,
        Layout: d.Layout,
        Flags: d.Flags,
    })
}

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
    /// A device of the adapter whose LUID is `luid`.
    pub fn on(luid: LUID) -> Self {
        Device {
            base: Base::default(),
            luid,
        }
    }

    pub fn GetNodeCount(&self) -> u32 {
        1
    }

    pub fn GetAdapterLuid(&self) -> LUID {
        self.luid
    }

    /// Sizes and alignments of the resources together, as the front would place them; on an
    /// invalid description, a size of `u64::MAX` as Direct3D 12 answers.
    pub fn GetResourceAllocationInfo(
        &self,
        _mask: u32,
        count: u32,
        descs: *const D3D12_RESOURCE_DESC,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        if descs.is_null() {
            return allocation_failed();
        }
        // SAFETY: `count` descriptions, as the API requires.
        allocation(
            unsafe { many(descs, count) }.iter().map(resource_desc),
            None,
        )
    }

    /// As `GetResourceAllocationInfo`, also giving each resource's offset, size and alignment.
    pub fn GetResourceAllocationInfo1(
        &self,
        _mask: u32,
        count: u32,
        descs: *const D3D12_RESOURCE_DESC,
        infos: *mut D3D12_RESOURCE_ALLOCATION_INFO1,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        // SAFETY: `count` descriptions and, when given, as many infos, as the API requires.
        let (descs, infos) = unsafe { (many(descs, count), many_mut(infos, count)) };
        allocation(descs.iter().map(resource_desc), infos)
    }

    /// As `GetResourceAllocationInfo1`, of `D3D12_RESOURCE_DESC1` descriptions.
    pub fn GetResourceAllocationInfo2(
        &self,
        _mask: u32,
        count: u32,
        descs: *const D3D12_RESOURCE_DESC1,
        infos: *mut D3D12_RESOURCE_ALLOCATION_INFO1,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        // SAFETY: as `GetResourceAllocationInfo1`.
        let (descs, infos) = unsafe { (many(descs, count), many_mut(infos, count)) };
        let descs: Option<Vec<_>> = descs.iter().map(|d| desc_from_desc1(d).ok()).collect();
        match descs {
            Some(d) => allocation(d.iter().map(resource_desc), infos),
            None => allocation_failed(),
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
        let id = gpu::call(|d| d.create_queue()).map_err(hr)?;
        give::<ID3D12CommandQueue>(
            crate::abi::fixed(Queue(*desc, Base::child(), id).into()),
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
        list_type(r#type)?;
        let allocator = Allocator {
            kind: r#type,
            user: Arc::default(),
            base: Base::child(),
        };
        give::<ID3D12CommandAllocator>(allocator.into(), riid, out)
    }

    pub fn CreateCommandList(
        &self,
        _nodemask: u32,
        r#type: D3D12_COMMAND_LIST_TYPE,
        allocator: Ref<ID3D12CommandAllocator>,
        initial: Ref<ID3D12PipelineState>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        list_type(r#type)?;
        let list = List {
            kind: r#type,
            rec: Mutex::new(CommandList::new()),
            allocator: Mutex::new(None),
            pass_end: Mutex::new(Vec::new()),
            base: Base::child(),
        };
        list.take(allocator)?;
        list.start(initial);
        give::<ID3D12GraphicsCommandList4>(list.into(), riid, out)
    }

    pub fn CreateDescriptorHeap(
        &self,
        pdesc: *const D3D12_DESCRIPTOR_HEAP_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let desc = *unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        // Only CBV, SRV and UAV heaps and sampler heaps are seen by shaders.
        if desc.Flags.0 & D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE.0 != 0
            && matches!(
                desc.Type,
                D3D12_DESCRIPTOR_HEAP_TYPE_RTV | D3D12_DESCRIPTOR_HEAP_TYPE_DSV
            )
        {
            return Err(E_INVALIDARG.into());
        }
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
        /// Live root signatures by their blobs: equal blobs give the same object, as Direct3D 12
        /// gives.
        static MADE: Mutex<Vec<(Vec<u8>, windows::core::Weak<ID3D12RootSignature>)>> =
            Mutex::new(Vec::new());
        // SAFETY: the caller's blob of `length` bytes.
        let bytes = unsafe { std::slice::from_raw_parts(blob.cast::<u8>(), length) }.to_vec();
        let mut made = MADE.lock().unwrap();
        made.retain(|(_, w)| w.upgrade().is_some());
        if let Some(root) = made
            .iter()
            .find(|(b, _)| *b == bytes)
            .and_then(|(_, w)| w.upgrade())
        {
            return give(root, riid, out);
        }
        let copy = bytes.clone();
        let signature = gpu::call(move |d| d.create_root_signature(&copy)).map_err(hr)?;
        let root: ID3D12RootSignature = RootSignature(signature, Base::child()).into();
        made.push((bytes, root.downgrade()?));
        give(root, riid, out)
    }

    pub fn CreateGraphicsPipelineState(
        &self,
        pdesc: *const D3D12_GRAPHICS_PIPELINE_STATE_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let d = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        let root = d.pRootSignature.as_ref().map(|r| {
            // SAFETY: root signatures come from this library.
            unsafe { r.as_impl() }.0.clone()
        });
        // SAFETY: the API requires valid bytecode and input layout pointers.
        let (vs, hs, ds, gs, ps) = unsafe {
            (
                code(&d.VS),
                code(&d.HS),
                code(&d.DS),
                code(&d.GS),
                code(&d.PS),
            )
        };
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
        // SAFETY: the declaration's arrays as the API requires.
        let stream_output = unsafe { stream_output(&d.StreamOutput) };
        let id = gpu::call(move |dev| {
            dev.create_graphics_pipeline(&GraphicsPipelineDesc {
                root,
                vs: &vs,
                hs: &hs,
                ds: &ds,
                gs: &gs,
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
                stream_output,
            })
        })
        .map_err(hr)?;
        give::<ID3D12PipelineState>(PipelineState(id, Base::child()).into(), riid, out)
    }

    pub fn CreateQueryHeap(
        &self,
        pdesc: *const D3D12_QUERY_HEAP_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let d = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        let (kind, count) = (d.Type.0 as u32, d.Count);
        let id = gpu::call(move |dev| dev.create_query_heap(kind, count)).map_err(hr)?;
        give::<ID3D12QueryHeap>(QueryHeap(id, Base::child()).into(), riid, out)
    }

    pub fn CreateCommandSignature(
        &self,
        pdesc: *const D3D12_COMMAND_SIGNATURE_DESC,
        root: Ref<ID3D12RootSignature>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description with `NumArgumentDescs` arguments.
        let d = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        let descs = unsafe { many(d.pArgumentDescs, d.NumArgumentDescs) };
        let mut arguments = Vec::with_capacity(descs.len());
        for a in descs {
            // SAFETY: each argument's union member is the one its type names.
            let argument = unsafe {
                match a.Type {
                    D3D12_INDIRECT_ARGUMENT_TYPE_DRAW => IndirectArgument::Draw,
                    D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED => IndirectArgument::DrawIndexed,
                    D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH => IndirectArgument::Dispatch,
                    D3D12_INDIRECT_ARGUMENT_TYPE_VERTEX_BUFFER_VIEW => {
                        IndirectArgument::VertexBuffer {
                            slot: a.Anonymous.VertexBuffer.Slot,
                        }
                    }
                    D3D12_INDIRECT_ARGUMENT_TYPE_INDEX_BUFFER_VIEW => IndirectArgument::IndexBuffer,
                    D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT => IndirectArgument::Constants {
                        root: a.Anonymous.Constant.RootParameterIndex,
                        offset: a.Anonymous.Constant.DestOffsetIn32BitValues,
                        count: a.Anonymous.Constant.Num32BitValuesToSet,
                    },
                    D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT_BUFFER_VIEW => {
                        IndirectArgument::RootView {
                            root: a.Anonymous.ConstantBufferView.RootParameterIndex,
                        }
                    }
                    D3D12_INDIRECT_ARGUMENT_TYPE_SHADER_RESOURCE_VIEW => {
                        IndirectArgument::RootView {
                            root: a.Anonymous.ShaderResourceView.RootParameterIndex,
                        }
                    }
                    D3D12_INDIRECT_ARGUMENT_TYPE_UNORDERED_ACCESS_VIEW => {
                        IndirectArgument::RootView {
                            root: a.Anonymous.UnorderedAccessView.RootParameterIndex,
                        }
                    }
                    _ => {
                        return Err(hr(Error::NotImplemented("this kind of indirect argument")));
                    }
                }
            };
            arguments.push(argument);
        }
        let signature = wcontainers_direct3d_12::list::CommandSignature::new(
            d.ByteStride,
            arguments,
            root.is_some(),
        )
        .map_err(hr)?;
        give::<ID3D12CommandSignature>(
            CommandSignature(Arc::new(signature), Base::child()).into(),
            riid,
            out,
        )
    }

    pub fn CreateComputePipelineState(
        &self,
        pdesc: *const D3D12_COMPUTE_PIPELINE_STATE_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // SAFETY: the API requires a description.
        let d = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        let root = d.pRootSignature.as_ref().map(|r| {
            // SAFETY: root signatures come from this library.
            unsafe { r.as_impl() }.0.clone()
        });
        // SAFETY: the API requires valid bytecode.
        let cs = unsafe { code(&d.CS) };
        let id =
            gpu::call(move |dev| dev.create_compute_pipeline(root.as_ref(), &cs)).map_err(hr)?;
        give::<ID3D12PipelineState>(PipelineState(id, Base::child()).into(), riid, out)
    }

    pub fn CreatePipelineState(
        &self,
        pdesc: *const D3D12_PIPELINE_STATE_STREAM_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        // SAFETY: the API requires a description of a valid stream.
        let desc = unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        match unsafe { crate::stream::read(desc) }? {
            crate::stream::Pipeline::Graphics(g) => {
                self.CreateGraphicsPipelineState(&*g, riid, out)
            }
            crate::stream::Pipeline::Compute(c) => self.CreateComputePipelineState(&c, riid, out),
        }
    }

    /// The front reports no pipeline libraries (`SHADER_CACHE` answers no `LIBRARY`), so this
    /// answers `DXGI_ERROR_UNSUPPORTED`, as Direct3D 12 does where a driver has none.
    pub fn CreatePipelineLibrary(
        &self,
        _blob: *const c_void,
        _length: usize,
        _riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        Err(DXGI_ERROR_UNSUPPORTED.into())
    }

    pub fn SetEventOnMultipleFenceCompletion(
        &self,
        fences: *const Option<ID3D12Fence>,
        values: *const u64,
        count: u32,
        flags: D3D12_MULTIPLE_FENCE_WAIT_FLAGS,
        event: HANDLE,
    ) -> Result<()> {
        let any = match flags.0 {
            _ if count == 0 => return Err(E_INVALIDARG.into()),
            0 => false,
            1 => true,
            _ => return Err(E_INVALIDARG.into()),
        };
        // SAFETY: `count` fences and values, as the API requires.
        let (fences, values) = unsafe { (many(fences, count), many(values, count)) };
        let mut waits = Vec::with_capacity(fences.len());
        for (f, &value) in fences.iter().zip(values) {
            // SAFETY: fences come from this library.
            waits.push((
                unsafe { f.as_ref().ok_or(E_INVALIDARG)?.as_impl() }.id,
                value,
            ));
        }
        fence_event(waits, any, event)
    }

    /// Resources are always resident, so residency calls have nothing to do.
    pub fn MakeResident(&self, _count: u32, _objects: *const Option<ID3D12Pageable>) -> Result<()> {
        Ok(())
    }

    pub fn Evict(&self, _count: u32, _objects: *const Option<ID3D12Pageable>) -> Result<()> {
        Ok(())
    }

    pub fn SetResidencyPriority(
        &self,
        _count: u32,
        _objects: *const Option<ID3D12Pageable>,
        _priorities: *const D3D12_RESIDENCY_PRIORITY,
    ) -> Result<()> {
        Ok(())
    }

    /// The objects are resident already, so the fence is signalled at once.
    pub fn EnqueueMakeResident(
        &self,
        _flags: D3D12_RESIDENCY_FLAGS,
        _count: u32,
        _objects: *const Option<ID3D12Pageable>,
        fence: Ref<ID3D12Fence>,
        value: u64,
    ) -> Result<()> {
        // SAFETY: fences come from this library.
        unsafe { fence.ok()?.as_impl() }.Signal(value)
    }

    /// A list made closed, with no allocator, as `ID3D12Device4` makes them.
    pub fn CreateCommandList1(
        &self,
        _nodemask: u32,
        r#type: D3D12_COMMAND_LIST_TYPE,
        flags: D3D12_COMMAND_LIST_FLAGS,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        list_type(r#type)?;
        if flags.0 != 0 {
            return Err(E_INVALIDARG.into());
        }
        let mut rec = CommandList::new();
        rec.close().map_err(hr)?;
        let list = List {
            kind: r#type,
            rec: Mutex::new(rec),
            allocator: Mutex::new(None),
            pass_end: Mutex::new(Vec::new()),
            base: Base::child(),
        };
        give::<ID3D12GraphicsCommandList4>(list.into(), riid, out)
    }

    /// No protected session can exist (`PROTECTED_RESOURCE_SESSION_SUPPORT` answers none).
    #[allow(clippy::too_many_arguments)]
    pub fn CreateCommittedResource1(
        &self,
        heap: *const D3D12_HEAP_PROPERTIES,
        flags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC,
        state: D3D12_RESOURCE_STATES,
        clear: *const D3D12_CLEAR_VALUE,
        session: Ref<ID3D12ProtectedResourceSession>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if session.is_some() {
            clear_out(out);
            return Err(E_INVALIDARG.into());
        }
        self.CreateCommittedResource(heap, flags, pdesc, state, clear, riid, out)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn CreateCommittedResource2(
        &self,
        heap: *const D3D12_HEAP_PROPERTIES,
        flags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC1,
        state: D3D12_RESOURCE_STATES,
        clear: *const D3D12_CLEAR_VALUE,
        session: Ref<ID3D12ProtectedResourceSession>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        // SAFETY: the API requires a description.
        let desc = desc_from_desc1(unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?)?;
        self.CreateCommittedResource1(heap, flags, &desc, state, clear, session, riid, out)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn CreatePlacedResource1(
        &self,
        heap: Ref<ID3D12Heap>,
        offset: u64,
        pdesc: *const D3D12_RESOURCE_DESC1,
        state: D3D12_RESOURCE_STATES,
        clear: *const D3D12_CLEAR_VALUE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        // SAFETY: the API requires a description.
        let desc = desc_from_desc1(unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?)?;
        self.CreatePlacedResource(heap, offset, &desc, state, clear, riid, out)
    }

    pub fn CreateHeap1(
        &self,
        pdesc: *const D3D12_HEAP_DESC,
        session: Ref<ID3D12ProtectedResourceSession>,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if session.is_some() {
            clear_out(out);
            return Err(E_INVALIDARG.into());
        }
        self.CreateHeap(pdesc, riid, out)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn GetCopyableFootprints1(
        &self,
        pdesc: *const D3D12_RESOURCE_DESC1,
        first: u32,
        count: u32,
        base: u64,
        layouts: *mut D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
        rows: *mut u32,
        row_sizes: *mut u64,
        total: *mut u64,
    ) {
        // SAFETY: the API requires a description.
        let desc = unsafe { pdesc.as_ref() }.and_then(|d| desc_from_desc1(d).ok());
        let pdesc = desc.as_ref().map_or(std::ptr::null(), |d| d as *const _);
        self.GetCopyableFootprints(pdesc, first, count, base, layouts, rows, row_sizes, total)
    }

    /// No meta command exists.
    pub fn EnumerateMetaCommands(
        &self,
        count: *mut u32,
        _descs: *mut D3D12_META_COMMAND_DESC,
    ) -> Result<()> {
        // SAFETY: the program's count, as the API requires.
        *unsafe { count.as_mut() }.ok_or(E_INVALIDARG)? = 0;
        Ok(())
    }

    pub fn EnumerateMetaCommandParameters(
        &self,
        _command: *const GUID,
        _stage: D3D12_META_COMMAND_PARAMETER_STAGE,
        _total: *mut u32,
        _count: *mut u32,
        _descs: *mut D3D12_META_COMMAND_PARAMETER_DESC,
    ) -> Result<()> {
        Err(E_INVALIDARG.into())
    }

    pub fn CreateMetaCommand(
        &self,
        _command: *const GUID,
        _nodemask: u32,
        _data: *const c_void,
        _length: usize,
        _riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        Err(E_INVALIDARG.into())
    }

    /// The only serialized data is ray tracing's, which `D3D12_OPTIONS5` reports absent.
    pub fn CheckDriverMatchingIdentifier(
        &self,
        _kind: D3D12_SERIALIZED_DATA_TYPE,
        _identifier: *const D3D12_SERIALIZED_DATA_DRIVER_MATCHING_IDENTIFIER,
    ) -> D3D12_DRIVER_MATCHING_IDENTIFIER_STATUS {
        D3D12_DRIVER_MATCHING_IDENTIFIER_UNSUPPORTED_TYPE
    }

    pub fn RemoveDevice(&self) {
        gpu::call(|d| d.remove(Error::DeviceRemoved));
    }

    /// The front does no background work, so there is none to finish or measure.
    pub fn SetBackgroundProcessingMode(
        &self,
        _mode: D3D12_BACKGROUND_PROCESSING_MODE,
        _action: D3D12_MEASUREMENTS_ACTION,
        event: HANDLE,
        further: *mut BOOL,
    ) -> Result<()> {
        // SAFETY: the program's flag, when given.
        if let Some(f) = unsafe { further.as_mut() } {
            *f = BOOL(0);
        }
        if !event.is_invalid() && !event.0.is_null() {
            // SAFETY: the program's event.
            unsafe { windows::Win32::System::Threading::SetEvent(event)? };
        }
        Ok(())
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
                let r = unsafe { ours(r) };
                let layers = u32::from(r.desc.DepthOrArraySize);
                // SAFETY: an optional description, as the API allows.
                match unsafe { pdesc.as_ref() } {
                    // The whole of the top level: every slice of an array or a 3D texture.
                    None => Some(TargetView {
                        resource: r.id,
                        format: r.desc.Format.0 as u32,
                        mip: 0,
                        first_layer: 0,
                        layers: layers.max(1),
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
                            layers: t.ArraySize.min(layers.saturating_sub(t.FirstArraySlice)),
                        })
                    }
                    Some(v) if v.ViewDimension == D3D12_RTV_DIMENSION_TEXTURE2DMS => {
                        Some(TargetView {
                            resource: r.id,
                            format: v.Format.0 as u32,
                            mip: 0,
                            first_layer: 0,
                            layers: 1,
                        })
                    }
                    Some(v) if v.ViewDimension == D3D12_RTV_DIMENSION_TEXTURE2DMSARRAY => {
                        // SAFETY: the member the dimension names.
                        let t = unsafe { v.Anonymous.Texture2DMSArray };
                        Some(TargetView {
                            resource: r.id,
                            format: v.Format.0 as u32,
                            mip: 0,
                            first_layer: t.FirstArraySlice,
                            layers: t.ArraySize.min(layers.saturating_sub(t.FirstArraySlice)),
                        })
                    }
                    Some(v) if v.ViewDimension == D3D12_RTV_DIMENSION_TEXTURE3D => {
                        // SAFETY: the member the dimension names.
                        let t = unsafe { v.Anonymous.Texture3D };
                        let depth = (u32::from(r.desc.DepthOrArraySize) >> t.MipSlice).max(1);
                        Some(TargetView {
                            resource: r.id,
                            format: v.Format.0 as u32,
                            mip: t.MipSlice,
                            first_layer: t.FirstWSlice,
                            // All slices from the first, for a size of -1.
                            layers: t.WSize.min(depth.saturating_sub(t.FirstWSlice)),
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
        let descriptor = match (descriptor, resource.as_ref()) {
            (Some(v), Some(r)) => {
                // SAFETY: resources come from this library.
                let r = unsafe { ours(r) };
                // A view of format UNKNOWN reads the resource's own format.
                let format = if v.format == 0 {
                    r.desc.Format.0 as u32
                } else {
                    v.format
                };
                Descriptor::RenderTarget(TargetView {
                    format,
                    resource: holder(r, format),
                    ..v
                })
            }
            _ => Descriptor::Empty,
        };
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
            let r = unsafe { ours(r) };
            let own = r.desc.Format.0 as u32;
            // SAFETY: an optional description, as the API allows.
            let (format, mip, first_layer, layers, read_only) = match unsafe { pdesc.as_ref() } {
                // The whole of the top level, every slice of an array.
                None => (own, 0, 0, u32::from(r.desc.DepthOrArraySize.max(1)), 0),
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
                            D3D12_DSV_DIMENSION_TEXTURE2DMSARRAY => {
                                let t = v.Anonymous.Texture2DMSArray;
                                (0, t.FirstArraySlice, t.ArraySize)
                            }
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
        // SAFETY: an optional description, as the API allows. Address zero makes a null view.
        let descriptor = match unsafe { pdesc.as_ref() } {
            Some(d) if d.BufferLocation != 0 => Descriptor::ConstantBuffer {
                address: d.BufferLocation,
                size: d.SizeInBytes,
            },
            _ => Descriptor::Empty,
        };
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
            let (r, d) = unsafe { (ours(r), pdesc.as_ref()) };
            shader_resource_view(r, d).map(|v| {
                Descriptor::ShaderResource(ResourceView {
                    resource: holder(r, v.format),
                    ..v
                })
            })
        });
        // SAFETY: CPU descriptor handles point into a heap of this library.
        unsafe { *(dest.ptr as *mut Descriptor) = descriptor.unwrap_or_default() };
    }

    pub fn CreateUnorderedAccessView(
        &self,
        resource: Ref<ID3D12Resource>,
        counter: Ref<ID3D12Resource>,
        pdesc: *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
        dest: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        let descriptor = resource.as_ref().and_then(|r| {
            // SAFETY: resources come from this library; an optional description.
            let (r, d) = unsafe { (ours(r), pdesc.as_ref()) };
            // A buffer view's counter lies at its offset in the counter resource.
            let counter = match (counter.as_ref(), d) {
                (Some(c), Some(d)) if d.ViewDimension == D3D12_UAV_DIMENSION_BUFFER => {
                    // SAFETY: resources come from this library; the member the dimension names.
                    unsafe { ours(c).address + d.Anonymous.Buffer.CounterOffsetInBytes }
                }
                _ => 0,
            };
            unordered_access_view(r, d).map(|v| {
                Descriptor::UnorderedAccess(
                    ResourceView {
                        resource: holder(r, v.format),
                        ..v
                    }
                    .with_counter(counter),
                )
            })
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

    pub fn CreateHeap(
        &self,
        pdesc: *const D3D12_HEAP_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        // SAFETY: the API requires a description.
        let mut desc = *unsafe { pdesc.as_ref() }.ok_or(E_INVALIDARG)?;
        let kind = heap_kind(&desc.Properties)?;
        // Zeros read as the defaults: the one node, and 64 KiB alignment.
        let p = &mut desc.Properties;
        p.CreationNodeMask = p.CreationNodeMask.max(1);
        p.VisibleNodeMask = p.VisibleNodeMask.max(1);
        let (size, align, flags) = (desc.SizeInBytes, desc.Alignment, desc.Flags.0 as u32);
        if desc.Alignment == 0 {
            desc.Alignment = 64 << 10;
        }
        let id = gpu::call(move |d| d.create_heap(size, align, kind, flags)).map_err(hr)?;
        give::<ID3D12Heap>(
            crate::abi::fixed(Heap(desc, id, Base::child(), AtomicU32::new(1)).into()),
            riid,
            out,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn CreatePlacedResource(
        &self,
        heap: Ref<ID3D12Heap>,
        offset: u64,
        pdesc: *const D3D12_RESOURCE_DESC,
        state: D3D12_RESOURCE_STATES,
        clear: *const D3D12_CLEAR_VALUE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        let heap_interface = heap.ok()?;
        // SAFETY: heaps come from this library; the API requires a description.
        let (heap, desc) = unsafe {
            (
                heap_interface.as_impl(),
                *pdesc.as_ref().ok_or(E_INVALIDARG)?,
            )
        };
        let (memory, heap_desc) = (heap.1, heap.0);
        let kind = heap_kind(&heap_desc.Properties)?;
        let front = resource_desc(&desc);
        check_creation(kind, &front, state.0 as u32, !clear.is_null()).map_err(hr)?;
        let (id, address, host) = gpu::call(move |d| {
            let id = d.create_placed(memory, offset, &front)?;
            let host = if kind != HeapType::Default && front.dimension == DIMENSION_BUFFER {
                d.map(id)? as usize
            } else {
                0
            };
            Ok::<_, Error>((id, d.address(id), host))
        })
        .map_err(hr)?;
        let mut desc = desc;
        if desc.MipLevels == 0 {
            desc.MipLevels = front.mips() as u16;
        }
        give::<ID3D12Resource2>(
            crate::abi::fixed(
                Resource {
                    id,
                    desc,
                    address,
                    host,
                    back_buffer: false,
                    heap: heap_desc.Properties,
                    heap_flags: heap_desc.Flags,
                    cpu_maps: cpu_texture(kind, &front).then(AtomicU32::default),
                    _chain: None,
                    _heap: Some(HeapHold::new(heap_interface)),
                    base: Base::child(),
                }
                .into(),
            ),
            riid,
            out,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn CreateCommittedResource(
        &self,
        heap: *const D3D12_HEAP_PROPERTIES,
        flags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC,
        // The initial state is checked; it and the optimised clear value change nothing else.
        state: D3D12_RESOURCE_STATES,
        clear: *const D3D12_CLEAR_VALUE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        wcontainers_direct3d_12::resource::check_heap_flags(flags.0 as u32, true).map_err(hr)?;
        // SAFETY: the API requires both.
        let (heap, desc) = unsafe {
            (
                *heap.as_ref().ok_or(E_INVALIDARG)?,
                *pdesc.as_ref().ok_or(E_INVALIDARG)?,
            )
        };
        let kind = heap_kind(&heap)?;
        let front = resource_desc(&desc);
        check_creation(kind, &front, state.0 as u32, !clear.is_null()).map_err(hr)?;
        let (id, address, host) = gpu::call(move |d| {
            let id = d.create_committed(kind, &front)?;
            let host = if kind != HeapType::Default && front.dimension == DIMENSION_BUFFER {
                d.map(id)? as usize
            } else {
                0
            };
            Ok::<_, Error>((id, d.address(id), host))
        })
        .map_err(hr)?;
        let mut desc = desc;
        if desc.MipLevels == 0 {
            desc.MipLevels = front.mips() as u16;
        }
        give::<ID3D12Resource2>(
            crate::abi::fixed(
                Resource {
                    id,
                    desc,
                    address,
                    host,
                    back_buffer: false,
                    heap,
                    heap_flags: flags,
                    cpu_maps: cpu_texture(kind, &front).then(AtomicU32::default),
                    _chain: None,
                    _heap: None,
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

    /// A call breaking the API's rules removes the device with `DXGI_ERROR_INVALID_CALL`, as
    /// Direct3D 12 does; work the front cannot carry out with `DXGI_ERROR_DRIVER_INTERNAL_ERROR`.
    pub fn GetDeviceRemovedReason(&self) -> Result<()> {
        match gpu::call(|d| d.removed()) {
            None => Ok(()),
            Some(Error::Invalid(why)) => {
                Err(windows::core::Error::new(DXGI_ERROR_INVALID_CALL, why))
            }
            Some(Error::DeviceRemoved) => Err(DXGI_ERROR_DEVICE_REMOVED.into()),
            Some(e) => Err(windows::core::Error::new(
                DXGI_ERROR_DRIVER_INTERNAL_ERROR,
                format!("{e:?}"),
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
            // Copy queues write timestamps as direct ones do; the other fields are abilities
            // the front does not have (view instancing, barycentrics, full casting, immediate
            // writes reported by list type).
            D3D12_FEATURE_D3D12_OPTIONS3 => {
                let timestamps = gpu::call(|d| {
                    d.backend()
                        .caps()
                        .has(wcontainers_gpu::Features::TIMESTAMPS)
                });
                *unsafe { fill::<D3D12_FEATURE_DATA_D3D12_OPTIONS3>(data, size)? } =
                    D3D12_FEATURE_DATA_D3D12_OPTIONS3 {
                        CopyQueueTimestampQueriesSupported: timestamps.into(),
                        ..Default::default()
                    };
            }
            // Every field of these blocks is an ability the front does not have (plan section
            // 9: 16-bit shader operations, ray tracing, variable rate shading, mesh shaders,
            // sampler feedback), and zero says absent.
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
                d.HighestShaderModel = D3D_SHADER_MODEL(
                    features::shader_model(d.HighestShaderModel.0 as u32).map_err(hr)? as i32,
                );
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
                let mut s = format_support(&caps, d.Format.0 as u32);
                if features::typed_buffer(&caps, d.Format.0 as u32) {
                    s.support1 |= features::SUPPORT1_TYPED_BUFFER;
                }
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
            // Abilities the front lacks: protected sessions, cached pipeline blobs, existing heaps,
            // heap serialization, sharing across nodes, displayable textures and meta commands.
            D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_SUPPORT => {
                let d = unsafe {
                    fill::<D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_SUPPORT>(data, size)?
                };
                if d.NodeIndex != 0 {
                    return Err(E_INVALIDARG.into());
                }
                d.Support = Default::default();
            }
            D3D12_FEATURE_SHADER_CACHE => {
                *unsafe { fill::<D3D12_FEATURE_DATA_SHADER_CACHE>(data, size)? } =
                    Default::default();
            }
            // One submission order serves every queue, so priorities are taken and change
            // nothing; real-time priority needs privileges a program does not have here.
            D3D12_FEATURE_COMMAND_QUEUE_PRIORITY => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_COMMAND_QUEUE_PRIORITY>(data, size)? };
                list_type(d.CommandListType)?;
                d.PriorityForTypeIsSupported = b(matches!(
                    D3D12_COMMAND_QUEUE_PRIORITY(d.Priority as i32),
                    D3D12_COMMAND_QUEUE_PRIORITY_NORMAL | D3D12_COMMAND_QUEUE_PRIORITY_HIGH
                ));
            }
            D3D12_FEATURE_EXISTING_HEAPS => {
                *unsafe { fill::<D3D12_FEATURE_DATA_EXISTING_HEAPS>(data, size)? } =
                    Default::default();
            }
            D3D12_FEATURE_SERIALIZATION => {
                let d = unsafe { fill::<D3D12_FEATURE_DATA_SERIALIZATION>(data, size)? };
                if d.NodeIndex != 0 {
                    return Err(E_INVALIDARG.into());
                }
                d.HeapSerializationTier = Default::default();
            }
            D3D12_FEATURE_CROSS_NODE => {
                *unsafe { fill::<D3D12_FEATURE_DATA_CROSS_NODE>(data, size)? } = Default::default();
            }
            D3D12_FEATURE_DISPLAYABLE => {
                *unsafe { fill::<D3D12_FEATURE_DATA_DISPLAYABLE>(data, size)? } =
                    Default::default();
            }
            // No meta command exists, so every command named is unknown.
            D3D12_FEATURE_QUERY_META_COMMAND => return Err(E_INVALIDARG.into()),
            D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPE_COUNT => {
                let d = unsafe {
                    fill::<D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_TYPE_COUNT>(data, size)?
                };
                if d.NodeIndex != 0 {
                    return Err(E_INVALIDARG.into());
                }
                d.Count = 0;
            }
            D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPES => {
                let d = unsafe {
                    fill::<D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_TYPES>(data, size)?
                };
                if d.NodeIndex != 0 || d.Count != 0 {
                    return Err(E_INVALIDARG.into());
                }
            }
            // Features past `D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_TYPES` (34) are unknown to
            // the Windows 10 runtime this library stands in for, which refuses them and values
            // naming no feature so.
            _ => return Err(E_INVALIDARG.into()),
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
                // Every output is all ones, as Direct3D 12 answers a description it refuses.
                Err(_) => {
                    for i in 0..count as usize {
                        if !layouts.is_null() {
                            std::ptr::write_bytes(layouts.add(i), 0xff, 1);
                        }
                        if !rows.is_null() {
                            *rows.add(i) = u32::MAX;
                        }
                        if !row_sizes.is_null() {
                            *row_sizes.add(i) = u64::MAX;
                        }
                    }
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
pub struct Queue(D3D12_COMMAND_QUEUE_DESC, Base, QueueId);

impl Drop for Queue {
    fn drop(&mut self) {
        let id = self.2;
        gpu::call(move |d| d.destroy_queue(id));
    }
}

impl Queue {
    /// The description, with the one node the device has in the node mask.
    pub fn GetDesc(&self) -> D3D12_COMMAND_QUEUE_DESC {
        D3D12_COMMAND_QUEUE_DESC {
            NodeMask: self.0.NodeMask.max(1),
            ..self.0
        }
    }

    pub fn ExecuteCommandLists(&self, count: u32, lists: *const Option<ID3D12CommandList>) {
        if lists.is_null() {
            return;
        }
        // SAFETY: `count` lists, as the API requires.
        let lists = unsafe { std::slice::from_raw_parts(lists, count as usize) };
        let mut commands = Vec::with_capacity(lists.len());
        for l in lists.iter().flatten() {
            let Ok(l) = l.cast::<ID3D12GraphicsCommandList4>() else {
                self.unsupported("ExecuteCommandLists of a list of another kind");
                return;
            };
            // SAFETY: command lists come from this library.
            let list = unsafe { l.as_impl() };
            match list.rec.lock().unwrap().commands() {
                Some(c) => commands.push(Arc::clone(c)),
                None => {
                    eprintln!("wcontainers: ExecuteCommandLists of a list that is not closed");
                    gpu::call(|d| {
                        d.remove(Error::Invalid(
                            "ExecuteCommandLists of a list that is not closed",
                        ))
                    });
                    return;
                }
            }
        }
        let queue = self.2;
        if gpu::call(move |d| d.execute(queue, &commands)).is_err() {
            eprintln!(
                "wcontainers: the device was removed: {:?}",
                gpu::call(|d| d.removed())
            );
        }
    }

    pub fn Signal(&self, fence: Ref<ID3D12Fence>, value: u64) -> Result<()> {
        // SAFETY: fences come from this library.
        let fence = unsafe { fence.ok()?.as_impl() };
        let (queue, id) = (self.2, fence.id);
        gpu::call(move |d| d.queue_signal(queue, id, value)).map_err(hr)
    }

    /// Timestamps count nanoseconds where the back end has them.
    pub fn GetTimestampFrequency(&self) -> Result<u64> {
        if gpu::call(|d| {
            d.backend()
                .caps()
                .has(wcontainers_gpu::Features::TIMESTAMPS)
        }) {
            Ok(1_000_000_000)
        } else {
            Err(E_FAIL.into())
        }
    }

    /// The GPU's timestamp and `QueryPerformanceCounter` at about one moment; E_FAIL as for a
    /// queue without timestamps where the back end has none.
    pub fn GetClockCalibration(&self, gpu: *mut u64, cpu: *mut u64) -> Result<()> {
        if gpu.is_null() || cpu.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let (g, c) = gpu::call(|d| {
            if !d
                .backend()
                .caps()
                .has(wcontainers_gpu::Features::TIMESTAMPS)
            {
                return Ok(None);
            }
            d.clock_calibration(|| {
                let mut now = 0i64;
                // SAFETY: writes the counter into a local.
                let _ = unsafe {
                    windows::Win32::System::Performance::QueryPerformanceCounter(&mut now)
                };
                now as u64
            })
            .map(Some)
        })
        .map_err(hr)?
        .ok_or(windows::core::Error::from(E_FAIL))?;
        // SAFETY: the caller's two out pointers, checked above.
        unsafe {
            *gpu = g;
            *cpu = c;
        }
        Ok(())
    }

    pub fn Wait(&self, fence: Ref<ID3D12Fence>, value: u64) -> Result<()> {
        // SAFETY: fences come from this library.
        let fence = unsafe { fence.ok()?.as_impl() };
        let (queue, id) = (self.2, fence.id);
        gpu::call(move |d| d.queue_wait(queue, id, value)).map_err(hr)
    }
}

#[implement(ID3D12CommandAllocator)]
pub struct Allocator {
    kind: D3D12_COMMAND_LIST_TYPE,
    /// The list recording from this allocator (its address), or 0: one at a time, as the API
    /// requires.
    user: Arc<AtomicUsize>,
    base: Base,
}

impl Allocator {
    /// Lists keep their own commands, so there is nothing to free; a list still recording from the
    /// allocator makes this fail, as Direct3D 12 does.
    pub fn Reset(&self) -> Result<()> {
        if self.user.load(Ordering::Acquire) != 0 {
            return Err(E_FAIL.into());
        }
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

    pub fn SetEventOnCompletion(&self, value: u64, event: HANDLE) -> Result<()> {
        fence_event(vec![(self.id, value)], false, event)
    }

    /// Events this value reaches are set before the call returns, as Direct3D 12 does.
    pub fn Signal(&self, value: u64) -> Result<()> {
        let id = self.id;
        gpu::call(move |d| {
            d.cpu_signal(id, value);
            set_reached(d);
        });
        Ok(())
    }
}

/// With an event, the device thread sets it once every fence (or, with `any`, one of them)
/// reaches its value, and nothing waits here. With none, the call itself waits, as the API
/// defines.
fn fence_event(waits: Vec<(FenceId, u64)>, any: bool, event: HANDLE) -> Result<()> {
    if event.is_invalid() || event.0.is_null() {
        // No event: the call returns when the fences reach their values. The calling thread waits
        // on an event of its own, so the device thread stays free for the signal (which may be a
        // CPU signal from another thread).
        use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
        // SAFETY: an unnamed auto-reset event, closed below.
        let own = unsafe { CreateEventW(None, false, false, None)? };
        let token = own.0 as usize as u64;
        gpu::call(move |d| {
            d.add_waiter_many(waits, any, token);
            set_reached(d);
        });
        // SAFETY: `own` is open until closed here.
        unsafe {
            WaitForSingleObject(own, INFINITE);
            let _ = CloseHandle(own);
        }
        return Ok(());
    }
    let token = event.0 as usize as u64;
    // Values already reached set the event before the call returns.
    gpu::call(move |d| {
        d.add_waiter_many(waits, any, token);
        set_reached(d);
    });
    Ok(())
}

/// Sets the events whose fences have reached their values.
fn set_reached(d: &mut gpu::Front) {
    for event in d.reached_events() {
        // SAFETY: the event the program gave to wait on a fence.
        let _ = unsafe {
            windows::Win32::System::Threading::SetEvent(HANDLE(event as usize as *mut _))
        };
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        let id = self.id;
        gpu::call(move |d| d.destroy_fence(id));
    }
}

// ---- Resources and descriptors ----

#[implement(ID3D12Resource2)]
pub struct Resource {
    id: ResourceId,
    desc: D3D12_RESOURCE_DESC,
    address: u64,
    /// The memory of an upload or readback buffer, as a number.
    host: usize,
    /// A swap chain's back buffer, which the swap chain releases.
    back_buffer: bool,
    /// The heap's properties and the flags the resource was made with.
    heap: D3D12_HEAP_PROPERTIES,
    heap_flags: D3D12_HEAP_FLAGS,
    /// A texture in a custom heap the CPU sees: how many times it is mapped.
    cpu_maps: Option<AtomicU32>,
    /// A back buffer's share of its swap chain's reference.
    _chain: Option<Arc<crate::present::BufferHold>>,
    /// A placed resource's reference on its heap.
    _heap: Option<HeapHold>,
    base: Base,
}

/// Sets a creation call's out pointer to null, as Direct3D 12 leaves it when the call fails.
fn clear_out(out: *mut *mut c_void) {
    if !out.is_null() {
        // SAFETY: the caller's out pointer.
        unsafe { *out = std::ptr::null_mut() };
    }
}

/// The front's heap type for heap properties.
fn heap_kind(p: &D3D12_HEAP_PROPERTIES) -> Result<HeapType> {
    if p.Type == D3D12_HEAP_TYPE_CUSTOM {
        return wcontainers_direct3d_12::resource::custom_heap(
            p.CPUPageProperty.0 as u32,
            p.MemoryPoolPreference.0 as u32,
        )
        .map_err(hr);
    }
    HeapType::from_d3d12(p.Type.0 as u32).map_err(hr)
}

/// A texture in a custom heap the CPU sees, which maps with no pointer.
fn cpu_texture(kind: HeapType, desc: &ResourceDesc) -> bool {
    kind == HeapType::Shared && desc.dimension != DIMENSION_BUFFER
}

impl Resource {
    pub fn back_buffer(
        id: ResourceId,
        chain: Arc<crate::present::BufferHold>,
        desc: D3D12_RESOURCE_DESC,
    ) -> ID3D12Resource {
        let resource: ID3D12Resource2 = crate::abi::fixed(
            Resource {
                id,
                desc,
                address: 0,
                host: 0,
                back_buffer: true,
                heap: D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                heap_flags: D3D12_HEAP_FLAG_NONE,
                cpu_maps: None,
                _chain: Some(chain),
                _heap: None,
                base: Base::child(),
            }
            .into(),
        );
        resource.into()
    }

    /// No resource of this front has a protected session.
    pub fn GetProtectedResourceSession(
        &self,
        _riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        clear_out(out);
        Err(DXGI_ERROR_NOT_FOUND.into())
    }

    /// The description with an empty sampler feedback region (`D3D12_OPTIONS7` reports no
    /// sampler feedback).
    pub fn GetDesc1(&self) -> D3D12_RESOURCE_DESC1 {
        let d = self.desc;
        D3D12_RESOURCE_DESC1 {
            Dimension: d.Dimension,
            Alignment: d.Alignment,
            Width: d.Width,
            Height: d.Height,
            DepthOrArraySize: d.DepthOrArraySize,
            MipLevels: d.MipLevels,
            Format: d.Format,
            SampleDesc: d.SampleDesc,
            Layout: d.Layout,
            Flags: d.Flags,
            SamplerFeedbackMipRegion: Default::default(),
        }
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
                    CreationNodeMask: 1,
                    VisibleNodeMask: 1,
                    ..self.heap
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
        subresource: u32,
        _range: *const D3D12_RANGE,
        out: *mut *mut c_void,
    ) -> Result<()> {
        // A texture the CPU sees maps with no pointer, for `WriteToSubresource` and
        // `ReadFromSubresource`.
        if let Some(maps) = &self.cpu_maps {
            if !out.is_null()
                || subresource >= resource_desc(&self.desc).subresources().unwrap_or(0)
            {
                return Err(E_INVALIDARG.into());
            }
            maps.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        if subresource != 0 {
            return Err(E_INVALIDARG.into());
        }
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
    pub fn Unmap(&self, _subresource: u32, _range: *const D3D12_RANGE) {
        if let Some(maps) = &self.cpu_maps {
            let _ = maps.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
        }
    }

    /// The checks `WriteToSubresource` and `ReadFromSubresource` share: a texture in a heap the CPU
    /// sees, a subresource it has, and a box inside it on block edges. The box's origin and extent
    /// with the bytes a caller's layout of it spans, or `None` for an empty box, which moves
    /// nothing. Real Direct3D 12 does not ask for a `Map` first, whatever the documentation says.
    fn cpu_box(
        &self,
        subresource: u32,
        pbox: *const D3D12_BOX,
        row_pitch: u32,
        depth_pitch: u32,
    ) -> Result<Option<(Origin, Extent, usize)>> {
        let desc = resource_desc(&self.desc);
        if self.cpu_maps.is_none() || subresource >= desc.subresources().map_err(hr)? {
            return Err(E_INVALIDARG.into());
        }
        let mip = subresource % desc.mips();
        let size = |n: u64| (n >> mip).max(1) as u32;
        let (w, h) = (size(desc.width), size(u64::from(desc.height)));
        let d = if desc.dimension == wcontainers_direct3d_12::resource::DIMENSION_TEXTURE3D {
            size(u64::from(desc.depth_or_array_size))
        } else {
            1
        };
        let (bw, bh, bytes) = desc.texture_format().map_err(hr)?.0.block();
        // SAFETY: an optional box, as the API allows.
        let b = unsafe { pbox.as_ref() }.copied().unwrap_or(D3D12_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: w,
            bottom: h,
            back: d,
        });
        let edge = |v: u32, block: u32, end: u32| v.is_multiple_of(block) || v == end;
        if b.right > w
            || b.bottom > h
            || b.back > d
            || !b.left.is_multiple_of(bw)
            || !b.top.is_multiple_of(bh)
            || !edge(b.right, bw, w)
            || !edge(b.bottom, bh, h)
        {
            return Err(E_INVALIDARG.into());
        }
        if b.left >= b.right || b.top >= b.bottom || b.front >= b.back {
            return Ok(None);
        }
        let extent = Extent {
            width: b.right - b.left,
            height: b.bottom - b.top,
            depth_or_layers: b.back - b.front,
        };
        let row = (extent.width.div_ceil(bw) * bytes) as usize;
        let rows = extent.height.div_ceil(bh) as usize;
        let span = (extent.depth_or_layers as usize - 1) * depth_pitch as usize
            + (rows - 1) * row_pitch as usize
            + row;
        let origin = Origin {
            x: b.left,
            y: b.top,
            z: b.front,
        };
        Ok(Some((origin, extent, span)))
    }

    pub fn WriteToSubresource(
        &self,
        subresource: u32,
        pbox: *const D3D12_BOX,
        data: *const c_void,
        row_pitch: u32,
        depth_pitch: u32,
    ) -> Result<()> {
        let Some((origin, extent, span)) =
            self.cpu_box(subresource, pbox, row_pitch, depth_pitch)?
        else {
            return Ok(());
        };
        let (id, data) = (self.id, data as usize);
        gpu::call(move |d| {
            // SAFETY: the caller's bytes of the layout it gave, read before the call returns.
            let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, span) };
            d.write_subresource(
                id,
                subresource,
                origin,
                extent,
                bytes,
                (row_pitch, depth_pitch),
            )
        })
        .map_err(hr)
    }

    pub fn ReadFromSubresource(
        &self,
        data: *mut c_void,
        row_pitch: u32,
        depth_pitch: u32,
        subresource: u32,
        pbox: *const D3D12_BOX,
    ) -> Result<()> {
        let Some((origin, extent, span)) =
            self.cpu_box(subresource, pbox, row_pitch, depth_pitch)?
        else {
            return Ok(());
        };
        let (id, data) = (self.id, data as usize);
        gpu::call(move |d| {
            // SAFETY: the caller's memory of the layout it gave, written before the call returns.
            let bytes = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, span) };
            d.read_subresource(
                id,
                subresource,
                origin,
                extent,
                bytes,
                (row_pitch, depth_pitch),
            )
        })
        .map_err(hr)
    }
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
pub struct RootSignature(pub wcontainers_direct3d_12::root::RootSignature, Base);

/// A heap of `CreateHeap`: its description, the front's heap, and the program's references, which
/// `abi.rs` counts apart from its placed resources' (`HeapHold`).
#[implement(ID3D12Heap)]
pub struct Heap(D3D12_HEAP_DESC, MemoryId, Base, pub AtomicU32);

impl Heap {
    pub fn GetDesc(&self) -> D3D12_HEAP_DESC {
        self.0
    }
}

/// A placed resource's reference on its heap, which keeps the heap alive without counting among
/// the program's references (Direct3D 12 keeps a heap the program released while resources lie in
/// it, and the program may still place resources in it).
pub struct HeapHold(*mut c_void);

// SAFETY: the heap's counts are atomic and it is only read after creation.
unsafe impl Send for HeapHold {}
// SAFETY: as above.
unsafe impl Sync for HeapHold {}

impl HeapHold {
    pub fn new(heap: &ID3D12Heap) -> Self {
        let this = heap.as_raw();
        // SAFETY: a heap of this library, alive while the caller holds it.
        unsafe { crate::abi::heap_hold(this) };
        Self(this)
    }
}

impl Drop for HeapHold {
    fn drop(&mut self) {
        // SAFETY: the reference `new` took.
        unsafe { crate::abi::heap_let_go(self.0) };
    }
}

/// The program's reference count of a heap of this library.
///
/// # Safety
/// `this` is a live heap of this library.
pub unsafe fn heap_program_count(this: &*mut c_void) -> &AtomicU32 {
    // SAFETY: as the caller promises; the interface pointer is borrowed for the call.
    unsafe {
        let heap = ID3D12Heap::from_raw_borrowed(this).expect("a heap");
        &heap.as_impl().3
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        let id = self.1;
        gpu::call(move |d| d.destroy_heap(id));
    }
}

#[implement(ID3D12CommandSignature)]
pub struct CommandSignature(
    pub Arc<wcontainers_direct3d_12::list::CommandSignature>,
    Base,
);

#[implement(ID3D12QueryHeap)]
pub struct QueryHeap(pub QueryHeapId, Base);

impl Drop for QueryHeap {
    fn drop(&mut self) {
        let id = self.0;
        gpu::call(move |d| d.destroy_query_heap(id));
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

#[implement(ID3D12GraphicsCommandList4)]
pub struct List {
    kind: D3D12_COMMAND_LIST_TYPE,
    rec: Mutex<CommandList>,
    /// The user slot of the allocator the list records from while it is open.
    allocator: Mutex<Option<Arc<AtomicUsize>>>,
    /// The resolves the open render pass ends with.
    pass_end: Mutex<Vec<Command>>,
    base: Base,
}

impl Drop for List {
    fn drop(&mut self) {
        self.release();
    }
}

/// The descriptor a CPU handle points at.
///
/// # Safety
/// The handle points into a descriptor heap of this library.
unsafe fn descriptor(h: D3D12_CPU_DESCRIPTOR_HANDLE) -> Descriptor {
    // SAFETY: as the caller promises.
    unsafe { *(h.ptr as *const Descriptor) }
}

/// The resource a view of `r` in DXGI format `format` names: `r`'s own, or, for a typeless
/// texture viewed in a format the back end cannot reinterpret its bits as, its twin in that format
/// (`Device::view_holder`).
fn holder(r: &Resource, format: u32) -> ResourceId {
    let reinterprets = match (dxgi_format(r.desc.Format.0 as u32), dxgi_format(format)) {
        (Some((stored, true)), Some((view, _))) => stored.linear() != view.linear(),
        _ => false,
    };
    let id = r.id;
    if !reinterprets {
        return id;
    }
    gpu::call(move |d| d.view_holder(id, format)).unwrap_or_else(|e| {
        eprintln!("wcontainers: a view of a typeless texture: {e:?}");
        id
    })
}

/// Our object behind a resource interface of this library.
///
/// # Safety
/// The resource comes from this library.
pub unsafe fn ours(r: &ID3D12Resource) -> &Resource {
    // SAFETY: every resource of this library implements `ID3D12Resource2`, and an interface of a
    // single chain shares its pointer with the interfaces it extends.
    unsafe { std::mem::transmute::<&ID3D12Resource, &ID3D12Resource2>(r).as_impl() }
}

/// The resource behind an interface pointer of this library.
fn id(r: &Ref<ID3D12Resource>) -> Option<ResourceId> {
    // SAFETY: resources come from this library.
    r.as_ref().map(|r| unsafe { ours(r) }.id)
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

/// # Safety
/// `p` is null or points to `n` writable values.
unsafe fn many_mut<'a, T>(p: *mut T, n: u32) -> Option<&'a mut [T]> {
    // SAFETY: as the caller promises.
    (!p.is_null()).then(|| unsafe { std::slice::from_raw_parts_mut(p, n as usize) })
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

    /// Protected sessions are reported absent, so only none can be set.
    pub fn SetProtectedResourceSession(&self, session: Ref<ID3D12ProtectedResourceSession>) {
        if session.is_some() {
            self.unsupported("SetProtectedResourceSession");
        }
    }

    /// A render pass as the calls it stands for: its targets bound, the clears its beginning
    /// accesses ask, and at `EndRenderPass` the resolves its ending accesses ask. Discarding and
    /// preserving accesses keep the contents, which Direct3D 12 allows for both.
    pub fn BeginRenderPass(
        &self,
        count: u32,
        targets: *const D3D12_RENDER_PASS_RENDER_TARGET_DESC,
        depth: *const D3D12_RENDER_PASS_DEPTH_STENCIL_DESC,
        _flags: D3D12_RENDER_PASS_FLAGS,
    ) {
        // SAFETY: `count` descriptions and an optional one, as the API says.
        let (targets, depth) = unsafe { (many(targets, count), depth.as_ref()) };
        let mut end = Vec::new();
        let mut clears = Vec::new();
        for t in targets {
            // SAFETY: a handle of this library's heaps.
            let view = unsafe { descriptor(t.cpuDescriptor) };
            if t.BeginningAccess.Type == D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR {
                // SAFETY: a clear access holds its clear value.
                let color = unsafe { t.BeginningAccess.Anonymous.Clear.ClearValue.Anonymous.Color };
                clears.push(Command::ClearTarget {
                    view,
                    color,
                    rects: Vec::new(),
                });
            }
            if let Err(why) = self.resolves(&t.EndingAccess, &mut end) {
                return self.rec.lock().unwrap().fail(why);
            }
        }
        if let Some(d) = depth {
            // SAFETY: a handle of this library's heaps; clear accesses hold their clear values.
            let view = unsafe { descriptor(d.cpuDescriptor) };
            let clear = |a: &D3D12_RENDER_PASS_BEGINNING_ACCESS| {
                (a.Type == D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR)
                    .then_some(unsafe { a.Anonymous.Clear.ClearValue.Anonymous.DepthStencil })
            };
            let (depth_clear, stencil_clear) = (
                clear(&d.DepthBeginningAccess),
                clear(&d.StencilBeginningAccess),
            );
            if depth_clear.is_some() || stencil_clear.is_some() {
                clears.push(Command::ClearDepth {
                    view,
                    depth: depth_clear.map(|v| v.Depth),
                    stencil: stencil_clear.map(|v| v.Stencil),
                    rects: Vec::new(),
                });
            }
            for a in [&d.DepthEndingAccess, &d.StencilEndingAccess] {
                if a.Type == D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_RESOLVE {
                    self.unsupported("BeginRenderPass resolving a depth-stencil target");
                    return;
                }
            }
        }
        self.record(Command::Targets {
            // SAFETY: handles of this library's heaps.
            colors: targets
                .iter()
                .map(|t| unsafe { descriptor(t.cpuDescriptor) })
                .collect(),
            depth: depth.map(|d| unsafe { descriptor(d.cpuDescriptor) }),
        });
        clears.into_iter().for_each(|c| self.record(c));
        *self.pass_end.lock().unwrap() = end;
    }

    pub fn EndRenderPass(&self) {
        let end = std::mem::take(&mut *self.pass_end.lock().unwrap());
        end.into_iter().for_each(|c| self.record(c));
    }

    /// The resolves an ending access asks: whole subresources averaged (anything else is
    /// refused when the list closes).
    fn resolves(
        &self,
        a: &D3D12_RENDER_PASS_ENDING_ACCESS,
        out: &mut Vec<Command>,
    ) -> std::result::Result<(), Error> {
        if a.Type != D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_RESOLVE {
            return Ok(());
        }
        // SAFETY: a resolve access holds its parameters.
        let r = unsafe { &a.Anonymous.Resolve };
        let (Some(src), Some(dst)) = (r.pSrcResource.as_ref(), r.pDstResource.as_ref()) else {
            return Err(Error::Invalid("a render pass resolve with no resource"));
        };
        if r.ResolveMode != D3D12_RESOLVE_MODE_AVERAGE {
            return Err(Error::NotImplemented(
                "render pass resolves other than averaging",
            ));
        }
        // SAFETY: resources come from this library.
        let (src, dst) = unsafe { (ours(src), ours(dst)) };
        // SAFETY: as many parameters as the access says.
        for p in unsafe { many(r.pSubresourceParameters, r.SubresourceCount) } {
            let whole = p.DstX == 0
                && p.DstY == 0
                && p.SrcRect.left <= 0
                && p.SrcRect.top <= 0
                && p.SrcRect.right as i64 >= src.desc.Width as i64
                && p.SrcRect.bottom as i64 >= i64::from(src.desc.Height);
            if !whole {
                return Err(Error::NotImplemented(
                    "render pass resolves of part of a subresource",
                ));
            }
            out.push(Command::Resolve {
                dst: dst.id,
                dst_index: p.DstSubresource,
                src: src.id,
                src_index: p.SrcSubresource,
                format: r.Format.0 as u32,
            });
        }
        Ok(())
    }

    /// Starts recording from `allocator`: one of this list's type that no other open list
    /// records from.
    fn take(&self, allocator: Ref<ID3D12CommandAllocator>) -> Result<()> {
        // SAFETY: allocators come from this library.
        let a = allocator.as_ref().map(|a| unsafe { a.as_impl() });
        let a = a.filter(|a| a.kind == self.kind).ok_or(E_INVALIDARG)?;
        let me = self as *const List as usize;
        a.user
            .compare_exchange(0, me, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| E_INVALIDARG)?;
        *self.allocator.lock().unwrap() = Some(Arc::clone(&a.user));
        Ok(())
    }

    /// Stops recording: the allocator is free for another list.
    fn release(&self) {
        if let Some(user) = self.allocator.lock().unwrap().take() {
            user.store(0, Ordering::Release);
        }
    }

    /// Ends recording whether or not the list's calls were valid; a failed list must be reset.
    pub fn Close(&self) -> Result<()> {
        self.release();
        self.rec.lock().unwrap().close().map_err(|e| {
            eprintln!("wcontainers: Close: {e:?}");
            hr(e)
        })
    }

    /// Fails with `E_FAIL` while the list is open, as Direct3D 12 does.
    pub fn Reset(
        &self,
        allocator: Ref<ID3D12CommandAllocator>,
        initial: Ref<ID3D12PipelineState>,
    ) -> Result<()> {
        if self.allocator.lock().unwrap().is_some() {
            return Err(E_FAIL.into());
        }
        self.take(allocator)?;
        self.rec.lock().unwrap().reset();
        self.start(initial);
        Ok(())
    }

    /// Records a closed bundle's commands into a direct list.
    pub fn ExecuteBundle(&self, bundle: Ref<ID3D12GraphicsCommandList>) {
        let bundle = bundle
            .as_ref()
            .and_then(|b| b.cast::<ID3D12GraphicsCommandList4>().ok());
        let commands = bundle.as_ref().and_then(|b| {
            // SAFETY: command lists come from this library.
            let b: &List = unsafe { b.as_impl() };
            if b.kind != D3D12_COMMAND_LIST_TYPE_BUNDLE {
                return None;
            }
            b.rec.lock().unwrap().commands().cloned()
        });
        match commands {
            Some(c) if self.kind == D3D12_COMMAND_LIST_TYPE_DIRECT => {
                self.record(Command::Bundle(c));
            }
            _ => self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("ExecuteBundle of no closed bundle")),
        }
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
        let root = root.as_ref().map(|r| unsafe { r.as_impl() }.0.clone());
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
    /// Barriers order nothing here (the device runs work in submission order); transitions are
    /// checked as Direct3D 12 checks them at `Close`.
    pub fn ResourceBarrier(&self, count: u32, barriers: *const D3D12_RESOURCE_BARRIER) {
        // SAFETY: `count` barriers, as the API requires.
        for b in unsafe { many(barriers, count) } {
            if b.Type != D3D12_RESOURCE_BARRIER_TYPE_TRANSITION {
                continue;
            }
            // SAFETY: the member the type names.
            let t = unsafe { &b.Anonymous.Transition };
            let checked = if t.pResource.is_none() {
                Err(Error::Invalid("a transition of no resource"))
            } else if t.StateBefore == t.StateAfter {
                Err(Error::Invalid("a transition into the state it starts in"))
            } else {
                check_state(t.StateBefore.0 as u32).and(check_state(t.StateAfter.0 as u32))
            };
            if let Err(e) = checked {
                self.rec.lock().unwrap().fail(e);
            }
        }
    }

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

    pub fn SOSetTargets(
        &self,
        start: u32,
        count: u32,
        views: *const D3D12_STREAM_OUTPUT_BUFFER_VIEW,
    ) {
        // SAFETY: `count` views, as the API requires; none unbinds.
        let given = unsafe { many(views, count) };
        let views = if given.is_empty() {
            vec![StreamView::default(); count as usize]
        } else {
            given
                .iter()
                .map(|v| StreamView {
                    address: v.BufferLocation,
                    size: v.SizeInBytes,
                    filled: v.BufferFilledSizeLocation,
                })
                .collect()
        };
        self.record(Command::StreamTargets { start, views });
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

    pub fn BeginQuery(&self, heap: Ref<ID3D12QueryHeap>, kind: D3D12_QUERY_TYPE, index: u32) {
        let Some(h) = heap.as_ref() else {
            return self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("BeginQuery with no heap"));
        };
        // SAFETY: query heaps come from this library.
        let heap = unsafe { h.as_impl() }.0;
        self.record(Command::BeginQuery {
            heap,
            kind: kind.0 as u32,
            index,
        });
    }

    pub fn EndQuery(&self, heap: Ref<ID3D12QueryHeap>, kind: D3D12_QUERY_TYPE, index: u32) {
        let Some(h) = heap.as_ref() else {
            return self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("EndQuery with no heap"));
        };
        // SAFETY: query heaps come from this library.
        let heap = unsafe { h.as_impl() }.0;
        self.record(Command::EndQuery {
            heap,
            kind: kind.0 as u32,
            index,
        });
    }

    pub fn ResolveQueryData(
        &self,
        heap: Ref<ID3D12QueryHeap>,
        kind: D3D12_QUERY_TYPE,
        first: u32,
        count: u32,
        dst: Ref<ID3D12Resource>,
        offset: u64,
    ) {
        let (Some(h), Some(dst)) = (heap.as_ref(), id(&dst)) else {
            return self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("ResolveQueryData with no heap or buffer"));
        };
        // SAFETY: query heaps come from this library.
        let heap = unsafe { h.as_impl() }.0;
        self.record(Command::ResolveQueries {
            heap,
            kind: kind.0 as u32,
            first,
            count,
            dst,
            offset,
        });
    }

    pub fn SetPredication(
        &self,
        buffer: Ref<ID3D12Resource>,
        offset: u64,
        op: D3D12_PREDICATION_OP,
    ) {
        self.record(Command::Predication(
            id(&buffer).map(|b| (b, offset, op.0 as u32)),
        ));
    }

    /// Each value written in order with the work around it; every mode orders so already.
    pub fn WriteBufferImmediate(
        &self,
        count: u32,
        params: *const D3D12_WRITEBUFFERIMMEDIATE_PARAMETER,
        modes: *const D3D12_WRITEBUFFERIMMEDIATE_MODE,
    ) {
        // SAFETY: `count` parameters and, when given, as many modes, as the API requires.
        let (params, modes) = unsafe { (many(params, count), many(modes, count)) };
        if modes.iter().any(|m| !(0..=2).contains(&m.0)) {
            self.rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("an immediate write mode"));
            return;
        }
        for p in params {
            self.record(Command::WriteImmediate {
                address: p.Dest,
                value: p.Value,
            });
        }
    }

    pub fn Dispatch(&self, x: u32, y: u32, z: u32) {
        self.record(Command::Dispatch([x, y, z]));
    }

    pub fn ExecuteIndirect(
        &self,
        signature: Ref<ID3D12CommandSignature>,
        max: u32,
        args: Ref<ID3D12Resource>,
        args_offset: u64,
        count: Ref<ID3D12Resource>,
        count_offset: u64,
    ) {
        let args_ref = args;
        let (Some(signature), Some(args)) = (signature.as_ref(), id(&args_ref)) else {
            return self.rec.lock().unwrap().fail(Error::Invalid(
                "ExecuteIndirect with no signature or arguments",
            ));
        };
        // SAFETY: command signatures come from this library.
        let signature = unsafe { signature.as_impl() }.0.clone();
        let width = |r: &Ref<ID3D12Resource>| {
            // SAFETY: resources come from this library.
            r.as_ref().map_or(0, |r| unsafe { ours(r) }.desc.Width)
        };
        let (args_width, count_width) = (width(&args_ref), width(&count));
        if !signature.fits(max, args_offset, args_width)
            || (count.is_some()
                && (!count_offset.is_multiple_of(4) || count_offset + 4 > count_width))
        {
            return self.rec.lock().unwrap().fail(Error::Invalid(
                "ExecuteIndirect's arguments or count past their buffer's end",
            ));
        }
        self.record(Command::ExecuteIndirect {
            signature,
            max,
            args,
            args_offset,
            count: id(&count).map(|c| (c, count_offset)),
        });
    }

    /// A storage view clear; the view is read from its CPU handle when recorded.
    fn clear_storage(
        &self,
        cpu: &D3D12_CPU_DESCRIPTOR_HANDLE,
        values: [u32; 4],
        float: bool,
        count: u32,
        rects: *const RECT,
    ) {
        // SAFETY: a handle of this library's heaps and `count` rectangles.
        let (view, rects) = unsafe { (descriptor(*cpu), many(rects, count)) };
        let Descriptor::UnorderedAccess(view) = view else {
            return self.rec.lock().unwrap().fail(Error::Invalid(
                "clearing a descriptor that is no unordered access view",
            ));
        };
        self.record(Command::ClearStorage {
            view,
            values,
            float,
            rects: rects
                .iter()
                .map(|r| [r.left, r.top, r.right, r.bottom])
                .collect(),
        });
    }

    pub fn ClearUnorderedAccessViewUint(
        &self,
        _gpu: &D3D12_GPU_DESCRIPTOR_HANDLE,
        cpu: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _resource: Ref<ID3D12Resource>,
        values: *const u32,
        count: u32,
        rects: *const RECT,
    ) {
        // SAFETY: the API passes four values.
        let values = unsafe { *(values as *const [u32; 4]) };
        self.clear_storage(cpu, values, false, count, rects);
    }

    pub fn ClearUnorderedAccessViewFloat(
        &self,
        _gpu: &D3D12_GPU_DESCRIPTOR_HANDLE,
        cpu: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _resource: Ref<ID3D12Resource>,
        values: *const f32,
        count: u32,
        rects: *const RECT,
    ) {
        // SAFETY: the API passes four values.
        let values = unsafe { *(values as *const [f32; 4]) }.map(f32::to_bits);
        self.clear_storage(cpu, values, true, count, rects);
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
    pub fn ResolveSubresource(
        &self,
        dst: Ref<ID3D12Resource>,
        dst_index: u32,
        src: Ref<ID3D12Resource>,
        src_index: u32,
        format: DXGI_FORMAT,
    ) {
        let (Some(dst), Some(src)) = (id(&dst), id(&src)) else {
            return self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("ResolveSubresource with no resource"));
        };
        self.record(Command::Resolve {
            dst,
            dst_index,
            src,
            src_index,
            format: format.0 as u32,
        });
    }

    pub fn CopyResource(&self, dst: Ref<ID3D12Resource>, src: Ref<ID3D12Resource>) {
        let (Some(d), Some(s)) = (dst.as_ref(), src.as_ref()) else {
            return self
                .rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("CopyResource with no resource"));
        };
        // SAFETY: resources come from this library.
        let (d, s) = unsafe { (ours(d), ours(s)) };
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

    /// `DiscardResource`: the region's contents become undefined, which keeping them is; nothing
    /// is recorded (WebGPU has no load that leaves a target undefined).
    pub fn DiscardResource(&self, resource: Ref<ID3D12Resource>, _: *const D3D12_DISCARD_REGION) {
        if id(&resource).is_none() {
            self.rec
                .lock()
                .unwrap()
                .fail(Error::Invalid("DiscardResource with no resource"));
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
            let resource = unsafe { ours(l.pResource.as_ref()?) }.id;
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
        // Block-compressed copies start, and boxes start and end, at a block's corner, as
        // Direct3D 12 checks at `Close` even for levels smaller than a block.
        let block = |l: &D3D12_TEXTURE_COPY_LOCATION| {
            let format = if l.Type == D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX {
                // SAFETY: resources come from this library.
                unsafe { ours(l.pResource.as_ref()?) }.desc.Format.0 as u32
            } else {
                // SAFETY: the union member `Type` names.
                unsafe { l.Anonymous.PlacedFootprint }.Footprint.Format.0 as u32
            };
            let (w, h, _) = dxgi_format(format)?.0.block();
            Some((w, h))
        };
        let (dw, dh) = dst.and_then(block).unwrap_or((1, 1));
        let (sw, sh) = src.and_then(block).unwrap_or((1, 1));
        let unaligned = !x.is_multiple_of(dw)
            || !y.is_multiple_of(dh)
            || area.is_some_and(|b| {
                [b.left, b.right].iter().any(|v| !v.is_multiple_of(sw))
                    || [b.top, b.bottom].iter().any(|v| !v.is_multiple_of(sh))
            });
        if unaligned {
            self.rec.lock().unwrap().fail(Error::Invalid(
                "a block-compressed copy not at a block's corner",
            ));
            return;
        }
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
        mapping: 0,
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
            D3D12_RESOURCE_DIMENSION_TEXTURE1D if r.desc.DepthOrArraySize > 1 => 3,
            D3D12_RESOURCE_DIMENSION_TEXTURE1D => 2,
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
            D3D12_UAV_DIMENSION_TEXTURE1D => v.mip = d.Anonymous.Texture1D.MipSlice,
            D3D12_UAV_DIMENSION_TEXTURE1DARRAY => {
                let t = d.Anonymous.Texture1DArray;
                (v.mip, v.first_layer, v.layers) = (t.MipSlice, t.FirstArraySlice, t.ArraySize);
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

/// A pipeline's stream output declaration and `RasterizedStream`; `None` without entries.
///
/// # Safety
/// `d`'s arrays hold as many elements as it says, and its semantic names are C strings.
unsafe fn stream_output(
    d: &D3D12_STREAM_OUTPUT_DESC,
) -> Option<(wcontainers_shaders::stage::StreamOutput, u32)> {
    if d.NumEntries == 0 {
        return None;
    }
    // SAFETY: as the caller promises.
    let (entries, strides) = unsafe {
        (
            many(d.pSODeclaration, d.NumEntries),
            many(d.pBufferStrides, d.NumStrides),
        )
    };
    let mut so = wcontainers_shaders::stage::StreamOutput {
        entries: entries
            .iter()
            .map(|e| wcontainers_shaders::stage::StreamOutputEntry {
                stream: e.Stream as u8,
                // SAFETY: as the caller promises; a null name is a gap.
                semantic: (!e.SemanticName.is_null())
                    .then(|| unsafe { e.SemanticName.to_string() }.unwrap_or_default()),
                semantic_index: e.SemanticIndex,
                start_component: e.StartComponent,
                component_count: e.ComponentCount,
                slot: e.OutputSlot,
            })
            .collect(),
        strides: [0; 4],
    };
    for (s, &stride) in so.strides.iter_mut().zip(strides) {
        *s = stride;
    }
    Some((so, d.RasterizedStream))
}
