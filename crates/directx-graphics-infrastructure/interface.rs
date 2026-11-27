//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The library dxgi.dll: adapters, outputs, formats and swap chains.

use wcontainers_component_object_model as component_object_model;

use wcontainers_component_object_model::structures::HResult;

pub mod constants;
pub mod enumerations;
pub mod structures;

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The library dxgi.dll: adapters, outputs, formats and swap chains.
pub trait DirectXGraphicsInfrastructure: Sized + Send + Sync + 'static {
    type Adapter: Adapter4<Api = Self>;
    type Debug: Debug1;
    type DecodeSwapChain: DecodeSwapChain;
    type Device: Device4<Api = Self>;
    type DisplayControl: DisplayControl;
    type Factory: Factory7<Api = Self>;
    type FactoryMedia: FactoryMedia<Api = Self>;
    type InfoQueue: InfoQueue;
    type KeyedMutex: KeyedMutex<Api = Self>;
    type Output: Output6<Api = Self>;
    type OutputDuplication: OutputDuplication<Api = Self>;
    type Resource: Resource1<Api = Self>;
    type Surface: Surface2<Api = Self>;
    type SwapChain: SwapChain4<Api = Self>;
    type SwapChainMedia: SwapChainMedia;
    type GraphicsAnalysis: GraphicsAnalysis;
    /// The object an API built on this one hands out as a `IDXGISurface` (an exception line of src/api.rs).
    type Buffer: Surface2<Api = Self>;

    /// The header's `CreateDXGIFactory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-createdxgifactory).
    fn create_dxgi_factory(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self>, HResult>;
    /// The header's `CreateDXGIFactory1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-createdxgifactory1).
    fn create_dxgi_factory1(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self>, HResult>;
    /// The header's `CreateDXGIFactory2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-createdxgifactory2).
    fn create_dxgi_factory2(
        &self,
        flags: enumerations::CreateFactoryFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self>, HResult>;
    /// The header's `DXGIDeclareAdapterRemovalSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-dxgideclareadapterremovalsupport).
    fn dxgi_declare_adapter_removal_support(&self) -> Result<(), HResult>;
    /// The header's `DXGIDisableVBlankVirtualization`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-dxgidisablevblankvirtualization).
    fn dxgi_disable_v_blank_virtualization(&self) -> Result<(), HResult>;
    /// The header's `DXGIGetDebugInterface1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-dxgigetdebuginterface1).
    fn dxgi_get_debug_interface1(
        &self,
        flags: u32,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self>, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIAdapter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiadapter).
pub trait Adapter: Object {
    /// The header's `EnumOutputs`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiadapter-enumoutputs).
    fn enum_outputs(
        &self,
        output: u32,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Output, HResult>;
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiadapter-getdesc).
    fn get_desc(&self) -> Result<structures::AdapterDesc, HResult>;
    /// The header's `CheckInterfaceSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiadapter-checkinterfacesupport).
    fn check_interface_support(
        &self,
        interface_name: &component_object_model::structures::Guid,
    ) -> Result<i64, HResult>;
}

/// The header's `IDXGIAdapter1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiadapter1).
pub trait Adapter1: Adapter {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiadapter1-getdesc1).
    fn get_desc1(&self) -> Result<structures::AdapterDesc1, HResult>;
}

/// The header's `IDXGIAdapter2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiadapter2).
pub trait Adapter2: Adapter1 {
    /// The header's `GetDesc2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiadapter2-getdesc2).
    fn get_desc2(&self) -> Result<structures::AdapterDesc2, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIAdapter3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgiadapter3).
pub trait Adapter3: Adapter2 {
    /// The header's `RegisterHardwareContentProtectionTeardownStatusEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-registerhardwarecontentprotectionteardownstatusevent).
    fn register_hardware_content_protection_teardown_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterHardwareContentProtectionTeardownStatus`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-unregisterhardwarecontentprotectionteardownstatus).
    fn unregister_hardware_content_protection_teardown_status(&self, dw_cookie: u32);
    /// The header's `QueryVideoMemoryInfo`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo).
    fn query_video_memory_info(
        &self,
        node_index: u32,
        memory_segment_group: enumerations::MemorySegmentGroup,
    ) -> Result<structures::QueryVideoMemoryInfo, HResult>;
    /// The header's `SetVideoMemoryReservation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-setvideomemoryreservation).
    fn set_video_memory_reservation(
        &self,
        node_index: u32,
        memory_segment_group: enumerations::MemorySegmentGroup,
        reservation: u64,
    ) -> Result<(), HResult>;
    /// The header's `RegisterVideoMemoryBudgetChangeNotificationEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-registervideomemorybudgetchangenotificationevent).
    fn register_video_memory_budget_change_notification_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterVideoMemoryBudgetChangeNotification`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-unregistervideomemorybudgetchangenotification).
    fn unregister_video_memory_budget_change_notification(&self, dw_cookie: u32);
}

/// The header's `IDXGIAdapter4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgiadapter4).
pub trait Adapter4: Adapter3 {
    /// The header's `GetDesc3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgiadapter4-getdesc3).
    fn get_desc3(&self) -> Result<structures::AdapterDesc3, HResult>;
}

/// The header's `IDXGIDebug`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nn-dxgidebug-idxgidebug).
pub trait Debug: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `ReportLiveObjects`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgidebug-reportliveobjects).
    fn report_live_objects(
        &self,
        apiid: component_object_model::structures::Guid,
        flags: enumerations::DebugRloFlags,
    ) -> Result<(), HResult>;
}

/// The header's `IDXGIDebug1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nn-dxgidebug-idxgidebug1).
pub trait Debug1: Debug {
    /// The header's `EnableLeakTrackingForThread`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgidebug1-enableleaktrackingforthread).
    fn enable_leak_tracking_for_thread(&self);
    /// The header's `DisableLeakTrackingForThread`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgidebug1-disableleaktrackingforthread).
    fn disable_leak_tracking_for_thread(&self);
    /// The header's `IsLeakTrackingEnabledForThread`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgidebug1-isleaktrackingenabledforthread).
    fn is_leak_tracking_enabled_for_thread(&self) -> bool;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIDecodeSwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgidecodeswapchain).
pub trait DecodeSwapChain: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `PresentBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-presentbuffer).
    fn present_buffer(
        &self,
        buffer_to_present: u32,
        sync_interval: u32,
        flags: enumerations::Present,
    ) -> Result<(), HResult>;
    /// The header's `SetSourceRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-setsourcerect).
    fn set_source_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), HResult>;
    /// The header's `SetTargetRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-settargetrect).
    fn set_target_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), HResult>;
    /// The header's `SetDestSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-setdestsize).
    fn set_dest_size(&self, width: u32, height: u32) -> Result<(), HResult>;
    /// The header's `GetSourceRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-getsourcerect).
    fn get_source_rect(&self) -> Result<component_object_model::structures::Rect, HResult>;
    /// The header's `GetTargetRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-gettargetrect).
    fn get_target_rect(&self) -> Result<component_object_model::structures::Rect, HResult>;
    /// The header's `GetDestSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-getdestsize).
    fn get_dest_size(&self) -> Result<(u32, u32), HResult>;
    /// The header's `SetColorSpace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-setcolorspace).
    fn set_color_space(
        &self,
        color_space: enumerations::MultiplaneOverlayYcbcrFlags,
    ) -> Result<(), HResult>;
    /// The header's `GetColorSpace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidecodeswapchain-getcolorspace).
    fn get_color_space(&self) -> enumerations::MultiplaneOverlayYcbcrFlags;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgidevice).
pub trait Device: Object {
    /// The header's `GetAdapter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice-getadapter).
    fn get_adapter(&self)
    -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Adapter, HResult>;
    /// The header's `CreateSurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice-createsurface).
    fn create_surface(
        &self,
        desc: &structures::SurfaceDesc,
        num_surfaces: u32,
        usage: enumerations::Usage,
        shared_resource: Option<&structures::SharedResource>,
    ) -> Result<Vec<Option<<Self::Api as DirectXGraphicsInfrastructure>::Surface>>, HResult>;
    /// The header's `QueryResourceResidency`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice-queryresourceresidency).
    fn query_resource_residency(
        &self,
        resources: &[Option<UnknownRef<'_, Self::Api>>],
    ) -> Result<Vec<enumerations::Residency>, HResult>;
    /// The header's `SetGPUThreadPriority`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice-setgputhreadpriority).
    fn set_gpu_thread_priority(&self, priority: i32) -> Result<(), HResult>;
    /// The header's `GetGPUThreadPriority`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice-getgputhreadpriority).
    fn get_gpu_thread_priority(&self) -> Result<i32, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIDevice1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgidevice1).
pub trait Device1: Device {
    /// The header's `SetMaximumFrameLatency`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice1-setmaximumframelatency).
    fn set_maximum_frame_latency(&self, max_latency: u32) -> Result<(), HResult>;
    /// The header's `GetMaximumFrameLatency`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevice1-getmaximumframelatency).
    fn get_maximum_frame_latency(&self) -> Result<u32, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIDevice2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgidevice2).
pub trait Device2: Device1 {
    /// The header's `OfferResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgidevice2-offerresources).
    fn offer_resources(
        &self,
        resources: &[Option<&<Self::Api as DirectXGraphicsInfrastructure>::Resource>],
        priority: enumerations::OfferResourcePriority,
    ) -> Result<(), HResult>;
    /// The header's `ReclaimResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgidevice2-reclaimresources).
    fn reclaim_resources(
        &self,
        resources: &[Option<&<Self::Api as DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<Vec<bool>, HResult>;
    /// The header's `EnqueueSetEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgidevice2-enqueuesetevent).
    fn enqueue_set_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), HResult>;
}

/// The header's `IDXGIDevice3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgidevice3).
pub trait Device3: Device2 {
    /// The header's `Trim`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgidevice3-trim).
    fn trim(&self);
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIDevice4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgidevice4).
pub trait Device4: Device3 {
    /// The header's `OfferResources1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgidevice4-offerresources1).
    fn offer_resources1(
        &self,
        resources: &[Option<&<Self::Api as DirectXGraphicsInfrastructure>::Resource>],
        priority: enumerations::OfferResourcePriority,
        flags: u32,
    ) -> Result<(), HResult>;
    /// The header's `ReclaimResources1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgidevice4-reclaimresources1).
    fn reclaim_resources1(
        &self,
        resources: &[Option<&<Self::Api as DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<Vec<enumerations::ReclaimResourceResults>, HResult>;
}

/// The header's `IDXGIDeviceSubObject`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgidevicesubobject).
pub trait DeviceSubObject: Object {
    /// The header's `GetDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgidevicesubobject-getdevice).
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

/// The header's `IDXGIDisplayControl`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgidisplaycontrol).
pub trait DisplayControl: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `IsStereoEnabled`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgidisplaycontrol-isstereoenabled).
    fn is_stereo_enabled(&self) -> bool;
    /// The header's `SetStereoEnabled`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgidisplaycontrol-setstereoenabled).
    fn set_stereo_enabled(&self, enabled: bool);
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIFactory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgifactory).
pub trait Factory: Object {
    /// The header's `EnumAdapters`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory-enumadapters).
    fn enum_adapters(
        &self,
        adapter: u32,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Adapter, HResult>;
    /// The header's `MakeWindowAssociation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory-makewindowassociation).
    fn make_window_association(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        flags: enumerations::MwaFlags,
    ) -> Result<(), HResult>;
    /// The header's `GetWindowAssociation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory-getwindowassociation).
    fn get_window_association(&self) -> Result<component_object_model::structures::Hwnd, HResult>;
    /// The header's `CreateSwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory-createswapchain).
    fn create_swap_chain(
        &self,
        device: UnknownRef<'_, Self::Api>,
        desc: &structures::SwapChainDesc,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::SwapChain, HResult>;
    /// The header's `CreateSoftwareAdapter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory-createsoftwareadapter).
    fn create_software_adapter(
        &self,
        module: component_object_model::structures::Hmodule,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Adapter, HResult>;
}

/// The header's `IDXGIFactory1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgifactory1).
pub trait Factory1: Factory {
    /// The header's `EnumAdapters1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory1-enumadapters1).
    fn enum_adapters1(
        &self,
        adapter: u32,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Adapter, HResult>;
    /// The header's `IsCurrent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory1-iscurrent).
    fn is_current(&self) -> bool;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIFactory2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgifactory2).
pub trait Factory2: Factory1 {
    /// The header's `IsWindowedStereoEnabled`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-iswindowedstereoenabled).
    fn is_windowed_stereo_enabled(&self) -> bool;
    /// The header's `CreateSwapChainForHwnd`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforhwnd).
    fn create_swap_chain_for_hwnd(
        &self,
        device: UnknownRef<'_, Self::Api>,
        h_wnd: component_object_model::structures::Hwnd,
        desc: &structures::SwapChainDesc1,
        fullscreen_desc: Option<&structures::SwapChainFullscreenDesc>,
        restrict_to_output: Option<&<Self::Api as DirectXGraphicsInfrastructure>::Output>,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::SwapChain, HResult>;
    /// The header's `CreateSwapChainForCoreWindow`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcorewindow).
    fn create_swap_chain_for_core_window(
        &self,
        device: UnknownRef<'_, Self::Api>,
        window: UnknownRef<'_, Self::Api>,
        desc: &structures::SwapChainDesc1,
        restrict_to_output: Option<&<Self::Api as DirectXGraphicsInfrastructure>::Output>,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::SwapChain, HResult>;
    /// The header's `GetSharedResourceAdapterLuid`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-getsharedresourceadapterluid).
    fn get_shared_resource_adapter_luid(
        &self,
        h_resource: component_object_model::structures::Handle,
    ) -> Result<component_object_model::structures::Luid, HResult>;
    /// The header's `RegisterStereoStatusWindow`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-registerstereostatuswindow).
    fn register_stereo_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, HResult>;
    /// The header's `RegisterStereoStatusEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-registerstereostatusevent).
    fn register_stereo_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterStereoStatus`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-unregisterstereostatus).
    fn unregister_stereo_status(&self, dw_cookie: u32);
    /// The header's `RegisterOcclusionStatusWindow`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-registerocclusionstatuswindow).
    fn register_occlusion_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, HResult>;
    /// The header's `RegisterOcclusionStatusEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-registerocclusionstatusevent).
    fn register_occlusion_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterOcclusionStatus`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-unregisterocclusionstatus).
    fn unregister_occlusion_status(&self, dw_cookie: u32);
    /// The header's `CreateSwapChainForComposition`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcomposition).
    fn create_swap_chain_for_composition(
        &self,
        device: UnknownRef<'_, Self::Api>,
        desc: &structures::SwapChainDesc1,
        restrict_to_output: Option<&<Self::Api as DirectXGraphicsInfrastructure>::Output>,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::SwapChain, HResult>;
}

/// The header's `IDXGIFactory3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgifactory3).
pub trait Factory3: Factory2 {
    /// The header's `GetCreationFlags`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgifactory3-getcreationflags).
    fn get_creation_flags(&self) -> enumerations::CreateFactoryFlags;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIFactory4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgifactory4).
pub trait Factory4: Factory3 {
    /// The header's `EnumAdapterByLuid`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgifactory4-enumadapterbyluid).
    fn enum_adapter_by_luid(
        &self,
        adapter_luid: component_object_model::structures::Luid,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `EnumWarpAdapter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgifactory4-enumwarpadapter).
    fn enum_warp_adapter(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

/// The header's `IDXGIFactory5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgifactory5).
pub trait Factory5: Factory4 {
    /// The header's `CheckFeatureSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgifactory5-checkfeaturesupport).
    fn check_feature_support(
        &self,
        feature: enumerations::Feature,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
}

/// The header's `IDXGIFactory6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgifactory6).
pub trait Factory6: Factory5 {
    /// The header's `EnumAdapterByGpuPreference`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory6-enumadapterbygpupreference).
    fn enum_adapter_by_gpu_preference(
        &self,
        adapter: u32,
        gpu_preference: enumerations::GpuPreference,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIFactory7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgifactory7).
pub trait Factory7: Factory6 {
    /// The header's `RegisterAdaptersChangedEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory7-registeradapterschangedevent).
    fn register_adapters_changed_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterAdaptersChangedEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory7-unregisteradapterschangedevent).
    fn unregister_adapters_changed_event(&self, dw_cookie: u32) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIFactoryMedia`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgifactorymedia).
pub trait FactoryMedia: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: DirectXGraphicsInfrastructure;

    /// The header's `CreateSwapChainForCompositionSurfaceHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgifactorymedia-createswapchainforcompositionsurfacehandle).
    fn create_swap_chain_for_composition_surface_handle(
        &self,
        device: UnknownRef<'_, Self::Api>,
        h_surface: component_object_model::structures::Handle,
        desc: &structures::SwapChainDesc1,
        restrict_to_output: Option<&<Self::Api as DirectXGraphicsInfrastructure>::Output>,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::SwapChain, HResult>;
    /// The header's `CreateDecodeSwapChainForCompositionSurfaceHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgifactorymedia-createdecodeswapchainforcompositionsurfacehandle).
    fn create_decode_swap_chain_for_composition_surface_handle(
        &self,
        device: UnknownRef<'_, Self::Api>,
        h_surface: component_object_model::structures::Handle,
        desc: &structures::DecodeSwapChainDesc,
        yuv_decode_buffers: &<Self::Api as DirectXGraphicsInfrastructure>::Resource,
        restrict_to_output: Option<&<Self::Api as DirectXGraphicsInfrastructure>::Output>,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::DecodeSwapChain, HResult>;
}

pub trait GraphicsAnalysis: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    fn begin_capture(&self);
    fn end_capture(&self);
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIInfoQueue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nn-dxgidebug-idxgiinfoqueue).
pub trait InfoQueue: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `SetMessageCountLimit`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-setmessagecountlimit).
    fn set_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
        message_count_limit: u64,
    ) -> Result<(), HResult>;
    /// The header's `ClearStoredMessages`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-clearstoredmessages).
    fn clear_stored_messages(&self, producer: component_object_model::structures::Guid);
    /// The header's `GetMessage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getmessage).
    fn get_message(
        &self,
        producer: component_object_model::structures::Guid,
        message_index: u64,
        message: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `GetNumStoredMessagesAllowedByRetrievalFilters`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getnumstoredmessagesallowedbyretrievalfilters).
    fn get_num_stored_messages_allowed_by_retrieval_filters(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64;
    /// The header's `GetNumStoredMessages`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getnumstoredmessages).
    fn get_num_stored_messages(&self, producer: component_object_model::structures::Guid) -> u64;
    /// The header's `GetNumMessagesDiscardedByMessageCountLimit`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getnummessagesdiscardedbymessagecountlimit).
    fn get_num_messages_discarded_by_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64;
    /// The header's `GetMessageCountLimit`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getmessagecountlimit).
    fn get_message_count_limit(&self, producer: component_object_model::structures::Guid) -> u64;
    /// The header's `GetNumMessagesAllowedByStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getnummessagesallowedbystoragefilter).
    fn get_num_messages_allowed_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64;
    /// The header's `GetNumMessagesDeniedByStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getnummessagesdeniedbystoragefilter).
    fn get_num_messages_denied_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64;
    /// The header's `AddStorageFilterEntries`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-addstoragefilterentries).
    fn add_storage_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &structures::InfoQueueFilter,
    ) -> Result<(), HResult>;
    /// The header's `GetStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getstoragefilter).
    fn get_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `ClearStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-clearstoragefilter).
    fn clear_storage_filter(&self, producer: component_object_model::structures::Guid);
    /// The header's `PushEmptyStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushemptystoragefilter).
    fn push_empty_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), HResult>;
    /// The header's `PushDenyAllStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushdenyallstoragefilter).
    fn push_deny_all_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), HResult>;
    /// The header's `PushCopyOfStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushcopyofstoragefilter).
    fn push_copy_of_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), HResult>;
    /// The header's `PushStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushstoragefilter).
    fn push_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &structures::InfoQueueFilter,
    ) -> Result<(), HResult>;
    /// The header's `PopStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-popstoragefilter).
    fn pop_storage_filter(&self, producer: component_object_model::structures::Guid);
    /// The header's `GetStorageFilterStackSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getstoragefilterstacksize).
    fn get_storage_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32;
    /// The header's `AddRetrievalFilterEntries`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-addretrievalfilterentries).
    fn add_retrieval_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &structures::InfoQueueFilter,
    ) -> Result<(), HResult>;
    /// The header's `GetRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getretrievalfilter).
    fn get_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `ClearRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-clearretrievalfilter).
    fn clear_retrieval_filter(&self, producer: component_object_model::structures::Guid);
    /// The header's `PushEmptyRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushemptyretrievalfilter).
    fn push_empty_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), HResult>;
    /// The header's `PushDenyAllRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushdenyallretrievalfilter).
    fn push_deny_all_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), HResult>;
    /// The header's `PushCopyOfRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushcopyofretrievalfilter).
    fn push_copy_of_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), HResult>;
    /// The header's `PushRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-pushretrievalfilter).
    fn push_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &structures::InfoQueueFilter,
    ) -> Result<(), HResult>;
    /// The header's `PopRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-popretrievalfilter).
    fn pop_retrieval_filter(&self, producer: component_object_model::structures::Guid);
    /// The header's `GetRetrievalFilterStackSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getretrievalfilterstacksize).
    fn get_retrieval_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32;
    /// The header's `AddMessage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-addmessage).
    fn add_message(
        &self,
        producer: component_object_model::structures::Guid,
        category: enumerations::InfoQueueMessageCategory,
        severity: enumerations::InfoQueueMessageSeverity,
        id: i32,
        description: &std::ffi::CStr,
    ) -> Result<(), HResult>;
    /// The header's `AddApplicationMessage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-addapplicationmessage).
    fn add_application_message(
        &self,
        severity: enumerations::InfoQueueMessageSeverity,
        description: &std::ffi::CStr,
    ) -> Result<(), HResult>;
    /// The header's `SetBreakOnCategory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-setbreakoncategory).
    fn set_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: enumerations::InfoQueueMessageCategory,
        b_enable: bool,
    ) -> Result<(), HResult>;
    /// The header's `SetBreakOnSeverity`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-setbreakonseverity).
    fn set_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: enumerations::InfoQueueMessageSeverity,
        b_enable: bool,
    ) -> Result<(), HResult>;
    /// The header's `SetBreakOnID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-setbreakonid).
    fn set_break_on_id(
        &self,
        producer: component_object_model::structures::Guid,
        id: i32,
        b_enable: bool,
    ) -> Result<(), HResult>;
    /// The header's `GetBreakOnCategory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getbreakoncategory).
    fn get_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: enumerations::InfoQueueMessageCategory,
    ) -> bool;
    /// The header's `GetBreakOnSeverity`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getbreakonseverity).
    fn get_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: enumerations::InfoQueueMessageSeverity,
    ) -> bool;
    /// The header's `GetBreakOnID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getbreakonid).
    fn get_break_on_id(&self, producer: component_object_model::structures::Guid, id: i32) -> bool;
    /// The header's `SetMuteDebugOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-setmutedebugoutput).
    fn set_mute_debug_output(
        &self,
        producer: component_object_model::structures::Guid,
        b_mute: bool,
    );
    /// The header's `GetMuteDebugOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgidebug/nf-dxgidebug-idxgiinfoqueue-getmutedebugoutput).
    fn get_mute_debug_output(&self, producer: component_object_model::structures::Guid) -> bool;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIKeyedMutex`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgikeyedmutex).
pub trait KeyedMutex: DeviceSubObject {
    /// The header's `AcquireSync`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgikeyedmutex-acquiresync).
    fn acquire_sync(&self, key: u64, dw_milliseconds: u32) -> Result<(), HResult>;
    /// The header's `ReleaseSync`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgikeyedmutex-releasesync).
    fn release_sync(&self, key: u64) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIObject`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiobject).
pub trait Object: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: DirectXGraphicsInfrastructure;

    /// The header's `SetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiobject-setprivatedata).
    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), HResult>;
    /// The header's `SetPrivateDataInterface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiobject-setprivatedatainterface).
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<UnknownRef<'_, Self::Api>>,
    ) -> Result<(), HResult>;
    /// The header's `GetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiobject-getprivatedata).
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `GetParent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiobject-getparent).
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgioutput).
pub trait Output: Object {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-getdesc).
    fn get_desc(&self) -> Result<structures::OutputDesc, HResult>;
    /// The header's `GetDisplayModeList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-getdisplaymodelist).
    fn get_display_mode_list(
        &self,
        enum_format: enumerations::Format,
        flags: enumerations::EnumModes,
    ) -> Result<Vec<structures::ModeDesc>, HResult>;
    /// The header's `FindClosestMatchingMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-findclosestmatchingmode).
    fn find_closest_matching_mode(
        &self,
        mode_to_match: &structures::ModeDesc,
        concerned_device: Option<UnknownRef<'_, Self::Api>>,
    ) -> Result<structures::ModeDesc, HResult>;
    /// The header's `WaitForVBlank`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-waitforvblank).
    fn wait_for_v_blank(&self) -> Result<(), HResult>;
    /// The header's `TakeOwnership`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-takeownership).
    fn take_ownership(
        &self,
        device: UnknownRef<'_, Self::Api>,
        exclusive: bool,
    ) -> Result<(), HResult>;
    /// The header's `ReleaseOwnership`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-releaseownership).
    fn release_ownership(&self);
    /// The header's `GetGammaControlCapabilities`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-getgammacontrolcapabilities).
    fn get_gamma_control_capabilities(
        &self,
    ) -> Result<structures::GammaControlCapabilities, HResult>;
    /// The header's `SetGammaControl`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-setgammacontrol).
    fn set_gamma_control(&self, array: &structures::GammaControl) -> Result<(), HResult>;
    /// The header's `GetGammaControl`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-getgammacontrol).
    fn get_gamma_control(&self) -> Result<structures::GammaControl, HResult>;
    /// The header's `SetDisplaySurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-setdisplaysurface).
    fn set_display_surface(
        &self,
        scanout_surface: &<Self::Api as DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), HResult>;
    /// The header's `GetDisplaySurfaceData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-getdisplaysurfacedata).
    fn get_display_surface_data(
        &self,
        destination: &<Self::Api as DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), HResult>;
    /// The header's `GetFrameStatistics`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgioutput-getframestatistics).
    fn get_frame_statistics(&self) -> Result<structures::FrameStatistics, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIOutput1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgioutput1).
pub trait Output1: Output {
    /// The header's `GetDisplayModeList1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-getdisplaymodelist1).
    fn get_display_mode_list1(
        &self,
        enum_format: enumerations::Format,
        flags: enumerations::EnumModes,
    ) -> Result<Vec<structures::ModeDesc1>, HResult>;
    /// The header's `FindClosestMatchingMode1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-findclosestmatchingmode1).
    fn find_closest_matching_mode1(
        &self,
        mode_to_match: &structures::ModeDesc1,
        concerned_device: Option<UnknownRef<'_, Self::Api>>,
    ) -> Result<structures::ModeDesc1, HResult>;
    /// The header's `GetDisplaySurfaceData1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-getdisplaysurfacedata1).
    fn get_display_surface_data1(
        &self,
        destination: &<Self::Api as DirectXGraphicsInfrastructure>::Resource,
    ) -> Result<(), HResult>;
    /// The header's `DuplicateOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput).
    fn duplicate_output(
        &self,
        device: UnknownRef<'_, Self::Api>,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::OutputDuplication, HResult>;
}

/// The header's `IDXGIOutput2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgioutput2).
pub trait Output2: Output1 {
    /// The header's `SupportsOverlays`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgioutput2-supportsoverlays).
    fn supports_overlays(&self) -> bool;
}

/// The header's `IDXGIOutput3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgioutput3).
pub trait Output3: Output2 {
    /// The header's `CheckOverlaySupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgioutput3-checkoverlaysupport).
    fn check_overlay_support(
        &self,
        enum_format: enumerations::Format,
        concerned_device: UnknownRef<'_, Self::Api>,
    ) -> Result<u32, HResult>;
}

/// The header's `IDXGIOutput4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgioutput4).
pub trait Output4: Output3 {
    /// The header's `CheckOverlayColorSpaceSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgioutput4-checkoverlaycolorspacesupport).
    fn check_overlay_color_space_support(
        &self,
        format: enumerations::Format,
        color_space: enumerations::ColorSpaceType,
        concerned_device: UnknownRef<'_, Self::Api>,
    ) -> Result<u32, HResult>;
}

/// The header's `IDXGIOutput5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgioutput5).
pub trait Output5: Output4 {
    /// The header's `DuplicateOutput1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgioutput5-duplicateoutput1).
    fn duplicate_output1(
        &self,
        device: UnknownRef<'_, Self::Api>,
        flags: u32,
        supported_formats: &[enumerations::Format],
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::OutputDuplication, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIOutput6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nn-dxgi1_6-idxgioutput6).
pub trait Output6: Output5 {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgioutput6-getdesc1).
    fn get_desc1(&self) -> Result<structures::OutputDesc1, HResult>;
    /// The header's `CheckHardwareCompositionSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgioutput6-checkhardwarecompositionsupport).
    fn check_hardware_composition_support(&self) -> Result<u32, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIOutputDuplication`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgioutputduplication).
pub trait OutputDuplication: Object {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-getdesc).
    fn get_desc(&self) -> structures::OutduplDesc;
    /// The header's `AcquireNextFrame`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe).
    fn acquire_next_frame(
        &self,
        timeout_in_milliseconds: u32,
    ) -> Result<
        (
            structures::OutduplFrameInfo,
            <Self::Api as DirectXGraphicsInfrastructure>::Resource,
        ),
        HResult,
    >;
    /// The header's `GetFrameDirtyRects`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-getframedirtyrects).
    fn get_frame_dirty_rects(
        &self,
        dirty_rects_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, HResult>;
    /// The header's `GetFrameMoveRects`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-getframemoverects).
    fn get_frame_move_rects(
        &self,
        move_rect_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, HResult>;
    /// The header's `GetFramePointerShape`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-getframepointershape).
    fn get_frame_pointer_shape(
        &self,
        pointer_shape_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(u32, structures::OutduplPointerShapeInfo), HResult>;
    /// The header's `MapDesktopSurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-mapdesktopsurface).
    fn map_desktop_surface(&self) -> Result<structures::MappedRect, HResult>;
    /// The header's `UnMapDesktopSurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-unmapdesktopsurface).
    fn un_map_desktop_surface(&self) -> Result<(), HResult>;
    /// The header's `ReleaseFrame`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-releaseframe).
    fn release_frame(&self) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiresource).
pub trait Resource: DeviceSubObject {
    /// The header's `GetSharedHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiresource-getsharedhandle).
    fn get_shared_handle(&self) -> Result<component_object_model::structures::Handle, HResult>;
    /// The header's `GetUsage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiresource-getusage).
    fn get_usage(&self) -> Result<enumerations::Usage, HResult>;
    /// The header's `SetEvictionPriority`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiresource-setevictionpriority).
    fn set_eviction_priority(
        &self,
        eviction_priority: enumerations::ResourcePriority,
    ) -> Result<(), HResult>;
    /// The header's `GetEvictionPriority`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiresource-getevictionpriority).
    fn get_eviction_priority(&self) -> Result<enumerations::ResourcePriority, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGIResource1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiresource1).
pub trait Resource1: Resource {
    /// The header's `CreateSubresourceSurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiresource1-createsubresourcesurface).
    fn create_subresource_surface(
        &self,
        index: u32,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Surface, HResult>;
    /// The header's `CreateSharedHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiresource1-createsharedhandle).
    fn create_shared_handle(
        &self,
        attributes: Option<&component_object_model::structures::SecurityAttributes>,
        dw_access: u32,
        lp_name: Option<&[u16]>,
    ) -> Result<component_object_model::structures::Handle, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISurface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgisurface).
pub trait Surface: DeviceSubObject {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgisurface-getdesc).
    fn get_desc(&self) -> Result<structures::SurfaceDesc, HResult>;
    /// The header's `Map`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgisurface-map).
    fn map(&self, map_flags: enumerations::MapFlags) -> Result<structures::MappedRect, HResult>;
    /// The header's `Unmap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgisurface-unmap).
    fn unmap(&self) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISurface1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgisurface1).
pub trait Surface1: Surface {
    /// The header's `GetDC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgisurface1-getdc).
    fn get_dc(&self, discard: bool) -> Result<component_object_model::structures::Hdc, HResult>;
    /// The header's `ReleaseDC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgisurface1-releasedc).
    fn release_dc(
        &self,
        dirty_rect: Option<&component_object_model::structures::Rect>,
    ) -> Result<(), HResult>;
}

/// The header's `IDXGISurface2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgisurface2).
pub trait Surface2: Surface1 {
    /// The header's `GetResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgisurface2-getresource).
    fn get_resource(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<(UnknownObject<Self::Api>, u32), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgiswapchain).
pub trait SwapChain: DeviceSubObject {
    /// The header's `Present`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-present).
    fn present(&self, sync_interval: u32, flags: enumerations::Present) -> Result<(), HResult>;
    /// The header's `GetBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getbuffer).
    fn get_buffer(
        &self,
        buffer: u32,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Buffer, HResult>;
    /// The header's `SetFullscreenState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-setfullscreenstate).
    fn set_fullscreen_state(
        &self,
        fullscreen: bool,
        target: Option<&<Self::Api as DirectXGraphicsInfrastructure>::Output>,
    ) -> Result<(), HResult>;
    /// The header's `GetFullscreenState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getfullscreenstate).
    fn get_fullscreen_state(
        &self,
    ) -> Result<(bool, <Self::Api as DirectXGraphicsInfrastructure>::Output), HResult>;
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getdesc).
    fn get_desc(&self) -> Result<structures::SwapChainDesc, HResult>;
    /// The header's `ResizeBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-resizebuffers).
    fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: enumerations::Format,
        swap_chain_flags: u32,
    ) -> Result<(), HResult>;
    /// The header's `ResizeTarget`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-resizetarget).
    fn resize_target(&self, new_target_parameters: &structures::ModeDesc) -> Result<(), HResult>;
    /// The header's `GetContainingOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getcontainingoutput).
    fn get_containing_output(
        &self,
    ) -> Result<<Self::Api as DirectXGraphicsInfrastructure>::Output, HResult>;
    /// The header's `GetFrameStatistics`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getframestatistics).
    fn get_frame_statistics(&self) -> Result<structures::FrameStatistics, HResult>;
    /// The header's `GetLastPresentCount`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getlastpresentcount).
    fn get_last_present_count(&self) -> Result<u32, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISwapChain1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiswapchain1).
pub trait SwapChain1: SwapChain {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-getdesc1).
    fn get_desc1(&self) -> Result<structures::SwapChainDesc1, HResult>;
    /// The header's `GetFullscreenDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-getfullscreendesc).
    fn get_fullscreen_desc(&self) -> Result<structures::SwapChainFullscreenDesc, HResult>;
    /// The header's `GetHwnd`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-gethwnd).
    fn get_hwnd(&self) -> Result<component_object_model::structures::Hwnd, HResult>;
    /// The header's `GetCoreWindow`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-getcorewindow).
    fn get_core_window(
        &self,
        refiid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `Present1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-present1).
    fn present1(
        &self,
        sync_interval: u32,
        present_flags: enumerations::Present,
        present_parameters: &structures::PresentParameters,
    ) -> Result<(), HResult>;
    /// The header's `IsTemporaryMonoSupported`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-istemporarymonosupported).
    fn is_temporary_mono_supported(&self) -> bool;
    /// The header's `GetRestrictToOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-getrestricttooutput).
    fn get_restrict_to_output(
        &self,
    ) -> Result<Option<<Self::Api as DirectXGraphicsInfrastructure>::Output>, HResult>;
    /// The header's `SetBackgroundColor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-setbackgroundcolor).
    fn set_background_color(&self, color: &structures::Rgba) -> Result<(), HResult>;
    /// The header's `GetBackgroundColor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-getbackgroundcolor).
    fn get_background_color(&self) -> Result<structures::Rgba, HResult>;
    /// The header's `SetRotation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-setrotation).
    fn set_rotation(&self, rotation: enumerations::ModeRotation) -> Result<(), HResult>;
    /// The header's `GetRotation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-getrotation).
    fn get_rotation(&self) -> Result<enumerations::ModeRotation, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISwapChain2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgiswapchain2).
pub trait SwapChain2: SwapChain1 {
    /// The header's `SetSourceSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-setsourcesize).
    fn set_source_size(&self, width: u32, height: u32) -> Result<(), HResult>;
    /// The header's `GetSourceSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getsourcesize).
    fn get_source_size(&self) -> Result<(u32, u32), HResult>;
    /// The header's `SetMaximumFrameLatency`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-setmaximumframelatency).
    fn set_maximum_frame_latency(&self, max_latency: u32) -> Result<(), HResult>;
    /// The header's `GetMaximumFrameLatency`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getmaximumframelatency).
    fn get_maximum_frame_latency(&self) -> Result<u32, HResult>;
    /// The header's `GetFrameLatencyWaitableObject`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject).
    fn get_frame_latency_waitable_object(&self) -> component_object_model::structures::Handle;
    /// The header's `SetMatrixTransform`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-setmatrixtransform).
    fn set_matrix_transform(&self, matrix: &structures::Matrix3x2F) -> Result<(), HResult>;
    /// The header's `GetMatrixTransform`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getmatrixtransform).
    fn get_matrix_transform(&self) -> Result<structures::Matrix3x2F, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISwapChain3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nn-dxgi1_4-idxgiswapchain3).
pub trait SwapChain3: SwapChain2 {
    /// The header's `GetCurrentBackBufferIndex`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiswapchain3-getcurrentbackbufferindex).
    fn get_current_back_buffer_index(&self) -> u32;
    /// The header's `CheckColorSpaceSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiswapchain3-checkcolorspacesupport).
    fn check_color_space_support(
        &self,
        color_space: enumerations::ColorSpaceType,
    ) -> Result<u32, HResult>;
    /// The header's `SetColorSpace1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiswapchain3-setcolorspace1).
    fn set_color_space1(&self, color_space: enumerations::ColorSpaceType) -> Result<(), HResult>;
    /// The header's `ResizeBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiswapchain3-resizebuffers1).
    fn resize_buffers1(
        &self,
        width: u32,
        height: u32,
        format: enumerations::Format,
        swap_chain_flags: u32,
        creation_node_mask: &[u32],
        present_queue: &[Option<UnknownRef<'_, Self::Api>>],
    ) -> Result<(), HResult>;
}

/// The header's `IDXGISwapChain4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nn-dxgi1_5-idxgiswapchain4).
pub trait SwapChain4: SwapChain3 {
    /// The header's `SetHDRMetaData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_5/nf-dxgi1_5-idxgiswapchain4-sethdrmetadata).
    fn set_hdr_meta_data(
        &self,
        type_: enumerations::HdrMetadataType,
        meta_data: Option<&[u8]>,
    ) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `IDXGISwapChainMedia`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nn-dxgi1_3-idxgiswapchainmedia).
pub trait SwapChainMedia: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `GetFrameStatisticsMedia`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchainmedia-getframestatisticsmedia).
    fn get_frame_statistics_media(&self) -> Result<structures::FrameStatisticsMedia, HResult>;
    /// The header's `SetPresentDuration`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchainmedia-setpresentduration).
    fn set_present_duration(&self, duration: u32) -> Result<(), HResult>;
    /// The header's `CheckPresentDurationSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchainmedia-checkpresentdurationsupport).
    fn check_present_duration_support(
        &self,
        desired_present_duration: u32,
    ) -> Result<(u32, u32), HResult>;
}

/// An `IUnknown*`: an object of one of this API's kinds, or of none.
pub enum UnknownRef<'a, A: DirectXGraphicsInfrastructure> {
    Adapter(&'a <A as DirectXGraphicsInfrastructure>::Adapter),
    Debug(&'a <A as DirectXGraphicsInfrastructure>::Debug),
    DecodeSwapChain(&'a <A as DirectXGraphicsInfrastructure>::DecodeSwapChain),
    Device(&'a <A as DirectXGraphicsInfrastructure>::Device),
    DisplayControl(&'a <A as DirectXGraphicsInfrastructure>::DisplayControl),
    Factory(&'a <A as DirectXGraphicsInfrastructure>::Factory),
    FactoryMedia(&'a <A as DirectXGraphicsInfrastructure>::FactoryMedia),
    InfoQueue(&'a <A as DirectXGraphicsInfrastructure>::InfoQueue),
    KeyedMutex(&'a <A as DirectXGraphicsInfrastructure>::KeyedMutex),
    Output(&'a <A as DirectXGraphicsInfrastructure>::Output),
    OutputDuplication(&'a <A as DirectXGraphicsInfrastructure>::OutputDuplication),
    Resource(&'a <A as DirectXGraphicsInfrastructure>::Resource),
    Surface(&'a <A as DirectXGraphicsInfrastructure>::Surface),
    SwapChain(&'a <A as DirectXGraphicsInfrastructure>::SwapChain),
    SwapChainMedia(&'a <A as DirectXGraphicsInfrastructure>::SwapChainMedia),
    GraphicsAnalysis(&'a <A as DirectXGraphicsInfrastructure>::GraphicsAnalysis),
    Unknown(&'a component_object_model::structures::Foreign),
}

/// An `IUnknown*`: an object of one of this API's kinds, or of none.
pub enum UnknownObject<A: DirectXGraphicsInfrastructure> {
    Adapter(<A as DirectXGraphicsInfrastructure>::Adapter),
    Debug(<A as DirectXGraphicsInfrastructure>::Debug),
    DecodeSwapChain(<A as DirectXGraphicsInfrastructure>::DecodeSwapChain),
    Device(<A as DirectXGraphicsInfrastructure>::Device),
    DisplayControl(<A as DirectXGraphicsInfrastructure>::DisplayControl),
    Factory(<A as DirectXGraphicsInfrastructure>::Factory),
    FactoryMedia(<A as DirectXGraphicsInfrastructure>::FactoryMedia),
    InfoQueue(<A as DirectXGraphicsInfrastructure>::InfoQueue),
    KeyedMutex(<A as DirectXGraphicsInfrastructure>::KeyedMutex),
    Output(<A as DirectXGraphicsInfrastructure>::Output),
    OutputDuplication(<A as DirectXGraphicsInfrastructure>::OutputDuplication),
    Resource(<A as DirectXGraphicsInfrastructure>::Resource),
    Surface(<A as DirectXGraphicsInfrastructure>::Surface),
    SwapChain(<A as DirectXGraphicsInfrastructure>::SwapChain),
    SwapChainMedia(<A as DirectXGraphicsInfrastructure>::SwapChainMedia),
    GraphicsAnalysis(<A as DirectXGraphicsInfrastructure>::GraphicsAnalysis),
    Unknown(component_object_model::structures::Foreign),
}
