//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The C tables of this API's interfaces, and its library's exports.

use wcontainers_component_object_model_direct as component_object_model_direct;

use wcontainers_component_object_model_direct::{Interface, Raw, UnknownTable};

/// `IDXGIAdapter`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct AdapterTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub adapter_enum_outputs: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub adapter_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter_check_interface_support: unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
}

// SAFETY: `AdapterTable` lays out `IDXGIAdapter` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIAdapter"
)]
unsafe impl Interface for AdapterTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIAdapter1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Adapter1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub adapter_enum_outputs: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub adapter_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter_check_interface_support: unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub adapter1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `Adapter1Table` lays out `IDXGIAdapter1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIAdapter1"
)]
unsafe impl Interface for Adapter1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIAdapter2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Adapter2Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub adapter_enum_outputs: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub adapter_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter_check_interface_support: unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub adapter1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter2_get_desc2: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `Adapter2Table` lays out `IDXGIAdapter2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIAdapter2"
)]
unsafe impl Interface for Adapter2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIAdapter3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Adapter3Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub adapter_enum_outputs: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub adapter_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter_check_interface_support: unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub adapter1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter2_get_desc2: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub adapter3_register_hardware_content_protection_teardown_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub adapter3_unregister_hardware_content_protection_teardown_status:
        unsafe extern "system" fn(Raw, u32),
    pub adapter3_query_video_memory_info: unsafe extern "system" fn(Raw, u32, i32, *mut u8) -> i32,
    pub adapter3_set_video_memory_reservation: unsafe extern "system" fn(Raw, u32, i32, u64) -> i32,
    pub adapter3_register_video_memory_budget_change_notification_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub adapter3_unregister_video_memory_budget_change_notification:
        unsafe extern "system" fn(Raw, u32),
}

// SAFETY: `Adapter3Table` lays out `IDXGIAdapter3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIAdapter3"
)]
unsafe impl Interface for Adapter3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIAdapter4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Adapter4Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) adapter_enum_outputs: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) adapter_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) adapter_check_interface_support:
        unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub(crate) adapter1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) adapter2_get_desc2: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) adapter3_register_hardware_content_protection_teardown_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) adapter3_unregister_hardware_content_protection_teardown_status:
        unsafe extern "system" fn(Raw, u32),
    pub(crate) adapter3_query_video_memory_info:
        unsafe extern "system" fn(Raw, u32, i32, *mut u8) -> i32,
    pub(crate) adapter3_set_video_memory_reservation:
        unsafe extern "system" fn(Raw, u32, i32, u64) -> i32,
    pub(crate) adapter3_register_video_memory_budget_change_notification_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) adapter3_unregister_video_memory_budget_change_notification:
        unsafe extern "system" fn(Raw, u32),
    pub(crate) adapter4_get_desc3: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `Adapter4Table` lays out `IDXGIAdapter4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIAdapter4"
)]
unsafe impl Interface for Adapter4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDebug`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DebugTable {
    pub unknown: UnknownTable,
    pub debug_report_live_objects:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32) -> i32,
}

// SAFETY: `DebugTable` lays out `IDXGIDebug` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDebug"
)]
unsafe impl Interface for DebugTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDebug1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Debug1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) debug_report_live_objects:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32) -> i32,
    pub(crate) debug1_enable_leak_tracking_for_thread: unsafe extern "system" fn(Raw),
    pub(crate) debug1_disable_leak_tracking_for_thread: unsafe extern "system" fn(Raw),
    pub(crate) debug1_is_leak_tracking_enabled_for_thread: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `Debug1Table` lays out `IDXGIDebug1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDebug1"
)]
unsafe impl Interface for Debug1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDecodeSwapChain`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DecodeSwapChainTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) decode_swap_chain_present_buffer:
        unsafe extern "system" fn(Raw, u32, u32, u32) -> i32,
    pub(crate) decode_swap_chain_set_source_rect: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) decode_swap_chain_set_target_rect: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) decode_swap_chain_set_dest_size: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub(crate) decode_swap_chain_get_source_rect: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) decode_swap_chain_get_target_rect: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) decode_swap_chain_get_dest_size:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) decode_swap_chain_set_color_space: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) decode_swap_chain_get_color_space: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `DecodeSwapChainTable` lays out `IDXGIDecodeSwapChain` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDecodeSwapChain"
)]
unsafe impl Interface for DecodeSwapChainTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDevice`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_get_adapter: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_surface:
        unsafe extern "system" fn(Raw, *const u8, u32, u32, *const u8, *mut Raw) -> i32,
    pub device_query_resource_residency:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, u32) -> i32,
    pub device_set_gpu_thread_priority: unsafe extern "system" fn(Raw, i32) -> i32,
    pub device_get_gpu_thread_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `DeviceTable` lays out `IDXGIDevice` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDevice"
)]
unsafe impl Interface for DeviceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDevice1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_get_adapter: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_surface:
        unsafe extern "system" fn(Raw, *const u8, u32, u32, *const u8, *mut Raw) -> i32,
    pub device_query_resource_residency:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, u32) -> i32,
    pub device_set_gpu_thread_priority: unsafe extern "system" fn(Raw, i32) -> i32,
    pub device_get_gpu_thread_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device1_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device1_get_maximum_frame_latency: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `Device1Table` lays out `IDXGIDevice1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDevice1"
)]
unsafe impl Interface for Device1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDevice2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device2Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_get_adapter: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_surface:
        unsafe extern "system" fn(Raw, *const u8, u32, u32, *const u8, *mut Raw) -> i32,
    pub device_query_resource_residency:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, u32) -> i32,
    pub device_set_gpu_thread_priority: unsafe extern "system" fn(Raw, i32) -> i32,
    pub device_get_gpu_thread_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device1_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device1_get_maximum_frame_latency: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device2_offer_resources: unsafe extern "system" fn(Raw, u32, *const Raw, i32) -> i32,
    pub device2_reclaim_resources: unsafe extern "system" fn(Raw, u32, *const Raw, *mut u8) -> i32,
    pub device2_enqueue_set_event: unsafe extern "system" fn(Raw, usize) -> i32,
}

// SAFETY: `Device2Table` lays out `IDXGIDevice2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDevice2"
)]
unsafe impl Interface for Device2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDevice3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device3Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_get_adapter: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_surface:
        unsafe extern "system" fn(Raw, *const u8, u32, u32, *const u8, *mut Raw) -> i32,
    pub device_query_resource_residency:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, u32) -> i32,
    pub device_set_gpu_thread_priority: unsafe extern "system" fn(Raw, i32) -> i32,
    pub device_get_gpu_thread_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device1_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device1_get_maximum_frame_latency: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device2_offer_resources: unsafe extern "system" fn(Raw, u32, *const Raw, i32) -> i32,
    pub device2_reclaim_resources: unsafe extern "system" fn(Raw, u32, *const Raw, *mut u8) -> i32,
    pub device2_enqueue_set_event: unsafe extern "system" fn(Raw, usize) -> i32,
    pub device3_trim: unsafe extern "system" fn(Raw),
}

// SAFETY: `Device3Table` lays out `IDXGIDevice3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDevice3"
)]
unsafe impl Interface for Device3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDevice4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device4Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_get_adapter: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_surface:
        unsafe extern "system" fn(Raw, *const u8, u32, u32, *const u8, *mut Raw) -> i32,
    pub device_query_resource_residency:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, u32) -> i32,
    pub device_set_gpu_thread_priority: unsafe extern "system" fn(Raw, i32) -> i32,
    pub device_get_gpu_thread_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device1_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device1_get_maximum_frame_latency: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub device2_offer_resources: unsafe extern "system" fn(Raw, u32, *const Raw, i32) -> i32,
    pub device2_reclaim_resources: unsafe extern "system" fn(Raw, u32, *const Raw, *mut u8) -> i32,
    pub device2_enqueue_set_event: unsafe extern "system" fn(Raw, usize) -> i32,
    pub device3_trim: unsafe extern "system" fn(Raw),
    pub device4_offer_resources1: unsafe extern "system" fn(Raw, u32, *const Raw, i32, u32) -> i32,
    pub device4_reclaim_resources1: unsafe extern "system" fn(Raw, u32, *const Raw, *mut u8) -> i32,
}

// SAFETY: `Device4Table` lays out `IDXGIDevice4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDevice4"
)]
unsafe impl Interface for Device4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDeviceSubObject`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceSubObjectTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
}

// SAFETY: `DeviceSubObjectTable` lays out `IDXGIDeviceSubObject` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDeviceSubObject"
)]
unsafe impl Interface for DeviceSubObjectTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIDisplayControl`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DisplayControlTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) display_control_is_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) display_control_set_stereo_enabled: unsafe extern "system" fn(Raw, i32),
}

// SAFETY: `DisplayControlTable` lays out `IDXGIDisplayControl` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIDisplayControl"
)]
unsafe impl Interface for DisplayControlTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct FactoryTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
}

// SAFETY: `FactoryTable` lays out `IDXGIFactory` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory"
)]
unsafe impl Interface for FactoryTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Factory1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `Factory1Table` lays out `IDXGIFactory1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory1"
)]
unsafe impl Interface for Factory1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Factory2Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_is_windowed_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_create_swap_chain_for_hwnd:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_create_swap_chain_for_core_window:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_get_shared_resource_adapter_luid:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_register_stereo_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_stereo_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_stereo_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_register_occlusion_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_occlusion_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_occlusion_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_create_swap_chain_for_composition:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
}

// SAFETY: `Factory2Table` lays out `IDXGIFactory2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory2"
)]
unsafe impl Interface for Factory2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Factory3Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_is_windowed_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_create_swap_chain_for_hwnd:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_create_swap_chain_for_core_window:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_get_shared_resource_adapter_luid:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_register_stereo_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_stereo_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_stereo_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_register_occlusion_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_occlusion_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_occlusion_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_create_swap_chain_for_composition:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory3_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
}

// SAFETY: `Factory3Table` lays out `IDXGIFactory3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory3"
)]
unsafe impl Interface for Factory3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Factory4Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_is_windowed_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_create_swap_chain_for_hwnd:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_create_swap_chain_for_core_window:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_get_shared_resource_adapter_luid:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_register_stereo_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_stereo_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_stereo_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_register_occlusion_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_occlusion_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_occlusion_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_create_swap_chain_for_composition:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory3_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub factory4_enum_adapter_by_luid: unsafe extern "system" fn(Raw, u64, *mut u8, *mut u8) -> i32,
    pub factory4_enum_warp_adapter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
}

// SAFETY: `Factory4Table` lays out `IDXGIFactory4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory4"
)]
unsafe impl Interface for Factory4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory5`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Factory5Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_is_windowed_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_create_swap_chain_for_hwnd:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_create_swap_chain_for_core_window:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_get_shared_resource_adapter_luid:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_register_stereo_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_stereo_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_stereo_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_register_occlusion_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_occlusion_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_occlusion_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_create_swap_chain_for_composition:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory3_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub factory4_enum_adapter_by_luid: unsafe extern "system" fn(Raw, u64, *mut u8, *mut u8) -> i32,
    pub factory4_enum_warp_adapter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory5_check_feature_support: unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
}

// SAFETY: `Factory5Table` lays out `IDXGIFactory5` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory5"
)]
unsafe impl Interface for Factory5Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory6`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Factory6Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub factory_create_swap_chain: unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub factory_create_software_adapter: unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_is_windowed_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub factory2_create_swap_chain_for_hwnd:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_create_swap_chain_for_core_window:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory2_get_shared_resource_adapter_luid:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_register_stereo_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_stereo_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_stereo_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_register_occlusion_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub factory2_register_occlusion_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub factory2_unregister_occlusion_status: unsafe extern "system" fn(Raw, u32),
    pub factory2_create_swap_chain_for_composition:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub factory3_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub factory4_enum_adapter_by_luid: unsafe extern "system" fn(Raw, u64, *mut u8, *mut u8) -> i32,
    pub factory4_enum_warp_adapter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub factory5_check_feature_support: unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub factory6_enum_adapter_by_gpu_preference:
        unsafe extern "system" fn(Raw, u32, i32, *const u8, *mut Raw) -> i32,
}

// SAFETY: `Factory6Table` lays out `IDXGIFactory6` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory6"
)]
unsafe impl Interface for Factory6Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactory7`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Factory7Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) factory_enum_adapters: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) factory_make_window_association: unsafe extern "system" fn(Raw, usize, u32) -> i32,
    pub(crate) factory_get_window_association: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) factory_create_swap_chain:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) factory_create_software_adapter:
        unsafe extern "system" fn(Raw, usize, *mut Raw) -> i32,
    pub(crate) factory1_enum_adapters1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) factory1_is_current: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) factory2_is_windowed_stereo_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) factory2_create_swap_chain_for_hwnd:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, *const u8, Raw, *mut Raw) -> i32,
    pub(crate) factory2_create_swap_chain_for_core_window:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub(crate) factory2_get_shared_resource_adapter_luid:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) factory2_register_stereo_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub(crate) factory2_register_stereo_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) factory2_unregister_stereo_status: unsafe extern "system" fn(Raw, u32),
    pub(crate) factory2_register_occlusion_status_window:
        unsafe extern "system" fn(Raw, usize, u32, *mut u8) -> i32,
    pub(crate) factory2_register_occlusion_status_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) factory2_unregister_occlusion_status: unsafe extern "system" fn(Raw, u32),
    pub(crate) factory2_create_swap_chain_for_composition:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *mut Raw) -> i32,
    pub(crate) factory3_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) factory4_enum_adapter_by_luid:
        unsafe extern "system" fn(Raw, u64, *mut u8, *mut u8) -> i32,
    pub(crate) factory4_enum_warp_adapter:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) factory5_check_feature_support:
        unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub(crate) factory6_enum_adapter_by_gpu_preference:
        unsafe extern "system" fn(Raw, u32, i32, *const u8, *mut Raw) -> i32,
    pub(crate) factory7_register_adapters_changed_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) factory7_unregister_adapters_changed_event:
        unsafe extern "system" fn(Raw, u32) -> i32,
}

// SAFETY: `Factory7Table` lays out `IDXGIFactory7` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactory7"
)]
unsafe impl Interface for Factory7Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIFactoryMedia`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct FactoryMediaTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) factory_media_create_swap_chain_for_composition_surface_handle:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, Raw, *mut Raw) -> i32,
    pub(crate) factory_media_create_decode_swap_chain_for_composition_surface_handle:
        unsafe extern "system" fn(Raw, Raw, usize, *const u8, Raw, Raw, *mut Raw) -> i32,
}

// SAFETY: `FactoryMediaTable` lays out `IDXGIFactoryMedia` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIFactoryMedia"
)]
unsafe impl Interface for FactoryMediaTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIInfoQueue`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct InfoQueueTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) info_queue_set_message_count_limit:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, u64) -> i32,
    pub(crate) info_queue_clear_stored_messages:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi),
    pub(crate) info_queue_get_message: unsafe extern "system" fn(
        Raw,
        component_object_model_direct::GuidAbi,
        u64,
        *mut u8,
        *mut u8,
    ) -> i32,
    pub(crate) info_queue_get_num_stored_messages_allowed_by_retrieval_filters:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u64,
    pub(crate) info_queue_get_num_stored_messages:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u64,
    pub(crate) info_queue_get_num_messages_discarded_by_message_count_limit:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u64,
    pub(crate) info_queue_get_message_count_limit:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u64,
    pub(crate) info_queue_get_num_messages_allowed_by_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u64,
    pub(crate) info_queue_get_num_messages_denied_by_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u64,
    pub(crate) info_queue_add_storage_filter_entries:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, *mut u8) -> i32,
    pub(crate) info_queue_get_storage_filter: unsafe extern "system" fn(
        Raw,
        component_object_model_direct::GuidAbi,
        *mut u8,
        *mut u8,
    ) -> i32,
    pub(crate) info_queue_clear_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi),
    pub(crate) info_queue_push_empty_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
    pub(crate) info_queue_push_deny_all_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
    pub(crate) info_queue_push_copy_of_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
    pub(crate) info_queue_push_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, *mut u8) -> i32,
    pub(crate) info_queue_pop_storage_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi),
    pub(crate) info_queue_get_storage_filter_stack_size:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u32,
    pub(crate) info_queue_add_retrieval_filter_entries:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, *mut u8) -> i32,
    pub(crate) info_queue_get_retrieval_filter: unsafe extern "system" fn(
        Raw,
        component_object_model_direct::GuidAbi,
        *mut u8,
        *mut u8,
    ) -> i32,
    pub(crate) info_queue_clear_retrieval_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi),
    pub(crate) info_queue_push_empty_retrieval_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
    pub(crate) info_queue_push_deny_all_retrieval_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
    pub(crate) info_queue_push_copy_of_retrieval_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
    pub(crate) info_queue_push_retrieval_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, *mut u8) -> i32,
    pub(crate) info_queue_pop_retrieval_filter:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi),
    pub(crate) info_queue_get_retrieval_filter_stack_size:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> u32,
    pub(crate) info_queue_add_message: unsafe extern "system" fn(
        Raw,
        component_object_model_direct::GuidAbi,
        i32,
        i32,
        i32,
        *const u8,
    ) -> i32,
    pub(crate) info_queue_add_application_message:
        unsafe extern "system" fn(Raw, i32, *const u8) -> i32,
    pub(crate) info_queue_set_break_on_category:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32, i32) -> i32,
    pub(crate) info_queue_set_break_on_severity:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32, i32) -> i32,
    pub(crate) info_queue_set_break_on_id:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32, i32) -> i32,
    pub(crate) info_queue_get_break_on_category:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32) -> i32,
    pub(crate) info_queue_get_break_on_severity:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32) -> i32,
    pub(crate) info_queue_get_break_on_id:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32) -> i32,
    pub(crate) info_queue_set_mute_debug_output:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi, i32),
    pub(crate) info_queue_get_mute_debug_output:
        unsafe extern "system" fn(Raw, component_object_model_direct::GuidAbi) -> i32,
}

// SAFETY: `InfoQueueTable` lays out `IDXGIInfoQueue` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIInfoQueue"
)]
unsafe impl Interface for InfoQueueTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIKeyedMutex`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct KeyedMutexTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_sub_object_get_device:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) keyed_mutex_acquire_sync: unsafe extern "system" fn(Raw, u64, u32) -> i32,
    pub(crate) keyed_mutex_release_sync: unsafe extern "system" fn(Raw, u64) -> i32,
}

// SAFETY: `KeyedMutexTable` lays out `IDXGIKeyedMutex` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIKeyedMutex"
)]
unsafe impl Interface for KeyedMutexTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIObject`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct ObjectTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
}

// SAFETY: `ObjectTable` lays out `IDXGIObject` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIObject"
)]
unsafe impl Interface for ObjectTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct OutputTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub output_release_ownership: unsafe extern "system" fn(Raw),
    pub output_get_gamma_control_capabilities: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `OutputTable` lays out `IDXGIOutput` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput"
)]
unsafe impl Interface for OutputTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Output1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub output_release_ownership: unsafe extern "system" fn(Raw),
    pub output_get_gamma_control_capabilities: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output1_get_display_mode_list1:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output1_find_closest_matching_mode1:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output1_get_display_surface_data1: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output1_duplicate_output: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32,
}

// SAFETY: `Output1Table` lays out `IDXGIOutput1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput1"
)]
unsafe impl Interface for Output1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Output2Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub output_release_ownership: unsafe extern "system" fn(Raw),
    pub output_get_gamma_control_capabilities: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output1_get_display_mode_list1:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output1_find_closest_matching_mode1:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output1_get_display_surface_data1: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output1_duplicate_output: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32,
    pub output2_supports_overlays: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `Output2Table` lays out `IDXGIOutput2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput2"
)]
unsafe impl Interface for Output2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Output3Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub output_release_ownership: unsafe extern "system" fn(Raw),
    pub output_get_gamma_control_capabilities: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output1_get_display_mode_list1:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output1_find_closest_matching_mode1:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output1_get_display_surface_data1: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output1_duplicate_output: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32,
    pub output2_supports_overlays: unsafe extern "system" fn(Raw) -> i32,
    pub output3_check_overlay_support: unsafe extern "system" fn(Raw, i32, Raw, *mut u8) -> i32,
}

// SAFETY: `Output3Table` lays out `IDXGIOutput3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput3"
)]
unsafe impl Interface for Output3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Output4Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub output_release_ownership: unsafe extern "system" fn(Raw),
    pub output_get_gamma_control_capabilities: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output1_get_display_mode_list1:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output1_find_closest_matching_mode1:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output1_get_display_surface_data1: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output1_duplicate_output: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32,
    pub output2_supports_overlays: unsafe extern "system" fn(Raw) -> i32,
    pub output3_check_overlay_support: unsafe extern "system" fn(Raw, i32, Raw, *mut u8) -> i32,
    pub output4_check_overlay_color_space_support:
        unsafe extern "system" fn(Raw, i32, i32, Raw, *mut u8) -> i32,
}

// SAFETY: `Output4Table` lays out `IDXGIOutput4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput4"
)]
unsafe impl Interface for Output4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput5`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Output5Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub output_release_ownership: unsafe extern "system" fn(Raw),
    pub output_get_gamma_control_capabilities: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub output1_get_display_mode_list1:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub output1_find_closest_matching_mode1:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub output1_get_display_surface_data1: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub output1_duplicate_output: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32,
    pub output2_supports_overlays: unsafe extern "system" fn(Raw) -> i32,
    pub output3_check_overlay_support: unsafe extern "system" fn(Raw, i32, Raw, *mut u8) -> i32,
    pub output4_check_overlay_color_space_support:
        unsafe extern "system" fn(Raw, i32, i32, Raw, *mut u8) -> i32,
    pub output5_duplicate_output1:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *const u8, *mut Raw) -> i32,
}

// SAFETY: `Output5Table` lays out `IDXGIOutput5` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput5"
)]
unsafe impl Interface for Output5Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutput6`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Output6Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) output_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) output_get_display_mode_list:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub(crate) output_find_closest_matching_mode:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub(crate) output_wait_for_v_blank: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) output_take_ownership: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub(crate) output_release_ownership: unsafe extern "system" fn(Raw),
    pub(crate) output_get_gamma_control_capabilities:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) output_set_gamma_control: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) output_get_gamma_control: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) output_set_display_surface: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub(crate) output_get_display_surface_data: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub(crate) output_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) output1_get_display_mode_list1:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8, *mut u8) -> i32,
    pub(crate) output1_find_closest_matching_mode1:
        unsafe extern "system" fn(Raw, *const u8, *mut u8, Raw) -> i32,
    pub(crate) output1_get_display_surface_data1: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub(crate) output1_duplicate_output: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32,
    pub(crate) output2_supports_overlays: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) output3_check_overlay_support:
        unsafe extern "system" fn(Raw, i32, Raw, *mut u8) -> i32,
    pub(crate) output4_check_overlay_color_space_support:
        unsafe extern "system" fn(Raw, i32, i32, Raw, *mut u8) -> i32,
    pub(crate) output5_duplicate_output1:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *const u8, *mut Raw) -> i32,
    pub(crate) output6_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) output6_check_hardware_composition_support:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `Output6Table` lays out `IDXGIOutput6` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutput6"
)]
unsafe impl Interface for Output6Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIOutputDuplication`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct OutputDuplicationTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) output_duplication_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) output_duplication_acquire_next_frame:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut Raw) -> i32,
    pub(crate) output_duplication_get_frame_dirty_rects:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8) -> i32,
    pub(crate) output_duplication_get_frame_move_rects:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8) -> i32,
    pub(crate) output_duplication_get_frame_pointer_shape:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) output_duplication_map_desktop_surface:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) output_duplication_un_map_desktop_surface: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) output_duplication_release_frame: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `OutputDuplicationTable` lays out `IDXGIOutputDuplication` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIOutputDuplication"
)]
unsafe impl Interface for OutputDuplicationTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIResource`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct ResourceTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub resource_get_shared_handle: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub resource_get_usage: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32) -> i32,
    pub resource_get_eviction_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `ResourceTable` lays out `IDXGIResource` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIResource"
)]
unsafe impl Interface for ResourceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGIResource1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Resource1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub resource_get_shared_handle: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub resource_get_usage: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32) -> i32,
    pub resource_get_eviction_priority: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub resource1_create_subresource_surface: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub resource1_create_shared_handle:
        unsafe extern "system" fn(Raw, *mut u8, u32, *const u8, *mut u8) -> i32,
}

// SAFETY: `Resource1Table` lays out `IDXGIResource1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGIResource1"
)]
unsafe impl Interface for Resource1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISurface`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct SurfaceTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub surface_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub surface_map: unsafe extern "system" fn(Raw, *mut u8, u32) -> i32,
    pub surface_unmap: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `SurfaceTable` lays out `IDXGISurface` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISurface"
)]
unsafe impl Interface for SurfaceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISurface1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Surface1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub surface_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub surface_map: unsafe extern "system" fn(Raw, *mut u8, u32) -> i32,
    pub surface_unmap: unsafe extern "system" fn(Raw) -> i32,
    pub surface1_get_dc: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub surface1_release_dc: unsafe extern "system" fn(Raw, *const u8) -> i32,
}

// SAFETY: `Surface1Table` lays out `IDXGISurface1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISurface1"
)]
unsafe impl Interface for Surface1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISurface2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Surface2Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub surface_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub surface_map: unsafe extern "system" fn(Raw, *mut u8, u32) -> i32,
    pub surface_unmap: unsafe extern "system" fn(Raw) -> i32,
    pub surface1_get_dc: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub surface1_release_dc: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub surface2_get_resource: unsafe extern "system" fn(Raw, *const u8, *mut Raw, *mut u8) -> i32,
}

// SAFETY: `Surface2Table` lays out `IDXGISurface2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISurface2"
)]
unsafe impl Interface for Surface2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISwapChain`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct SwapChainTable {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain_present: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub swap_chain_get_buffer: unsafe extern "system" fn(Raw, u32, *const u8, *mut Raw) -> i32,
    pub swap_chain_set_fullscreen_state: unsafe extern "system" fn(Raw, i32, Raw) -> i32,
    pub swap_chain_get_fullscreen_state: unsafe extern "system" fn(Raw, *mut u8, *mut Raw) -> i32,
    pub swap_chain_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_resize_buffers: unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32) -> i32,
    pub swap_chain_resize_target: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain_get_containing_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_get_last_present_count: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `SwapChainTable` lays out `IDXGISwapChain` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISwapChain"
)]
unsafe impl Interface for SwapChainTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISwapChain1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct SwapChain1Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain_present: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub swap_chain_get_buffer: unsafe extern "system" fn(Raw, u32, *const u8, *mut Raw) -> i32,
    pub swap_chain_set_fullscreen_state: unsafe extern "system" fn(Raw, i32, Raw) -> i32,
    pub swap_chain_get_fullscreen_state: unsafe extern "system" fn(Raw, *mut u8, *mut Raw) -> i32,
    pub swap_chain_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_resize_buffers: unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32) -> i32,
    pub swap_chain_resize_target: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain_get_containing_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_get_last_present_count: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_fullscreen_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_hwnd: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_core_window: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain1_present1: unsafe extern "system" fn(Raw, u32, u32, *mut u8) -> i32,
    pub swap_chain1_is_temporary_mono_supported: unsafe extern "system" fn(Raw) -> i32,
    pub swap_chain1_get_restrict_to_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain1_set_background_color: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain1_get_background_color: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_set_rotation: unsafe extern "system" fn(Raw, i32) -> i32,
    pub swap_chain1_get_rotation: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `SwapChain1Table` lays out `IDXGISwapChain1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISwapChain1"
)]
unsafe impl Interface for SwapChain1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISwapChain2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct SwapChain2Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain_present: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub swap_chain_get_buffer: unsafe extern "system" fn(Raw, u32, *const u8, *mut Raw) -> i32,
    pub swap_chain_set_fullscreen_state: unsafe extern "system" fn(Raw, i32, Raw) -> i32,
    pub swap_chain_get_fullscreen_state: unsafe extern "system" fn(Raw, *mut u8, *mut Raw) -> i32,
    pub swap_chain_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_resize_buffers: unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32) -> i32,
    pub swap_chain_resize_target: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain_get_containing_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_get_last_present_count: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_fullscreen_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_hwnd: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_core_window: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain1_present1: unsafe extern "system" fn(Raw, u32, u32, *mut u8) -> i32,
    pub swap_chain1_is_temporary_mono_supported: unsafe extern "system" fn(Raw) -> i32,
    pub swap_chain1_get_restrict_to_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain1_set_background_color: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain1_get_background_color: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_set_rotation: unsafe extern "system" fn(Raw, i32) -> i32,
    pub swap_chain1_get_rotation: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain2_set_source_size: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub swap_chain2_get_source_size: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub swap_chain2_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub swap_chain2_get_maximum_frame_latency: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain2_get_frame_latency_waitable_object: unsafe extern "system" fn(Raw) -> usize,
    pub swap_chain2_set_matrix_transform: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain2_get_matrix_transform: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `SwapChain2Table` lays out `IDXGISwapChain2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISwapChain2"
)]
unsafe impl Interface for SwapChain2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISwapChain3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct SwapChain3Table {
    pub unknown: UnknownTable,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_sub_object_get_device: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain_present: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub swap_chain_get_buffer: unsafe extern "system" fn(Raw, u32, *const u8, *mut Raw) -> i32,
    pub swap_chain_set_fullscreen_state: unsafe extern "system" fn(Raw, i32, Raw) -> i32,
    pub swap_chain_get_fullscreen_state: unsafe extern "system" fn(Raw, *mut u8, *mut Raw) -> i32,
    pub swap_chain_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_resize_buffers: unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32) -> i32,
    pub swap_chain_resize_target: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain_get_containing_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain_get_last_present_count: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_fullscreen_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_hwnd: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_get_core_window: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub swap_chain1_present1: unsafe extern "system" fn(Raw, u32, u32, *mut u8) -> i32,
    pub swap_chain1_is_temporary_mono_supported: unsafe extern "system" fn(Raw) -> i32,
    pub swap_chain1_get_restrict_to_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub swap_chain1_set_background_color: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain1_get_background_color: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain1_set_rotation: unsafe extern "system" fn(Raw, i32) -> i32,
    pub swap_chain1_get_rotation: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain2_set_source_size: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub swap_chain2_get_source_size: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub swap_chain2_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub swap_chain2_get_maximum_frame_latency: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain2_get_frame_latency_waitable_object: unsafe extern "system" fn(Raw) -> usize,
    pub swap_chain2_set_matrix_transform: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub swap_chain2_get_matrix_transform: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub swap_chain3_get_current_back_buffer_index: unsafe extern "system" fn(Raw) -> u32,
    pub swap_chain3_check_color_space_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub swap_chain3_set_color_space1: unsafe extern "system" fn(Raw, i32) -> i32,
    pub swap_chain3_resize_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32, *mut u8, *mut u8) -> i32,
}

// SAFETY: `SwapChain3Table` lays out `IDXGISwapChain3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISwapChain3"
)]
unsafe impl Interface for SwapChain3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISwapChain4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct SwapChain4Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_get_parent: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_sub_object_get_device:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) swap_chain_present: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub(crate) swap_chain_get_buffer:
        unsafe extern "system" fn(Raw, u32, *const u8, *mut Raw) -> i32,
    pub(crate) swap_chain_set_fullscreen_state: unsafe extern "system" fn(Raw, i32, Raw) -> i32,
    pub(crate) swap_chain_get_fullscreen_state:
        unsafe extern "system" fn(Raw, *mut u8, *mut Raw) -> i32,
    pub(crate) swap_chain_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain_resize_buffers:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32) -> i32,
    pub(crate) swap_chain_resize_target: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) swap_chain_get_containing_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub(crate) swap_chain_get_frame_statistics: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain_get_last_present_count: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain1_get_desc1: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain1_get_fullscreen_desc: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain1_get_hwnd: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain1_get_core_window:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) swap_chain1_present1: unsafe extern "system" fn(Raw, u32, u32, *mut u8) -> i32,
    pub(crate) swap_chain1_is_temporary_mono_supported: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) swap_chain1_get_restrict_to_output: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub(crate) swap_chain1_set_background_color: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) swap_chain1_get_background_color: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain1_set_rotation: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) swap_chain1_get_rotation: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain2_set_source_size: unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub(crate) swap_chain2_get_source_size: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) swap_chain2_set_maximum_frame_latency: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) swap_chain2_get_maximum_frame_latency:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain2_get_frame_latency_waitable_object:
        unsafe extern "system" fn(Raw) -> usize,
    pub(crate) swap_chain2_set_matrix_transform: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) swap_chain2_get_matrix_transform: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain3_get_current_back_buffer_index: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) swap_chain3_check_color_space_support:
        unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub(crate) swap_chain3_set_color_space1: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) swap_chain3_resize_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32, *mut u8, *mut u8) -> i32,
    pub(crate) swap_chain4_set_hdr_meta_data:
        unsafe extern "system" fn(Raw, i32, u32, *const u8) -> i32,
}

// SAFETY: `SwapChain4Table` lays out `IDXGISwapChain4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISwapChain4"
)]
unsafe impl Interface for SwapChain4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGISwapChainMedia`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct SwapChainMediaTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) swap_chain_media_get_frame_statistics_media:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) swap_chain_media_set_present_duration: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) swap_chain_media_check_present_duration_support:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8) -> i32,
}

// SAFETY: `SwapChainMediaTable` lays out `IDXGISwapChainMedia` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGISwapChainMedia"
)]
unsafe impl Interface for SwapChainMediaTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `IDXGraphicsAnalysis`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct GraphicsAnalysisTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) graphics_analysis_begin_capture: unsafe extern "system" fn(Raw),
    pub(crate) graphics_analysis_end_capture: unsafe extern "system" fn(Raw),
}

// SAFETY: `GraphicsAnalysisTable` lays out `IDXGraphicsAnalysis` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of IDXGraphicsAnalysis"
)]
unsafe impl Interface for GraphicsAnalysisTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// The library's exports, as C calls them.
#[derive(Clone, Copy)]
pub struct Library {
    pub create_dxgi_factory: unsafe extern "system" fn(*const u8, *mut Raw) -> i32,
    pub create_dxgi_factory1: unsafe extern "system" fn(*const u8, *mut Raw) -> i32,
    pub create_dxgi_factory2: unsafe extern "system" fn(u32, *const u8, *mut Raw) -> i32,
    pub dxgi_declare_adapter_removal_support: unsafe extern "system" fn() -> i32,
    pub dxgi_disable_v_blank_virtualization: unsafe extern "system" fn() -> i32,
    pub dxgi_get_debug_interface1: unsafe extern "system" fn(u32, *const u8, *mut Raw) -> i32,
}
