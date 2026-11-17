//! The methods of each object's interfaces that the front does not do yet, made from the `windows`
//! crate's interface traits: each reports itself as not implemented (a command list's also fails
//! its `Close`), and the methods objects.rs does are passed to it.

#![allow(clippy::too_many_arguments, unused_variables)]

use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;

use crate::objects::*;

impl IDXGIFactory4_Impl for Factory_Impl {
    fn EnumAdapterByLuid(
        &self,
        _adapterluid: &windows::Win32::Foundation::LUID,
        _riid: *const windows_core::GUID,
        _ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("EnumAdapterByLuid"))
    }
    fn EnumWarpAdapter(
        &self,
        _riid: *const windows_core::GUID,
        _ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("EnumWarpAdapter"))
    }
}

impl IDXGIFactory3_Impl for Factory_Impl {
    fn GetCreationFlags(&self) -> DXGI_CREATE_FACTORY_FLAGS {
        self.unsupported("GetCreationFlags");
        Default::default()
    }
}

impl IDXGIFactory2_Impl for Factory_Impl {
    fn IsWindowedStereoEnabled(&self) -> windows_core::BOOL {
        self.unsupported("IsWindowedStereoEnabled");
        Default::default()
    }
    fn CreateSwapChainForHwnd(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
        _hwnd: windows::Win32::Foundation::HWND,
        _pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        _pfullscreendesc: *const DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        _prestricttooutput: windows_core::Ref<IDXGIOutput>,
    ) -> windows_core::Result<IDXGISwapChain1> {
        Err(self.unsupported("CreateSwapChainForHwnd"))
    }
    fn CreateSwapChainForCoreWindow(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
        _pwindow: windows_core::Ref<windows_core::IUnknown>,
        _pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        _prestricttooutput: windows_core::Ref<IDXGIOutput>,
    ) -> windows_core::Result<IDXGISwapChain1> {
        Err(self.unsupported("CreateSwapChainForCoreWindow"))
    }
    fn GetSharedResourceAdapterLuid(
        &self,
        _hresource: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<windows::Win32::Foundation::LUID> {
        Err(self.unsupported("GetSharedResourceAdapterLuid"))
    }
    fn RegisterStereoStatusWindow(
        &self,
        _windowhandle: windows::Win32::Foundation::HWND,
        _wmsg: u32,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("RegisterStereoStatusWindow"))
    }
    fn RegisterStereoStatusEvent(
        &self,
        _hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("RegisterStereoStatusEvent"))
    }
    fn UnregisterStereoStatus(&self, _dwcookie: u32) {
        self.unsupported("UnregisterStereoStatus");
    }
    fn RegisterOcclusionStatusWindow(
        &self,
        _windowhandle: windows::Win32::Foundation::HWND,
        _wmsg: u32,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("RegisterOcclusionStatusWindow"))
    }
    fn RegisterOcclusionStatusEvent(
        &self,
        _hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("RegisterOcclusionStatusEvent"))
    }
    fn UnregisterOcclusionStatus(&self, _dwcookie: u32) {
        self.unsupported("UnregisterOcclusionStatus");
    }
    fn CreateSwapChainForComposition(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
        _pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        _prestricttooutput: windows_core::Ref<IDXGIOutput>,
    ) -> windows_core::Result<IDXGISwapChain1> {
        Err(self.unsupported("CreateSwapChainForComposition"))
    }
}

impl IDXGIFactory1_Impl for Factory_Impl {
    fn EnumAdapters1(&self, adapter: u32) -> windows_core::Result<IDXGIAdapter1> {
        Factory::EnumAdapters1(self, adapter)
    }
    fn IsCurrent(&self) -> windows_core::BOOL {
        self.unsupported("IsCurrent");
        Default::default()
    }
}

impl IDXGIFactory_Impl for Factory_Impl {
    fn EnumAdapters(&self, adapter: u32) -> windows_core::Result<IDXGIAdapter> {
        Factory::EnumAdapters(self, adapter)
    }
    fn MakeWindowAssociation(
        &self,
        _windowhandle: windows::Win32::Foundation::HWND,
        _flags: DXGI_MWA_FLAGS,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("MakeWindowAssociation"))
    }
    fn GetWindowAssociation(&self) -> windows_core::Result<windows::Win32::Foundation::HWND> {
        Err(self.unsupported("GetWindowAssociation"))
    }
    fn CreateSwapChain(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
        _pdesc: *const DXGI_SWAP_CHAIN_DESC,
        _ppswapchain: windows_core::OutRef<IDXGISwapChain>,
    ) -> windows_core::HRESULT {
        self.unsupported("CreateSwapChain").code()
    }
    fn CreateSoftwareAdapter(
        &self,
        _module: windows::Win32::Foundation::HMODULE,
    ) -> windows_core::Result<IDXGIAdapter> {
        Err(self.unsupported("CreateSoftwareAdapter"))
    }
}

impl IDXGIObject_Impl for Factory_Impl {
    fn SetPrivateData(
        &self,
        _name: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _name: *const windows_core::GUID,
        _punknown: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn GetPrivateData(
        &self,
        _name: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn GetParent(
        &self,
        _riid: *const windows_core::GUID,
        _ppparent: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetParent"))
    }
}

impl IDXGIAdapter1_Impl for Adapter_Impl {
    fn GetDesc1(&self) -> windows_core::Result<DXGI_ADAPTER_DESC1> {
        Adapter::GetDesc1(self)
    }
}

impl IDXGIAdapter_Impl for Adapter_Impl {
    fn EnumOutputs(&self, _output: u32) -> windows_core::Result<IDXGIOutput> {
        Err(self.unsupported("EnumOutputs"))
    }
    fn GetDesc(&self) -> windows_core::Result<DXGI_ADAPTER_DESC> {
        Adapter::GetDesc(self)
    }
    fn CheckInterfaceSupport(
        &self,
        _interfacename: *const windows_core::GUID,
    ) -> windows_core::Result<i64> {
        Err(self.unsupported("CheckInterfaceSupport"))
    }
}

impl IDXGIObject_Impl for Adapter_Impl {
    fn SetPrivateData(
        &self,
        _name: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _name: *const windows_core::GUID,
        _punknown: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn GetPrivateData(
        &self,
        _name: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn GetParent(
        &self,
        _riid: *const windows_core::GUID,
        _ppparent: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetParent"))
    }
}

impl ID3D12Device_Impl for Device_Impl {
    fn GetNodeCount(&self) -> u32 {
        Device::GetNodeCount(self)
    }
    fn CreateCommandQueue(
        &self,
        pdesc: *const D3D12_COMMAND_QUEUE_DESC,
        riid: *const windows_core::GUID,
        ppcommandqueue: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommandQueue(self, pdesc, riid, ppcommandqueue)
    }
    fn CreateCommandAllocator(
        &self,
        r#type: D3D12_COMMAND_LIST_TYPE,
        riid: *const windows_core::GUID,
        ppcommandallocator: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommandAllocator(self, r#type, riid, ppcommandallocator)
    }
    fn CreateGraphicsPipelineState(
        &self,
        pdesc: *const D3D12_GRAPHICS_PIPELINE_STATE_DESC,
        riid: *const windows_core::GUID,
        pppipelinestate: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateGraphicsPipelineState(self, pdesc, riid, pppipelinestate)
    }
    fn CreateComputePipelineState(
        &self,
        _pdesc: *const D3D12_COMPUTE_PIPELINE_STATE_DESC,
        _riid: *const windows_core::GUID,
        _pppipelinestate: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateComputePipelineState"))
    }
    fn CreateCommandList(
        &self,
        nodemask: u32,
        r#type: D3D12_COMMAND_LIST_TYPE,
        pcommandallocator: windows_core::Ref<ID3D12CommandAllocator>,
        pinitialstate: windows_core::Ref<ID3D12PipelineState>,
        riid: *const windows_core::GUID,
        ppcommandlist: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommandList(
            self,
            nodemask,
            r#type,
            pcommandallocator,
            pinitialstate,
            riid,
            ppcommandlist,
        )
    }
    fn CheckFeatureSupport(
        &self,
        feature: D3D12_FEATURE,
        pfeaturesupportdata: *mut core::ffi::c_void,
        featuresupportdatasize: u32,
    ) -> windows_core::Result<()> {
        Device::CheckFeatureSupport(self, feature, pfeaturesupportdata, featuresupportdatasize)
    }
    fn CreateDescriptorHeap(
        &self,
        pdescriptorheapdesc: *const D3D12_DESCRIPTOR_HEAP_DESC,
        riid: *const windows_core::GUID,
        ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateDescriptorHeap(self, pdescriptorheapdesc, riid, ppvheap)
    }
    fn GetDescriptorHandleIncrementSize(
        &self,
        descriptorheaptype: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) -> u32 {
        Device::GetDescriptorHandleIncrementSize(self, descriptorheaptype)
    }
    fn CreateRootSignature(
        &self,
        nodemask: u32,
        pblobwithrootsignature: *const core::ffi::c_void,
        bloblengthinbytes: usize,
        riid: *const windows_core::GUID,
        ppvrootsignature: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateRootSignature(
            self,
            nodemask,
            pblobwithrootsignature,
            bloblengthinbytes,
            riid,
            ppvrootsignature,
        )
    }
    fn CreateConstantBufferView(
        &self,
        _pdesc: *const D3D12_CONSTANT_BUFFER_VIEW_DESC,
        _destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("CreateConstantBufferView");
    }
    fn CreateShaderResourceView(
        &self,
        _presource: windows_core::Ref<ID3D12Resource>,
        _pdesc: *const D3D12_SHADER_RESOURCE_VIEW_DESC,
        _destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("CreateShaderResourceView");
    }
    fn CreateUnorderedAccessView(
        &self,
        _presource: windows_core::Ref<ID3D12Resource>,
        _pcounterresource: windows_core::Ref<ID3D12Resource>,
        _pdesc: *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
        _destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("CreateUnorderedAccessView");
    }
    fn CreateRenderTargetView(
        &self,
        presource: windows_core::Ref<ID3D12Resource>,
        pdesc: *const D3D12_RENDER_TARGET_VIEW_DESC,
        destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        Device::CreateRenderTargetView(self, presource, pdesc, destdescriptor)
    }
    fn CreateDepthStencilView(
        &self,
        _presource: windows_core::Ref<ID3D12Resource>,
        _pdesc: *const D3D12_DEPTH_STENCIL_VIEW_DESC,
        _destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("CreateDepthStencilView");
    }
    fn CreateSampler(
        &self,
        _pdesc: *const D3D12_SAMPLER_DESC,
        _destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("CreateSampler");
    }
    fn CopyDescriptors(
        &self,
        _numdestdescriptorranges: u32,
        _pdestdescriptorrangestarts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        _pdestdescriptorrangesizes: *const u32,
        _numsrcdescriptorranges: u32,
        _psrcdescriptorrangestarts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        _psrcdescriptorrangesizes: *const u32,
        _descriptorheapstype: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) {
        self.unsupported("CopyDescriptors");
    }
    fn CopyDescriptorsSimple(
        &self,
        _numdescriptors: u32,
        _destdescriptorrangestart: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _srcdescriptorrangestart: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _descriptorheapstype: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) {
        self.unsupported("CopyDescriptorsSimple");
    }
    fn GetResourceAllocationInfo(
        &self,
        _visiblemask: u32,
        _numresourcedescs: u32,
        _presourcedescs: *const D3D12_RESOURCE_DESC,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        self.unsupported("GetResourceAllocationInfo");
        Default::default()
    }
    fn GetCustomHeapProperties(
        &self,
        _nodemask: u32,
        _heaptype: D3D12_HEAP_TYPE,
    ) -> D3D12_HEAP_PROPERTIES {
        self.unsupported("GetCustomHeapProperties");
        Default::default()
    }
    fn CreateCommittedResource(
        &self,
        pheapproperties: *const D3D12_HEAP_PROPERTIES,
        heapflags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC,
        initialresourcestate: D3D12_RESOURCE_STATES,
        poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        riidresource: *const windows_core::GUID,
        ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommittedResource(
            self,
            pheapproperties,
            heapflags,
            pdesc,
            initialresourcestate,
            poptimizedclearvalue,
            riidresource,
            ppvresource,
        )
    }
    fn CreateHeap(
        &self,
        _pdesc: *const D3D12_HEAP_DESC,
        _riid: *const windows_core::GUID,
        _ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateHeap"))
    }
    fn CreatePlacedResource(
        &self,
        _pheap: windows_core::Ref<ID3D12Heap>,
        _heapoffset: u64,
        _pdesc: *const D3D12_RESOURCE_DESC,
        _initialstate: D3D12_RESOURCE_STATES,
        _poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        _riid: *const windows_core::GUID,
        _ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreatePlacedResource"))
    }
    fn CreateReservedResource(
        &self,
        _pdesc: *const D3D12_RESOURCE_DESC,
        _initialstate: D3D12_RESOURCE_STATES,
        _poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        _riid: *const windows_core::GUID,
        _ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateReservedResource"))
    }
    fn CreateSharedHandle(
        &self,
        _pobject: windows_core::Ref<ID3D12DeviceChild>,
        _pattributes: *const windows::Win32::Security::SECURITY_ATTRIBUTES,
        _access: u32,
        _name: &windows_core::PCWSTR,
    ) -> windows_core::Result<windows::Win32::Foundation::HANDLE> {
        Err(self.unsupported("CreateSharedHandle"))
    }
    fn OpenSharedHandle(
        &self,
        _nthandle: windows::Win32::Foundation::HANDLE,
        _riid: *const windows_core::GUID,
        _ppvobj: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("OpenSharedHandle"))
    }
    fn OpenSharedHandleByName(
        &self,
        _name: &windows_core::PCWSTR,
        _access: u32,
    ) -> windows_core::Result<windows::Win32::Foundation::HANDLE> {
        Err(self.unsupported("OpenSharedHandleByName"))
    }
    fn MakeResident(
        &self,
        _numobjects: u32,
        _ppobjects: *const Option<ID3D12Pageable>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("MakeResident"))
    }
    fn Evict(
        &self,
        _numobjects: u32,
        _ppobjects: *const Option<ID3D12Pageable>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("Evict"))
    }
    fn CreateFence(
        &self,
        initialvalue: u64,
        flags: D3D12_FENCE_FLAGS,
        riid: *const windows_core::GUID,
        ppfence: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateFence(self, initialvalue, flags, riid, ppfence)
    }
    fn GetDeviceRemovedReason(&self) -> windows_core::Result<()> {
        Device::GetDeviceRemovedReason(self)
    }
    fn GetCopyableFootprints(
        &self,
        presourcedesc: *const D3D12_RESOURCE_DESC,
        firstsubresource: u32,
        numsubresources: u32,
        baseoffset: u64,
        playouts: *mut D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
        pnumrows: *mut u32,
        prowsizeinbytes: *mut u64,
        ptotalbytes: *mut u64,
    ) {
        Device::GetCopyableFootprints(
            self,
            presourcedesc,
            firstsubresource,
            numsubresources,
            baseoffset,
            playouts,
            pnumrows,
            prowsizeinbytes,
            ptotalbytes,
        )
    }
    fn CreateQueryHeap(
        &self,
        _pdesc: *const D3D12_QUERY_HEAP_DESC,
        _riid: *const windows_core::GUID,
        _ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateQueryHeap"))
    }
    fn SetStablePowerState(&self, _enable: windows_core::BOOL) -> windows_core::Result<()> {
        Err(self.unsupported("SetStablePowerState"))
    }
    fn CreateCommandSignature(
        &self,
        _pdesc: *const D3D12_COMMAND_SIGNATURE_DESC,
        _prootsignature: windows_core::Ref<ID3D12RootSignature>,
        _riid: *const windows_core::GUID,
        _ppvcommandsignature: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateCommandSignature"))
    }
    fn GetResourceTiling(
        &self,
        _ptiledresource: windows_core::Ref<ID3D12Resource>,
        _pnumtilesforentireresource: *mut u32,
        _ppackedmipdesc: *mut D3D12_PACKED_MIP_INFO,
        _pstandardtileshapefornonpackedmips: *mut D3D12_TILE_SHAPE,
        _pnumsubresourcetilings: *mut u32,
        _firstsubresourcetilingtoget: u32,
        _psubresourcetilingsfornonpackedmips: *mut D3D12_SUBRESOURCE_TILING,
    ) {
        self.unsupported("GetResourceTiling");
    }
    fn GetAdapterLuid(&self) -> windows::Win32::Foundation::LUID {
        self.unsupported("GetAdapterLuid");
        Default::default()
    }
}

impl ID3D12Object_Impl for Device_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12CommandQueue_Impl for Queue_Impl {
    fn UpdateTileMappings(
        &self,
        _presource: windows_core::Ref<ID3D12Resource>,
        _numresourceregions: u32,
        _presourceregionstartcoordinates: *const D3D12_TILED_RESOURCE_COORDINATE,
        _presourceregionsizes: *const D3D12_TILE_REGION_SIZE,
        _pheap: windows_core::Ref<ID3D12Heap>,
        _numranges: u32,
        _prangeflags: *const D3D12_TILE_RANGE_FLAGS,
        _pheaprangestartoffsets: *const u32,
        _prangetilecounts: *const u32,
        _flags: D3D12_TILE_MAPPING_FLAGS,
    ) {
        self.unsupported("UpdateTileMappings");
    }
    fn CopyTileMappings(
        &self,
        _pdstresource: windows_core::Ref<ID3D12Resource>,
        _pdstregionstartcoordinate: *const D3D12_TILED_RESOURCE_COORDINATE,
        _psrcresource: windows_core::Ref<ID3D12Resource>,
        _psrcregionstartcoordinate: *const D3D12_TILED_RESOURCE_COORDINATE,
        _pregionsize: *const D3D12_TILE_REGION_SIZE,
        _flags: D3D12_TILE_MAPPING_FLAGS,
    ) {
        self.unsupported("CopyTileMappings");
    }
    fn ExecuteCommandLists(
        &self,
        numcommandlists: u32,
        ppcommandlists: *const Option<ID3D12CommandList>,
    ) {
        Queue::ExecuteCommandLists(self, numcommandlists, ppcommandlists)
    }
    fn SetMarker(&self, _metadata: u32, _pdata: *const core::ffi::c_void, _size: u32) {
        self.unsupported("SetMarker");
    }
    fn BeginEvent(&self, _metadata: u32, _pdata: *const core::ffi::c_void, _size: u32) {
        self.unsupported("BeginEvent");
    }
    fn EndEvent(&self) {
        self.unsupported("EndEvent");
    }
    fn Signal(
        &self,
        pfence: windows_core::Ref<ID3D12Fence>,
        value: u64,
    ) -> windows_core::Result<()> {
        Queue::Signal(self, pfence, value)
    }
    fn Wait(
        &self,
        _pfence: windows_core::Ref<ID3D12Fence>,
        _value: u64,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("Wait"))
    }
    fn GetTimestampFrequency(&self) -> windows_core::Result<u64> {
        Err(self.unsupported("GetTimestampFrequency"))
    }
    fn GetClockCalibration(
        &self,
        _pgputimestamp: *mut u64,
        _pcputimestamp: *mut u64,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetClockCalibration"))
    }
    fn GetDesc(&self) -> D3D12_COMMAND_QUEUE_DESC {
        Queue::GetDesc(self)
    }
}

impl ID3D12Pageable_Impl for Queue_Impl {}

impl ID3D12DeviceChild_Impl for Queue_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for Queue_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12CommandAllocator_Impl for Allocator_Impl {
    fn Reset(&self) -> windows_core::Result<()> {
        Allocator::Reset(self)
    }
}

impl ID3D12Pageable_Impl for Allocator_Impl {}

impl ID3D12DeviceChild_Impl for Allocator_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for Allocator_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12GraphicsCommandList_Impl for List_Impl {
    fn Close(&self) -> windows_core::Result<()> {
        List::Close(self)
    }
    fn Reset(
        &self,
        pallocator: windows_core::Ref<ID3D12CommandAllocator>,
        pinitialstate: windows_core::Ref<ID3D12PipelineState>,
    ) -> windows_core::Result<()> {
        List::Reset(self, pallocator, pinitialstate)
    }
    fn ClearState(&self, _ppipelinestate: windows_core::Ref<ID3D12PipelineState>) {
        self.unsupported("ClearState");
    }
    fn DrawInstanced(
        &self,
        vertexcountperinstance: u32,
        instancecount: u32,
        startvertexlocation: u32,
        startinstancelocation: u32,
    ) {
        List::DrawInstanced(
            self,
            vertexcountperinstance,
            instancecount,
            startvertexlocation,
            startinstancelocation,
        )
    }
    fn DrawIndexedInstanced(
        &self,
        indexcountperinstance: u32,
        instancecount: u32,
        startindexlocation: u32,
        basevertexlocation: i32,
        startinstancelocation: u32,
    ) {
        List::DrawIndexedInstanced(
            self,
            indexcountperinstance,
            instancecount,
            startindexlocation,
            basevertexlocation,
            startinstancelocation,
        )
    }
    fn Dispatch(&self, _threadgroupcountx: u32, _threadgroupcounty: u32, _threadgroupcountz: u32) {
        self.unsupported("Dispatch");
    }
    fn CopyBufferRegion(
        &self,
        pdstbuffer: windows_core::Ref<ID3D12Resource>,
        dstoffset: u64,
        psrcbuffer: windows_core::Ref<ID3D12Resource>,
        srcoffset: u64,
        numbytes: u64,
    ) {
        List::CopyBufferRegion(self, pdstbuffer, dstoffset, psrcbuffer, srcoffset, numbytes)
    }
    fn CopyTextureRegion(
        &self,
        pdst: *const D3D12_TEXTURE_COPY_LOCATION,
        dstx: u32,
        dsty: u32,
        dstz: u32,
        psrc: *const D3D12_TEXTURE_COPY_LOCATION,
        psrcbox: *const D3D12_BOX,
    ) {
        List::CopyTextureRegion(self, pdst, dstx, dsty, dstz, psrc, psrcbox)
    }
    fn CopyResource(
        &self,
        _pdstresource: windows_core::Ref<ID3D12Resource>,
        _psrcresource: windows_core::Ref<ID3D12Resource>,
    ) {
        self.unsupported("CopyResource");
    }
    fn CopyTiles(
        &self,
        _ptiledresource: windows_core::Ref<ID3D12Resource>,
        _ptileregionstartcoordinate: *const D3D12_TILED_RESOURCE_COORDINATE,
        _ptileregionsize: *const D3D12_TILE_REGION_SIZE,
        _pbuffer: windows_core::Ref<ID3D12Resource>,
        _bufferstartoffsetinbytes: u64,
        _flags: D3D12_TILE_COPY_FLAGS,
    ) {
        self.unsupported("CopyTiles");
    }
    fn ResolveSubresource(
        &self,
        _pdstresource: windows_core::Ref<ID3D12Resource>,
        _dstsubresource: u32,
        _psrcresource: windows_core::Ref<ID3D12Resource>,
        _srcsubresource: u32,
        _format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    ) {
        self.unsupported("ResolveSubresource");
    }
    fn IASetPrimitiveTopology(
        &self,
        primitivetopology: windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY,
    ) {
        List::IASetPrimitiveTopology(self, primitivetopology)
    }
    fn RSSetViewports(&self, numviewports: u32, pviewports: *const D3D12_VIEWPORT) {
        List::RSSetViewports(self, numviewports, pviewports)
    }
    fn RSSetScissorRects(&self, numrects: u32, prects: *const windows::Win32::Foundation::RECT) {
        List::RSSetScissorRects(self, numrects, prects)
    }
    fn OMSetBlendFactor(&self, blendfactor: *const f32) {
        List::OMSetBlendFactor(self, blendfactor)
    }
    fn OMSetStencilRef(&self, stencilref: u32) {
        List::OMSetStencilRef(self, stencilref)
    }
    fn SetPipelineState(&self, ppipelinestate: windows_core::Ref<ID3D12PipelineState>) {
        List::SetPipelineState(self, ppipelinestate)
    }
    fn ResourceBarrier(&self, numbarriers: u32, pbarriers: *const D3D12_RESOURCE_BARRIER) {
        List::ResourceBarrier(self, numbarriers, pbarriers)
    }
    fn ExecuteBundle(&self, _pcommandlist: windows_core::Ref<ID3D12GraphicsCommandList>) {
        self.unsupported("ExecuteBundle");
    }
    fn SetDescriptorHeaps(
        &self,
        _numdescriptorheaps: u32,
        _ppdescriptorheaps: *const Option<ID3D12DescriptorHeap>,
    ) {
        self.unsupported("SetDescriptorHeaps");
    }
    fn SetComputeRootSignature(&self, _prootsignature: windows_core::Ref<ID3D12RootSignature>) {
        self.unsupported("SetComputeRootSignature");
    }
    fn SetGraphicsRootSignature(&self, prootsignature: windows_core::Ref<ID3D12RootSignature>) {
        List::SetGraphicsRootSignature(self, prootsignature)
    }
    fn SetComputeRootDescriptorTable(
        &self,
        _rootparameterindex: u32,
        _basedescriptor: &D3D12_GPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("SetComputeRootDescriptorTable");
    }
    fn SetGraphicsRootDescriptorTable(
        &self,
        _rootparameterindex: u32,
        _basedescriptor: &D3D12_GPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("SetGraphicsRootDescriptorTable");
    }
    fn SetComputeRoot32BitConstant(
        &self,
        _rootparameterindex: u32,
        _srcdata: u32,
        _destoffsetin32bitvalues: u32,
    ) {
        self.unsupported("SetComputeRoot32BitConstant");
    }
    fn SetGraphicsRoot32BitConstant(
        &self,
        _rootparameterindex: u32,
        _srcdata: u32,
        _destoffsetin32bitvalues: u32,
    ) {
        self.unsupported("SetGraphicsRoot32BitConstant");
    }
    fn SetComputeRoot32BitConstants(
        &self,
        _rootparameterindex: u32,
        _num32bitvaluestoset: u32,
        _psrcdata: *const core::ffi::c_void,
        _destoffsetin32bitvalues: u32,
    ) {
        self.unsupported("SetComputeRoot32BitConstants");
    }
    fn SetGraphicsRoot32BitConstants(
        &self,
        _rootparameterindex: u32,
        _num32bitvaluestoset: u32,
        _psrcdata: *const core::ffi::c_void,
        _destoffsetin32bitvalues: u32,
    ) {
        self.unsupported("SetGraphicsRoot32BitConstants");
    }
    fn SetComputeRootConstantBufferView(&self, _rootparameterindex: u32, _bufferlocation: u64) {
        self.unsupported("SetComputeRootConstantBufferView");
    }
    fn SetGraphicsRootConstantBufferView(&self, _rootparameterindex: u32, _bufferlocation: u64) {
        self.unsupported("SetGraphicsRootConstantBufferView");
    }
    fn SetComputeRootShaderResourceView(&self, _rootparameterindex: u32, _bufferlocation: u64) {
        self.unsupported("SetComputeRootShaderResourceView");
    }
    fn SetGraphicsRootShaderResourceView(&self, _rootparameterindex: u32, _bufferlocation: u64) {
        self.unsupported("SetGraphicsRootShaderResourceView");
    }
    fn SetComputeRootUnorderedAccessView(&self, _rootparameterindex: u32, _bufferlocation: u64) {
        self.unsupported("SetComputeRootUnorderedAccessView");
    }
    fn SetGraphicsRootUnorderedAccessView(&self, _rootparameterindex: u32, _bufferlocation: u64) {
        self.unsupported("SetGraphicsRootUnorderedAccessView");
    }
    fn IASetIndexBuffer(&self, pview: *const D3D12_INDEX_BUFFER_VIEW) {
        List::IASetIndexBuffer(self, pview)
    }
    fn IASetVertexBuffers(
        &self,
        startslot: u32,
        numviews: u32,
        pviews: *const D3D12_VERTEX_BUFFER_VIEW,
    ) {
        List::IASetVertexBuffers(self, startslot, numviews, pviews)
    }
    fn SOSetTargets(
        &self,
        _startslot: u32,
        _numviews: u32,
        _pviews: *const D3D12_STREAM_OUTPUT_BUFFER_VIEW,
    ) {
        self.unsupported("SOSetTargets");
    }
    fn OMSetRenderTargets(
        &self,
        numrendertargetdescriptors: u32,
        prendertargetdescriptors: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        rtssinglehandletodescriptorrange: windows_core::BOOL,
        pdepthstencildescriptor: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        List::OMSetRenderTargets(
            self,
            numrendertargetdescriptors,
            prendertargetdescriptors,
            rtssinglehandletodescriptorrange,
            pdepthstencildescriptor,
        )
    }
    fn ClearDepthStencilView(
        &self,
        _depthstencilview: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _clearflags: D3D12_CLEAR_FLAGS,
        _depth: f32,
        _stencil: u8,
        _numrects: u32,
        _prects: *const windows::Win32::Foundation::RECT,
    ) {
        self.unsupported("ClearDepthStencilView");
    }
    fn ClearRenderTargetView(
        &self,
        rendertargetview: &D3D12_CPU_DESCRIPTOR_HANDLE,
        colorrgba: *const f32,
        numrects: u32,
        prects: *const windows::Win32::Foundation::RECT,
    ) {
        List::ClearRenderTargetView(self, rendertargetview, colorrgba, numrects, prects)
    }
    fn ClearUnorderedAccessViewUint(
        &self,
        _viewgpuhandleincurrentheap: &D3D12_GPU_DESCRIPTOR_HANDLE,
        _viewcpuhandle: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _presource: windows_core::Ref<ID3D12Resource>,
        _values: *const u32,
        _numrects: u32,
        _prects: *const windows::Win32::Foundation::RECT,
    ) {
        self.unsupported("ClearUnorderedAccessViewUint");
    }
    fn ClearUnorderedAccessViewFloat(
        &self,
        _viewgpuhandleincurrentheap: &D3D12_GPU_DESCRIPTOR_HANDLE,
        _viewcpuhandle: &D3D12_CPU_DESCRIPTOR_HANDLE,
        _presource: windows_core::Ref<ID3D12Resource>,
        _values: *const f32,
        _numrects: u32,
        _prects: *const windows::Win32::Foundation::RECT,
    ) {
        self.unsupported("ClearUnorderedAccessViewFloat");
    }
    fn DiscardResource(
        &self,
        _presource: windows_core::Ref<ID3D12Resource>,
        _pregion: *const D3D12_DISCARD_REGION,
    ) {
        self.unsupported("DiscardResource");
    }
    fn BeginQuery(
        &self,
        _pqueryheap: windows_core::Ref<ID3D12QueryHeap>,
        r#type: D3D12_QUERY_TYPE,
        _index: u32,
    ) {
        self.unsupported("BeginQuery");
    }
    fn EndQuery(
        &self,
        _pqueryheap: windows_core::Ref<ID3D12QueryHeap>,
        r#type: D3D12_QUERY_TYPE,
        _index: u32,
    ) {
        self.unsupported("EndQuery");
    }
    fn ResolveQueryData(
        &self,
        _pqueryheap: windows_core::Ref<ID3D12QueryHeap>,
        r#type: D3D12_QUERY_TYPE,
        _startindex: u32,
        _numqueries: u32,
        _pdestinationbuffer: windows_core::Ref<ID3D12Resource>,
        _aligneddestinationbufferoffset: u64,
    ) {
        self.unsupported("ResolveQueryData");
    }
    fn SetPredication(
        &self,
        _pbuffer: windows_core::Ref<ID3D12Resource>,
        _alignedbufferoffset: u64,
        _operation: D3D12_PREDICATION_OP,
    ) {
        self.unsupported("SetPredication");
    }
    fn SetMarker(&self, _metadata: u32, _pdata: *const core::ffi::c_void, _size: u32) {
        self.unsupported("SetMarker");
    }
    fn BeginEvent(&self, _metadata: u32, _pdata: *const core::ffi::c_void, _size: u32) {
        self.unsupported("BeginEvent");
    }
    fn EndEvent(&self) {
        self.unsupported("EndEvent");
    }
    fn ExecuteIndirect(
        &self,
        _pcommandsignature: windows_core::Ref<ID3D12CommandSignature>,
        _maxcommandcount: u32,
        _pargumentbuffer: windows_core::Ref<ID3D12Resource>,
        _argumentbufferoffset: u64,
        _pcountbuffer: windows_core::Ref<ID3D12Resource>,
        _countbufferoffset: u64,
    ) {
        self.unsupported("ExecuteIndirect");
    }
}

impl ID3D12CommandList_Impl for List_Impl {
    fn GetType(&self) -> D3D12_COMMAND_LIST_TYPE {
        List::GetType(self)
    }
}

impl ID3D12DeviceChild_Impl for List_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for List_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12Fence_Impl for Fence_Impl {
    fn GetCompletedValue(&self) -> u64 {
        Fence::GetCompletedValue(self)
    }
    fn SetEventOnCompletion(
        &self,
        value: u64,
        hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<()> {
        Fence::SetEventOnCompletion(self, value, hevent)
    }
    fn Signal(&self, value: u64) -> windows_core::Result<()> {
        Fence::Signal(self, value)
    }
}

impl ID3D12Pageable_Impl for Fence_Impl {}

impl ID3D12DeviceChild_Impl for Fence_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for Fence_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12Resource_Impl for Resource_Impl {
    fn Map(
        &self,
        subresource: u32,
        preadrange: *const D3D12_RANGE,
        ppdata: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Resource::Map(self, subresource, preadrange, ppdata)
    }
    fn Unmap(&self, subresource: u32, pwrittenrange: *const D3D12_RANGE) {
        Resource::Unmap(self, subresource, pwrittenrange)
    }
    fn GetDesc(&self) -> D3D12_RESOURCE_DESC {
        Resource::GetDesc(self)
    }
    fn GetGPUVirtualAddress(&self) -> u64 {
        Resource::GetGPUVirtualAddress(self)
    }
    fn WriteToSubresource(
        &self,
        _dstsubresource: u32,
        _pdstbox: *const D3D12_BOX,
        _psrcdata: *const core::ffi::c_void,
        _srcrowpitch: u32,
        _srcdepthpitch: u32,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("WriteToSubresource"))
    }
    fn ReadFromSubresource(
        &self,
        _pdstdata: *mut core::ffi::c_void,
        _dstrowpitch: u32,
        _dstdepthpitch: u32,
        _srcsubresource: u32,
        _psrcbox: *const D3D12_BOX,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("ReadFromSubresource"))
    }
    fn GetHeapProperties(
        &self,
        _pheapproperties: *mut D3D12_HEAP_PROPERTIES,
        _pheapflags: *mut D3D12_HEAP_FLAGS,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetHeapProperties"))
    }
}

impl ID3D12Pageable_Impl for Resource_Impl {}

impl ID3D12DeviceChild_Impl for Resource_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for Resource_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12RootSignature_Impl for RootSignature_Impl {}

impl ID3D12DeviceChild_Impl for RootSignature_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for RootSignature_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12PipelineState_Impl for PipelineState_Impl {
    fn GetCachedBlob(&self) -> windows_core::Result<windows::Win32::Graphics::Direct3D::ID3DBlob> {
        Err(self.unsupported("GetCachedBlob"))
    }
}

impl ID3D12Pageable_Impl for PipelineState_Impl {}

impl ID3D12DeviceChild_Impl for PipelineState_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for PipelineState_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3D12DescriptorHeap_Impl for DescriptorHeap_Impl {
    fn GetDesc(&self) -> D3D12_DESCRIPTOR_HEAP_DESC {
        DescriptorHeap::GetDesc(self)
    }
    fn GetCPUDescriptorHandleForHeapStart(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        DescriptorHeap::GetCPUDescriptorHandleForHeapStart(self)
    }
    fn GetGPUDescriptorHandleForHeapStart(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        DescriptorHeap::GetGPUDescriptorHandleForHeapStart(self)
    }
}

impl ID3D12Pageable_Impl for DescriptorHeap_Impl {}

impl ID3D12DeviceChild_Impl for DescriptorHeap_Impl {
    fn GetDevice(
        &self,
        _riid: *const windows_core::GUID,
        _ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDevice"))
    }
}

impl ID3D12Object_Impl for DescriptorHeap_Impl {
    fn GetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _pdatasize: *mut u32,
        _pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetPrivateData"))
    }
    fn SetPrivateData(
        &self,
        _guid: *const windows_core::GUID,
        _datasize: u32,
        _pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateData"))
    }
    fn SetPrivateDataInterface(
        &self,
        _guid: *const windows_core::GUID,
        _pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetPrivateDataInterface"))
    }
    fn SetName(&self, _name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        Err(self.unsupported("SetName"))
    }
}

impl ID3DBlob_Impl for Blob_Impl {
    fn GetBufferPointer(&self) -> *mut core::ffi::c_void {
        Blob::GetBufferPointer(self)
    }
    fn GetBufferSize(&self) -> usize {
        Blob::GetBufferSize(self)
    }
}
