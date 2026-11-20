//! The methods of each object's interfaces that the front does not do yet, made by com.py from the
//! `windows` crate's interface traits: each reports itself as not implemented (a command list's also
//! fails its `Close`), and the methods objects.rs and present.rs do are passed to them.

#![allow(clippy::too_many_arguments, unused_variables)]

use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;

use crate::base::HasBase;
use crate::objects::*;
use crate::present::*;

impl IDXGIFactory7_Impl for Factory_Impl {
    fn RegisterAdaptersChangedEvent(
        &self,
        hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<u32> {
        Factory::RegisterAdaptersChangedEvent(self, hevent)
    }
    fn UnregisterAdaptersChangedEvent(&self, dwcookie: u32) -> windows_core::Result<()> {
        Factory::UnregisterAdaptersChangedEvent(self, dwcookie)
    }
}

impl IDXGIFactory6_Impl for Factory_Impl {
    fn EnumAdapterByGpuPreference(
        &self,
        adapter: u32,
        gpupreference: DXGI_GPU_PREFERENCE,
        riid: *const windows_core::GUID,
        ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Factory::EnumAdapterByGpuPreference(self, adapter, gpupreference, riid, ppvadapter)
    }
}

impl IDXGIFactory5_Impl for Factory_Impl {
    fn CheckFeatureSupport(
        &self,
        feature: DXGI_FEATURE,
        pfeaturesupportdata: *mut core::ffi::c_void,
        featuresupportdatasize: u32,
    ) -> windows_core::Result<()> {
        Factory::CheckFeatureSupport(self, feature, pfeaturesupportdata, featuresupportdatasize)
    }
}

impl IDXGIFactory4_Impl for Factory_Impl {
    fn EnumAdapterByLuid(
        &self,
        adapterluid: &windows::Win32::Foundation::LUID,
        riid: *const windows_core::GUID,
        ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Factory::EnumAdapterByLuid(self, adapterluid, riid, ppvadapter)
    }
    fn EnumWarpAdapter(
        &self,
        riid: *const windows_core::GUID,
        ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Factory::EnumWarpAdapter(self, riid, ppvadapter)
    }
}

impl IDXGIFactory3_Impl for Factory_Impl {
    fn GetCreationFlags(&self) -> DXGI_CREATE_FACTORY_FLAGS {
        Factory::GetCreationFlags(self)
    }
}

impl IDXGIFactory2_Impl for Factory_Impl {
    fn IsWindowedStereoEnabled(&self) -> windows_core::BOOL {
        Factory::IsWindowedStereoEnabled(self)
    }
    fn CreateSwapChainForHwnd(
        &self,
        pdevice: windows_core::Ref<windows_core::IUnknown>,
        hwnd: windows::Win32::Foundation::HWND,
        pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        pfullscreendesc: *const DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        prestricttooutput: windows_core::Ref<IDXGIOutput>,
    ) -> windows_core::Result<IDXGISwapChain1> {
        Factory::CreateSwapChainForHwnd(
            self,
            pdevice,
            hwnd,
            pdesc,
            pfullscreendesc,
            prestricttooutput,
        )
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
        Factory::IsCurrent(self)
    }
}

impl IDXGIFactory_Impl for Factory_Impl {
    fn EnumAdapters(&self, adapter: u32) -> windows_core::Result<IDXGIAdapter> {
        Factory::EnumAdapters(self, adapter)
    }
    fn MakeWindowAssociation(
        &self,
        windowhandle: windows::Win32::Foundation::HWND,
        flags: DXGI_MWA_FLAGS,
    ) -> windows_core::Result<()> {
        Factory::MakeWindowAssociation(self, windowhandle, flags)
    }
    fn GetWindowAssociation(&self) -> windows_core::Result<windows::Win32::Foundation::HWND> {
        Factory::GetWindowAssociation(self)
    }
    fn CreateSwapChain(
        &self,
        pdevice: windows_core::Ref<windows_core::IUnknown>,
        pdesc: *const DXGI_SWAP_CHAIN_DESC,
        ppswapchain: windows_core::OutRef<IDXGISwapChain>,
    ) -> windows_core::HRESULT {
        Factory::CreateSwapChain(self, pdevice, pdesc, ppswapchain)
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

impl IDXGIAdapter4_Impl for Adapter_Impl {
    fn GetDesc3(&self) -> windows_core::Result<DXGI_ADAPTER_DESC3> {
        Adapter::GetDesc3(self)
    }
}

impl IDXGIAdapter3_Impl for Adapter_Impl {
    fn RegisterHardwareContentProtectionTeardownStatusEvent(
        &self,
        _hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("RegisterHardwareContentProtectionTeardownStatusEvent"))
    }
    fn UnregisterHardwareContentProtectionTeardownStatus(&self, _dwcookie: u32) {
        self.unsupported("UnregisterHardwareContentProtectionTeardownStatus");
    }
    fn QueryVideoMemoryInfo(
        &self,
        _nodeindex: u32,
        _memorysegmentgroup: DXGI_MEMORY_SEGMENT_GROUP,
        _pvideomemoryinfo: *mut DXGI_QUERY_VIDEO_MEMORY_INFO,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("QueryVideoMemoryInfo"))
    }
    fn SetVideoMemoryReservation(
        &self,
        _nodeindex: u32,
        _memorysegmentgroup: DXGI_MEMORY_SEGMENT_GROUP,
        _reservation: u64,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetVideoMemoryReservation"))
    }
    fn RegisterVideoMemoryBudgetChangeNotificationEvent(
        &self,
        _hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("RegisterVideoMemoryBudgetChangeNotificationEvent"))
    }
    fn UnregisterVideoMemoryBudgetChangeNotification(&self, _dwcookie: u32) {
        self.unsupported("UnregisterVideoMemoryBudgetChangeNotification");
    }
}

impl IDXGIAdapter2_Impl for Adapter_Impl {
    fn GetDesc2(&self) -> windows_core::Result<DXGI_ADAPTER_DESC2> {
        Adapter::GetDesc2(self)
    }
}

impl IDXGIAdapter1_Impl for Adapter_Impl {
    fn GetDesc1(&self) -> windows_core::Result<DXGI_ADAPTER_DESC1> {
        Adapter::GetDesc1(self)
    }
}

impl IDXGIAdapter_Impl for Adapter_Impl {
    fn EnumOutputs(&self, output: u32) -> windows_core::Result<IDXGIOutput> {
        Adapter::EnumOutputs(self, output)
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
        riid: *const windows_core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Adapter::GetParent(self, riid, ppparent)
    }
}

impl IDXGIOutput6_Impl for Output_Impl {
    fn GetDesc1(&self) -> windows_core::Result<DXGI_OUTPUT_DESC1> {
        Output::GetDesc1(self)
    }
    fn CheckHardwareCompositionSupport(&self) -> windows_core::Result<u32> {
        Output::CheckHardwareCompositionSupport(self)
    }
}

impl IDXGIOutput5_Impl for Output_Impl {
    fn DuplicateOutput1(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
        _flags: u32,
        _supportedformatscount: u32,
        _psupportedformats: *const Common::DXGI_FORMAT,
    ) -> windows_core::Result<IDXGIOutputDuplication> {
        Err(self.unsupported("DuplicateOutput1"))
    }
}

impl IDXGIOutput4_Impl for Output_Impl {
    fn CheckOverlayColorSpaceSupport(
        &self,
        _format: Common::DXGI_FORMAT,
        _colorspace: Common::DXGI_COLOR_SPACE_TYPE,
        _pconcerneddevice: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("CheckOverlayColorSpaceSupport"))
    }
}

impl IDXGIOutput3_Impl for Output_Impl {
    fn CheckOverlaySupport(
        &self,
        _enumformat: Common::DXGI_FORMAT,
        _pconcerneddevice: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<u32> {
        Err(self.unsupported("CheckOverlaySupport"))
    }
}

impl IDXGIOutput2_Impl for Output_Impl {
    fn SupportsOverlays(&self) -> windows_core::BOOL {
        Output::SupportsOverlays(self)
    }
}

impl IDXGIOutput1_Impl for Output_Impl {
    fn GetDisplayModeList1(
        &self,
        enumformat: Common::DXGI_FORMAT,
        flags: DXGI_ENUM_MODES,
        pnummodes: *mut u32,
        pdesc: *mut DXGI_MODE_DESC1,
    ) -> windows_core::Result<()> {
        Output::GetDisplayModeList1(self, enumformat, flags, pnummodes, pdesc)
    }
    fn FindClosestMatchingMode1(
        &self,
        pmodetomatch: *const DXGI_MODE_DESC1,
        pclosestmatch: *mut DXGI_MODE_DESC1,
        pconcerneddevice: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Output::FindClosestMatchingMode1(self, pmodetomatch, pclosestmatch, pconcerneddevice)
    }
    fn GetDisplaySurfaceData1(
        &self,
        _pdestination: windows_core::Ref<IDXGIResource>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDisplaySurfaceData1"))
    }
    fn DuplicateOutput(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<IDXGIOutputDuplication> {
        Err(self.unsupported("DuplicateOutput"))
    }
}

impl IDXGIOutput_Impl for Output_Impl {
    fn GetDesc(&self) -> windows_core::Result<DXGI_OUTPUT_DESC> {
        Output::GetDesc(self)
    }
    fn GetDisplayModeList(
        &self,
        enumformat: Common::DXGI_FORMAT,
        flags: DXGI_ENUM_MODES,
        pnummodes: *mut u32,
        pdesc: *mut Common::DXGI_MODE_DESC,
    ) -> windows_core::Result<()> {
        Output::GetDisplayModeList(self, enumformat, flags, pnummodes, pdesc)
    }
    fn FindClosestMatchingMode(
        &self,
        pmodetomatch: *const Common::DXGI_MODE_DESC,
        pclosestmatch: *mut Common::DXGI_MODE_DESC,
        pconcerneddevice: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Output::FindClosestMatchingMode(self, pmodetomatch, pclosestmatch, pconcerneddevice)
    }
    fn WaitForVBlank(&self) -> windows_core::Result<()> {
        Err(self.unsupported("WaitForVBlank"))
    }
    fn TakeOwnership(
        &self,
        _pdevice: windows_core::Ref<windows_core::IUnknown>,
        _exclusive: windows_core::BOOL,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("TakeOwnership"))
    }
    fn ReleaseOwnership(&self) {
        self.unsupported("ReleaseOwnership");
    }
    fn GetGammaControlCapabilities(
        &self,
        _pgammacaps: *mut Common::DXGI_GAMMA_CONTROL_CAPABILITIES,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetGammaControlCapabilities"))
    }
    fn SetGammaControl(
        &self,
        _parray: *const Common::DXGI_GAMMA_CONTROL,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetGammaControl"))
    }
    fn GetGammaControl(
        &self,
        _parray: *mut Common::DXGI_GAMMA_CONTROL,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetGammaControl"))
    }
    fn SetDisplaySurface(
        &self,
        _pscanoutsurface: windows_core::Ref<IDXGISurface>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("SetDisplaySurface"))
    }
    fn GetDisplaySurfaceData(
        &self,
        _pdestination: windows_core::Ref<IDXGISurface>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetDisplaySurfaceData"))
    }
    fn GetFrameStatistics(&self, _pstats: *mut DXGI_FRAME_STATISTICS) -> windows_core::Result<()> {
        Err(self.unsupported("GetFrameStatistics"))
    }
}

impl IDXGIObject_Impl for Output_Impl {
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
        riid: *const windows_core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Output::GetParent(self, riid, ppparent)
    }
}

impl IDXGISwapChain4_Impl for SwapChain_Impl {
    fn SetHDRMetaData(
        &self,
        r#type: DXGI_HDR_METADATA_TYPE,
        size: u32,
        pmetadata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        SwapChain::SetHDRMetaData(self, r#type, size, pmetadata)
    }
}

impl IDXGISwapChain3_Impl for SwapChain_Impl {
    fn GetCurrentBackBufferIndex(&self) -> u32 {
        SwapChain::GetCurrentBackBufferIndex(self)
    }
    fn CheckColorSpaceSupport(
        &self,
        colorspace: Common::DXGI_COLOR_SPACE_TYPE,
    ) -> windows_core::Result<u32> {
        SwapChain::CheckColorSpaceSupport(self, colorspace)
    }
    fn SetColorSpace1(
        &self,
        colorspace: Common::DXGI_COLOR_SPACE_TYPE,
    ) -> windows_core::Result<()> {
        SwapChain::SetColorSpace1(self, colorspace)
    }
    fn ResizeBuffers1(
        &self,
        _buffercount: u32,
        _width: u32,
        _height: u32,
        _format: Common::DXGI_FORMAT,
        _swapchainflags: &DXGI_SWAP_CHAIN_FLAG,
        _pcreationnodemask: *const u32,
        _pppresentqueue: *const Option<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("ResizeBuffers1"))
    }
}

impl IDXGISwapChain2_Impl for SwapChain_Impl {
    fn SetSourceSize(&self, _width: u32, _height: u32) -> windows_core::Result<()> {
        Err(self.unsupported("SetSourceSize"))
    }
    fn GetSourceSize(&self, _pwidth: *mut u32, _pheight: *mut u32) -> windows_core::Result<()> {
        Err(self.unsupported("GetSourceSize"))
    }
    fn SetMaximumFrameLatency(&self, maxlatency: u32) -> windows_core::Result<()> {
        SwapChain::SetMaximumFrameLatency(self, maxlatency)
    }
    fn GetMaximumFrameLatency(&self) -> windows_core::Result<u32> {
        SwapChain::GetMaximumFrameLatency(self)
    }
    fn GetFrameLatencyWaitableObject(&self) -> windows::Win32::Foundation::HANDLE {
        SwapChain::GetFrameLatencyWaitableObject(self)
    }
    fn SetMatrixTransform(&self, _pmatrix: *const DXGI_MATRIX_3X2_F) -> windows_core::Result<()> {
        Err(self.unsupported("SetMatrixTransform"))
    }
    fn GetMatrixTransform(&self, _pmatrix: *mut DXGI_MATRIX_3X2_F) -> windows_core::Result<()> {
        Err(self.unsupported("GetMatrixTransform"))
    }
}

impl IDXGISwapChain1_Impl for SwapChain_Impl {
    fn GetDesc1(&self) -> windows_core::Result<DXGI_SWAP_CHAIN_DESC1> {
        SwapChain::GetDesc1(self)
    }
    fn GetFullscreenDesc(&self) -> windows_core::Result<DXGI_SWAP_CHAIN_FULLSCREEN_DESC> {
        Err(self.unsupported("GetFullscreenDesc"))
    }
    fn GetHwnd(&self) -> windows_core::Result<windows::Win32::Foundation::HWND> {
        SwapChain::GetHwnd(self)
    }
    fn GetCoreWindow(
        &self,
        _refiid: *const windows_core::GUID,
        _ppunk: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("GetCoreWindow"))
    }
    fn Present1(
        &self,
        syncinterval: u32,
        presentflags: DXGI_PRESENT,
        ppresentparameters: *const DXGI_PRESENT_PARAMETERS,
    ) -> windows_core::HRESULT {
        SwapChain::Present1(self, syncinterval, presentflags, ppresentparameters)
    }
    fn IsTemporaryMonoSupported(&self) -> windows_core::BOOL {
        SwapChain::IsTemporaryMonoSupported(self)
    }
    fn GetRestrictToOutput(&self) -> windows_core::Result<IDXGIOutput> {
        Err(self.unsupported("GetRestrictToOutput"))
    }
    fn SetBackgroundColor(&self, _pcolor: *const DXGI_RGBA) -> windows_core::Result<()> {
        Err(self.unsupported("SetBackgroundColor"))
    }
    fn GetBackgroundColor(&self) -> windows_core::Result<DXGI_RGBA> {
        Err(self.unsupported("GetBackgroundColor"))
    }
    fn SetRotation(&self, _rotation: Common::DXGI_MODE_ROTATION) -> windows_core::Result<()> {
        Err(self.unsupported("SetRotation"))
    }
    fn GetRotation(&self) -> windows_core::Result<Common::DXGI_MODE_ROTATION> {
        Err(self.unsupported("GetRotation"))
    }
}

impl IDXGISwapChain_Impl for SwapChain_Impl {
    fn Present(&self, syncinterval: u32, flags: DXGI_PRESENT) -> windows_core::HRESULT {
        SwapChain::Present(self, syncinterval, flags)
    }
    fn GetBuffer(
        &self,
        buffer: u32,
        riid: *const windows_core::GUID,
        ppsurface: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        SwapChain::GetBuffer(self, buffer, riid, ppsurface)
    }
    fn SetFullscreenState(
        &self,
        fullscreen: windows_core::BOOL,
        ptarget: windows_core::Ref<IDXGIOutput>,
    ) -> windows_core::Result<()> {
        SwapChain::SetFullscreenState(self, fullscreen, ptarget)
    }
    fn GetFullscreenState(
        &self,
        pfullscreen: *mut windows_core::BOOL,
        pptarget: windows_core::OutRef<IDXGIOutput>,
    ) -> windows_core::Result<()> {
        SwapChain::GetFullscreenState(self, pfullscreen, pptarget)
    }
    fn GetDesc(&self) -> windows_core::Result<DXGI_SWAP_CHAIN_DESC> {
        SwapChain::GetDesc(self)
    }
    fn ResizeBuffers(
        &self,
        buffercount: u32,
        width: u32,
        height: u32,
        newformat: Common::DXGI_FORMAT,
        swapchainflags: &DXGI_SWAP_CHAIN_FLAG,
    ) -> windows_core::Result<()> {
        SwapChain::ResizeBuffers(self, buffercount, width, height, newformat, swapchainflags)
    }
    fn ResizeTarget(
        &self,
        _pnewtargetparameters: *const Common::DXGI_MODE_DESC,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("ResizeTarget"))
    }
    fn GetContainingOutput(&self) -> windows_core::Result<IDXGIOutput> {
        SwapChain::GetContainingOutput(self)
    }
    fn GetFrameStatistics(&self, _pstats: *mut DXGI_FRAME_STATISTICS) -> windows_core::Result<()> {
        Err(self.unsupported("GetFrameStatistics"))
    }
    fn GetLastPresentCount(&self) -> windows_core::Result<u32> {
        SwapChain::GetLastPresentCount(self)
    }
}

impl IDXGIDeviceSubObject_Impl for SwapChain_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        SwapChain::GetDevice(self, riid, ppdevice)
    }
}

impl IDXGIObject_Impl for SwapChain_Impl {
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
        riid: *const windows_core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        SwapChain::GetParent(self, riid, ppparent)
    }
}

impl ID3D12Device8_Impl for Device_Impl {
    fn GetResourceAllocationInfo2(
        &self,
        visiblemask: u32,
        numresourcedescs: u32,
        presourcedescs: *const D3D12_RESOURCE_DESC1,
        presourceallocationinfo1: *mut D3D12_RESOURCE_ALLOCATION_INFO1,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        Device::GetResourceAllocationInfo2(
            self,
            visiblemask,
            numresourcedescs,
            presourcedescs,
            presourceallocationinfo1,
        )
    }
    fn CreateCommittedResource2(
        &self,
        pheapproperties: *const D3D12_HEAP_PROPERTIES,
        heapflags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC1,
        initialresourcestate: D3D12_RESOURCE_STATES,
        poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        pprotectedsession: windows_core::Ref<ID3D12ProtectedResourceSession>,
        riidresource: *const windows_core::GUID,
        ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommittedResource2(
            self,
            pheapproperties,
            heapflags,
            pdesc,
            initialresourcestate,
            poptimizedclearvalue,
            pprotectedsession,
            riidresource,
            ppvresource,
        )
    }
    fn CreatePlacedResource1(
        &self,
        pheap: windows_core::Ref<ID3D12Heap>,
        heapoffset: u64,
        pdesc: *const D3D12_RESOURCE_DESC1,
        initialstate: D3D12_RESOURCE_STATES,
        poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        riid: *const windows_core::GUID,
        ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreatePlacedResource1(
            self,
            pheap,
            heapoffset,
            pdesc,
            initialstate,
            poptimizedclearvalue,
            riid,
            ppvresource,
        )
    }
    fn CreateSamplerFeedbackUnorderedAccessView(
        &self,
        _ptargetedresource: windows_core::Ref<ID3D12Resource>,
        _pfeedbackresource: windows_core::Ref<ID3D12Resource>,
        _destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        self.unsupported("CreateSamplerFeedbackUnorderedAccessView");
    }
    fn GetCopyableFootprints1(
        &self,
        presourcedesc: *const D3D12_RESOURCE_DESC1,
        firstsubresource: u32,
        numsubresources: u32,
        baseoffset: u64,
        playouts: *mut D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
        pnumrows: *mut u32,
        prowsizeinbytes: *mut u64,
        ptotalbytes: *mut u64,
    ) {
        Device::GetCopyableFootprints1(
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
}

impl ID3D12Device7_Impl for Device_Impl {
    fn AddToStateObject(
        &self,
        _paddition: *const D3D12_STATE_OBJECT_DESC,
        _pstateobjecttogrowfrom: windows_core::Ref<ID3D12StateObject>,
        _riid: *const windows_core::GUID,
        _ppnewstateobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("AddToStateObject"))
    }
    fn CreateProtectedResourceSession1(
        &self,
        _pdesc: *const D3D12_PROTECTED_RESOURCE_SESSION_DESC1,
        _riid: *const windows_core::GUID,
        _ppsession: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateProtectedResourceSession1"))
    }
}

impl ID3D12Device6_Impl for Device_Impl {
    fn SetBackgroundProcessingMode(
        &self,
        mode: D3D12_BACKGROUND_PROCESSING_MODE,
        measurementsaction: D3D12_MEASUREMENTS_ACTION,
        heventtosignaluponcompletion: windows::Win32::Foundation::HANDLE,
        pbfurthermeasurementsdesired: *mut windows_core::BOOL,
    ) -> windows_core::Result<()> {
        Device::SetBackgroundProcessingMode(
            self,
            mode,
            measurementsaction,
            heventtosignaluponcompletion,
            pbfurthermeasurementsdesired,
        )
    }
}

impl ID3D12Device5_Impl for Device_Impl {
    fn CreateLifetimeTracker(
        &self,
        _powner: windows_core::Ref<ID3D12LifetimeOwner>,
        _riid: *const windows_core::GUID,
        _ppvtracker: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateLifetimeTracker"))
    }
    fn RemoveDevice(&self) {
        Device::RemoveDevice(self)
    }
    fn EnumerateMetaCommands(
        &self,
        pnummetacommands: *mut u32,
        pdescs: *mut D3D12_META_COMMAND_DESC,
    ) -> windows_core::Result<()> {
        Device::EnumerateMetaCommands(self, pnummetacommands, pdescs)
    }
    fn EnumerateMetaCommandParameters(
        &self,
        commandid: *const windows_core::GUID,
        stage: D3D12_META_COMMAND_PARAMETER_STAGE,
        ptotalstructuresizeinbytes: *mut u32,
        pparametercount: *mut u32,
        pparameterdescs: *mut D3D12_META_COMMAND_PARAMETER_DESC,
    ) -> windows_core::Result<()> {
        Device::EnumerateMetaCommandParameters(
            self,
            commandid,
            stage,
            ptotalstructuresizeinbytes,
            pparametercount,
            pparameterdescs,
        )
    }
    fn CreateMetaCommand(
        &self,
        commandid: *const windows_core::GUID,
        nodemask: u32,
        pcreationparametersdata: *const core::ffi::c_void,
        creationparametersdatasizeinbytes: usize,
        riid: *const windows_core::GUID,
        ppmetacommand: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateMetaCommand(
            self,
            commandid,
            nodemask,
            pcreationparametersdata,
            creationparametersdatasizeinbytes,
            riid,
            ppmetacommand,
        )
    }
    fn CreateStateObject(
        &self,
        _pdesc: *const D3D12_STATE_OBJECT_DESC,
        _riid: *const windows_core::GUID,
        _ppstateobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateStateObject"))
    }
    fn GetRaytracingAccelerationStructurePrebuildInfo(
        &self,
        _pdesc: *const D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS,
        _pinfo: *mut D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO,
    ) {
        self.unsupported("GetRaytracingAccelerationStructurePrebuildInfo");
    }
    fn CheckDriverMatchingIdentifier(
        &self,
        serializeddatatype: D3D12_SERIALIZED_DATA_TYPE,
        pidentifiertocheck: *const D3D12_SERIALIZED_DATA_DRIVER_MATCHING_IDENTIFIER,
    ) -> D3D12_DRIVER_MATCHING_IDENTIFIER_STATUS {
        Device::CheckDriverMatchingIdentifier(self, serializeddatatype, pidentifiertocheck)
    }
}

impl ID3D12Device4_Impl for Device_Impl {
    fn CreateCommandList1(
        &self,
        nodemask: u32,
        r#type: D3D12_COMMAND_LIST_TYPE,
        flags: D3D12_COMMAND_LIST_FLAGS,
        riid: *const windows_core::GUID,
        ppcommandlist: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommandList1(self, nodemask, r#type, flags, riid, ppcommandlist)
    }
    fn CreateProtectedResourceSession(
        &self,
        _pdesc: *const D3D12_PROTECTED_RESOURCE_SESSION_DESC,
        _riid: *const windows_core::GUID,
        _ppsession: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateProtectedResourceSession"))
    }
    fn CreateCommittedResource1(
        &self,
        pheapproperties: *const D3D12_HEAP_PROPERTIES,
        heapflags: D3D12_HEAP_FLAGS,
        pdesc: *const D3D12_RESOURCE_DESC,
        initialresourcestate: D3D12_RESOURCE_STATES,
        poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        pprotectedsession: windows_core::Ref<ID3D12ProtectedResourceSession>,
        riidresource: *const windows_core::GUID,
        ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommittedResource1(
            self,
            pheapproperties,
            heapflags,
            pdesc,
            initialresourcestate,
            poptimizedclearvalue,
            pprotectedsession,
            riidresource,
            ppvresource,
        )
    }
    fn CreateHeap1(
        &self,
        pdesc: *const D3D12_HEAP_DESC,
        pprotectedsession: windows_core::Ref<ID3D12ProtectedResourceSession>,
        riid: *const windows_core::GUID,
        ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateHeap1(self, pdesc, pprotectedsession, riid, ppvheap)
    }
    fn CreateReservedResource1(
        &self,
        _pdesc: *const D3D12_RESOURCE_DESC,
        _initialstate: D3D12_RESOURCE_STATES,
        _poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        _pprotectedsession: windows_core::Ref<ID3D12ProtectedResourceSession>,
        _riid: *const windows_core::GUID,
        _ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("CreateReservedResource1"))
    }
    fn GetResourceAllocationInfo1(
        &self,
        visiblemask: u32,
        numresourcedescs: u32,
        presourcedescs: *const D3D12_RESOURCE_DESC,
        presourceallocationinfo1: *mut D3D12_RESOURCE_ALLOCATION_INFO1,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        Device::GetResourceAllocationInfo1(
            self,
            visiblemask,
            numresourcedescs,
            presourcedescs,
            presourceallocationinfo1,
        )
    }
}

impl ID3D12Device3_Impl for Device_Impl {
    fn OpenExistingHeapFromAddress(
        &self,
        _paddress: *const core::ffi::c_void,
        _riid: *const windows_core::GUID,
        _ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("OpenExistingHeapFromAddress"))
    }
    fn OpenExistingHeapFromFileMapping(
        &self,
        _hfilemapping: windows::Win32::Foundation::HANDLE,
        _riid: *const windows_core::GUID,
        _ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Err(self.unsupported("OpenExistingHeapFromFileMapping"))
    }
    fn EnqueueMakeResident(
        &self,
        flags: D3D12_RESIDENCY_FLAGS,
        numobjects: u32,
        ppobjects: *const Option<ID3D12Pageable>,
        pfencetosignal: windows_core::Ref<ID3D12Fence>,
        fencevaluetosignal: u64,
    ) -> windows_core::Result<()> {
        Device::EnqueueMakeResident(
            self,
            flags,
            numobjects,
            ppobjects,
            pfencetosignal,
            fencevaluetosignal,
        )
    }
}

impl ID3D12Device2_Impl for Device_Impl {
    fn CreatePipelineState(
        &self,
        pdesc: *const D3D12_PIPELINE_STATE_STREAM_DESC,
        riid: *const windows_core::GUID,
        pppipelinestate: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreatePipelineState(self, pdesc, riid, pppipelinestate)
    }
}

impl ID3D12Device1_Impl for Device_Impl {
    fn CreatePipelineLibrary(
        &self,
        plibraryblob: *const core::ffi::c_void,
        bloblength: usize,
        riid: *const windows_core::GUID,
        pppipelinelibrary: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreatePipelineLibrary(self, plibraryblob, bloblength, riid, pppipelinelibrary)
    }
    fn SetEventOnMultipleFenceCompletion(
        &self,
        ppfences: *const Option<ID3D12Fence>,
        pfencevalues: *const u64,
        numfences: u32,
        flags: D3D12_MULTIPLE_FENCE_WAIT_FLAGS,
        hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows_core::Result<()> {
        Device::SetEventOnMultipleFenceCompletion(
            self,
            ppfences,
            pfencevalues,
            numfences,
            flags,
            hevent,
        )
    }
    fn SetResidencyPriority(
        &self,
        numobjects: u32,
        ppobjects: *const Option<ID3D12Pageable>,
        ppriorities: *const D3D12_RESIDENCY_PRIORITY,
    ) -> windows_core::Result<()> {
        Device::SetResidencyPriority(self, numobjects, ppobjects, ppriorities)
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
        pdesc: *const D3D12_COMPUTE_PIPELINE_STATE_DESC,
        riid: *const windows_core::GUID,
        pppipelinestate: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateComputePipelineState(self, pdesc, riid, pppipelinestate)
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
        pdesc: *const D3D12_CONSTANT_BUFFER_VIEW_DESC,
        destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        Device::CreateConstantBufferView(self, pdesc, destdescriptor)
    }
    fn CreateShaderResourceView(
        &self,
        presource: windows_core::Ref<ID3D12Resource>,
        pdesc: *const D3D12_SHADER_RESOURCE_VIEW_DESC,
        destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        Device::CreateShaderResourceView(self, presource, pdesc, destdescriptor)
    }
    fn CreateUnorderedAccessView(
        &self,
        presource: windows_core::Ref<ID3D12Resource>,
        pcounterresource: windows_core::Ref<ID3D12Resource>,
        pdesc: *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
        destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        Device::CreateUnorderedAccessView(self, presource, pcounterresource, pdesc, destdescriptor)
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
        presource: windows_core::Ref<ID3D12Resource>,
        pdesc: *const D3D12_DEPTH_STENCIL_VIEW_DESC,
        destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        Device::CreateDepthStencilView(self, presource, pdesc, destdescriptor)
    }
    fn CreateSampler(
        &self,
        pdesc: *const D3D12_SAMPLER_DESC,
        destdescriptor: &D3D12_CPU_DESCRIPTOR_HANDLE,
    ) {
        Device::CreateSampler(self, pdesc, destdescriptor)
    }
    fn CopyDescriptors(
        &self,
        numdestdescriptorranges: u32,
        pdestdescriptorrangestarts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        pdestdescriptorrangesizes: *const u32,
        numsrcdescriptorranges: u32,
        psrcdescriptorrangestarts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
        psrcdescriptorrangesizes: *const u32,
        descriptorheapstype: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) {
        Device::CopyDescriptors(
            self,
            numdestdescriptorranges,
            pdestdescriptorrangestarts,
            pdestdescriptorrangesizes,
            numsrcdescriptorranges,
            psrcdescriptorrangestarts,
            psrcdescriptorrangesizes,
            descriptorheapstype,
        )
    }
    fn CopyDescriptorsSimple(
        &self,
        numdescriptors: u32,
        destdescriptorrangestart: &D3D12_CPU_DESCRIPTOR_HANDLE,
        srcdescriptorrangestart: &D3D12_CPU_DESCRIPTOR_HANDLE,
        descriptorheapstype: D3D12_DESCRIPTOR_HEAP_TYPE,
    ) {
        Device::CopyDescriptorsSimple(
            self,
            numdescriptors,
            destdescriptorrangestart,
            srcdescriptorrangestart,
            descriptorheapstype,
        )
    }
    fn GetResourceAllocationInfo(
        &self,
        visiblemask: u32,
        numresourcedescs: u32,
        presourcedescs: *const D3D12_RESOURCE_DESC,
    ) -> D3D12_RESOURCE_ALLOCATION_INFO {
        Device::GetResourceAllocationInfo(self, visiblemask, numresourcedescs, presourcedescs)
    }
    fn GetCustomHeapProperties(
        &self,
        nodemask: u32,
        heaptype: D3D12_HEAP_TYPE,
    ) -> D3D12_HEAP_PROPERTIES {
        Device::GetCustomHeapProperties(self, nodemask, heaptype)
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
        pdesc: *const D3D12_HEAP_DESC,
        riid: *const windows_core::GUID,
        ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateHeap(self, pdesc, riid, ppvheap)
    }
    fn CreatePlacedResource(
        &self,
        pheap: windows_core::Ref<ID3D12Heap>,
        heapoffset: u64,
        pdesc: *const D3D12_RESOURCE_DESC,
        initialstate: D3D12_RESOURCE_STATES,
        poptimizedclearvalue: *const D3D12_CLEAR_VALUE,
        riid: *const windows_core::GUID,
        ppvresource: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreatePlacedResource(
            self,
            pheap,
            heapoffset,
            pdesc,
            initialstate,
            poptimizedclearvalue,
            riid,
            ppvresource,
        )
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
        numobjects: u32,
        ppobjects: *const Option<ID3D12Pageable>,
    ) -> windows_core::Result<()> {
        Device::MakeResident(self, numobjects, ppobjects)
    }
    fn Evict(
        &self,
        numobjects: u32,
        ppobjects: *const Option<ID3D12Pageable>,
    ) -> windows_core::Result<()> {
        Device::Evict(self, numobjects, ppobjects)
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
        pdesc: *const D3D12_QUERY_HEAP_DESC,
        riid: *const windows_core::GUID,
        ppvheap: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateQueryHeap(self, pdesc, riid, ppvheap)
    }
    fn SetStablePowerState(&self, _enable: windows_core::BOOL) -> windows_core::Result<()> {
        Err(self.unsupported("SetStablePowerState"))
    }
    fn CreateCommandSignature(
        &self,
        pdesc: *const D3D12_COMMAND_SIGNATURE_DESC,
        prootsignature: windows_core::Ref<ID3D12RootSignature>,
        riid: *const windows_core::GUID,
        ppvcommandsignature: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Device::CreateCommandSignature(self, pdesc, prootsignature, riid, ppvcommandsignature)
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
        Device::GetAdapterLuid(self)
    }
}

impl ID3D12Object_Impl for Device_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
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
    fn SetMarker(&self, metadata: u32, pdata: *const core::ffi::c_void, size: u32) {
        Queue::SetMarker(self, metadata, pdata, size)
    }
    fn BeginEvent(&self, metadata: u32, pdata: *const core::ffi::c_void, size: u32) {
        Queue::BeginEvent(self, metadata, pdata, size)
    }
    fn EndEvent(&self) {
        Queue::EndEvent(self)
    }
    fn Signal(
        &self,
        pfence: windows_core::Ref<ID3D12Fence>,
        value: u64,
    ) -> windows_core::Result<()> {
        Queue::Signal(self, pfence, value)
    }
    fn Wait(&self, pfence: windows_core::Ref<ID3D12Fence>, value: u64) -> windows_core::Result<()> {
        Queue::Wait(self, pfence, value)
    }
    fn GetTimestampFrequency(&self) -> windows_core::Result<u64> {
        Queue::GetTimestampFrequency(self)
    }
    fn GetClockCalibration(
        &self,
        pgputimestamp: *mut u64,
        pcputimestamp: *mut u64,
    ) -> windows_core::Result<()> {
        Queue::GetClockCalibration(self, pgputimestamp, pcputimestamp)
    }
    fn GetDesc(&self) -> D3D12_COMMAND_QUEUE_DESC {
        Queue::GetDesc(self)
    }
}

impl ID3D12Pageable_Impl for Queue_Impl {}

impl ID3D12DeviceChild_Impl for Queue_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for Queue_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
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
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for Allocator_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12GraphicsCommandList6_Impl for List_Impl {
    fn DispatchMesh(
        &self,
        _threadgroupcountx: u32,
        _threadgroupcounty: u32,
        _threadgroupcountz: u32,
    ) {
        self.unsupported("DispatchMesh");
    }
}

impl ID3D12GraphicsCommandList5_Impl for List_Impl {
    fn RSSetShadingRate(
        &self,
        _baseshadingrate: D3D12_SHADING_RATE,
        _combiners: *const D3D12_SHADING_RATE_COMBINER,
    ) {
        self.unsupported("RSSetShadingRate");
    }
    fn RSSetShadingRateImage(&self, _shadingrateimage: windows_core::Ref<ID3D12Resource>) {
        self.unsupported("RSSetShadingRateImage");
    }
}

impl ID3D12GraphicsCommandList4_Impl for List_Impl {
    fn BeginRenderPass(
        &self,
        numrendertargets: u32,
        prendertargets: *const D3D12_RENDER_PASS_RENDER_TARGET_DESC,
        pdepthstencil: *const D3D12_RENDER_PASS_DEPTH_STENCIL_DESC,
        flags: D3D12_RENDER_PASS_FLAGS,
    ) {
        List::BeginRenderPass(self, numrendertargets, prendertargets, pdepthstencil, flags)
    }
    fn EndRenderPass(&self) {
        List::EndRenderPass(self)
    }
    fn InitializeMetaCommand(
        &self,
        _pmetacommand: windows_core::Ref<ID3D12MetaCommand>,
        _pinitializationparametersdata: *const core::ffi::c_void,
        _initializationparametersdatasizeinbytes: usize,
    ) {
        self.unsupported("InitializeMetaCommand");
    }
    fn ExecuteMetaCommand(
        &self,
        _pmetacommand: windows_core::Ref<ID3D12MetaCommand>,
        _pexecutionparametersdata: *const core::ffi::c_void,
        _executionparametersdatasizeinbytes: usize,
    ) {
        self.unsupported("ExecuteMetaCommand");
    }
    fn BuildRaytracingAccelerationStructure(
        &self,
        _pdesc: *const D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC,
        _numpostbuildinfodescs: u32,
        _ppostbuildinfodescs: *const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_DESC,
    ) {
        self.unsupported("BuildRaytracingAccelerationStructure");
    }
    fn EmitRaytracingAccelerationStructurePostbuildInfo(
        &self,
        _pdesc: *const D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_DESC,
        _numsourceaccelerationstructures: u32,
        _psourceaccelerationstructuredata: *const u64,
    ) {
        self.unsupported("EmitRaytracingAccelerationStructurePostbuildInfo");
    }
    fn CopyRaytracingAccelerationStructure(
        &self,
        _destaccelerationstructuredata: u64,
        _sourceaccelerationstructuredata: u64,
        _mode: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_COPY_MODE,
    ) {
        self.unsupported("CopyRaytracingAccelerationStructure");
    }
    fn SetPipelineState1(&self, _pstateobject: windows_core::Ref<ID3D12StateObject>) {
        self.unsupported("SetPipelineState1");
    }
    fn DispatchRays(&self, _pdesc: *const D3D12_DISPATCH_RAYS_DESC) {
        self.unsupported("DispatchRays");
    }
}

impl ID3D12GraphicsCommandList3_Impl for List_Impl {
    fn SetProtectedResourceSession(
        &self,
        pprotectedresourcesession: windows_core::Ref<ID3D12ProtectedResourceSession>,
    ) {
        List::SetProtectedResourceSession(self, pprotectedresourcesession)
    }
}

impl ID3D12GraphicsCommandList2_Impl for List_Impl {
    fn WriteBufferImmediate(
        &self,
        count: u32,
        pparams: *const D3D12_WRITEBUFFERIMMEDIATE_PARAMETER,
        pmodes: *const D3D12_WRITEBUFFERIMMEDIATE_MODE,
    ) {
        List::WriteBufferImmediate(self, count, pparams, pmodes)
    }
}

impl ID3D12GraphicsCommandList1_Impl for List_Impl {
    fn AtomicCopyBufferUINT(
        &self,
        _pdstbuffer: windows_core::Ref<ID3D12Resource>,
        _dstoffset: u64,
        _psrcbuffer: windows_core::Ref<ID3D12Resource>,
        _srcoffset: u64,
        _dependencies: u32,
        _ppdependentresources: *const Option<ID3D12Resource>,
        _pdependentsubresourceranges: *const D3D12_SUBRESOURCE_RANGE_UINT64,
    ) {
        self.unsupported("AtomicCopyBufferUINT");
    }
    fn AtomicCopyBufferUINT64(
        &self,
        _pdstbuffer: windows_core::Ref<ID3D12Resource>,
        _dstoffset: u64,
        _psrcbuffer: windows_core::Ref<ID3D12Resource>,
        _srcoffset: u64,
        _dependencies: u32,
        _ppdependentresources: *const Option<ID3D12Resource>,
        _pdependentsubresourceranges: *const D3D12_SUBRESOURCE_RANGE_UINT64,
    ) {
        self.unsupported("AtomicCopyBufferUINT64");
    }
    fn OMSetDepthBounds(&self, _min: f32, _max: f32) {
        self.unsupported("OMSetDepthBounds");
    }
    fn SetSamplePositions(
        &self,
        _numsamplesperpixel: u32,
        _numpixels: u32,
        _psamplepositions: *const D3D12_SAMPLE_POSITION,
    ) {
        self.unsupported("SetSamplePositions");
    }
    fn ResolveSubresourceRegion(
        &self,
        _pdstresource: windows_core::Ref<ID3D12Resource>,
        _dstsubresource: u32,
        _dstx: u32,
        _dsty: u32,
        _psrcresource: windows_core::Ref<ID3D12Resource>,
        _srcsubresource: u32,
        _psrcrect: *const windows::Win32::Foundation::RECT,
        _format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        _resolvemode: D3D12_RESOLVE_MODE,
    ) {
        self.unsupported("ResolveSubresourceRegion");
    }
    fn SetViewInstanceMask(&self, _mask: u32) {
        self.unsupported("SetViewInstanceMask");
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
    fn Dispatch(&self, threadgroupcountx: u32, threadgroupcounty: u32, threadgroupcountz: u32) {
        List::Dispatch(
            self,
            threadgroupcountx,
            threadgroupcounty,
            threadgroupcountz,
        )
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
        pdstresource: windows_core::Ref<ID3D12Resource>,
        psrcresource: windows_core::Ref<ID3D12Resource>,
    ) {
        List::CopyResource(self, pdstresource, psrcresource)
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
        pdstresource: windows_core::Ref<ID3D12Resource>,
        dstsubresource: u32,
        psrcresource: windows_core::Ref<ID3D12Resource>,
        srcsubresource: u32,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    ) {
        List::ResolveSubresource(
            self,
            pdstresource,
            dstsubresource,
            psrcresource,
            srcsubresource,
            format,
        )
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
    fn ExecuteBundle(&self, pcommandlist: windows_core::Ref<ID3D12GraphicsCommandList>) {
        List::ExecuteBundle(self, pcommandlist)
    }
    fn SetDescriptorHeaps(
        &self,
        numdescriptorheaps: u32,
        ppdescriptorheaps: *const Option<ID3D12DescriptorHeap>,
    ) {
        List::SetDescriptorHeaps(self, numdescriptorheaps, ppdescriptorheaps)
    }
    fn SetComputeRootSignature(&self, prootsignature: windows_core::Ref<ID3D12RootSignature>) {
        List::SetComputeRootSignature(self, prootsignature)
    }
    fn SetGraphicsRootSignature(&self, prootsignature: windows_core::Ref<ID3D12RootSignature>) {
        List::SetGraphicsRootSignature(self, prootsignature)
    }
    fn SetComputeRootDescriptorTable(
        &self,
        rootparameterindex: u32,
        basedescriptor: &D3D12_GPU_DESCRIPTOR_HANDLE,
    ) {
        List::SetComputeRootDescriptorTable(self, rootparameterindex, basedescriptor)
    }
    fn SetGraphicsRootDescriptorTable(
        &self,
        rootparameterindex: u32,
        basedescriptor: &D3D12_GPU_DESCRIPTOR_HANDLE,
    ) {
        List::SetGraphicsRootDescriptorTable(self, rootparameterindex, basedescriptor)
    }
    fn SetComputeRoot32BitConstant(
        &self,
        rootparameterindex: u32,
        srcdata: u32,
        destoffsetin32bitvalues: u32,
    ) {
        List::SetComputeRoot32BitConstant(
            self,
            rootparameterindex,
            srcdata,
            destoffsetin32bitvalues,
        )
    }
    fn SetGraphicsRoot32BitConstant(
        &self,
        rootparameterindex: u32,
        srcdata: u32,
        destoffsetin32bitvalues: u32,
    ) {
        List::SetGraphicsRoot32BitConstant(
            self,
            rootparameterindex,
            srcdata,
            destoffsetin32bitvalues,
        )
    }
    fn SetComputeRoot32BitConstants(
        &self,
        rootparameterindex: u32,
        num32bitvaluestoset: u32,
        psrcdata: *const core::ffi::c_void,
        destoffsetin32bitvalues: u32,
    ) {
        List::SetComputeRoot32BitConstants(
            self,
            rootparameterindex,
            num32bitvaluestoset,
            psrcdata,
            destoffsetin32bitvalues,
        )
    }
    fn SetGraphicsRoot32BitConstants(
        &self,
        rootparameterindex: u32,
        num32bitvaluestoset: u32,
        psrcdata: *const core::ffi::c_void,
        destoffsetin32bitvalues: u32,
    ) {
        List::SetGraphicsRoot32BitConstants(
            self,
            rootparameterindex,
            num32bitvaluestoset,
            psrcdata,
            destoffsetin32bitvalues,
        )
    }
    fn SetComputeRootConstantBufferView(&self, rootparameterindex: u32, bufferlocation: u64) {
        List::SetComputeRootConstantBufferView(self, rootparameterindex, bufferlocation)
    }
    fn SetGraphicsRootConstantBufferView(&self, rootparameterindex: u32, bufferlocation: u64) {
        List::SetGraphicsRootConstantBufferView(self, rootparameterindex, bufferlocation)
    }
    fn SetComputeRootShaderResourceView(&self, rootparameterindex: u32, bufferlocation: u64) {
        List::SetComputeRootShaderResourceView(self, rootparameterindex, bufferlocation)
    }
    fn SetGraphicsRootShaderResourceView(&self, rootparameterindex: u32, bufferlocation: u64) {
        List::SetGraphicsRootShaderResourceView(self, rootparameterindex, bufferlocation)
    }
    fn SetComputeRootUnorderedAccessView(&self, rootparameterindex: u32, bufferlocation: u64) {
        List::SetComputeRootUnorderedAccessView(self, rootparameterindex, bufferlocation)
    }
    fn SetGraphicsRootUnorderedAccessView(&self, rootparameterindex: u32, bufferlocation: u64) {
        List::SetGraphicsRootUnorderedAccessView(self, rootparameterindex, bufferlocation)
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
        startslot: u32,
        numviews: u32,
        pviews: *const D3D12_STREAM_OUTPUT_BUFFER_VIEW,
    ) {
        List::SOSetTargets(self, startslot, numviews, pviews)
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
        depthstencilview: &D3D12_CPU_DESCRIPTOR_HANDLE,
        clearflags: D3D12_CLEAR_FLAGS,
        depth: f32,
        stencil: u8,
        numrects: u32,
        prects: *const windows::Win32::Foundation::RECT,
    ) {
        List::ClearDepthStencilView(
            self,
            depthstencilview,
            clearflags,
            depth,
            stencil,
            numrects,
            prects,
        )
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
        viewgpuhandleincurrentheap: &D3D12_GPU_DESCRIPTOR_HANDLE,
        viewcpuhandle: &D3D12_CPU_DESCRIPTOR_HANDLE,
        presource: windows_core::Ref<ID3D12Resource>,
        values: *const u32,
        numrects: u32,
        prects: *const windows::Win32::Foundation::RECT,
    ) {
        List::ClearUnorderedAccessViewUint(
            self,
            viewgpuhandleincurrentheap,
            viewcpuhandle,
            presource,
            values,
            numrects,
            prects,
        )
    }
    fn ClearUnorderedAccessViewFloat(
        &self,
        viewgpuhandleincurrentheap: &D3D12_GPU_DESCRIPTOR_HANDLE,
        viewcpuhandle: &D3D12_CPU_DESCRIPTOR_HANDLE,
        presource: windows_core::Ref<ID3D12Resource>,
        values: *const f32,
        numrects: u32,
        prects: *const windows::Win32::Foundation::RECT,
    ) {
        List::ClearUnorderedAccessViewFloat(
            self,
            viewgpuhandleincurrentheap,
            viewcpuhandle,
            presource,
            values,
            numrects,
            prects,
        )
    }
    fn DiscardResource(
        &self,
        presource: windows_core::Ref<ID3D12Resource>,
        pregion: *const D3D12_DISCARD_REGION,
    ) {
        List::DiscardResource(self, presource, pregion)
    }
    fn BeginQuery(
        &self,
        pqueryheap: windows_core::Ref<ID3D12QueryHeap>,
        r#type: D3D12_QUERY_TYPE,
        index: u32,
    ) {
        List::BeginQuery(self, pqueryheap, r#type, index)
    }
    fn EndQuery(
        &self,
        pqueryheap: windows_core::Ref<ID3D12QueryHeap>,
        r#type: D3D12_QUERY_TYPE,
        index: u32,
    ) {
        List::EndQuery(self, pqueryheap, r#type, index)
    }
    fn ResolveQueryData(
        &self,
        pqueryheap: windows_core::Ref<ID3D12QueryHeap>,
        r#type: D3D12_QUERY_TYPE,
        startindex: u32,
        numqueries: u32,
        pdestinationbuffer: windows_core::Ref<ID3D12Resource>,
        aligneddestinationbufferoffset: u64,
    ) {
        List::ResolveQueryData(
            self,
            pqueryheap,
            r#type,
            startindex,
            numqueries,
            pdestinationbuffer,
            aligneddestinationbufferoffset,
        )
    }
    fn SetPredication(
        &self,
        pbuffer: windows_core::Ref<ID3D12Resource>,
        alignedbufferoffset: u64,
        operation: D3D12_PREDICATION_OP,
    ) {
        List::SetPredication(self, pbuffer, alignedbufferoffset, operation)
    }
    fn SetMarker(&self, metadata: u32, pdata: *const core::ffi::c_void, size: u32) {
        List::SetMarker(self, metadata, pdata, size)
    }
    fn BeginEvent(&self, metadata: u32, pdata: *const core::ffi::c_void, size: u32) {
        List::BeginEvent(self, metadata, pdata, size)
    }
    fn EndEvent(&self) {
        List::EndEvent(self)
    }
    fn ExecuteIndirect(
        &self,
        pcommandsignature: windows_core::Ref<ID3D12CommandSignature>,
        maxcommandcount: u32,
        pargumentbuffer: windows_core::Ref<ID3D12Resource>,
        argumentbufferoffset: u64,
        pcountbuffer: windows_core::Ref<ID3D12Resource>,
        countbufferoffset: u64,
    ) {
        List::ExecuteIndirect(
            self,
            pcommandsignature,
            maxcommandcount,
            pargumentbuffer,
            argumentbufferoffset,
            pcountbuffer,
            countbufferoffset,
        )
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
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for List_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12Fence1_Impl for Fence_Impl {
    fn GetCreationFlags(&self) -> D3D12_FENCE_FLAGS {
        Fence::GetCreationFlags(self)
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
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for Fence_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12Resource2_Impl for Resource_Impl {
    fn GetDesc1(&self) -> D3D12_RESOURCE_DESC1 {
        Resource::GetDesc1(self)
    }
}

impl ID3D12Resource1_Impl for Resource_Impl {
    fn GetProtectedResourceSession(
        &self,
        riid: *const windows_core::GUID,
        ppprotectedsession: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        Resource::GetProtectedResourceSession(self, riid, ppprotectedsession)
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
        dstsubresource: u32,
        pdstbox: *const D3D12_BOX,
        psrcdata: *const core::ffi::c_void,
        srcrowpitch: u32,
        srcdepthpitch: u32,
    ) -> windows_core::Result<()> {
        Resource::WriteToSubresource(
            self,
            dstsubresource,
            pdstbox,
            psrcdata,
            srcrowpitch,
            srcdepthpitch,
        )
    }
    fn ReadFromSubresource(
        &self,
        pdstdata: *mut core::ffi::c_void,
        dstrowpitch: u32,
        dstdepthpitch: u32,
        srcsubresource: u32,
        psrcbox: *const D3D12_BOX,
    ) -> windows_core::Result<()> {
        Resource::ReadFromSubresource(
            self,
            pdstdata,
            dstrowpitch,
            dstdepthpitch,
            srcsubresource,
            psrcbox,
        )
    }
    fn GetHeapProperties(
        &self,
        pheapproperties: *mut D3D12_HEAP_PROPERTIES,
        pheapflags: *mut D3D12_HEAP_FLAGS,
    ) -> windows_core::Result<()> {
        Resource::GetHeapProperties(self, pheapproperties, pheapflags)
    }
}

impl ID3D12Pageable_Impl for Resource_Impl {}

impl ID3D12DeviceChild_Impl for Resource_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for Resource_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12RootSignature_Impl for RootSignature_Impl {}

impl ID3D12DeviceChild_Impl for RootSignature_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for RootSignature_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
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
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for PipelineState_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12QueryHeap_Impl for QueryHeap_Impl {}

impl ID3D12Pageable_Impl for QueryHeap_Impl {}

impl ID3D12DeviceChild_Impl for QueryHeap_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for QueryHeap_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12CommandSignature_Impl for CommandSignature_Impl {}

impl ID3D12Pageable_Impl for CommandSignature_Impl {}

impl ID3D12DeviceChild_Impl for CommandSignature_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for CommandSignature_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
    }
}

impl ID3D12Heap_Impl for Heap_Impl {
    fn GetDesc(&self) -> D3D12_HEAP_DESC {
        Heap::GetDesc(self)
    }
}

impl ID3D12Pageable_Impl for Heap_Impl {}

impl ID3D12DeviceChild_Impl for Heap_Impl {
    fn GetDevice(
        &self,
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for Heap_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
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
        riid: *const windows_core::GUID,
        ppvdevice: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetDevice(riid, ppvdevice)
    }
}

impl ID3D12Object_Impl for DescriptorHeap_Impl {
    fn GetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().GetPrivateData(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows_core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateData(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows_core::GUID,
        pdata: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        self.base().SetPrivateDataInterface(guid, pdata)
    }
    fn SetName(&self, name: &windows_core::PCWSTR) -> windows_core::Result<()> {
        self.base().SetName(name)
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
