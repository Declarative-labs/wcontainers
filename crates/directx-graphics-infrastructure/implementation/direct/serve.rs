//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The serving glue: raw C calls in, any implementation of the traits out.

#![allow(
    unused_variables,
    unused_imports,
    non_snake_case,
    clippy::let_unit_value,
    clippy::unit_arg,
    reason = "generated: every method has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_component_object_model_direct as component_object_model_direct;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

use core::marker::PhantomData;
use std::any::Any;
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_component_object_model::constants::{
    E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER,
};
use wcontainers_component_object_model::structures::Foreign;
use wcontainers_component_object_model::structures::Guid;
use wcontainers_component_object_model_direct::{
    ForeignUnknown, Found, IID_IUNKNOWN, Interfaces, Lendings, MOST_TEXT_UNITS, Raw, Recorded,
    Shadow, Table, counted, guarded, made_by_of, object, query, read_bytes, read_only_at,
    read_text, record, record_related, relate, relate_at, related, related_at, related_of, serve,
    status, unknown_table, writable_at, write_bytes,
};

use crate::KEY;
use crate::abi::*;
use crate::data::*;

/// The tables of this API's interfaces, for objects of type `V`.
pub struct Tables<V>(PhantomData<V>);

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Adapter> Tables<V> {
    /// `IDXGIAdapter`'s table for objects of type `V`.
    pub const ADAPTER: &'static AdapterTable = &AdapterTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        adapter_enum_outputs: adapter_enum_outputs::<V>,
        adapter_get_desc: adapter_get_desc::<V>,
        adapter_check_interface_support: adapter_check_interface_support::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Adapter1> Tables<V> {
    /// `IDXGIAdapter1`'s table for objects of type `V`.
    pub const ADAPTER1: &'static Adapter1Table = &Adapter1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        adapter_enum_outputs: adapter_enum_outputs::<V>,
        adapter_get_desc: adapter_get_desc::<V>,
        adapter_check_interface_support: adapter_check_interface_support::<V>,
        adapter1_get_desc1: adapter1_get_desc1::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Adapter2> Tables<V> {
    /// `IDXGIAdapter2`'s table for objects of type `V`.
    pub const ADAPTER2: &'static Adapter2Table = &Adapter2Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        adapter_enum_outputs: adapter_enum_outputs::<V>,
        adapter_get_desc: adapter_get_desc::<V>,
        adapter_check_interface_support: adapter_check_interface_support::<V>,
        adapter1_get_desc1: adapter1_get_desc1::<V>,
        adapter2_get_desc2: adapter2_get_desc2::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Adapter3> Tables<V> {
    /// `IDXGIAdapter3`'s table for objects of type `V`.
    pub const ADAPTER3: &'static Adapter3Table = &Adapter3Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        adapter_enum_outputs: adapter_enum_outputs::<V>,
        adapter_get_desc: adapter_get_desc::<V>,
        adapter_check_interface_support: adapter_check_interface_support::<V>,
        adapter1_get_desc1: adapter1_get_desc1::<V>,
        adapter2_get_desc2: adapter2_get_desc2::<V>,
        adapter3_register_hardware_content_protection_teardown_status_event:
            adapter3_register_hardware_content_protection_teardown_status_event::<V>,
        adapter3_unregister_hardware_content_protection_teardown_status:
            adapter3_unregister_hardware_content_protection_teardown_status::<V>,
        adapter3_query_video_memory_info: adapter3_query_video_memory_info::<V>,
        adapter3_set_video_memory_reservation: adapter3_set_video_memory_reservation::<V>,
        adapter3_register_video_memory_budget_change_notification_event:
            adapter3_register_video_memory_budget_change_notification_event::<V>,
        adapter3_unregister_video_memory_budget_change_notification:
            adapter3_unregister_video_memory_budget_change_notification::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Adapter4> Tables<V> {
    /// `IDXGIAdapter4`'s table for objects of type `V`.
    pub(crate) const ADAPTER4: &'static Adapter4Table = &Adapter4Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        adapter_enum_outputs: adapter_enum_outputs::<V>,
        adapter_get_desc: adapter_get_desc::<V>,
        adapter_check_interface_support: adapter_check_interface_support::<V>,
        adapter1_get_desc1: adapter1_get_desc1::<V>,
        adapter2_get_desc2: adapter2_get_desc2::<V>,
        adapter3_register_hardware_content_protection_teardown_status_event:
            adapter3_register_hardware_content_protection_teardown_status_event::<V>,
        adapter3_unregister_hardware_content_protection_teardown_status:
            adapter3_unregister_hardware_content_protection_teardown_status::<V>,
        adapter3_query_video_memory_info: adapter3_query_video_memory_info::<V>,
        adapter3_set_video_memory_reservation: adapter3_set_video_memory_reservation::<V>,
        adapter3_register_video_memory_budget_change_notification_event:
            adapter3_register_video_memory_budget_change_notification_event::<V>,
        adapter3_unregister_video_memory_budget_change_notification:
            adapter3_unregister_video_memory_budget_change_notification::<V>,
        adapter4_get_desc3: adapter4_get_desc3::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Debug> Tables<V> {
    /// `IDXGIDebug`'s table for objects of type `V`.
    pub const DEBUG: &'static DebugTable = &DebugTable {
        unknown: unknown_table::<V>(),
        debug_report_live_objects: debug_report_live_objects::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Debug1> Tables<V> {
    /// `IDXGIDebug1`'s table for objects of type `V`.
    pub(crate) const DEBUG1: &'static Debug1Table = &Debug1Table {
        unknown: unknown_table::<V>(),
        debug_report_live_objects: debug_report_live_objects::<V>,
        debug1_enable_leak_tracking_for_thread: debug1_enable_leak_tracking_for_thread::<V>,
        debug1_disable_leak_tracking_for_thread: debug1_disable_leak_tracking_for_thread::<V>,
        debug1_is_leak_tracking_enabled_for_thread: debug1_is_leak_tracking_enabled_for_thread::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::DecodeSwapChain> Tables<V> {
    /// `IDXGIDecodeSwapChain`'s table for objects of type `V`.
    pub(crate) const DECODE_SWAP_CHAIN: &'static DecodeSwapChainTable = &DecodeSwapChainTable {
        unknown: unknown_table::<V>(),
        decode_swap_chain_present_buffer: decode_swap_chain_present_buffer::<V>,
        decode_swap_chain_set_source_rect: decode_swap_chain_set_source_rect::<V>,
        decode_swap_chain_set_target_rect: decode_swap_chain_set_target_rect::<V>,
        decode_swap_chain_set_dest_size: decode_swap_chain_set_dest_size::<V>,
        decode_swap_chain_get_source_rect: decode_swap_chain_get_source_rect::<V>,
        decode_swap_chain_get_target_rect: decode_swap_chain_get_target_rect::<V>,
        decode_swap_chain_get_dest_size: decode_swap_chain_get_dest_size::<V>,
        decode_swap_chain_set_color_space: decode_swap_chain_set_color_space::<V>,
        decode_swap_chain_get_color_space: decode_swap_chain_get_color_space::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Device> Tables<V> {
    /// `IDXGIDevice`'s table for objects of type `V`.
    pub const DEVICE: &'static DeviceTable = &DeviceTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_get_adapter: device_get_adapter::<V>,
        device_create_surface: device_create_surface::<V>,
        device_query_resource_residency: device_query_resource_residency_refused,
        device_set_gpu_thread_priority: device_set_gpu_thread_priority::<V>,
        device_get_gpu_thread_priority: device_get_gpu_thread_priority::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Device1> Tables<V> {
    /// `IDXGIDevice1`'s table for objects of type `V`.
    pub const DEVICE1: &'static Device1Table = &Device1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_get_adapter: device_get_adapter::<V>,
        device_create_surface: device_create_surface::<V>,
        device_query_resource_residency: device_query_resource_residency_refused,
        device_set_gpu_thread_priority: device_set_gpu_thread_priority::<V>,
        device_get_gpu_thread_priority: device_get_gpu_thread_priority::<V>,
        device1_set_maximum_frame_latency: device1_set_maximum_frame_latency::<V>,
        device1_get_maximum_frame_latency: device1_get_maximum_frame_latency::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Device2> Tables<V> {
    /// `IDXGIDevice2`'s table for objects of type `V`.
    pub const DEVICE2: &'static Device2Table = &Device2Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_get_adapter: device_get_adapter::<V>,
        device_create_surface: device_create_surface::<V>,
        device_query_resource_residency: device_query_resource_residency_refused,
        device_set_gpu_thread_priority: device_set_gpu_thread_priority::<V>,
        device_get_gpu_thread_priority: device_get_gpu_thread_priority::<V>,
        device1_set_maximum_frame_latency: device1_set_maximum_frame_latency::<V>,
        device1_get_maximum_frame_latency: device1_get_maximum_frame_latency::<V>,
        device2_offer_resources: device2_offer_resources::<V>,
        device2_reclaim_resources: device2_reclaim_resources::<V>,
        device2_enqueue_set_event: device2_enqueue_set_event::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Device3> Tables<V> {
    /// `IDXGIDevice3`'s table for objects of type `V`.
    pub const DEVICE3: &'static Device3Table = &Device3Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_get_adapter: device_get_adapter::<V>,
        device_create_surface: device_create_surface::<V>,
        device_query_resource_residency: device_query_resource_residency_refused,
        device_set_gpu_thread_priority: device_set_gpu_thread_priority::<V>,
        device_get_gpu_thread_priority: device_get_gpu_thread_priority::<V>,
        device1_set_maximum_frame_latency: device1_set_maximum_frame_latency::<V>,
        device1_get_maximum_frame_latency: device1_get_maximum_frame_latency::<V>,
        device2_offer_resources: device2_offer_resources::<V>,
        device2_reclaim_resources: device2_reclaim_resources::<V>,
        device2_enqueue_set_event: device2_enqueue_set_event::<V>,
        device3_trim: device3_trim::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Device4> Tables<V> {
    /// `IDXGIDevice4`'s table for objects of type `V`.
    pub const DEVICE4: &'static Device4Table = &Device4Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_get_adapter: device_get_adapter::<V>,
        device_create_surface: device_create_surface::<V>,
        device_query_resource_residency: device_query_resource_residency_refused,
        device_set_gpu_thread_priority: device_set_gpu_thread_priority::<V>,
        device_get_gpu_thread_priority: device_get_gpu_thread_priority::<V>,
        device1_set_maximum_frame_latency: device1_set_maximum_frame_latency::<V>,
        device1_get_maximum_frame_latency: device1_get_maximum_frame_latency::<V>,
        device2_offer_resources: device2_offer_resources::<V>,
        device2_reclaim_resources: device2_reclaim_resources::<V>,
        device2_enqueue_set_event: device2_enqueue_set_event::<V>,
        device3_trim: device3_trim::<V>,
        device4_offer_resources1: device4_offer_resources1::<V>,
        device4_reclaim_resources1: device4_reclaim_resources1::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::DeviceSubObject> Tables<V> {
    /// `IDXGIDeviceSubObject`'s table for objects of type `V`.
    pub const DEVICE_SUB_OBJECT: &'static DeviceSubObjectTable = &DeviceSubObjectTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::DisplayControl> Tables<V> {
    /// `IDXGIDisplayControl`'s table for objects of type `V`.
    pub(crate) const DISPLAY_CONTROL: &'static DisplayControlTable = &DisplayControlTable {
        unknown: unknown_table::<V>(),
        display_control_is_stereo_enabled: display_control_is_stereo_enabled::<V>,
        display_control_set_stereo_enabled: display_control_set_stereo_enabled::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory> Tables<V> {
    /// `IDXGIFactory`'s table for objects of type `V`.
    pub const FACTORY: &'static FactoryTable = &FactoryTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory1> Tables<V> {
    /// `IDXGIFactory1`'s table for objects of type `V`.
    pub const FACTORY1: &'static Factory1Table = &Factory1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory2> Tables<V> {
    /// `IDXGIFactory2`'s table for objects of type `V`.
    pub const FACTORY2: &'static Factory2Table = &Factory2Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
        factory2_is_windowed_stereo_enabled: factory2_is_windowed_stereo_enabled::<V>,
        factory2_create_swap_chain_for_hwnd: factory2_create_swap_chain_for_hwnd::<V>,
        factory2_create_swap_chain_for_core_window: factory2_create_swap_chain_for_core_window::<V>,
        factory2_get_shared_resource_adapter_luid: factory2_get_shared_resource_adapter_luid::<V>,
        factory2_register_stereo_status_window: factory2_register_stereo_status_window::<V>,
        factory2_register_stereo_status_event: factory2_register_stereo_status_event::<V>,
        factory2_unregister_stereo_status: factory2_unregister_stereo_status::<V>,
        factory2_register_occlusion_status_window: factory2_register_occlusion_status_window::<V>,
        factory2_register_occlusion_status_event: factory2_register_occlusion_status_event::<V>,
        factory2_unregister_occlusion_status: factory2_unregister_occlusion_status::<V>,
        factory2_create_swap_chain_for_composition: factory2_create_swap_chain_for_composition::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory3> Tables<V> {
    /// `IDXGIFactory3`'s table for objects of type `V`.
    pub const FACTORY3: &'static Factory3Table = &Factory3Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
        factory2_is_windowed_stereo_enabled: factory2_is_windowed_stereo_enabled::<V>,
        factory2_create_swap_chain_for_hwnd: factory2_create_swap_chain_for_hwnd::<V>,
        factory2_create_swap_chain_for_core_window: factory2_create_swap_chain_for_core_window::<V>,
        factory2_get_shared_resource_adapter_luid: factory2_get_shared_resource_adapter_luid::<V>,
        factory2_register_stereo_status_window: factory2_register_stereo_status_window::<V>,
        factory2_register_stereo_status_event: factory2_register_stereo_status_event::<V>,
        factory2_unregister_stereo_status: factory2_unregister_stereo_status::<V>,
        factory2_register_occlusion_status_window: factory2_register_occlusion_status_window::<V>,
        factory2_register_occlusion_status_event: factory2_register_occlusion_status_event::<V>,
        factory2_unregister_occlusion_status: factory2_unregister_occlusion_status::<V>,
        factory2_create_swap_chain_for_composition: factory2_create_swap_chain_for_composition::<V>,
        factory3_get_creation_flags: factory3_get_creation_flags::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory4> Tables<V> {
    /// `IDXGIFactory4`'s table for objects of type `V`.
    pub const FACTORY4: &'static Factory4Table = &Factory4Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
        factory2_is_windowed_stereo_enabled: factory2_is_windowed_stereo_enabled::<V>,
        factory2_create_swap_chain_for_hwnd: factory2_create_swap_chain_for_hwnd::<V>,
        factory2_create_swap_chain_for_core_window: factory2_create_swap_chain_for_core_window::<V>,
        factory2_get_shared_resource_adapter_luid: factory2_get_shared_resource_adapter_luid::<V>,
        factory2_register_stereo_status_window: factory2_register_stereo_status_window::<V>,
        factory2_register_stereo_status_event: factory2_register_stereo_status_event::<V>,
        factory2_unregister_stereo_status: factory2_unregister_stereo_status::<V>,
        factory2_register_occlusion_status_window: factory2_register_occlusion_status_window::<V>,
        factory2_register_occlusion_status_event: factory2_register_occlusion_status_event::<V>,
        factory2_unregister_occlusion_status: factory2_unregister_occlusion_status::<V>,
        factory2_create_swap_chain_for_composition: factory2_create_swap_chain_for_composition::<V>,
        factory3_get_creation_flags: factory3_get_creation_flags::<V>,
        factory4_enum_adapter_by_luid: factory4_enum_adapter_by_luid_refused,
        factory4_enum_warp_adapter: factory4_enum_warp_adapter::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory5> Tables<V> {
    /// `IDXGIFactory5`'s table for objects of type `V`.
    pub const FACTORY5: &'static Factory5Table = &Factory5Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
        factory2_is_windowed_stereo_enabled: factory2_is_windowed_stereo_enabled::<V>,
        factory2_create_swap_chain_for_hwnd: factory2_create_swap_chain_for_hwnd::<V>,
        factory2_create_swap_chain_for_core_window: factory2_create_swap_chain_for_core_window::<V>,
        factory2_get_shared_resource_adapter_luid: factory2_get_shared_resource_adapter_luid::<V>,
        factory2_register_stereo_status_window: factory2_register_stereo_status_window::<V>,
        factory2_register_stereo_status_event: factory2_register_stereo_status_event::<V>,
        factory2_unregister_stereo_status: factory2_unregister_stereo_status::<V>,
        factory2_register_occlusion_status_window: factory2_register_occlusion_status_window::<V>,
        factory2_register_occlusion_status_event: factory2_register_occlusion_status_event::<V>,
        factory2_unregister_occlusion_status: factory2_unregister_occlusion_status::<V>,
        factory2_create_swap_chain_for_composition: factory2_create_swap_chain_for_composition::<V>,
        factory3_get_creation_flags: factory3_get_creation_flags::<V>,
        factory4_enum_adapter_by_luid: factory4_enum_adapter_by_luid_refused,
        factory4_enum_warp_adapter: factory4_enum_warp_adapter::<V>,
        factory5_check_feature_support: factory5_check_feature_support::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory6> Tables<V> {
    /// `IDXGIFactory6`'s table for objects of type `V`.
    pub const FACTORY6: &'static Factory6Table = &Factory6Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
        factory2_is_windowed_stereo_enabled: factory2_is_windowed_stereo_enabled::<V>,
        factory2_create_swap_chain_for_hwnd: factory2_create_swap_chain_for_hwnd::<V>,
        factory2_create_swap_chain_for_core_window: factory2_create_swap_chain_for_core_window::<V>,
        factory2_get_shared_resource_adapter_luid: factory2_get_shared_resource_adapter_luid::<V>,
        factory2_register_stereo_status_window: factory2_register_stereo_status_window::<V>,
        factory2_register_stereo_status_event: factory2_register_stereo_status_event::<V>,
        factory2_unregister_stereo_status: factory2_unregister_stereo_status::<V>,
        factory2_register_occlusion_status_window: factory2_register_occlusion_status_window::<V>,
        factory2_register_occlusion_status_event: factory2_register_occlusion_status_event::<V>,
        factory2_unregister_occlusion_status: factory2_unregister_occlusion_status::<V>,
        factory2_create_swap_chain_for_composition: factory2_create_swap_chain_for_composition::<V>,
        factory3_get_creation_flags: factory3_get_creation_flags::<V>,
        factory4_enum_adapter_by_luid: factory4_enum_adapter_by_luid_refused,
        factory4_enum_warp_adapter: factory4_enum_warp_adapter::<V>,
        factory5_check_feature_support: factory5_check_feature_support::<V>,
        factory6_enum_adapter_by_gpu_preference: factory6_enum_adapter_by_gpu_preference::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Factory7> Tables<V> {
    /// `IDXGIFactory7`'s table for objects of type `V`.
    pub(crate) const FACTORY7: &'static Factory7Table = &Factory7Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        factory_enum_adapters: factory_enum_adapters::<V>,
        factory_make_window_association: factory_make_window_association::<V>,
        factory_get_window_association: factory_get_window_association::<V>,
        factory_create_swap_chain: factory_create_swap_chain::<V>,
        factory_create_software_adapter: factory_create_software_adapter::<V>,
        factory1_enum_adapters1: factory1_enum_adapters1::<V>,
        factory1_is_current: factory1_is_current::<V>,
        factory2_is_windowed_stereo_enabled: factory2_is_windowed_stereo_enabled::<V>,
        factory2_create_swap_chain_for_hwnd: factory2_create_swap_chain_for_hwnd::<V>,
        factory2_create_swap_chain_for_core_window: factory2_create_swap_chain_for_core_window::<V>,
        factory2_get_shared_resource_adapter_luid: factory2_get_shared_resource_adapter_luid::<V>,
        factory2_register_stereo_status_window: factory2_register_stereo_status_window::<V>,
        factory2_register_stereo_status_event: factory2_register_stereo_status_event::<V>,
        factory2_unregister_stereo_status: factory2_unregister_stereo_status::<V>,
        factory2_register_occlusion_status_window: factory2_register_occlusion_status_window::<V>,
        factory2_register_occlusion_status_event: factory2_register_occlusion_status_event::<V>,
        factory2_unregister_occlusion_status: factory2_unregister_occlusion_status::<V>,
        factory2_create_swap_chain_for_composition: factory2_create_swap_chain_for_composition::<V>,
        factory3_get_creation_flags: factory3_get_creation_flags::<V>,
        factory4_enum_adapter_by_luid: factory4_enum_adapter_by_luid_refused,
        factory4_enum_warp_adapter: factory4_enum_warp_adapter::<V>,
        factory5_check_feature_support: factory5_check_feature_support::<V>,
        factory6_enum_adapter_by_gpu_preference: factory6_enum_adapter_by_gpu_preference::<V>,
        factory7_register_adapters_changed_event: factory7_register_adapters_changed_event::<V>,
        factory7_unregister_adapters_changed_event: factory7_unregister_adapters_changed_event::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::FactoryMedia> Tables<V> {
    /// `IDXGIFactoryMedia`'s table for objects of type `V`.
    pub(crate) const FACTORY_MEDIA: &'static FactoryMediaTable = &FactoryMediaTable {
        unknown: unknown_table::<V>(),
        factory_media_create_swap_chain_for_composition_surface_handle:
            factory_media_create_swap_chain_for_composition_surface_handle::<V>,
        factory_media_create_decode_swap_chain_for_composition_surface_handle:
            factory_media_create_decode_swap_chain_for_composition_surface_handle::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::InfoQueue> Tables<V> {
    /// `IDXGIInfoQueue`'s table for objects of type `V`.
    pub(crate) const INFO_QUEUE: &'static InfoQueueTable = &InfoQueueTable {
        unknown: unknown_table::<V>(),
        info_queue_set_message_count_limit: info_queue_set_message_count_limit::<V>,
        info_queue_clear_stored_messages: info_queue_clear_stored_messages::<V>,
        info_queue_get_message: info_queue_get_message_refused,
        info_queue_get_num_stored_messages_allowed_by_retrieval_filters:
            info_queue_get_num_stored_messages_allowed_by_retrieval_filters::<V>,
        info_queue_get_num_stored_messages: info_queue_get_num_stored_messages::<V>,
        info_queue_get_num_messages_discarded_by_message_count_limit:
            info_queue_get_num_messages_discarded_by_message_count_limit::<V>,
        info_queue_get_message_count_limit: info_queue_get_message_count_limit::<V>,
        info_queue_get_num_messages_allowed_by_storage_filter:
            info_queue_get_num_messages_allowed_by_storage_filter::<V>,
        info_queue_get_num_messages_denied_by_storage_filter:
            info_queue_get_num_messages_denied_by_storage_filter::<V>,
        info_queue_add_storage_filter_entries: info_queue_add_storage_filter_entries_refused,
        info_queue_get_storage_filter: info_queue_get_storage_filter_refused,
        info_queue_clear_storage_filter: info_queue_clear_storage_filter::<V>,
        info_queue_push_empty_storage_filter: info_queue_push_empty_storage_filter::<V>,
        info_queue_push_deny_all_storage_filter: info_queue_push_deny_all_storage_filter::<V>,
        info_queue_push_copy_of_storage_filter: info_queue_push_copy_of_storage_filter::<V>,
        info_queue_push_storage_filter: info_queue_push_storage_filter_refused,
        info_queue_pop_storage_filter: info_queue_pop_storage_filter::<V>,
        info_queue_get_storage_filter_stack_size: info_queue_get_storage_filter_stack_size::<V>,
        info_queue_add_retrieval_filter_entries: info_queue_add_retrieval_filter_entries_refused,
        info_queue_get_retrieval_filter: info_queue_get_retrieval_filter_refused,
        info_queue_clear_retrieval_filter: info_queue_clear_retrieval_filter::<V>,
        info_queue_push_empty_retrieval_filter: info_queue_push_empty_retrieval_filter::<V>,
        info_queue_push_deny_all_retrieval_filter: info_queue_push_deny_all_retrieval_filter::<V>,
        info_queue_push_copy_of_retrieval_filter: info_queue_push_copy_of_retrieval_filter::<V>,
        info_queue_push_retrieval_filter: info_queue_push_retrieval_filter_refused,
        info_queue_pop_retrieval_filter: info_queue_pop_retrieval_filter::<V>,
        info_queue_get_retrieval_filter_stack_size: info_queue_get_retrieval_filter_stack_size::<V>,
        info_queue_add_message: info_queue_add_message::<V>,
        info_queue_add_application_message: info_queue_add_application_message::<V>,
        info_queue_set_break_on_category: info_queue_set_break_on_category::<V>,
        info_queue_set_break_on_severity: info_queue_set_break_on_severity::<V>,
        info_queue_set_break_on_id: info_queue_set_break_on_id::<V>,
        info_queue_get_break_on_category: info_queue_get_break_on_category::<V>,
        info_queue_get_break_on_severity: info_queue_get_break_on_severity::<V>,
        info_queue_get_break_on_id: info_queue_get_break_on_id::<V>,
        info_queue_set_mute_debug_output: info_queue_set_mute_debug_output::<V>,
        info_queue_get_mute_debug_output: info_queue_get_mute_debug_output::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::KeyedMutex> Tables<V> {
    /// `IDXGIKeyedMutex`'s table for objects of type `V`.
    pub(crate) const KEYED_MUTEX: &'static KeyedMutexTable = &KeyedMutexTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        keyed_mutex_acquire_sync: keyed_mutex_acquire_sync::<V>,
        keyed_mutex_release_sync: keyed_mutex_release_sync::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Object> Tables<V> {
    /// `IDXGIObject`'s table for objects of type `V`.
    pub const OBJECT: &'static ObjectTable = &ObjectTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output> Tables<V> {
    /// `IDXGIOutput`'s table for objects of type `V`.
    pub const OUTPUT: &'static OutputTable = &OutputTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output1> Tables<V> {
    /// `IDXGIOutput1`'s table for objects of type `V`.
    pub const OUTPUT1: &'static Output1Table = &Output1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
        output1_get_display_mode_list1: output1_get_display_mode_list1::<V>,
        output1_find_closest_matching_mode1: output1_find_closest_matching_mode1::<V>,
        output1_get_display_surface_data1: output1_get_display_surface_data1::<V>,
        output1_duplicate_output: output1_duplicate_output::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output2> Tables<V> {
    /// `IDXGIOutput2`'s table for objects of type `V`.
    pub const OUTPUT2: &'static Output2Table = &Output2Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
        output1_get_display_mode_list1: output1_get_display_mode_list1::<V>,
        output1_find_closest_matching_mode1: output1_find_closest_matching_mode1::<V>,
        output1_get_display_surface_data1: output1_get_display_surface_data1::<V>,
        output1_duplicate_output: output1_duplicate_output::<V>,
        output2_supports_overlays: output2_supports_overlays::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output3> Tables<V> {
    /// `IDXGIOutput3`'s table for objects of type `V`.
    pub const OUTPUT3: &'static Output3Table = &Output3Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
        output1_get_display_mode_list1: output1_get_display_mode_list1::<V>,
        output1_find_closest_matching_mode1: output1_find_closest_matching_mode1::<V>,
        output1_get_display_surface_data1: output1_get_display_surface_data1::<V>,
        output1_duplicate_output: output1_duplicate_output::<V>,
        output2_supports_overlays: output2_supports_overlays::<V>,
        output3_check_overlay_support: output3_check_overlay_support::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output4> Tables<V> {
    /// `IDXGIOutput4`'s table for objects of type `V`.
    pub const OUTPUT4: &'static Output4Table = &Output4Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
        output1_get_display_mode_list1: output1_get_display_mode_list1::<V>,
        output1_find_closest_matching_mode1: output1_find_closest_matching_mode1::<V>,
        output1_get_display_surface_data1: output1_get_display_surface_data1::<V>,
        output1_duplicate_output: output1_duplicate_output::<V>,
        output2_supports_overlays: output2_supports_overlays::<V>,
        output3_check_overlay_support: output3_check_overlay_support::<V>,
        output4_check_overlay_color_space_support: output4_check_overlay_color_space_support::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output5> Tables<V> {
    /// `IDXGIOutput5`'s table for objects of type `V`.
    pub const OUTPUT5: &'static Output5Table = &Output5Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
        output1_get_display_mode_list1: output1_get_display_mode_list1::<V>,
        output1_find_closest_matching_mode1: output1_find_closest_matching_mode1::<V>,
        output1_get_display_surface_data1: output1_get_display_surface_data1::<V>,
        output1_duplicate_output: output1_duplicate_output::<V>,
        output2_supports_overlays: output2_supports_overlays::<V>,
        output3_check_overlay_support: output3_check_overlay_support::<V>,
        output4_check_overlay_color_space_support: output4_check_overlay_color_space_support::<V>,
        output5_duplicate_output1: output5_duplicate_output1::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Output6> Tables<V> {
    /// `IDXGIOutput6`'s table for objects of type `V`.
    pub(crate) const OUTPUT6: &'static Output6Table = &Output6Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        output_get_desc: output_get_desc::<V>,
        output_get_display_mode_list: output_get_display_mode_list::<V>,
        output_find_closest_matching_mode: output_find_closest_matching_mode::<V>,
        output_wait_for_v_blank: output_wait_for_v_blank::<V>,
        output_take_ownership: output_take_ownership::<V>,
        output_release_ownership: output_release_ownership::<V>,
        output_get_gamma_control_capabilities: output_get_gamma_control_capabilities::<V>,
        output_set_gamma_control: output_set_gamma_control::<V>,
        output_get_gamma_control: output_get_gamma_control::<V>,
        output_set_display_surface: output_set_display_surface::<V>,
        output_get_display_surface_data: output_get_display_surface_data::<V>,
        output_get_frame_statistics: output_get_frame_statistics::<V>,
        output1_get_display_mode_list1: output1_get_display_mode_list1::<V>,
        output1_find_closest_matching_mode1: output1_find_closest_matching_mode1::<V>,
        output1_get_display_surface_data1: output1_get_display_surface_data1::<V>,
        output1_duplicate_output: output1_duplicate_output::<V>,
        output2_supports_overlays: output2_supports_overlays::<V>,
        output3_check_overlay_support: output3_check_overlay_support::<V>,
        output4_check_overlay_color_space_support: output4_check_overlay_color_space_support::<V>,
        output5_duplicate_output1: output5_duplicate_output1::<V>,
        output6_get_desc1: output6_get_desc1::<V>,
        output6_check_hardware_composition_support: output6_check_hardware_composition_support::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::OutputDuplication> Tables<V> {
    /// `IDXGIOutputDuplication`'s table for objects of type `V`.
    pub(crate) const OUTPUT_DUPLICATION: &'static OutputDuplicationTable =
        &OutputDuplicationTable {
            unknown: unknown_table::<V>(),
            object_set_private_data: object_set_private_data::<V>,
            object_set_private_data_interface: object_set_private_data_interface::<V>,
            object_get_private_data: object_get_private_data_refused,
            object_get_parent: object_get_parent::<V>,
            output_duplication_get_desc: output_duplication_get_desc::<V>,
            output_duplication_acquire_next_frame: output_duplication_acquire_next_frame::<V>,
            output_duplication_get_frame_dirty_rects: output_duplication_get_frame_dirty_rects::<V>,
            output_duplication_get_frame_move_rects: output_duplication_get_frame_move_rects::<V>,
            output_duplication_get_frame_pointer_shape: output_duplication_get_frame_pointer_shape::<
                V,
            >,
            output_duplication_map_desktop_surface: output_duplication_map_desktop_surface_refused,
            output_duplication_un_map_desktop_surface: output_duplication_un_map_desktop_surface::<V>,
            output_duplication_release_frame: output_duplication_release_frame::<V>,
        };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Resource> Tables<V> {
    /// `IDXGIResource`'s table for objects of type `V`.
    pub const RESOURCE: &'static ResourceTable = &ResourceTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        resource_get_shared_handle: resource_get_shared_handle::<V>,
        resource_get_usage: resource_get_usage::<V>,
        resource_set_eviction_priority: resource_set_eviction_priority::<V>,
        resource_get_eviction_priority: resource_get_eviction_priority::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Resource1> Tables<V> {
    /// `IDXGIResource1`'s table for objects of type `V`.
    pub const RESOURCE1: &'static Resource1Table = &Resource1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        resource_get_shared_handle: resource_get_shared_handle::<V>,
        resource_get_usage: resource_get_usage::<V>,
        resource_set_eviction_priority: resource_set_eviction_priority::<V>,
        resource_get_eviction_priority: resource_get_eviction_priority::<V>,
        resource1_create_subresource_surface: resource1_create_subresource_surface::<V>,
        resource1_create_shared_handle: resource1_create_shared_handle_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Surface> Tables<V> {
    /// `IDXGISurface`'s table for objects of type `V`.
    pub const SURFACE: &'static SurfaceTable = &SurfaceTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        surface_get_desc: surface_get_desc::<V>,
        surface_map: surface_map_refused,
        surface_unmap: surface_unmap::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Surface1> Tables<V> {
    /// `IDXGISurface1`'s table for objects of type `V`.
    pub const SURFACE1: &'static Surface1Table = &Surface1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        surface_get_desc: surface_get_desc::<V>,
        surface_map: surface_map_refused,
        surface_unmap: surface_unmap::<V>,
        surface1_get_dc: surface1_get_dc::<V>,
        surface1_release_dc: surface1_release_dc::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::Surface2> Tables<V> {
    /// `IDXGISurface2`'s table for objects of type `V`.
    pub const SURFACE2: &'static Surface2Table = &Surface2Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        surface_get_desc: surface_get_desc::<V>,
        surface_map: surface_map_refused,
        surface_unmap: surface_unmap::<V>,
        surface1_get_dc: surface1_get_dc::<V>,
        surface1_release_dc: surface1_release_dc::<V>,
        surface2_get_resource: surface2_get_resource::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::SwapChain> Tables<V> {
    /// `IDXGISwapChain`'s table for objects of type `V`.
    pub const SWAP_CHAIN: &'static SwapChainTable = &SwapChainTable {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        swap_chain_present: swap_chain_present::<V>,
        swap_chain_get_buffer: swap_chain_get_buffer::<V>,
        swap_chain_set_fullscreen_state: swap_chain_set_fullscreen_state::<V>,
        swap_chain_get_fullscreen_state: swap_chain_get_fullscreen_state::<V>,
        swap_chain_get_desc: swap_chain_get_desc::<V>,
        swap_chain_resize_buffers: swap_chain_resize_buffers::<V>,
        swap_chain_resize_target: swap_chain_resize_target::<V>,
        swap_chain_get_containing_output: swap_chain_get_containing_output::<V>,
        swap_chain_get_frame_statistics: swap_chain_get_frame_statistics::<V>,
        swap_chain_get_last_present_count: swap_chain_get_last_present_count::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::SwapChain1> Tables<V> {
    /// `IDXGISwapChain1`'s table for objects of type `V`.
    pub const SWAP_CHAIN1: &'static SwapChain1Table = &SwapChain1Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        swap_chain_present: swap_chain_present::<V>,
        swap_chain_get_buffer: swap_chain_get_buffer::<V>,
        swap_chain_set_fullscreen_state: swap_chain_set_fullscreen_state::<V>,
        swap_chain_get_fullscreen_state: swap_chain_get_fullscreen_state::<V>,
        swap_chain_get_desc: swap_chain_get_desc::<V>,
        swap_chain_resize_buffers: swap_chain_resize_buffers::<V>,
        swap_chain_resize_target: swap_chain_resize_target::<V>,
        swap_chain_get_containing_output: swap_chain_get_containing_output::<V>,
        swap_chain_get_frame_statistics: swap_chain_get_frame_statistics::<V>,
        swap_chain_get_last_present_count: swap_chain_get_last_present_count::<V>,
        swap_chain1_get_desc1: swap_chain1_get_desc1::<V>,
        swap_chain1_get_fullscreen_desc: swap_chain1_get_fullscreen_desc::<V>,
        swap_chain1_get_hwnd: swap_chain1_get_hwnd::<V>,
        swap_chain1_get_core_window: swap_chain1_get_core_window::<V>,
        swap_chain1_present1: swap_chain1_present1_refused,
        swap_chain1_is_temporary_mono_supported: swap_chain1_is_temporary_mono_supported::<V>,
        swap_chain1_get_restrict_to_output: swap_chain1_get_restrict_to_output::<V>,
        swap_chain1_set_background_color: swap_chain1_set_background_color::<V>,
        swap_chain1_get_background_color: swap_chain1_get_background_color::<V>,
        swap_chain1_set_rotation: swap_chain1_set_rotation::<V>,
        swap_chain1_get_rotation: swap_chain1_get_rotation::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::SwapChain2> Tables<V> {
    /// `IDXGISwapChain2`'s table for objects of type `V`.
    pub const SWAP_CHAIN2: &'static SwapChain2Table = &SwapChain2Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        swap_chain_present: swap_chain_present::<V>,
        swap_chain_get_buffer: swap_chain_get_buffer::<V>,
        swap_chain_set_fullscreen_state: swap_chain_set_fullscreen_state::<V>,
        swap_chain_get_fullscreen_state: swap_chain_get_fullscreen_state::<V>,
        swap_chain_get_desc: swap_chain_get_desc::<V>,
        swap_chain_resize_buffers: swap_chain_resize_buffers::<V>,
        swap_chain_resize_target: swap_chain_resize_target::<V>,
        swap_chain_get_containing_output: swap_chain_get_containing_output::<V>,
        swap_chain_get_frame_statistics: swap_chain_get_frame_statistics::<V>,
        swap_chain_get_last_present_count: swap_chain_get_last_present_count::<V>,
        swap_chain1_get_desc1: swap_chain1_get_desc1::<V>,
        swap_chain1_get_fullscreen_desc: swap_chain1_get_fullscreen_desc::<V>,
        swap_chain1_get_hwnd: swap_chain1_get_hwnd::<V>,
        swap_chain1_get_core_window: swap_chain1_get_core_window::<V>,
        swap_chain1_present1: swap_chain1_present1_refused,
        swap_chain1_is_temporary_mono_supported: swap_chain1_is_temporary_mono_supported::<V>,
        swap_chain1_get_restrict_to_output: swap_chain1_get_restrict_to_output::<V>,
        swap_chain1_set_background_color: swap_chain1_set_background_color::<V>,
        swap_chain1_get_background_color: swap_chain1_get_background_color::<V>,
        swap_chain1_set_rotation: swap_chain1_set_rotation::<V>,
        swap_chain1_get_rotation: swap_chain1_get_rotation::<V>,
        swap_chain2_set_source_size: swap_chain2_set_source_size::<V>,
        swap_chain2_get_source_size: swap_chain2_get_source_size::<V>,
        swap_chain2_set_maximum_frame_latency: swap_chain2_set_maximum_frame_latency::<V>,
        swap_chain2_get_maximum_frame_latency: swap_chain2_get_maximum_frame_latency::<V>,
        swap_chain2_get_frame_latency_waitable_object:
            swap_chain2_get_frame_latency_waitable_object::<V>,
        swap_chain2_set_matrix_transform: swap_chain2_set_matrix_transform::<V>,
        swap_chain2_get_matrix_transform: swap_chain2_get_matrix_transform::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::SwapChain3> Tables<V> {
    /// `IDXGISwapChain3`'s table for objects of type `V`.
    pub const SWAP_CHAIN3: &'static SwapChain3Table = &SwapChain3Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        swap_chain_present: swap_chain_present::<V>,
        swap_chain_get_buffer: swap_chain_get_buffer::<V>,
        swap_chain_set_fullscreen_state: swap_chain_set_fullscreen_state::<V>,
        swap_chain_get_fullscreen_state: swap_chain_get_fullscreen_state::<V>,
        swap_chain_get_desc: swap_chain_get_desc::<V>,
        swap_chain_resize_buffers: swap_chain_resize_buffers::<V>,
        swap_chain_resize_target: swap_chain_resize_target::<V>,
        swap_chain_get_containing_output: swap_chain_get_containing_output::<V>,
        swap_chain_get_frame_statistics: swap_chain_get_frame_statistics::<V>,
        swap_chain_get_last_present_count: swap_chain_get_last_present_count::<V>,
        swap_chain1_get_desc1: swap_chain1_get_desc1::<V>,
        swap_chain1_get_fullscreen_desc: swap_chain1_get_fullscreen_desc::<V>,
        swap_chain1_get_hwnd: swap_chain1_get_hwnd::<V>,
        swap_chain1_get_core_window: swap_chain1_get_core_window::<V>,
        swap_chain1_present1: swap_chain1_present1_refused,
        swap_chain1_is_temporary_mono_supported: swap_chain1_is_temporary_mono_supported::<V>,
        swap_chain1_get_restrict_to_output: swap_chain1_get_restrict_to_output::<V>,
        swap_chain1_set_background_color: swap_chain1_set_background_color::<V>,
        swap_chain1_get_background_color: swap_chain1_get_background_color::<V>,
        swap_chain1_set_rotation: swap_chain1_set_rotation::<V>,
        swap_chain1_get_rotation: swap_chain1_get_rotation::<V>,
        swap_chain2_set_source_size: swap_chain2_set_source_size::<V>,
        swap_chain2_get_source_size: swap_chain2_get_source_size::<V>,
        swap_chain2_set_maximum_frame_latency: swap_chain2_set_maximum_frame_latency::<V>,
        swap_chain2_get_maximum_frame_latency: swap_chain2_get_maximum_frame_latency::<V>,
        swap_chain2_get_frame_latency_waitable_object:
            swap_chain2_get_frame_latency_waitable_object::<V>,
        swap_chain2_set_matrix_transform: swap_chain2_set_matrix_transform::<V>,
        swap_chain2_get_matrix_transform: swap_chain2_get_matrix_transform::<V>,
        swap_chain3_get_current_back_buffer_index: swap_chain3_get_current_back_buffer_index::<V>,
        swap_chain3_check_color_space_support: swap_chain3_check_color_space_support::<V>,
        swap_chain3_set_color_space1: swap_chain3_set_color_space1::<V>,
        swap_chain3_resize_buffers1: swap_chain3_resize_buffers1_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::SwapChain4> Tables<V> {
    /// `IDXGISwapChain4`'s table for objects of type `V`.
    pub(crate) const SWAP_CHAIN4: &'static SwapChain4Table = &SwapChain4Table {
        unknown: unknown_table::<V>(),
        object_set_private_data: object_set_private_data::<V>,
        object_set_private_data_interface: object_set_private_data_interface::<V>,
        object_get_private_data: object_get_private_data_refused,
        object_get_parent: object_get_parent::<V>,
        device_sub_object_get_device: device_sub_object_get_device::<V>,
        swap_chain_present: swap_chain_present::<V>,
        swap_chain_get_buffer: swap_chain_get_buffer::<V>,
        swap_chain_set_fullscreen_state: swap_chain_set_fullscreen_state::<V>,
        swap_chain_get_fullscreen_state: swap_chain_get_fullscreen_state::<V>,
        swap_chain_get_desc: swap_chain_get_desc::<V>,
        swap_chain_resize_buffers: swap_chain_resize_buffers::<V>,
        swap_chain_resize_target: swap_chain_resize_target::<V>,
        swap_chain_get_containing_output: swap_chain_get_containing_output::<V>,
        swap_chain_get_frame_statistics: swap_chain_get_frame_statistics::<V>,
        swap_chain_get_last_present_count: swap_chain_get_last_present_count::<V>,
        swap_chain1_get_desc1: swap_chain1_get_desc1::<V>,
        swap_chain1_get_fullscreen_desc: swap_chain1_get_fullscreen_desc::<V>,
        swap_chain1_get_hwnd: swap_chain1_get_hwnd::<V>,
        swap_chain1_get_core_window: swap_chain1_get_core_window::<V>,
        swap_chain1_present1: swap_chain1_present1_refused,
        swap_chain1_is_temporary_mono_supported: swap_chain1_is_temporary_mono_supported::<V>,
        swap_chain1_get_restrict_to_output: swap_chain1_get_restrict_to_output::<V>,
        swap_chain1_set_background_color: swap_chain1_set_background_color::<V>,
        swap_chain1_get_background_color: swap_chain1_get_background_color::<V>,
        swap_chain1_set_rotation: swap_chain1_set_rotation::<V>,
        swap_chain1_get_rotation: swap_chain1_get_rotation::<V>,
        swap_chain2_set_source_size: swap_chain2_set_source_size::<V>,
        swap_chain2_get_source_size: swap_chain2_get_source_size::<V>,
        swap_chain2_set_maximum_frame_latency: swap_chain2_set_maximum_frame_latency::<V>,
        swap_chain2_get_maximum_frame_latency: swap_chain2_get_maximum_frame_latency::<V>,
        swap_chain2_get_frame_latency_waitable_object:
            swap_chain2_get_frame_latency_waitable_object::<V>,
        swap_chain2_set_matrix_transform: swap_chain2_set_matrix_transform::<V>,
        swap_chain2_get_matrix_transform: swap_chain2_get_matrix_transform::<V>,
        swap_chain3_get_current_back_buffer_index: swap_chain3_get_current_back_buffer_index::<V>,
        swap_chain3_check_color_space_support: swap_chain3_check_color_space_support::<V>,
        swap_chain3_set_color_space1: swap_chain3_set_color_space1::<V>,
        swap_chain3_resize_buffers1: swap_chain3_resize_buffers1_refused,
        swap_chain4_set_hdr_meta_data: swap_chain4_set_hdr_meta_data::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::SwapChainMedia> Tables<V> {
    /// `IDXGISwapChainMedia`'s table for objects of type `V`.
    pub(crate) const SWAP_CHAIN_MEDIA: &'static SwapChainMediaTable = &SwapChainMediaTable {
        unknown: unknown_table::<V>(),
        swap_chain_media_get_frame_statistics_media: swap_chain_media_get_frame_statistics_media::<V>,
        swap_chain_media_set_present_duration: swap_chain_media_set_present_duration::<V>,
        swap_chain_media_check_present_duration_support:
            swap_chain_media_check_present_duration_support::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: directx_graphics_infrastructure::GraphicsAnalysis> Tables<V> {
    /// `IDXGraphicsAnalysis`'s table for objects of type `V`.
    pub(crate) const GRAPHICS_ANALYSIS: &'static GraphicsAnalysisTable = &GraphicsAnalysisTable {
        unknown: unknown_table::<V>(),
        graphics_analysis_begin_capture: graphics_analysis_begin_capture::<V>,
        graphics_analysis_end_capture: graphics_analysis_end_capture::<V>,
    };
}

/// The tables an object of the kind `Adapter` answers, by interface; IUnknown's is the first.
pub fn adapter_interfaces<V: directx_graphics_infrastructure::Adapter4>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Adapter4Table` lays out `IDXGIAdapter4`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::ADAPTER4) });
    }
    None
}

/// The tables an object of the kind `Debug` answers, by interface; IUnknown's is the first.
pub fn debug_interfaces<V: directx_graphics_infrastructure::Debug1>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Debug1Table` lays out `IDXGIDebug1`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DEBUG1) });
    }
    None
}

/// The tables an object of the kind `DecodeSwapChain` answers, by interface; IUnknown's is the first.
pub fn decode_swap_chain_interfaces<V: directx_graphics_infrastructure::DecodeSwapChain>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN]
            .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `DecodeSwapChainTable` lays out `IDXGIDecodeSwapChain`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DECODE_SWAP_CHAIN) });
    }
    None
}

/// The tables an object of the kind `Device` answers, by interface; IUnknown's is the first.
pub fn device_interfaces<V: directx_graphics_infrastructure::Device4>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Device4Table` lays out `IDXGIDevice4`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DEVICE4) });
    }
    None
}

/// The tables an object of the kind `DisplayControl` answers, by interface; IUnknown's is the first.
pub fn display_control_interfaces<V: directx_graphics_infrastructure::DisplayControl>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL]
            .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `DisplayControlTable` lays out `IDXGIDisplayControl`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DISPLAY_CONTROL) });
    }
    None
}

/// The tables an object of the kind `Factory` answers, by interface; IUnknown's is the first.
pub fn factory_interfaces<V: directx_graphics_infrastructure::Factory7>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Factory7Table` lays out `IDXGIFactory7`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::FACTORY7) });
    }
    None
}

/// The tables an object of the kind `FactoryMedia` answers, by interface; IUnknown's is the first.
pub fn factory_media_interfaces<V: directx_graphics_infrastructure::FactoryMedia>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA].contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `FactoryMediaTable` lays out `IDXGIFactoryMedia`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::FACTORY_MEDIA) });
    }
    None
}

/// The tables an object of the kind `InfoQueue` answers, by interface; IUnknown's is the first.
pub fn info_queue_interfaces<V: directx_graphics_infrastructure::InfoQueue>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE].contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `InfoQueueTable` lays out `IDXGIInfoQueue`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::INFO_QUEUE) });
    }
    None
}

/// The tables an object of the kind `KeyedMutex` answers, by interface; IUnknown's is the first.
pub fn keyed_mutex_interfaces<V: directx_graphics_infrastructure::KeyedMutex>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `KeyedMutexTable` lays out `IDXGIKeyedMutex`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::KEYED_MUTEX) });
    }
    None
}

/// The tables an object of the kind `Output` answers, by interface; IUnknown's is the first.
pub fn output_interfaces<V: directx_graphics_infrastructure::Output6>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Output6Table` lays out `IDXGIOutput6`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::OUTPUT6) });
    }
    None
}

/// The tables an object of the kind `OutputDuplication` answers, by interface; IUnknown's is the first.
pub fn output_duplication_interfaces<V: directx_graphics_infrastructure::OutputDuplication>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `OutputDuplicationTable` lays out `IDXGIOutputDuplication`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::OUTPUT_DUPLICATION) });
    }
    None
}

/// The tables an object of the kind `Resource` answers, by interface; IUnknown's is the first.
pub fn resource_interfaces<V: directx_graphics_infrastructure::Resource1>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
            directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Resource1Table` lays out `IDXGIResource1`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::RESOURCE1) });
    }
    None
}

/// The tables an object of the kind `Surface` answers, by interface; IUnknown's is the first.
pub fn surface_interfaces<V: directx_graphics_infrastructure::Surface2>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
            directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
            directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `Surface2Table` lays out `IDXGISurface2`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::SURFACE2) });
    }
    None
}

/// The tables an object of the kind `SwapChain` answers, by interface; IUnknown's is the first.
pub fn swap_chain_interfaces<V: directx_graphics_infrastructure::SwapChain4>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `SwapChain4Table` lays out `IDXGISwapChain4`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::SWAP_CHAIN4) });
    }
    None
}

/// The tables an object of the kind `SwapChainMedia` answers, by interface; IUnknown's is the first.
pub fn swap_chain_media_interfaces<V: directx_graphics_infrastructure::SwapChainMedia>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA]
            .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `SwapChainMediaTable` lays out `IDXGISwapChainMedia`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::SWAP_CHAIN_MEDIA) });
    }
    None
}

/// The tables an object of the kind `GraphicsAnalysis` answers, by interface; IUnknown's is the first.
pub fn graphics_analysis_interfaces<V: directx_graphics_infrastructure::GraphicsAnalysis>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS]
            .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `GraphicsAnalysisTable` lays out `IDXGraphicsAnalysis`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::GRAPHICS_ANALYSIS) });
    }
    None
}

/// An `IUnknown*` the C side passed: one of our objects of a kind of this API, held for the call, or any other.
pub enum UnknownFound<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure> {
    Adapter(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>),
    Debug(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>),
    DecodeSwapChain(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>),
    Device(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>),
    DisplayControl(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>),
    Factory(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>),
    FactoryMedia(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>),
    InfoQueue(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>),
    KeyedMutex(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>),
    Output(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>),
    OutputDuplication(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>),
    Resource(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>),
    Surface(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>),
    SwapChain(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>),
    SwapChainMedia(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>),
    GraphicsAnalysis(Found<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>),
    Unknown(Foreign),
}

impl<A: directx_graphics_infrastructure::DirectXGraphicsInfrastructure> UnknownFound<A> {
    /// Finds the object at this address: one of ours, else the C side's, of which it takes a reference; none for null.
    ///
    /// # Safety
    /// `raw` is null or a live COM object, for the call.
    #[expect(unsafe_code, reason = "an IUnknown* the C side passes in")]
    pub unsafe fn find(raw: Raw) -> Option<UnknownFound<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            let address = raw.addr();
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
            >(KEY, address)
            {
                return Some(UnknownFound::Adapter(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug,
            >(KEY, address)
            {
                return Some(UnknownFound::Debug(found));
            }
            if let Some(found) = object::<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, address) { return Some(UnknownFound::DecodeSwapChain(found)); }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device,
            >(KEY, address)
            {
                return Some(UnknownFound::Device(found));
            }
            if let Some(found) = object::<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, address) { return Some(UnknownFound::DisplayControl(found)); }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory,
            >(KEY, address)
            {
                return Some(UnknownFound::Factory(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia,
            >(KEY, address)
            {
                return Some(UnknownFound::FactoryMedia(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue,
            >(KEY, address)
            {
                return Some(UnknownFound::InfoQueue(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex,
            >(KEY, address)
            {
                return Some(UnknownFound::KeyedMutex(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
            >(KEY, address)
            {
                return Some(UnknownFound::Output(found));
            }
            if let Some(found) = object::<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, address) { return Some(UnknownFound::OutputDuplication(found)); }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
            >(KEY, address)
            {
                return Some(UnknownFound::Resource(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
            >(KEY, address)
            {
                return Some(UnknownFound::Surface(found));
            }
            if let Some(found) = object::<
                <A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
            >(KEY, address)
            {
                return Some(UnknownFound::SwapChain(found));
            }
            if let Some(found) = object::<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, address) { return Some(UnknownFound::SwapChainMedia(found)); }
            if let Some(found) = object::<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, address) { return Some(UnknownFound::GraphicsAnalysis(found)); }
            #[expect(unsafe_code, reason = "an IUnknown* the C side passes in")]
            // SAFETY: the caller's promise.
            let foreign = unsafe { ForeignUnknown::from_borrowed(raw) }?;
            Some(UnknownFound::Unknown(Foreign::new(
                address,
                Arc::new(foreign),
            )))
        };
        body()
    }

    /// The object as the trait takes it.
    pub fn get(&self) -> directx_graphics_infrastructure::UnknownRef<'_, A> {
        match self {
            UnknownFound::Adapter(found) => {
                directx_graphics_infrastructure::UnknownRef::Adapter(found.value())
            }
            UnknownFound::Debug(found) => {
                directx_graphics_infrastructure::UnknownRef::Debug(found.value())
            }
            UnknownFound::DecodeSwapChain(found) => {
                directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(found.value())
            }
            UnknownFound::Device(found) => {
                directx_graphics_infrastructure::UnknownRef::Device(found.value())
            }
            UnknownFound::DisplayControl(found) => {
                directx_graphics_infrastructure::UnknownRef::DisplayControl(found.value())
            }
            UnknownFound::Factory(found) => {
                directx_graphics_infrastructure::UnknownRef::Factory(found.value())
            }
            UnknownFound::FactoryMedia(found) => {
                directx_graphics_infrastructure::UnknownRef::FactoryMedia(found.value())
            }
            UnknownFound::InfoQueue(found) => {
                directx_graphics_infrastructure::UnknownRef::InfoQueue(found.value())
            }
            UnknownFound::KeyedMutex(found) => {
                directx_graphics_infrastructure::UnknownRef::KeyedMutex(found.value())
            }
            UnknownFound::Output(found) => {
                directx_graphics_infrastructure::UnknownRef::Output(found.value())
            }
            UnknownFound::OutputDuplication(found) => {
                directx_graphics_infrastructure::UnknownRef::OutputDuplication(found.value())
            }
            UnknownFound::Resource(found) => {
                directx_graphics_infrastructure::UnknownRef::Resource(found.value())
            }
            UnknownFound::Surface(found) => {
                directx_graphics_infrastructure::UnknownRef::Surface(found.value())
            }
            UnknownFound::SwapChain(found) => {
                directx_graphics_infrastructure::UnknownRef::SwapChain(found.value())
            }
            UnknownFound::SwapChainMedia(found) => {
                directx_graphics_infrastructure::UnknownRef::SwapChainMedia(found.value())
            }
            UnknownFound::GraphicsAnalysis(found) => {
                directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(found.value())
            }
            UnknownFound::Unknown(foreign) => {
                directx_graphics_infrastructure::UnknownRef::Unknown(foreign)
            }
        }
    }
}

/// The tables of each type served as a `Buffer`, by the glue of the API that binds it.
static BOUND_BUFFER: Mutex<Vec<(core::any::TypeId, Interfaces)>> = Mutex::new(Vec::new());

/// Registers the tables of `B`, the type an API built on this one binds as its `Buffer`.
pub fn bind_buffer<B: 'static>(interfaces: Interfaces) {
    let mut bound = BOUND_BUFFER.lock().unwrap_or_else(PoisonError::into_inner);
    if !bound
        .iter()
        .any(|(kind, _)| *kind == core::any::TypeId::of::<B>())
    {
        bound.push((core::any::TypeId::of::<B>(), interfaces));
    }
}

/// Returns the tables registered for `B`, if any.
pub(crate) fn bound_buffer<B: 'static>() -> Option<Interfaces> {
    let bound = BOUND_BUFFER.lock().unwrap_or_else(PoisonError::into_inner);
    bound
        .iter()
        .find(|(kind, _)| *kind == core::any::TypeId::of::<B>())
        .map(|(_, interfaces)| *interfaces)
}

/// Every module served, for the life of the program: the exports find theirs by type.
static MODULES: Mutex<Vec<Arc<dyn Any + Send + Sync>>> = Mutex::new(Vec::new());

fn module<M: Send + Sync + 'static>() -> Option<Arc<M>> {
    let held = MODULES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|module| module.is::<M>())
        .cloned();
    held.and_then(|module| module.downcast::<M>().ok())
}

/// Serves `module` as the library: the exports C calls, which call it. A module of a type already served
/// is not replaced.
pub fn exports<M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    module: M,
) -> Library {
    {
        let mut modules = MODULES.lock().unwrap_or_else(PoisonError::into_inner);
        if !modules.iter().any(|held| held.is::<M>()) {
            modules.push(Arc::new(module));
        }
    }
    Library {
        create_dxgi_factory: export_create_dxgi_factory::<M>,
        create_dxgi_factory1: export_create_dxgi_factory1::<M>,
        create_dxgi_factory2: export_create_dxgi_factory2::<M>,
        dxgi_declare_adapter_removal_support: export_dxgi_declare_adapter_removal_support::<M>,
        dxgi_disable_v_blank_virtualization: export_dxgi_disable_v_blank_virtualization::<M>,
        dxgi_get_debug_interface1: export_dxgi_get_debug_interface1::<M>,
    }
}

/// The calling glue over the serving glue's own exports of `module`: `module` reached through both glues, as if
/// it were the system's library.
pub fn served<M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure>(
    module: M,
) -> crate::SystemDirectXGraphicsInfrastructure {
    let library = exports(module);
    #[expect(
        unsafe_code,
        reason = "our own serving glue's exports, which take what the metadata gives"
    )]
    // SAFETY: each entry of `library` is this crate's serving export of that name, written from the same metadata.
    unsafe {
        crate::SystemDirectXGraphicsInfrastructure::new(library)
    }
}

#[cfg(windows)]
#[link(name = "dxgi")]
#[expect(
    unsafe_code,
    reason = "dxgi.dll's own exports, declared with the parameters the metadata gives"
)]
unsafe extern "system" {
    #[link_name = "CreateDXGIFactory"]
    fn system_create_dxgi_factory(riid: *const u8, factory: *mut Raw) -> i32;
    #[link_name = "CreateDXGIFactory1"]
    fn system_create_dxgi_factory1(riid: *const u8, factory: *mut Raw) -> i32;
    #[link_name = "CreateDXGIFactory2"]
    fn system_create_dxgi_factory2(flags: u32, riid: *const u8, factory: *mut Raw) -> i32;
    #[link_name = "DXGIDeclareAdapterRemovalSupport"]
    fn system_dxgi_declare_adapter_removal_support() -> i32;
    #[link_name = "DXGIDisableVBlankVirtualization"]
    fn system_dxgi_disable_v_blank_virtualization() -> i32;
    #[link_name = "DXGIGetDebugInterface1"]
    fn system_dxgi_get_debug_interface1(flags: u32, riid: *const u8, debug: *mut Raw) -> i32;
}

#[cfg(windows)]
impl crate::SystemDirectXGraphicsInfrastructure {
    /// Returns the API over the system's dxgi.dll, linked by name: each entry is that library's own export of its
    /// name, declared here with the parameters the metadata gives.
    pub fn system() -> crate::SystemDirectXGraphicsInfrastructure {
        let library = Library {
            create_dxgi_factory: system_create_dxgi_factory,
            create_dxgi_factory1: system_create_dxgi_factory1,
            create_dxgi_factory2: system_create_dxgi_factory2,
            dxgi_declare_adapter_removal_support: system_dxgi_declare_adapter_removal_support,
            dxgi_disable_v_blank_virtualization: system_dxgi_disable_v_blank_virtualization,
            dxgi_get_debug_interface1: system_dxgi_get_debug_interface1,
        };
        #[expect(unsafe_code, reason = "dxgi.dll's own exports, linked by name above")]
        // SAFETY: each entry of `library` is dxgi.dll's export of that name, declared above with the metadata's parameters.
        unsafe {
            crate::SystemDirectXGraphicsInfrastructure::new(library)
        }
    }
}

/// `IDXGIAdapter::EnumOutputs` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter::EnumOutputs, which the C side calls"
)]
pub unsafe extern "system" fn adapter_enum_outputs<V: directx_graphics_infrastructure::Adapter>(
    this: Raw,
    output: u32,
    output_: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if output_.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let output = output;
            let output__value = found.value().enum_outputs(output)?;
            let this_weak = found.weak();
            drop(found);
            let output__record =
                this_weak.map_or_else(record, |weak| record_related("Adapter", weak));
            let output__served = serve(KEY, output__value, output__record, output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if output_.is_null() {
                0
            } else {
                output__served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let output__written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter::EnumOutputs: ppOutput, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppOutput` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(output_.cast::<u8>(), &output__written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter::GetDesc` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGIAdapter::GetDesc, which the C side calls")]
pub unsafe extern "system" fn adapter_get_desc<V: directx_graphics_infrastructure::Adapter>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDescLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_adapter_desc(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter::GetDesc: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter::CheckInterfaceSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter::CheckInterfaceSupport, which the C side calls"
)]
pub unsafe extern "system" fn adapter_check_interface_support<
    V: directx_graphics_infrastructure::Adapter,
>(
    this: Raw,
    interface_name: *const u8,
    umd_version: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if umd_version.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut interface_name_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let interface_name_null = interface_name.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIAdapter::CheckInterfaceSupport: InterfaceName, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `InterfaceName` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let interface_name_read =
                unsafe { read_bytes(interface_name, &mut interface_name_bytes) };
            let interface_name = interface_name_read.then(|| decode_guid(&interface_name_bytes));
            let interface_name = interface_name.ok_or(E_INVALIDARG)?;
            let umd_version_value = found.value().check_interface_support(&interface_name)?;
            drop(found);
            let mut umd_version_bytes = [u8::MIN; 8];
            put_i64(&mut umd_version_bytes, usize::MIN, umd_version_value);
            Ok((umd_version_bytes,))
        });
        let umd_version_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 8], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter::CheckInterfaceSupport: pUMDVersion, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pUMDVersion` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(umd_version.cast::<u8>(), &umd_version_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter1::GetDesc1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter1::GetDesc1, which the C side calls"
)]
pub unsafe extern "system" fn adapter1_get_desc1<V: directx_graphics_infrastructure::Adapter1>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc1()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout>()];
            encode_windows_win32_graphics_dxgi_dxgi_adapter_desc1(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter1::GetDesc1: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter2::GetDesc2` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter2::GetDesc2, which the C side calls"
)]
pub unsafe extern "system" fn adapter2_get_desc2<V: directx_graphics_infrastructure::Adapter2>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc2()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout>()];
            encode_windows_win32_graphics_dxgi_dxgi_adapter_desc2(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter2::GetDesc2: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent, which the C side calls"
)]
pub unsafe extern "system" fn adapter3_register_hardware_content_protection_teardown_status_event<
    V: directx_graphics_infrastructure::Adapter3,
>(
    this: Raw,
    h_event: usize,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_event = component_object_model::structures::Handle(h_event);
            let pdw_cookie_value = found
                .value()
                .register_hardware_content_protection_teardown_status_event(h_event)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus, which the C side calls"
)]
pub unsafe extern "system" fn adapter3_unregister_hardware_content_protection_teardown_status<
    V: directx_graphics_infrastructure::Adapter3,
>(
    this: Raw,
    dw_cookie: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let dw_cookie = dw_cookie;
            let () = found
                .value()
                .unregister_hardware_content_protection_teardown_status(dw_cookie);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIAdapter3::QueryVideoMemoryInfo` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter3::QueryVideoMemoryInfo, which the C side calls"
)]
pub unsafe extern "system" fn adapter3_query_video_memory_info<
    V: directx_graphics_infrastructure::Adapter3,
>(
    this: Raw,
    node_index: u32,
    memory_segment_group: i32,
    video_memory_info: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if video_memory_info.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let node_index = node_index;
            let memory_segment_group =
                directx_graphics_infrastructure::enumerations::MemorySegmentGroup(
                    memory_segment_group,
                );
            let video_memory_info_value = found
                .value()
                .query_video_memory_info(node_index, memory_segment_group)?;
            drop(found);
            let mut video_memory_info_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_query_video_memory_info(
                &video_memory_info_value,
                &mut video_memory_info_bytes,
            );
            Ok((video_memory_info_bytes,))
        });
        let video_memory_info_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::QueryVideoMemoryInfo: pVideoMemoryInfo, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pVideoMemoryInfo` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(video_memory_info.cast::<u8>(), &video_memory_info_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter3::SetVideoMemoryReservation` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter3::SetVideoMemoryReservation, which the C side calls"
)]
pub unsafe extern "system" fn adapter3_set_video_memory_reservation<
    V: directx_graphics_infrastructure::Adapter3,
>(
    this: Raw,
    node_index: u32,
    memory_segment_group: i32,
    reservation: u64,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let node_index = node_index;
            let memory_segment_group =
                directx_graphics_infrastructure::enumerations::MemorySegmentGroup(
                    memory_segment_group,
                );
            let reservation = reservation;
            let () = found.value().set_video_memory_reservation(
                node_index,
                memory_segment_group,
                reservation,
            )?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent, which the C side calls"
)]
pub unsafe extern "system" fn adapter3_register_video_memory_budget_change_notification_event<
    V: directx_graphics_infrastructure::Adapter3,
>(
    this: Raw,
    h_event: usize,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_event = component_object_model::structures::Handle(h_event);
            let pdw_cookie_value = found
                .value()
                .register_video_memory_budget_change_notification_event(h_event)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification, which the C side calls"
)]
pub unsafe extern "system" fn adapter3_unregister_video_memory_budget_change_notification<
    V: directx_graphics_infrastructure::Adapter3,
>(
    this: Raw,
    dw_cookie: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let dw_cookie = dw_cookie;
            let () = found
                .value()
                .unregister_video_memory_budget_change_notification(dw_cookie);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIAdapter4::GetDesc3` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIAdapter4::GetDesc3, which the C side calls"
)]
pub(crate) unsafe extern "system" fn adapter4_get_desc3<
    V: directx_graphics_infrastructure::Adapter4,
>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc3()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout>()];
            encode_windows_win32_graphics_dxgi_dxgi_adapter_desc3(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter4::GetDesc3: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDebug::ReportLiveObjects` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDebug::ReportLiveObjects, which the C side calls"
)]
pub unsafe extern "system" fn debug_report_live_objects<
    V: directx_graphics_infrastructure::Debug,
>(
    this: Raw,
    apiid: component_object_model_direct::GuidAbi,
    flags: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let apiid = apiid.guid();
            let flags = directx_graphics_infrastructure::enumerations::DebugRloFlags(flags);
            let () = found.value().report_live_objects(apiid, flags)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDebug1::EnableLeakTrackingForThread` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDebug1::EnableLeakTrackingForThread, which the C side calls"
)]
pub(crate) unsafe extern "system" fn debug1_enable_leak_tracking_for_thread<
    V: directx_graphics_infrastructure::Debug1,
>(
    this: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let () = found.value().enable_leak_tracking_for_thread();
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIDebug1::DisableLeakTrackingForThread` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDebug1::DisableLeakTrackingForThread, which the C side calls"
)]
pub(crate) unsafe extern "system" fn debug1_disable_leak_tracking_for_thread<
    V: directx_graphics_infrastructure::Debug1,
>(
    this: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let () = found.value().disable_leak_tracking_for_thread();
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIDebug1::IsLeakTrackingEnabledForThread` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDebug1::IsLeakTrackingEnabledForThread, which the C side calls"
)]
pub(crate) unsafe extern "system" fn debug1_is_leak_tracking_enabled_for_thread<
    V: directx_graphics_infrastructure::Debug1,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().is_leak_tracking_enabled_for_thread();
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIDecodeSwapChain::PresentBuffer` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::PresentBuffer, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_present_buffer<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    buffer_to_present: u32,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let buffer_to_present = buffer_to_present;
            let sync_interval = sync_interval;
            let flags = directx_graphics_infrastructure::enumerations::Present(flags);
            let () = found
                .value()
                .present_buffer(buffer_to_present, sync_interval, flags)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::SetSourceRect` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::SetSourceRect, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_set_source_rect<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    rect: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut rect_bytes = [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()];
            let rect_null = rect.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIDecodeSwapChain::SetSourceRect: pRect, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pRect` is `In` and `Const`: one `Rect` of 16 bytes, readable for the call.
            let rect_read = unsafe { read_bytes(rect, &mut rect_bytes) };
            let rect = rect_read.then(|| decode_windows_win32_foundation_rect(&rect_bytes));
            let rect = rect.ok_or(E_INVALIDARG)?;
            let () = found.value().set_source_rect(&rect)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::SetTargetRect` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::SetTargetRect, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_set_target_rect<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    rect: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut rect_bytes = [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()];
            let rect_null = rect.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIDecodeSwapChain::SetTargetRect: pRect, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pRect` is `In` and `Const`: one `Rect` of 16 bytes, readable for the call.
            let rect_read = unsafe { read_bytes(rect, &mut rect_bytes) };
            let rect = rect_read.then(|| decode_windows_win32_foundation_rect(&rect_bytes));
            let rect = rect.ok_or(E_INVALIDARG)?;
            let () = found.value().set_target_rect(&rect)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::SetDestSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::SetDestSize, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_set_dest_size<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    width: u32,
    height: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let width = width;
            let height = height;
            let () = found.value().set_dest_size(width, height)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::GetSourceRect` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::GetSourceRect, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_get_source_rect<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    rect: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if rect.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let rect_value = found.value().get_source_rect()?;
            drop(found);
            let mut rect_bytes = [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()];
            encode_windows_win32_foundation_rect(&rect_value, &mut rect_bytes);
            Ok((rect_bytes,))
        });
        let rect_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetSourceRect: pRect, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pRect` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(rect.cast::<u8>(), &rect_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::GetTargetRect` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::GetTargetRect, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_get_target_rect<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    rect: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if rect.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let rect_value = found.value().get_target_rect()?;
            drop(found);
            let mut rect_bytes = [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()];
            encode_windows_win32_foundation_rect(&rect_value, &mut rect_bytes);
            Ok((rect_bytes,))
        });
        let rect_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetTargetRect: pRect, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pRect` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(rect.cast::<u8>(), &rect_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::GetDestSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::GetDestSize, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_get_dest_size<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    width: *mut u8,
    height: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if width.is_null() {
                return Err(E_POINTER);
            }
            if height.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let (width_value, height_value) = found.value().get_dest_size()?;
            drop(found);
            let mut width_bytes = [u8::MIN; 4];
            put_u32(&mut width_bytes, usize::MIN, width_value);
            let mut height_bytes = [u8::MIN; 4];
            put_u32(&mut height_bytes, usize::MIN, height_value);
            Ok((width_bytes, height_bytes))
        });
        let width_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetDestSize: pWidth, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pWidth` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(width.cast::<u8>(), &width_written) };
        let height_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetDestSize: pHeight, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pHeight` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(height.cast::<u8>(), &height_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::SetColorSpace` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::SetColorSpace, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_set_color_space<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
    color_space: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let color_space =
                directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags(
                    color_space,
                );
            let () = found.value().set_color_space(color_space)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDecodeSwapChain::GetColorSpace` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDecodeSwapChain::GetColorSpace, which the C side calls"
)]
pub(crate) unsafe extern "system" fn decode_swap_chain_get_color_space<
    V: directx_graphics_infrastructure::DecodeSwapChain,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_color_space();
            drop(found);
            Some((returned.0,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIDevice::GetAdapter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice::GetAdapter, which the C side calls"
)]
pub unsafe extern "system" fn device_get_adapter<V: directx_graphics_infrastructure::Device>(
    this: Raw,
    adapter: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if adapter.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let adapter_value = found.value().get_adapter()?;
            let this_weak = found.weak();
            drop(found);
            let adapter_record =
                this_weak.map_or_else(record, |weak| record_related("Device", weak));
            let adapter_served = serve(KEY, adapter_value, adapter_record, adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if adapter.is_null() {
                0
            } else {
                adapter_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let adapter_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDevice::GetAdapter: pAdapter, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pAdapter` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(adapter.cast::<u8>(), &adapter_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice::CreateSurface` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice::CreateSurface, which the C side calls"
)]
pub unsafe extern "system" fn device_create_surface<V: directx_graphics_infrastructure::Device>(
    this: Raw,
    desc: *const u8,
    num_surfaces: u32,
    usage: u32,
    shared_resource: *const u8,
    surface: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let surface_length: Option<usize> = usize::try_from(num_surfaces)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * size_of::<usize>());
        let answer = guarded(Err(E_FAIL), move || {
            let surface_length = surface_length.ok_or(E_INVALIDARG)?;
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIDevice::CreateSurface: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `SurfaceDesc` of 20 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_surface_desc(&desc_bytes));
            let desc = desc.ok_or(E_INVALIDARG)?;
            let num_surfaces = num_surfaces;
            let usage = directx_graphics_infrastructure::enumerations::Usage(usage);
            let mut shared_resource_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSharedResourceLayout>()];
            let shared_resource_null = shared_resource.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIDevice::CreateSurface: pSharedResource, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pSharedResource` is `In` and `Const`: one `SharedResource` of 8 bytes, readable for the call.
            let shared_resource_read =
                unsafe { read_bytes(shared_resource, &mut shared_resource_bytes) };
            let shared_resource = shared_resource_read.then(|| {
                decode_windows_win32_graphics_dxgi_dxgi_shared_resource(&shared_resource_bytes)
            });
            if !shared_resource_read && !shared_resource_null {
                return Err(E_INVALIDARG);
            }
            let surface_value = found.value().create_surface(
                &desc,
                num_surfaces,
                usage,
                shared_resource.as_ref(),
            )?;
            drop(found);
            let mut surface_value = surface_value;
            let surface_room = if surface.is_null() {
                usize::MIN
            } else {
                surface_length / size_of::<usize>()
            };
            if surface_value.len() > surface_room {
                for extra in surface_value.drain(surface_room..) {
                    guarded((), move || drop(extra));
                }
            }
            let mut surface_bytes = vec![u8::MIN; surface_length];
            if !surface.is_null() {
                for (value, bytes) in surface_value.into_iter().zip(
                    surface_bytes
                        .as_chunks_mut::<{ size_of::<usize>() }>()
                        .0
                        .iter_mut(),
                ) {
                    let address = value.and_then(|value| serve(KEY, value, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>)).map_or(0, |served| served.into_address().get());
                    put_usize(bytes, usize::MIN, address);
                }
            }
            Ok((surface_bytes,))
        });
        if let Some(length) = surface_length {
            let surface_written = answer
                .as_ref()
                .ok()
                .map_or_else(|| vec![u8::MIN; length], |written| written.0.clone());
            #[expect(
                unsafe_code,
                reason = "IDXGIDevice::CreateSurface: ppSurface, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `NumSurfaces` as the count of `ppSurface`, which is `Out`: that many elements of size_of::<usize>() bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(surface.cast::<u8>(), &surface_written) };
        }
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice::SetGPUThreadPriority` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice::SetGPUThreadPriority, which the C side calls"
)]
pub unsafe extern "system" fn device_set_gpu_thread_priority<
    V: directx_graphics_infrastructure::Device,
>(
    this: Raw,
    priority: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let priority = priority;
            let () = found.value().set_gpu_thread_priority(priority)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice::GetGPUThreadPriority` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice::GetGPUThreadPriority, which the C side calls"
)]
pub unsafe extern "system" fn device_get_gpu_thread_priority<
    V: directx_graphics_infrastructure::Device,
>(
    this: Raw,
    priority: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if priority.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let priority_value = found.value().get_gpu_thread_priority()?;
            drop(found);
            let mut priority_bytes = [u8::MIN; 4];
            put_i32(&mut priority_bytes, usize::MIN, priority_value);
            Ok((priority_bytes,))
        });
        let priority_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDevice::GetGPUThreadPriority: pPriority, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pPriority` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(priority.cast::<u8>(), &priority_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// How many calls reached an entry this serving glue refuses by its form, in the whole program.
static REFUSALS_REACHED: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Returns how many calls reached an entry this serving glue refuses by its form, in the whole program: none, where
/// every caller is the calling glue, which refuses each such call itself.
pub fn refusals_reached() -> usize {
    REFUSALS_REACHED.load(core::sync::atomic::Ordering::Relaxed)
}

/// `IDXGIDevice::QueryResourceResidency` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn device_query_resource_residency_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: *mut u8,
    _p2: u32,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIDevice1::SetMaximumFrameLatency` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice1::SetMaximumFrameLatency, which the C side calls"
)]
pub unsafe extern "system" fn device1_set_maximum_frame_latency<
    V: directx_graphics_infrastructure::Device1,
>(
    this: Raw,
    max_latency: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let max_latency = max_latency;
            let () = found.value().set_maximum_frame_latency(max_latency)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice1::GetMaximumFrameLatency` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice1::GetMaximumFrameLatency, which the C side calls"
)]
pub unsafe extern "system" fn device1_get_maximum_frame_latency<
    V: directx_graphics_infrastructure::Device1,
>(
    this: Raw,
    max_latency: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if max_latency.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let max_latency_value = found.value().get_maximum_frame_latency()?;
            drop(found);
            let mut max_latency_bytes = [u8::MIN; 4];
            put_u32(&mut max_latency_bytes, usize::MIN, max_latency_value);
            Ok((max_latency_bytes,))
        });
        let max_latency_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDevice1::GetMaximumFrameLatency: pMaxLatency, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pMaxLatency` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(max_latency.cast::<u8>(), &max_latency_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice2::OfferResources` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice2::OfferResources, which the C side calls"
)]
pub unsafe extern "system" fn device2_offer_resources<
    V: directx_graphics_infrastructure::Device2,
>(
    this: Raw,
    num_resources: u32,
    resources: *const Raw,
    priority: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let resources_count = usize::try_from(num_resources)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let resources_bytes = if resources_count == 0 {
                Some(Vec::new())
            } else if resources.is_null() {
                None
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGIDevice2::OfferResources: ppResources, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumResources` as the count of `ppResources`: that many pointers, readable for the call.
                unsafe {
                    read_only_at(resources.cast::<u8>(), resources_count * size_of::<usize>())
                }
            };
            let mut resources_found = Vec::new();
            for element in resources_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<{ size_of::<usize>() }>()
                .0
            {
                let address = get_usize(element, usize::MIN);
                resources_found.push(if address == 0 { None } else { Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, address).ok_or(E_INVALIDARG)?) });
            }
            let resources: Vec<Option<&<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>> = resources_found.iter().map(|held| held.as_ref().map(|held| held.value())).collect();
            let priority =
                directx_graphics_infrastructure::enumerations::OfferResourcePriority(priority);
            let () = found.value().offer_resources(&resources, priority)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice2::ReclaimResources` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice2::ReclaimResources, which the C side calls"
)]
pub unsafe extern "system" fn device2_reclaim_resources<
    V: directx_graphics_infrastructure::Device2,
>(
    this: Raw,
    num_resources: u32,
    resources: *const Raw,
    discarded: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let discarded_length: Option<usize> = usize::try_from(num_resources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 4);
        let answer = guarded(Err(E_FAIL), move || {
            let discarded_length = discarded_length.ok_or(E_INVALIDARG)?;
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let resources_count = usize::try_from(num_resources)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let resources_bytes = if resources_count == 0 {
                Some(Vec::new())
            } else if resources.is_null() {
                None
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGIDevice2::ReclaimResources: ppResources, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumResources` as the count of `ppResources`: that many pointers, readable for the call.
                unsafe {
                    read_only_at(resources.cast::<u8>(), resources_count * size_of::<usize>())
                }
            };
            let mut resources_found = Vec::new();
            for element in resources_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<{ size_of::<usize>() }>()
                .0
            {
                let address = get_usize(element, usize::MIN);
                resources_found.push(if address == 0 { None } else { Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, address).ok_or(E_INVALIDARG)?) });
            }
            let resources: Vec<Option<&<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>> = resources_found.iter().map(|held| held.as_ref().map(|held| held.value())).collect();
            let discarded_value = found.value().reclaim_resources(&resources)?;
            drop(found);
            let mut discarded_bytes = vec![u8::MIN; discarded_length];
            for (value, bytes) in discarded_value
                .iter()
                .zip(discarded_bytes.as_chunks_mut::<4>().0.iter_mut())
            {
                put_i32(bytes, usize::MIN, i32::from(*value));
            }
            Ok((discarded_bytes,))
        });
        if let Some(length) = discarded_length {
            let discarded_written = answer
                .as_ref()
                .ok()
                .map_or_else(|| vec![u8::MIN; length], |written| written.0.clone());
            #[expect(
                unsafe_code,
                reason = "IDXGIDevice2::ReclaimResources: pDiscarded, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `NumResources` as the count of `pDiscarded`, which is `Out`: that many elements of 4 bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(discarded.cast::<u8>(), &discarded_written) };
        }
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice2::EnqueueSetEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice2::EnqueueSetEvent, which the C side calls"
)]
pub unsafe extern "system" fn device2_enqueue_set_event<
    V: directx_graphics_infrastructure::Device2,
>(
    this: Raw,
    h_event: usize,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_event = component_object_model::structures::Handle(h_event);
            let () = found.value().enqueue_set_event(h_event)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice3::Trim` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGIDevice3::Trim, which the C side calls")]
pub unsafe extern "system" fn device3_trim<V: directx_graphics_infrastructure::Device3>(this: Raw) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let () = found.value().trim();
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIDevice4::OfferResources1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice4::OfferResources1, which the C side calls"
)]
pub unsafe extern "system" fn device4_offer_resources1<
    V: directx_graphics_infrastructure::Device4,
>(
    this: Raw,
    num_resources: u32,
    resources: *const Raw,
    priority: i32,
    flags: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let resources_count = usize::try_from(num_resources)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let resources_bytes = if resources_count == 0 {
                Some(Vec::new())
            } else if resources.is_null() {
                None
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGIDevice4::OfferResources1: ppResources, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumResources` as the count of `ppResources`: that many pointers, readable for the call.
                unsafe {
                    read_only_at(resources.cast::<u8>(), resources_count * size_of::<usize>())
                }
            };
            let mut resources_found = Vec::new();
            for element in resources_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<{ size_of::<usize>() }>()
                .0
            {
                let address = get_usize(element, usize::MIN);
                resources_found.push(if address == 0 { None } else { Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, address).ok_or(E_INVALIDARG)?) });
            }
            let resources: Vec<Option<&<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>> = resources_found.iter().map(|held| held.as_ref().map(|held| held.value())).collect();
            let priority =
                directx_graphics_infrastructure::enumerations::OfferResourcePriority(priority);
            let flags = flags;
            let () = found
                .value()
                .offer_resources1(&resources, priority, flags)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDevice4::ReclaimResources1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDevice4::ReclaimResources1, which the C side calls"
)]
pub unsafe extern "system" fn device4_reclaim_resources1<
    V: directx_graphics_infrastructure::Device4,
>(
    this: Raw,
    num_resources: u32,
    resources: *const Raw,
    results: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let results_length: Option<usize> = usize::try_from(num_resources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 4);
        let answer = guarded(Err(E_FAIL), move || {
            let results_length = results_length.ok_or(E_INVALIDARG)?;
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let resources_count = usize::try_from(num_resources)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let resources_bytes = if resources_count == 0 {
                Some(Vec::new())
            } else if resources.is_null() {
                None
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGIDevice4::ReclaimResources1: ppResources, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumResources` as the count of `ppResources`: that many pointers, readable for the call.
                unsafe {
                    read_only_at(resources.cast::<u8>(), resources_count * size_of::<usize>())
                }
            };
            let mut resources_found = Vec::new();
            for element in resources_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<{ size_of::<usize>() }>()
                .0
            {
                let address = get_usize(element, usize::MIN);
                resources_found.push(if address == 0 { None } else { Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, address).ok_or(E_INVALIDARG)?) });
            }
            let resources: Vec<Option<&<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>> = resources_found.iter().map(|held| held.as_ref().map(|held| held.value())).collect();
            let results_value = found.value().reclaim_resources1(&resources)?;
            drop(found);
            let mut results_bytes = vec![u8::MIN; results_length];
            for (value, bytes) in results_value
                .iter()
                .zip(results_bytes.as_chunks_mut::<4>().0.iter_mut())
            {
                put_i32(bytes, usize::MIN, value.0);
            }
            Ok((results_bytes,))
        });
        if let Some(length) = results_length {
            let results_written = answer
                .as_ref()
                .ok()
                .map_or_else(|| vec![u8::MIN; length], |written| written.0.clone());
            #[expect(
                unsafe_code,
                reason = "IDXGIDevice4::ReclaimResources1: pResults, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `NumResources` as the count of `pResults`, which is `Out`: that many elements of 4 bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(results.cast::<u8>(), &results_written) };
        }
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDeviceSubObject::GetDevice` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDeviceSubObject::GetDevice, which the C side calls"
)]
pub unsafe extern "system" fn device_sub_object_get_device<
    V: directx_graphics_infrastructure::DeviceSubObject,
>(
    this: Raw,
    riid: *const u8,
    device: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if device.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIDeviceSubObject::GetDevice: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let device_value = found.value().get_device(&riid)?;
            drop(found);
            let device_answer = match device_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if device.is_null() {
                0
            } else {
                device_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let device_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDeviceSubObject::GetDevice: ppDevice, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(device.cast::<u8>(), &device_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIDisplayControl::IsStereoEnabled` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDisplayControl::IsStereoEnabled, which the C side calls"
)]
pub(crate) unsafe extern "system" fn display_control_is_stereo_enabled<
    V: directx_graphics_infrastructure::DisplayControl,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().is_stereo_enabled();
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIDisplayControl::SetStereoEnabled` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIDisplayControl::SetStereoEnabled, which the C side calls"
)]
pub(crate) unsafe extern "system" fn display_control_set_stereo_enabled<
    V: directx_graphics_infrastructure::DisplayControl,
>(
    this: Raw,
    enabled: i32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let enabled = enabled != 0;
            let () = found.value().set_stereo_enabled(enabled);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIFactory::EnumAdapters` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory::EnumAdapters, which the C side calls"
)]
pub unsafe extern "system" fn factory_enum_adapters<V: directx_graphics_infrastructure::Factory>(
    this: Raw,
    adapter: u32,
    adapter_: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if adapter_.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let adapter = adapter;
            let adapter__value = found.value().enum_adapters(adapter)?;
            let this_weak = found.weak();
            drop(found);
            let adapter__record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let adapter__served = serve(KEY, adapter__value, adapter__record, adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if adapter_.is_null() {
                0
            } else {
                adapter__served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let adapter__written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::EnumAdapters: ppAdapter, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppAdapter` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(adapter_.cast::<u8>(), &adapter__written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory::MakeWindowAssociation` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory::MakeWindowAssociation, which the C side calls"
)]
pub unsafe extern "system" fn factory_make_window_association<
    V: directx_graphics_infrastructure::Factory,
>(
    this: Raw,
    window_handle: usize,
    flags: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let window_handle = component_object_model::structures::Hwnd(window_handle);
            let flags = directx_graphics_infrastructure::enumerations::MwaFlags(flags);
            let () = found
                .value()
                .make_window_association(window_handle, flags)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory::GetWindowAssociation` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory::GetWindowAssociation, which the C side calls"
)]
pub unsafe extern "system" fn factory_get_window_association<
    V: directx_graphics_infrastructure::Factory,
>(
    this: Raw,
    window_handle: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if window_handle.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let window_handle_value = found.value().get_window_association()?;
            drop(found);
            let mut window_handle_bytes = [u8::MIN; 8];
            put_usize(&mut window_handle_bytes, usize::MIN, window_handle_value.0);
            Ok((window_handle_bytes,))
        });
        let window_handle_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 8], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::GetWindowAssociation: pWindowHandle, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pWindowHandle` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(window_handle.cast::<u8>(), &window_handle_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory::CreateSwapChain` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory::CreateSwapChain, which the C side calls"
)]
pub unsafe extern "system" fn factory_create_swap_chain<
    V: directx_graphics_infrastructure::Factory,
>(
    this: Raw,
    device: Raw,
    desc: *const u8,
    swap_chain: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if swap_chain.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory::CreateSwapChain: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory::CreateSwapChain: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `SwapChainDesc` of 72 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(&desc_bytes));
            let desc = desc.ok_or(E_INVALIDARG)?;
            let swap_chain_value = found.value().create_swap_chain(device_found.get(), &desc)?;
            let this_weak = found.weak();
            drop(found);
            let swap_chain_record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let swap_chain_served = serve(KEY, swap_chain_value, swap_chain_record, swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if swap_chain.is_null() {
                0
            } else {
                swap_chain_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let swap_chain_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::CreateSwapChain: ppSwapChain, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(swap_chain.cast::<u8>(), &swap_chain_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory::CreateSoftwareAdapter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory::CreateSoftwareAdapter, which the C side calls"
)]
pub unsafe extern "system" fn factory_create_software_adapter<
    V: directx_graphics_infrastructure::Factory,
>(
    this: Raw,
    module: usize,
    adapter: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if adapter.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let module = component_object_model::structures::Hmodule(module);
            let adapter_value = found.value().create_software_adapter(module)?;
            let this_weak = found.weak();
            drop(found);
            let adapter_record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let adapter_served = serve(KEY, adapter_value, adapter_record, adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if adapter.is_null() {
                0
            } else {
                adapter_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let adapter_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::CreateSoftwareAdapter: ppAdapter, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppAdapter` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(adapter.cast::<u8>(), &adapter_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory1::EnumAdapters1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory1::EnumAdapters1, which the C side calls"
)]
pub unsafe extern "system" fn factory1_enum_adapters1<
    V: directx_graphics_infrastructure::Factory1,
>(
    this: Raw,
    adapter: u32,
    adapter_: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if adapter_.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let adapter = adapter;
            let adapter__value = found.value().enum_adapters1(adapter)?;
            let this_weak = found.weak();
            drop(found);
            let adapter__record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let adapter__served = serve(KEY, adapter__value, adapter__record, adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if adapter_.is_null() {
                0
            } else {
                adapter__served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let adapter__written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory1::EnumAdapters1: ppAdapter, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppAdapter` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(adapter_.cast::<u8>(), &adapter__written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory1::IsCurrent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory1::IsCurrent, which the C side calls"
)]
pub unsafe extern "system" fn factory1_is_current<V: directx_graphics_infrastructure::Factory1>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().is_current();
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIFactory2::IsWindowedStereoEnabled` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::IsWindowedStereoEnabled, which the C side calls"
)]
pub unsafe extern "system" fn factory2_is_windowed_stereo_enabled<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().is_windowed_stereo_enabled();
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIFactory2::CreateSwapChainForHwnd` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::CreateSwapChainForHwnd, which the C side calls"
)]
pub unsafe extern "system" fn factory2_create_swap_chain_for_hwnd<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    device: Raw,
    h_wnd: usize,
    desc: *const u8,
    fullscreen_desc: *const u8,
    restrict_to_output: Raw,
    swap_chain: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if swap_chain.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForHwnd: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let h_wnd = component_object_model::structures::Hwnd(h_wnd);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForHwnd: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `SwapChainDesc1` of 48 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&desc_bytes));
            let desc = desc.ok_or(E_INVALIDARG)?;
            let mut fullscreen_desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout>()];
            let fullscreen_desc_null = fullscreen_desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForHwnd: pFullscreenDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pFullscreenDesc` is `In` and `Const`: one `SwapChainFullscreenDesc` of 20 bytes, readable for the call.
            let fullscreen_desc_read =
                unsafe { read_bytes(fullscreen_desc, &mut fullscreen_desc_bytes) };
            let fullscreen_desc = fullscreen_desc_read.then(|| {
                decode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(
                    &fullscreen_desc_bytes,
                )
            });
            if !fullscreen_desc_read && !fullscreen_desc_null {
                return Err(E_INVALIDARG);
            }
            let restrict_to_output_found = if restrict_to_output.is_null() {
                None
            } else {
                Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, restrict_to_output.addr()).ok_or(E_INVALIDARG)?)
            };
            let swap_chain_value = found.value().create_swap_chain_for_hwnd(
                device_found.get(),
                h_wnd,
                &desc,
                fullscreen_desc.as_ref(),
                restrict_to_output_found.as_ref().map(|held| held.value()),
            )?;
            let this_weak = found.weak();
            drop(found);
            let swap_chain_record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let swap_chain_served = serve(KEY, swap_chain_value, swap_chain_record, swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if swap_chain.is_null() {
                0
            } else {
                swap_chain_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let swap_chain_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForHwnd: ppSwapChain, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(swap_chain.cast::<u8>(), &swap_chain_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::CreateSwapChainForCoreWindow` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::CreateSwapChainForCoreWindow, which the C side calls"
)]
pub unsafe extern "system" fn factory2_create_swap_chain_for_core_window<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    device: Raw,
    window: Raw,
    desc: *const u8,
    restrict_to_output: Raw,
    swap_chain: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if swap_chain.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForCoreWindow: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForCoreWindow: pWindow, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pWindow` is an `IUnknown*`: null or a live COM object, for the call.
            let window_found = unsafe { UnknownFound::<V::Api>::find(window) };
            let window_found = window_found.ok_or(E_INVALIDARG)?;
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForCoreWindow: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `SwapChainDesc1` of 48 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&desc_bytes));
            let desc = desc.ok_or(E_INVALIDARG)?;
            let restrict_to_output_found = if restrict_to_output.is_null() {
                None
            } else {
                Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, restrict_to_output.addr()).ok_or(E_INVALIDARG)?)
            };
            let swap_chain_value = found.value().create_swap_chain_for_core_window(
                device_found.get(),
                window_found.get(),
                &desc,
                restrict_to_output_found.as_ref().map(|held| held.value()),
            )?;
            let this_weak = found.weak();
            drop(found);
            let swap_chain_record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let swap_chain_served = serve(KEY, swap_chain_value, swap_chain_record, swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if swap_chain.is_null() {
                0
            } else {
                swap_chain_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let swap_chain_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForCoreWindow: ppSwapChain, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(swap_chain.cast::<u8>(), &swap_chain_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::GetSharedResourceAdapterLuid` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::GetSharedResourceAdapterLuid, which the C side calls"
)]
pub unsafe extern "system" fn factory2_get_shared_resource_adapter_luid<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    h_resource: usize,
    luid: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if luid.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_resource = component_object_model::structures::Handle(h_resource);
            let luid_value = found.value().get_shared_resource_adapter_luid(h_resource)?;
            drop(found);
            let mut luid_bytes = [u8::MIN; size_of::<WindowsWin32FoundationLuidLayout>()];
            encode_windows_win32_foundation_luid(&luid_value, &mut luid_bytes);
            Ok((luid_bytes,))
        });
        let luid_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32FoundationLuidLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::GetSharedResourceAdapterLuid: pLuid, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pLuid` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(luid.cast::<u8>(), &luid_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::RegisterStereoStatusWindow` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::RegisterStereoStatusWindow, which the C side calls"
)]
pub unsafe extern "system" fn factory2_register_stereo_status_window<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    window_handle: usize,
    w_msg: u32,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let window_handle = component_object_model::structures::Hwnd(window_handle);
            let w_msg = w_msg;
            let pdw_cookie_value = found
                .value()
                .register_stereo_status_window(window_handle, w_msg)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterStereoStatusWindow: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::RegisterStereoStatusEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::RegisterStereoStatusEvent, which the C side calls"
)]
pub unsafe extern "system" fn factory2_register_stereo_status_event<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    h_event: usize,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_event = component_object_model::structures::Handle(h_event);
            let pdw_cookie_value = found.value().register_stereo_status_event(h_event)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterStereoStatusEvent: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::UnregisterStereoStatus` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::UnregisterStereoStatus, which the C side calls"
)]
pub unsafe extern "system" fn factory2_unregister_stereo_status<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    dw_cookie: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let dw_cookie = dw_cookie;
            let () = found.value().unregister_stereo_status(dw_cookie);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIFactory2::RegisterOcclusionStatusWindow` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::RegisterOcclusionStatusWindow, which the C side calls"
)]
pub unsafe extern "system" fn factory2_register_occlusion_status_window<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    window_handle: usize,
    w_msg: u32,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let window_handle = component_object_model::structures::Hwnd(window_handle);
            let w_msg = w_msg;
            let pdw_cookie_value = found
                .value()
                .register_occlusion_status_window(window_handle, w_msg)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterOcclusionStatusWindow: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::RegisterOcclusionStatusEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::RegisterOcclusionStatusEvent, which the C side calls"
)]
pub unsafe extern "system" fn factory2_register_occlusion_status_event<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    h_event: usize,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_event = component_object_model::structures::Handle(h_event);
            let pdw_cookie_value = found.value().register_occlusion_status_event(h_event)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterOcclusionStatusEvent: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory2::UnregisterOcclusionStatus` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::UnregisterOcclusionStatus, which the C side calls"
)]
pub unsafe extern "system" fn factory2_unregister_occlusion_status<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    dw_cookie: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let dw_cookie = dw_cookie;
            let () = found.value().unregister_occlusion_status(dw_cookie);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIFactory2::CreateSwapChainForComposition` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory2::CreateSwapChainForComposition, which the C side calls"
)]
pub unsafe extern "system" fn factory2_create_swap_chain_for_composition<
    V: directx_graphics_infrastructure::Factory2,
>(
    this: Raw,
    device: Raw,
    desc: *const u8,
    restrict_to_output: Raw,
    swap_chain: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if swap_chain.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForComposition: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory2::CreateSwapChainForComposition: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `SwapChainDesc1` of 48 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&desc_bytes));
            let desc = desc.ok_or(E_INVALIDARG)?;
            let restrict_to_output_found = if restrict_to_output.is_null() {
                None
            } else {
                Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, restrict_to_output.addr()).ok_or(E_INVALIDARG)?)
            };
            let swap_chain_value = found.value().create_swap_chain_for_composition(
                device_found.get(),
                &desc,
                restrict_to_output_found.as_ref().map(|held| held.value()),
            )?;
            let this_weak = found.weak();
            drop(found);
            let swap_chain_record =
                this_weak.map_or_else(record, |weak| record_related("Factory", weak));
            let swap_chain_served = serve(KEY, swap_chain_value, swap_chain_record, swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if swap_chain.is_null() {
                0
            } else {
                swap_chain_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let swap_chain_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForComposition: ppSwapChain, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(swap_chain.cast::<u8>(), &swap_chain_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory3::GetCreationFlags` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory3::GetCreationFlags, which the C side calls"
)]
pub unsafe extern "system" fn factory3_get_creation_flags<
    V: directx_graphics_infrastructure::Factory3,
>(
    this: Raw,
) -> u32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_creation_flags();
            drop(found);
            Some((returned.0,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIFactory4::EnumWarpAdapter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory4::EnumWarpAdapter, which the C side calls"
)]
pub unsafe extern "system" fn factory4_enum_warp_adapter<
    V: directx_graphics_infrastructure::Factory4,
>(
    this: Raw,
    riid: *const u8,
    ppv_adapter: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if ppv_adapter.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory4::EnumWarpAdapter: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let ppv_adapter_value = found.value().enum_warp_adapter(&riid)?;
            drop(found);
            let ppv_adapter_answer = match ppv_adapter_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if ppv_adapter.is_null() {
                0
            } else {
                ppv_adapter_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let ppv_adapter_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory4::EnumWarpAdapter: ppvAdapter, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppvAdapter` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(ppv_adapter.cast::<u8>(), &ppv_adapter_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory4::EnumAdapterByLuid` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory4::EnumAdapterByLuid, which the C side calls"
)]
pub(crate) unsafe extern "system" fn factory4_enum_adapter_by_luid_refused(
    _this: Raw,
    _p0: u64,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    #[expect(
        unsafe_code,
        reason = "IDXGIFactory4::EnumAdapterByLuid: ppvAdapter, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvAdapter` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIFactory5::CheckFeatureSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory5::CheckFeatureSupport, which the C side calls"
)]
pub unsafe extern "system" fn factory5_check_feature_support<
    V: directx_graphics_infrastructure::Factory5,
>(
    this: Raw,
    feature: i32,
    feature_support_data: *mut u8,
    feature_support_data_size: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let feature = directx_graphics_infrastructure::enumerations::Feature(feature);
            let feature_support_data_length = usize::try_from(feature_support_data_size)
                .ok()
                .ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory5::CheckFeatureSupport: pFeatureSupportData, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `MemorySize` names `FeatureSupportDataSize` as the byte count of `pFeatureSupportData`, which is `Out`: that many bytes, writable for the call.
            let feature_support_data =
                unsafe { writable_at(feature_support_data, feature_support_data_length) };
            let feature_support_data = feature_support_data.ok_or(E_INVALIDARG)?;
            let () = found
                .value()
                .check_feature_support(feature, &feature_support_data)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory6::EnumAdapterByGpuPreference` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory6::EnumAdapterByGpuPreference, which the C side calls"
)]
pub unsafe extern "system" fn factory6_enum_adapter_by_gpu_preference<
    V: directx_graphics_infrastructure::Factory6,
>(
    this: Raw,
    adapter: u32,
    gpu_preference: i32,
    riid: *const u8,
    ppv_adapter: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if ppv_adapter.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let adapter = adapter;
            let gpu_preference =
                directx_graphics_infrastructure::enumerations::GpuPreference(gpu_preference);
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactory6::EnumAdapterByGpuPreference: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let ppv_adapter_value =
                found
                    .value()
                    .enum_adapter_by_gpu_preference(adapter, gpu_preference, &riid)?;
            drop(found);
            let ppv_adapter_answer = match ppv_adapter_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if ppv_adapter.is_null() {
                0
            } else {
                ppv_adapter_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let ppv_adapter_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory6::EnumAdapterByGpuPreference: ppvAdapter, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppvAdapter` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(ppv_adapter.cast::<u8>(), &ppv_adapter_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory7::RegisterAdaptersChangedEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory7::RegisterAdaptersChangedEvent, which the C side calls"
)]
pub(crate) unsafe extern "system" fn factory7_register_adapters_changed_event<
    V: directx_graphics_infrastructure::Factory7,
>(
    this: Raw,
    h_event: usize,
    pdw_cookie: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pdw_cookie.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let h_event = component_object_model::structures::Handle(h_event);
            let pdw_cookie_value = found.value().register_adapters_changed_event(h_event)?;
            drop(found);
            let mut pdw_cookie_bytes = [u8::MIN; 4];
            put_u32(&mut pdw_cookie_bytes, usize::MIN, pdw_cookie_value);
            Ok((pdw_cookie_bytes,))
        });
        let pdw_cookie_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory7::RegisterAdaptersChangedEvent: pdwCookie, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pdwCookie` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(pdw_cookie.cast::<u8>(), &pdw_cookie_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactory7::UnregisterAdaptersChangedEvent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactory7::UnregisterAdaptersChangedEvent, which the C side calls"
)]
pub(crate) unsafe extern "system" fn factory7_unregister_adapters_changed_event<
    V: directx_graphics_infrastructure::Factory7,
>(
    this: Raw,
    dw_cookie: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let dw_cookie = dw_cookie;
            let () = found.value().unregister_adapters_changed_event(dw_cookie)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle, which the C side calls"
)]
pub(crate) unsafe extern "system" fn factory_media_create_swap_chain_for_composition_surface_handle<
    V: directx_graphics_infrastructure::FactoryMedia,
>(
    this: Raw,
    device: Raw,
    h_surface: usize,
    desc: *const u8,
    restrict_to_output: Raw,
    swap_chain: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if swap_chain.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let h_surface = component_object_model::structures::Handle(h_surface);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `SwapChainDesc1` of 48 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&desc_bytes));
            let desc = desc.ok_or(E_INVALIDARG)?;
            let restrict_to_output_found = if restrict_to_output.is_null() {
                None
            } else {
                Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, restrict_to_output.addr()).ok_or(E_INVALIDARG)?)
            };
            let swap_chain_value = found
                .value()
                .create_swap_chain_for_composition_surface_handle(
                    device_found.get(),
                    h_surface,
                    &desc,
                    restrict_to_output_found.as_ref().map(|held| held.value()),
                )?;
            let this_weak = found.weak();
            drop(found);
            let swap_chain_record =
                this_weak.map_or_else(record, |weak| record_related("FactoryMedia", weak));
            let swap_chain_served = serve(KEY, swap_chain_value, swap_chain_record, swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if swap_chain.is_null() {
                0
            } else {
                swap_chain_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let swap_chain_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle: ppSwapChain, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(swap_chain.cast::<u8>(), &swap_chain_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle, which the C side calls"
)]
pub(crate) unsafe extern "system" fn factory_media_create_decode_swap_chain_for_composition_surface_handle<
    V: directx_graphics_infrastructure::FactoryMedia,
>(
    this: Raw,
    device: Raw,
    h_surface: usize,
    desc: *const u8,
    yuv_decode_buffers: Raw,
    restrict_to_output: Raw,
    swap_chain: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if swap_chain.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let h_surface = component_object_model::structures::Handle(h_surface);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiDecodeSwapChainDescLayout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `DecodeSwapChainDesc` of 4 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read.then(|| {
                decode_windows_win32_graphics_dxgi_dxgi_decode_swap_chain_desc(&desc_bytes)
            });
            let desc = desc.ok_or(E_INVALIDARG)?;
            let yuv_decode_buffers_found = object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, yuv_decode_buffers.addr()).ok_or(E_INVALIDARG)?;
            let restrict_to_output_found = if restrict_to_output.is_null() {
                None
            } else {
                Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, restrict_to_output.addr()).ok_or(E_INVALIDARG)?)
            };
            let swap_chain_value = found
                .value()
                .create_decode_swap_chain_for_composition_surface_handle(
                    device_found.get(),
                    h_surface,
                    &desc,
                    yuv_decode_buffers_found.value(),
                    restrict_to_output_found.as_ref().map(|held| held.value()),
                )?;
            let this_weak = found.weak();
            drop(found);
            let swap_chain_record =
                this_weak.map_or_else(record, |weak| record_related("FactoryMedia", weak));
            let swap_chain_served = serve(KEY, swap_chain_value, swap_chain_record, decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if swap_chain.is_null() {
                0
            } else {
                swap_chain_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let swap_chain_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle: ppSwapChain, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(swap_chain.cast::<u8>(), &swap_chain_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::SetMessageCountLimit` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::SetMessageCountLimit, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_set_message_count_limit<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    message_count_limit: u64,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let message_count_limit = message_count_limit;
            let () = found
                .value()
                .set_message_count_limit(producer, message_count_limit)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::ClearStoredMessages` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::ClearStoredMessages, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_clear_stored_messages<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let () = found.value().clear_stored_messages(producer);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_num_stored_messages_allowed_by_retrieval_filters<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found
                .value()
                .get_num_stored_messages_allowed_by_retrieval_filters(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetNumStoredMessages` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetNumStoredMessages, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_num_stored_messages<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found.value().get_num_stored_messages(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_num_messages_discarded_by_message_count_limit<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found
                .value()
                .get_num_messages_discarded_by_message_count_limit(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetMessageCountLimit` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetMessageCountLimit, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_message_count_limit<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found.value().get_message_count_limit(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_num_messages_allowed_by_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found
                .value()
                .get_num_messages_allowed_by_storage_filter(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_num_messages_denied_by_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found
                .value()
                .get_num_messages_denied_by_storage_filter(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::ClearStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::ClearStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_clear_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let () = found.value().clear_storage_filter(producer);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIInfoQueue::PushEmptyStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PushEmptyStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_push_empty_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let () = found.value().push_empty_storage_filter(producer)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::PushDenyAllStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PushDenyAllStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_push_deny_all_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let () = found.value().push_deny_all_storage_filter(producer)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::PushCopyOfStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PushCopyOfStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_push_copy_of_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let () = found.value().push_copy_of_storage_filter(producer)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::PopStorageFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PopStorageFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_pop_storage_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let () = found.value().pop_storage_filter(producer);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIInfoQueue::GetStorageFilterStackSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetStorageFilterStackSize, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_storage_filter_stack_size<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found.value().get_storage_filter_stack_size(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::ClearRetrievalFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::ClearRetrievalFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_clear_retrieval_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let () = found.value().clear_retrieval_filter(producer);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIInfoQueue::PushEmptyRetrievalFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PushEmptyRetrievalFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_push_empty_retrieval_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let () = found.value().push_empty_retrieval_filter(producer)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::PushDenyAllRetrievalFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PushDenyAllRetrievalFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_push_deny_all_retrieval_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let () = found.value().push_deny_all_retrieval_filter(producer)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::PushCopyOfRetrievalFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PushCopyOfRetrievalFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_push_copy_of_retrieval_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let () = found.value().push_copy_of_retrieval_filter(producer)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::PopRetrievalFilter` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::PopRetrievalFilter, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_pop_retrieval_filter<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let () = found.value().pop_retrieval_filter(producer);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIInfoQueue::GetRetrievalFilterStackSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetRetrievalFilterStackSize, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_retrieval_filter_stack_size<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> u32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found.value().get_retrieval_filter_stack_size(producer);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::AddMessage` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::AddMessage, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_add_message<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    category: i32,
    severity: i32,
    id: i32,
    description: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let category =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory(category);
            let severity =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(severity);
            let id = id;
            let description_null = description.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIInfoQueue::AddMessage: pDescription, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDescription` is a `PSTR` and `In`: units of a `u8` ending at the first zero unit, readable for the call, or null.
            let description_text =
                unsafe { read_text(description, size_of::<u8>(), MOST_TEXT_UNITS) };
            let text = description_text.ok_or(E_INVALIDARG)?;
            let description = std::ffi::CString::new(text).unwrap_or_default();
            let () = found
                .value()
                .add_message(producer, category, severity, id, &description)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::AddApplicationMessage` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::AddApplicationMessage, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_add_application_message<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    severity: i32,
    description: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let severity =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(severity);
            let description_null = description.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIInfoQueue::AddApplicationMessage: pDescription, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDescription` is a `PSTR` and `In`: units of a `u8` ending at the first zero unit, readable for the call, or null.
            let description_text =
                unsafe { read_text(description, size_of::<u8>(), MOST_TEXT_UNITS) };
            let text = description_text.ok_or(E_INVALIDARG)?;
            let description = std::ffi::CString::new(text).unwrap_or_default();
            let () = found
                .value()
                .add_application_message(severity, &description)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::SetBreakOnCategory` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::SetBreakOnCategory, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_set_break_on_category<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    category: i32,
    b_enable: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let category =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory(category);
            let b_enable = b_enable != 0;
            let () = found
                .value()
                .set_break_on_category(producer, category, b_enable)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::SetBreakOnSeverity` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::SetBreakOnSeverity, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_set_break_on_severity<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    severity: i32,
    b_enable: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let severity =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(severity);
            let b_enable = b_enable != 0;
            let () = found
                .value()
                .set_break_on_severity(producer, severity, b_enable)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::SetBreakOnID` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::SetBreakOnID, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_set_break_on_id<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    id: i32,
    b_enable: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let producer = producer.guid();
            let id = id;
            let b_enable = b_enable != 0;
            let () = found.value().set_break_on_id(producer, id, b_enable)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIInfoQueue::GetBreakOnCategory` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetBreakOnCategory, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_break_on_category<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    category: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let category =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory(category);
            let returned = found.value().get_break_on_category(producer, category);
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetBreakOnSeverity` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetBreakOnSeverity, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_break_on_severity<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    severity: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let severity =
                directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity(severity);
            let returned = found.value().get_break_on_severity(producer, severity);
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetBreakOnID` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetBreakOnID, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_break_on_id<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    id: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let id = id;
            let returned = found.value().get_break_on_id(producer, id);
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::SetMuteDebugOutput` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::SetMuteDebugOutput, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_set_mute_debug_output<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
    b_mute: i32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let b_mute = b_mute != 0;
            let () = found.value().set_mute_debug_output(producer, b_mute);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIInfoQueue::GetMuteDebugOutput` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIInfoQueue::GetMuteDebugOutput, which the C side calls"
)]
pub(crate) unsafe extern "system" fn info_queue_get_mute_debug_output<
    V: directx_graphics_infrastructure::InfoQueue,
>(
    this: Raw,
    producer: component_object_model_direct::GuidAbi,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let producer = producer.guid();
            let returned = found.value().get_mute_debug_output(producer);
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIInfoQueue::GetMessage` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_get_message_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: u64,
    _p2: *mut u8,
    _p3: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIInfoQueue::AddStorageFilterEntries` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_add_storage_filter_entries_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIInfoQueue::GetStorageFilter` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_get_storage_filter_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: *mut u8,
    _p2: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIInfoQueue::PushStorageFilter` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_push_storage_filter_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIInfoQueue::AddRetrievalFilterEntries` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_add_retrieval_filter_entries_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIInfoQueue::GetRetrievalFilter` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_get_retrieval_filter_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: *mut u8,
    _p2: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIInfoQueue::PushRetrievalFilter` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn info_queue_push_retrieval_filter_refused(
    _this: Raw,
    _p0: component_object_model_direct::GuidAbi,
    _p1: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIKeyedMutex::AcquireSync` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIKeyedMutex::AcquireSync, which the C side calls"
)]
pub(crate) unsafe extern "system" fn keyed_mutex_acquire_sync<
    V: directx_graphics_infrastructure::KeyedMutex,
>(
    this: Raw,
    key: u64,
    dw_milliseconds: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let key = key;
            let dw_milliseconds = dw_milliseconds;
            let () = found.value().acquire_sync(key, dw_milliseconds)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIKeyedMutex::ReleaseSync` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIKeyedMutex::ReleaseSync, which the C side calls"
)]
pub(crate) unsafe extern "system" fn keyed_mutex_release_sync<
    V: directx_graphics_infrastructure::KeyedMutex,
>(
    this: Raw,
    key: u64,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let key = key;
            let () = found.value().release_sync(key)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIObject::SetPrivateData` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIObject::SetPrivateData, which the C side calls"
)]
pub unsafe extern "system" fn object_set_private_data<
    V: directx_graphics_infrastructure::Object,
>(
    this: Raw,
    name: *const u8,
    data_size: u32,
    data: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut name_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let name_null = name.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIObject::SetPrivateData: Name, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `Name` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let name_read = unsafe { read_bytes(name, &mut name_bytes) };
            let name = name_read.then(|| decode_guid(&name_bytes));
            let name = name.ok_or(E_INVALIDARG)?;
            let data_count = usize::try_from(data_size)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let data_bytes = if data_count == 0 {
                Some(Vec::new())
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGIObject::SetPrivateData: pData, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `DataSize` as the count of `pData`: that many elements of 1 bytes, readable for the call.
                unsafe {
                    read_only_at(data, data_count)
                }
            };
            let data: Vec<_> = data_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<1>()
                .0
                .iter()
                .map(|element| get_u8(element, usize::MIN))
                .collect();
            let () = found.value().set_private_data(&name, &data)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIObject::SetPrivateDataInterface` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIObject::SetPrivateDataInterface, which the C side calls"
)]
pub unsafe extern "system" fn object_set_private_data_interface<
    V: directx_graphics_infrastructure::Object,
>(
    this: Raw,
    name: *const u8,
    unknown: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut name_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let name_null = name.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIObject::SetPrivateDataInterface: Name, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `Name` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let name_read = unsafe { read_bytes(name, &mut name_bytes) };
            let name = name_read.then(|| decode_guid(&name_bytes));
            let name = name.ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIObject::SetPrivateDataInterface: pUnknown, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pUnknown` is an `IUnknown*`: null or a live COM object, for the call.
            let unknown_found = unsafe { UnknownFound::<V::Api>::find(unknown) };
            if unknown_found.is_none() && !unknown.is_null() {
                return Err(E_INVALIDARG);
            }
            let () = found
                .value()
                .set_private_data_interface(&name, unknown_found.as_ref().map(|held| held.get()))?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIObject::GetParent` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGIObject::GetParent, which the C side calls")]
pub unsafe extern "system" fn object_get_parent<V: directx_graphics_infrastructure::Object>(
    this: Raw,
    riid: *const u8,
    parent: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if parent.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIObject::GetParent: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let parent_value = found.value().get_parent(&riid)?;
            drop(found);
            let parent_answer = match parent_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if parent.is_null() {
                0
            } else {
                parent_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let parent_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(parent.cast::<u8>(), &parent_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIObject::GetPrivateData` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn object_get_private_data_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: *mut u8,
    _p2: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIOutput::GetDesc` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGIOutput::GetDesc, which the C side calls")]
pub unsafe extern "system" fn output_get_desc<V: directx_graphics_infrastructure::Output>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutputDescLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_output_desc(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutputDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetDesc: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::GetDisplayModeList` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::GetDisplayModeList, which the C side calls"
)]
pub unsafe extern "system" fn output_get_display_mode_list<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    enum_format: i32,
    flags: u32,
    num_modes: *mut u8,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let mut num_modes_room = [u8::MIN; 4];
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetDisplayModeList: pNumModes, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pNumModes` is a UINT in and out: four bytes, readable for the call, or null.
        let num_modes_read = unsafe { read_bytes(num_modes, &mut num_modes_room) };
        let desc_length: Option<usize> = if !num_modes_read {
            None
        } else if desc.is_null() {
            Some(0)
        } else {
            usize::try_from(get_u32(&num_modes_room, 0))
                .ok()
                .filter(|room| *room <= 1 << 16)
                .map(|room| room * 28)
        };
        let answer = guarded(Err(E_FAIL), move || {
            let desc_length = desc_length.ok_or(E_INVALIDARG)?;
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let enum_format = directx_graphics_infrastructure::enumerations::Format(enum_format);
            let flags = directx_graphics_infrastructure::enumerations::EnumModes(flags);
            let desc_value = found.value().get_display_mode_list(enum_format, flags)?;
            drop(found);
            let mut desc_bytes = vec![u8::MIN; desc_length];
            for (value, bytes) in desc_value
                .iter()
                .zip(desc_bytes.as_chunks_mut::<28>().0.iter_mut())
            {
                encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(value, bytes);
            }
            let desc_number = u32::try_from(if desc.is_null() {
                desc_value.len()
            } else {
                desc_value.len().min(desc_length / 28)
            })
            .unwrap_or(u32::MAX)
            .to_le_bytes();
            Ok((desc_bytes, desc_number))
        });
        if let Some(length) = desc_length {
            let desc_written = answer
                .as_ref()
                .ok()
                .map_or_else(|| vec![u8::MIN; length], |written| written.0.clone());
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput::GetDisplayModeList: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `pNumModes` as the count of `pDesc`, which is `Out`: that many elements of 28 bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        }
        let num_modes_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<u32>()], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetDisplayModeList: pNumModes, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pNumModes` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(num_modes.cast::<u8>(), &num_modes_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::FindClosestMatchingMode` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::FindClosestMatchingMode, which the C side calls"
)]
pub unsafe extern "system" fn output_find_closest_matching_mode<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    mode_to_match: *const u8,
    closest_match: *mut u8,
    concerned_device: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if closest_match.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut mode_to_match_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()];
            let mode_to_match_null = mode_to_match.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput::FindClosestMatchingMode: pModeToMatch, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pModeToMatch` is `In` and `Const`: one `ModeDesc` of 28 bytes, readable for the call.
            let mode_to_match_read = unsafe { read_bytes(mode_to_match, &mut mode_to_match_bytes) };
            let mode_to_match = mode_to_match_read.then(|| {
                decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(&mode_to_match_bytes)
            });
            let mode_to_match = mode_to_match.ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput::FindClosestMatchingMode: pConcernedDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pConcernedDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let concerned_device_found = unsafe { UnknownFound::<V::Api>::find(concerned_device) };
            if concerned_device_found.is_none() && !concerned_device.is_null() {
                return Err(E_INVALIDARG);
            }
            let closest_match_value = found.value().find_closest_matching_mode(
                &mode_to_match,
                concerned_device_found.as_ref().map(|held| held.get()),
            )?;
            drop(found);
            let mut closest_match_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()];
            encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
                &closest_match_value,
                &mut closest_match_bytes,
            );
            Ok((closest_match_bytes,))
        });
        let closest_match_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::FindClosestMatchingMode: pClosestMatch, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pClosestMatch` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(closest_match.cast::<u8>(), &closest_match_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::WaitForVBlank` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::WaitForVBlank, which the C side calls"
)]
pub unsafe extern "system" fn output_wait_for_v_blank<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let () = found.value().wait_for_v_blank()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::TakeOwnership` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::TakeOwnership, which the C side calls"
)]
pub unsafe extern "system" fn output_take_ownership<V: directx_graphics_infrastructure::Output>(
    this: Raw,
    device: Raw,
    exclusive: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput::TakeOwnership: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let exclusive = exclusive != 0;
            let () = found
                .value()
                .take_ownership(device_found.get(), exclusive)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::ReleaseOwnership` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::ReleaseOwnership, which the C side calls"
)]
pub unsafe extern "system" fn output_release_ownership<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let () = found.value().release_ownership();
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIOutput::GetGammaControlCapabilities` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::GetGammaControlCapabilities, which the C side calls"
)]
pub unsafe extern "system" fn output_get_gamma_control_capabilities<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    gamma_caps: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if gamma_caps.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let gamma_caps_value = found.value().get_gamma_control_capabilities()?;
            drop(found);
            let mut gamma_caps_bytes = [u8::MIN;
                size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout>()];
            encode_windows_win32_graphics_dxgi_common_dxgi_gamma_control_capabilities(
                &gamma_caps_value,
                &mut gamma_caps_bytes,
            );
            Ok((gamma_caps_bytes,))
        });
        let gamma_caps_written = answer.as_ref().ok().map_or(
            [u8::MIN;
                size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetGammaControlCapabilities: pGammaCaps, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pGammaCaps` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(gamma_caps.cast::<u8>(), &gamma_caps_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::SetGammaControl` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::SetGammaControl, which the C side calls"
)]
pub unsafe extern "system" fn output_set_gamma_control<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    array: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut array_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout>()];
            let array_null = array.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput::SetGammaControl: pArray, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pArray` is `In` and `Const`: one `GammaControl` of 12324 bytes, readable for the call.
            let array_read = unsafe { read_bytes(array, &mut array_bytes) };
            let array = array_read.then(|| {
                decode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(&array_bytes)
            });
            let array = array.ok_or(E_INVALIDARG)?;
            let () = found.value().set_gamma_control(&array)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::GetGammaControl` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::GetGammaControl, which the C side calls"
)]
pub unsafe extern "system" fn output_get_gamma_control<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    array: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if array.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let array_value = found.value().get_gamma_control()?;
            drop(found);
            let mut array_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout>()];
            encode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(
                &array_value,
                &mut array_bytes,
            );
            Ok((array_bytes,))
        });
        let array_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetGammaControl: pArray, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pArray` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(array.cast::<u8>(), &array_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::SetDisplaySurface` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::SetDisplaySurface, which the C side calls"
)]
pub unsafe extern "system" fn output_set_display_surface<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    scanout_surface: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let scanout_surface_found = object::<
                <V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
            >(KEY, scanout_surface.addr())
            .ok_or(E_INVALIDARG)?;
            let () = found
                .value()
                .set_display_surface(scanout_surface_found.value())?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::GetDisplaySurfaceData` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::GetDisplaySurfaceData, which the C side calls"
)]
pub unsafe extern "system" fn output_get_display_surface_data<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    destination: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let destination_found = object::<
                <V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
            >(KEY, destination.addr())
            .ok_or(E_INVALIDARG)?;
            let () = found
                .value()
                .get_display_surface_data(destination_found.value())?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput::GetFrameStatistics` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput::GetFrameStatistics, which the C side calls"
)]
pub unsafe extern "system" fn output_get_frame_statistics<
    V: directx_graphics_infrastructure::Output,
>(
    this: Raw,
    stats: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if stats.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let stats_value = found.value().get_frame_statistics()?;
            drop(found);
            let mut stats_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_frame_statistics(
                &stats_value,
                &mut stats_bytes,
            );
            Ok((stats_bytes,))
        });
        let stats_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetFrameStatistics: pStats, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pStats` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(stats.cast::<u8>(), &stats_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput1::GetDisplayModeList1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput1::GetDisplayModeList1, which the C side calls"
)]
pub unsafe extern "system" fn output1_get_display_mode_list1<
    V: directx_graphics_infrastructure::Output1,
>(
    this: Raw,
    enum_format: i32,
    flags: u32,
    num_modes: *mut u8,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let mut num_modes_room = [u8::MIN; 4];
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::GetDisplayModeList1: pNumModes, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pNumModes` is a UINT in and out: four bytes, readable for the call, or null.
        let num_modes_read = unsafe { read_bytes(num_modes, &mut num_modes_room) };
        let desc_length: Option<usize> = if !num_modes_read {
            None
        } else if desc.is_null() {
            Some(0)
        } else {
            usize::try_from(get_u32(&num_modes_room, 0))
                .ok()
                .filter(|room| *room <= 1 << 16)
                .map(|room| room * 32)
        };
        let answer = guarded(Err(E_FAIL), move || {
            let desc_length = desc_length.ok_or(E_INVALIDARG)?;
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let enum_format = directx_graphics_infrastructure::enumerations::Format(enum_format);
            let flags = directx_graphics_infrastructure::enumerations::EnumModes(flags);
            let desc_value = found.value().get_display_mode_list1(enum_format, flags)?;
            drop(found);
            let mut desc_bytes = vec![u8::MIN; desc_length];
            for (value, bytes) in desc_value
                .iter()
                .zip(desc_bytes.as_chunks_mut::<32>().0.iter_mut())
            {
                encode_windows_win32_graphics_dxgi_dxgi_mode_desc1(value, bytes);
            }
            let desc_number = u32::try_from(if desc.is_null() {
                desc_value.len()
            } else {
                desc_value.len().min(desc_length / 32)
            })
            .unwrap_or(u32::MAX)
            .to_le_bytes();
            Ok((desc_bytes, desc_number))
        });
        if let Some(length) = desc_length {
            let desc_written = answer
                .as_ref()
                .ok()
                .map_or_else(|| vec![u8::MIN; length], |written| written.0.clone());
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput1::GetDisplayModeList1: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `pNumModes` as the count of `pDesc`, which is `Out`: that many elements of 32 bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        }
        let num_modes_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<u32>()], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::GetDisplayModeList1: pNumModes, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pNumModes` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(num_modes.cast::<u8>(), &num_modes_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput1::FindClosestMatchingMode1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput1::FindClosestMatchingMode1, which the C side calls"
)]
pub unsafe extern "system" fn output1_find_closest_matching_mode1<
    V: directx_graphics_infrastructure::Output1,
>(
    this: Raw,
    mode_to_match: *const u8,
    closest_match: *mut u8,
    concerned_device: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if closest_match.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut mode_to_match_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiModeDesc1Layout>()];
            let mode_to_match_null = mode_to_match.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput1::FindClosestMatchingMode1: pModeToMatch, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pModeToMatch` is `In` and `Const`: one `ModeDesc1` of 32 bytes, readable for the call.
            let mode_to_match_read = unsafe { read_bytes(mode_to_match, &mut mode_to_match_bytes) };
            let mode_to_match = mode_to_match_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_mode_desc1(&mode_to_match_bytes));
            let mode_to_match = mode_to_match.ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput1::FindClosestMatchingMode1: pConcernedDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pConcernedDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let concerned_device_found = unsafe { UnknownFound::<V::Api>::find(concerned_device) };
            if concerned_device_found.is_none() && !concerned_device.is_null() {
                return Err(E_INVALIDARG);
            }
            let closest_match_value = found.value().find_closest_matching_mode1(
                &mode_to_match,
                concerned_device_found.as_ref().map(|held| held.get()),
            )?;
            drop(found);
            let mut closest_match_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiModeDesc1Layout>()];
            encode_windows_win32_graphics_dxgi_dxgi_mode_desc1(
                &closest_match_value,
                &mut closest_match_bytes,
            );
            Ok((closest_match_bytes,))
        });
        let closest_match_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiModeDesc1Layout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::FindClosestMatchingMode1: pClosestMatch, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pClosestMatch` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(closest_match.cast::<u8>(), &closest_match_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput1::GetDisplaySurfaceData1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput1::GetDisplaySurfaceData1, which the C side calls"
)]
pub unsafe extern "system" fn output1_get_display_surface_data1<
    V: directx_graphics_infrastructure::Output1,
>(
    this: Raw,
    destination: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let destination_found = object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, destination.addr()).ok_or(E_INVALIDARG)?;
            let () = found
                .value()
                .get_display_surface_data1(destination_found.value())?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput1::DuplicateOutput` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput1::DuplicateOutput, which the C side calls"
)]
pub unsafe extern "system" fn output1_duplicate_output<
    V: directx_graphics_infrastructure::Output1,
>(
    this: Raw,
    device: Raw,
    output_duplication: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if output_duplication.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput1::DuplicateOutput: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let output_duplication_value = found.value().duplicate_output(device_found.get())?;
            let this_weak = found.weak();
            drop(found);
            let output_duplication_record =
                this_weak.map_or_else(record, |weak| record_related("Output", weak));
            let output_duplication_served = serve(KEY, output_duplication_value, output_duplication_record, output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if output_duplication.is_null() {
                0
            } else {
                output_duplication_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let output_duplication_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::DuplicateOutput: ppOutputDuplication, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppOutputDuplication` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ =
            unsafe { write_bytes(output_duplication.cast::<u8>(), &output_duplication_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput2::SupportsOverlays` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput2::SupportsOverlays, which the C side calls"
)]
pub unsafe extern "system" fn output2_supports_overlays<
    V: directx_graphics_infrastructure::Output2,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().supports_overlays();
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGIOutput3::CheckOverlaySupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput3::CheckOverlaySupport, which the C side calls"
)]
pub unsafe extern "system" fn output3_check_overlay_support<
    V: directx_graphics_infrastructure::Output3,
>(
    this: Raw,
    enum_format: i32,
    concerned_device: Raw,
    flags: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if flags.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let enum_format = directx_graphics_infrastructure::enumerations::Format(enum_format);
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput3::CheckOverlaySupport: pConcernedDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pConcernedDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let concerned_device_found = unsafe { UnknownFound::<V::Api>::find(concerned_device) };
            let concerned_device_found = concerned_device_found.ok_or(E_INVALIDARG)?;
            let flags_value = found
                .value()
                .check_overlay_support(enum_format, concerned_device_found.get())?;
            drop(found);
            let mut flags_bytes = [u8::MIN; 4];
            put_u32(&mut flags_bytes, usize::MIN, flags_value);
            Ok((flags_bytes,))
        });
        let flags_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput3::CheckOverlaySupport: pFlags, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pFlags` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(flags.cast::<u8>(), &flags_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput4::CheckOverlayColorSpaceSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput4::CheckOverlayColorSpaceSupport, which the C side calls"
)]
pub unsafe extern "system" fn output4_check_overlay_color_space_support<
    V: directx_graphics_infrastructure::Output4,
>(
    this: Raw,
    format: i32,
    color_space: i32,
    concerned_device: Raw,
    flags: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if flags.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let format = directx_graphics_infrastructure::enumerations::Format(format);
            let color_space =
                directx_graphics_infrastructure::enumerations::ColorSpaceType(color_space);
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput4::CheckOverlayColorSpaceSupport: pConcernedDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pConcernedDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let concerned_device_found = unsafe { UnknownFound::<V::Api>::find(concerned_device) };
            let concerned_device_found = concerned_device_found.ok_or(E_INVALIDARG)?;
            let flags_value = found.value().check_overlay_color_space_support(
                format,
                color_space,
                concerned_device_found.get(),
            )?;
            drop(found);
            let mut flags_bytes = [u8::MIN; 4];
            put_u32(&mut flags_bytes, usize::MIN, flags_value);
            Ok((flags_bytes,))
        });
        let flags_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput4::CheckOverlayColorSpaceSupport: pFlags, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pFlags` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(flags.cast::<u8>(), &flags_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput5::DuplicateOutput1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput5::DuplicateOutput1, which the C side calls"
)]
pub unsafe extern "system" fn output5_duplicate_output1<
    V: directx_graphics_infrastructure::Output5,
>(
    this: Raw,
    device: Raw,
    flags: u32,
    supported_formats_count: u32,
    supported_formats: *const u8,
    output_duplication: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if output_duplication.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutput5::DuplicateOutput1: pDevice, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDevice` is an `IUnknown*`: null or a live COM object, for the call.
            let device_found = unsafe { UnknownFound::<V::Api>::find(device) };
            let device_found = device_found.ok_or(E_INVALIDARG)?;
            let flags = flags;
            let supported_formats_count = usize::try_from(supported_formats_count)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let supported_formats_bytes = if supported_formats_count == 0 {
                Some(Vec::new())
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGIOutput5::DuplicateOutput1: pSupportedFormats, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `SupportedFormatsCount` as the count of `pSupportedFormats`: that many elements of 4 bytes, readable for the call.
                unsafe {
                    read_only_at(supported_formats, supported_formats_count * 4)
                }
            };
            let supported_formats: Vec<_> = supported_formats_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<4>()
                .0
                .iter()
                .map(|element| {
                    directx_graphics_infrastructure::enumerations::Format(get_i32(
                        element,
                        usize::MIN,
                    ))
                })
                .collect();
            let output_duplication_value =
                found
                    .value()
                    .duplicate_output1(device_found.get(), flags, &supported_formats)?;
            let this_weak = found.weak();
            drop(found);
            let output_duplication_record =
                this_weak.map_or_else(record, |weak| record_related("Output", weak));
            let output_duplication_served = serve(KEY, output_duplication_value, output_duplication_record, output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if output_duplication.is_null() {
                0
            } else {
                output_duplication_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let output_duplication_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput5::DuplicateOutput1: ppOutputDuplication, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppOutputDuplication` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ =
            unsafe { write_bytes(output_duplication.cast::<u8>(), &output_duplication_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput6::GetDesc1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGIOutput6::GetDesc1, which the C side calls")]
pub(crate) unsafe extern "system" fn output6_get_desc1<
    V: directx_graphics_infrastructure::Output6,
>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc1()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout>()];
            encode_windows_win32_graphics_dxgi_dxgi_output_desc1(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput6::GetDesc1: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutput6::CheckHardwareCompositionSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutput6::CheckHardwareCompositionSupport, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output6_check_hardware_composition_support<
    V: directx_graphics_infrastructure::Output6,
>(
    this: Raw,
    flags: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if flags.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let flags_value = found.value().check_hardware_composition_support()?;
            drop(found);
            let mut flags_bytes = [u8::MIN; 4];
            put_u32(&mut flags_bytes, usize::MIN, flags_value);
            Ok((flags_bytes,))
        });
        let flags_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput6::CheckHardwareCompositionSupport: pFlags, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pFlags` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(flags.cast::<u8>(), &flags_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::GetDesc` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::GetDesc, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_get_desc<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
    desc: *mut u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let desc_value = found.value().get_desc();
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutduplDescLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_outdupl_desc(&desc_value, &mut desc_bytes);
            Some((desc_bytes,))
        });
        let desc_written = answer.as_ref().as_ref().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutduplDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetDesc: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGIOutputDuplication::AcquireNextFrame` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::AcquireNextFrame, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_acquire_next_frame<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
    timeout_in_milliseconds: u32,
    frame_info: *mut u8,
    desktop_resource: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if frame_info.is_null() {
                return Err(E_POINTER);
            }
            if desktop_resource.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let timeout_in_milliseconds = timeout_in_milliseconds;
            let (frame_info_value, desktop_resource_value) =
                found.value().acquire_next_frame(timeout_in_milliseconds)?;
            let this_weak = found.weak();
            drop(found);
            let mut frame_info_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_outdupl_frame_info(
                &frame_info_value,
                &mut frame_info_bytes,
            );
            let desktop_resource_record =
                this_weak.map_or_else(record, |weak| record_related("OutputDuplication", weak));
            let desktop_resource_served = serve(KEY, desktop_resource_value, desktop_resource_record, resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
            Ok((
                frame_info_bytes,
                (if desktop_resource.is_null() {
                    0
                } else {
                    desktop_resource_served.into_address().get()
                })
                .to_ne_bytes(),
            ))
        });
        let frame_info_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::AcquireNextFrame: pFrameInfo, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pFrameInfo` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(frame_info.cast::<u8>(), &frame_info_written) };
        let desktop_resource_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::AcquireNextFrame: ppDesktopResource, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppDesktopResource` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(desktop_resource.cast::<u8>(), &desktop_resource_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::GetFrameDirtyRects` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::GetFrameDirtyRects, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_get_frame_dirty_rects<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
    dirty_rects_buffer_size: u32,
    dirty_rects_buffer: *mut u8,
    dirty_rects_buffer_size_required: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if dirty_rects_buffer_size_required.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let dirty_rects_buffer_length = usize::try_from(dirty_rects_buffer_size)
                .ok()
                .ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutputDuplication::GetFrameDirtyRects: pDirtyRectsBuffer, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `MemorySize` names `DirtyRectsBufferSize` as the byte count of `pDirtyRectsBuffer`, which is `Out`: that many bytes, writable for the call.
            let dirty_rects_buffer =
                unsafe { writable_at(dirty_rects_buffer, dirty_rects_buffer_length) };
            let dirty_rects_buffer = dirty_rects_buffer.ok_or(E_INVALIDARG)?;
            let dirty_rects_buffer_size_required_value =
                found.value().get_frame_dirty_rects(&dirty_rects_buffer)?;
            drop(found);
            let mut dirty_rects_buffer_size_required_bytes = [u8::MIN; 4];
            put_u32(
                &mut dirty_rects_buffer_size_required_bytes,
                usize::MIN,
                dirty_rects_buffer_size_required_value,
            );
            Ok((dirty_rects_buffer_size_required_bytes,))
        });
        let dirty_rects_buffer_size_required_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFrameDirtyRects: pDirtyRectsBufferSizeRequired, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDirtyRectsBufferSizeRequired` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe {
            write_bytes(
                dirty_rects_buffer_size_required.cast::<u8>(),
                &dirty_rects_buffer_size_required_written,
            )
        };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::GetFrameMoveRects` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::GetFrameMoveRects, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_get_frame_move_rects<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
    move_rects_buffer_size: u32,
    move_rect_buffer: *mut u8,
    move_rects_buffer_size_required: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if move_rects_buffer_size_required.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let move_rect_buffer_length = usize::try_from(move_rects_buffer_size)
                .ok()
                .ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutputDuplication::GetFrameMoveRects: pMoveRectBuffer, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `MemorySize` names `MoveRectsBufferSize` as the byte count of `pMoveRectBuffer`, which is `Out`: that many bytes, writable for the call.
            let move_rect_buffer =
                unsafe { writable_at(move_rect_buffer, move_rect_buffer_length) };
            let move_rect_buffer = move_rect_buffer.ok_or(E_INVALIDARG)?;
            let move_rects_buffer_size_required_value =
                found.value().get_frame_move_rects(&move_rect_buffer)?;
            drop(found);
            let mut move_rects_buffer_size_required_bytes = [u8::MIN; 4];
            put_u32(
                &mut move_rects_buffer_size_required_bytes,
                usize::MIN,
                move_rects_buffer_size_required_value,
            );
            Ok((move_rects_buffer_size_required_bytes,))
        });
        let move_rects_buffer_size_required_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFrameMoveRects: pMoveRectsBufferSizeRequired, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pMoveRectsBufferSizeRequired` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe {
            write_bytes(
                move_rects_buffer_size_required.cast::<u8>(),
                &move_rects_buffer_size_required_written,
            )
        };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::GetFramePointerShape` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::GetFramePointerShape, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_get_frame_pointer_shape<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
    pointer_shape_buffer_size: u32,
    pointer_shape_buffer: *mut u8,
    pointer_shape_buffer_size_required: *mut u8,
    pointer_shape_info: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if pointer_shape_buffer_size_required.is_null() {
                return Err(E_POINTER);
            }
            if pointer_shape_info.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let pointer_shape_buffer_length = usize::try_from(pointer_shape_buffer_size)
                .ok()
                .ok_or(E_INVALIDARG)?;
            #[expect(
                unsafe_code,
                reason = "IDXGIOutputDuplication::GetFramePointerShape: pPointerShapeBuffer, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `MemorySize` names `PointerShapeBufferSize` as the byte count of `pPointerShapeBuffer`, which is `Out`: that many bytes, writable for the call.
            let pointer_shape_buffer =
                unsafe { writable_at(pointer_shape_buffer, pointer_shape_buffer_length) };
            let pointer_shape_buffer = pointer_shape_buffer.ok_or(E_INVALIDARG)?;
            let (pointer_shape_buffer_size_required_value, pointer_shape_info_value) = found
                .value()
                .get_frame_pointer_shape(&pointer_shape_buffer)?;
            drop(found);
            let mut pointer_shape_buffer_size_required_bytes = [u8::MIN; 4];
            put_u32(
                &mut pointer_shape_buffer_size_required_bytes,
                usize::MIN,
                pointer_shape_buffer_size_required_value,
            );
            let mut pointer_shape_info_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_shape_info(
                &pointer_shape_info_value,
                &mut pointer_shape_info_bytes,
            );
            Ok((
                pointer_shape_buffer_size_required_bytes,
                pointer_shape_info_bytes,
            ))
        });
        let pointer_shape_buffer_size_required_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFramePointerShape: pPointerShapeBufferSizeRequired, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pPointerShapeBufferSizeRequired` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe {
            write_bytes(
                pointer_shape_buffer_size_required.cast::<u8>(),
                &pointer_shape_buffer_size_required_written,
            )
        };
        let pointer_shape_info_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout>()],
            |written| written.1,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFramePointerShape: pPointerShapeInfo, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pPointerShapeInfo` is `Out`: one value of this size to write, or null, for the call.
        let _ =
            unsafe { write_bytes(pointer_shape_info.cast::<u8>(), &pointer_shape_info_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::UnMapDesktopSurface` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::UnMapDesktopSurface, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_un_map_desktop_surface<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let () = found.value().un_map_desktop_surface()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::ReleaseFrame` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::ReleaseFrame, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_release_frame<
    V: directx_graphics_infrastructure::OutputDuplication,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let () = found.value().release_frame()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIOutputDuplication::MapDesktopSurface` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIOutputDuplication::MapDesktopSurface, which the C side calls"
)]
pub(crate) unsafe extern "system" fn output_duplication_map_desktop_surface_refused(
    _this: Raw,
    p0: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    #[expect(
        unsafe_code,
        reason = "IDXGIOutputDuplication::MapDesktopSurface: pLockedRect, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pLockedRect` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 16]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGIResource::GetSharedHandle` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIResource::GetSharedHandle, which the C side calls"
)]
pub unsafe extern "system" fn resource_get_shared_handle<
    V: directx_graphics_infrastructure::Resource,
>(
    this: Raw,
    shared_handle: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if shared_handle.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let shared_handle_value = found.value().get_shared_handle()?;
            drop(found);
            let mut shared_handle_bytes = [u8::MIN; 8];
            put_usize(&mut shared_handle_bytes, usize::MIN, shared_handle_value.0);
            Ok((shared_handle_bytes,))
        });
        let shared_handle_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 8], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIResource::GetSharedHandle: pSharedHandle, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pSharedHandle` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(shared_handle.cast::<u8>(), &shared_handle_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIResource::GetUsage` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIResource::GetUsage, which the C side calls"
)]
pub unsafe extern "system" fn resource_get_usage<V: directx_graphics_infrastructure::Resource>(
    this: Raw,
    usage: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if usage.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let usage_value = found.value().get_usage()?;
            drop(found);
            let mut usage_bytes = [u8::MIN; 4];
            put_u32(&mut usage_bytes, usize::MIN, usage_value.0);
            Ok((usage_bytes,))
        });
        let usage_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIResource::GetUsage: pUsage, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pUsage` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(usage.cast::<u8>(), &usage_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIResource::SetEvictionPriority` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIResource::SetEvictionPriority, which the C side calls"
)]
pub unsafe extern "system" fn resource_set_eviction_priority<
    V: directx_graphics_infrastructure::Resource,
>(
    this: Raw,
    eviction_priority: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let eviction_priority =
                directx_graphics_infrastructure::enumerations::ResourcePriority(eviction_priority);
            let () = found.value().set_eviction_priority(eviction_priority)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIResource::GetEvictionPriority` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIResource::GetEvictionPriority, which the C side calls"
)]
pub unsafe extern "system" fn resource_get_eviction_priority<
    V: directx_graphics_infrastructure::Resource,
>(
    this: Raw,
    eviction_priority: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if eviction_priority.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let eviction_priority_value = found.value().get_eviction_priority()?;
            drop(found);
            let mut eviction_priority_bytes = [u8::MIN; 4];
            put_u32(
                &mut eviction_priority_bytes,
                usize::MIN,
                eviction_priority_value.0,
            );
            Ok((eviction_priority_bytes,))
        });
        let eviction_priority_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIResource::GetEvictionPriority: pEvictionPriority, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pEvictionPriority` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(eviction_priority.cast::<u8>(), &eviction_priority_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIResource1::CreateSubresourceSurface` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIResource1::CreateSubresourceSurface, which the C side calls"
)]
pub unsafe extern "system" fn resource1_create_subresource_surface<
    V: directx_graphics_infrastructure::Resource1,
>(
    this: Raw,
    index: u32,
    surface: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if surface.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let index = index;
            let surface_value = found.value().create_subresource_surface(index)?;
            let this_weak = found.weak();
            drop(found);
            let surface_record =
                this_weak.map_or_else(record, |weak| record_related("Resource", weak));
            let surface_served = serve(KEY, surface_value, surface_record, surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if surface.is_null() {
                0
            } else {
                surface_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let surface_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIResource1::CreateSubresourceSurface: ppSurface, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSurface` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(surface.cast::<u8>(), &surface_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGIResource1::CreateSharedHandle` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "IDXGIResource1::CreateSharedHandle, which the C side calls"
)]
pub(crate) unsafe extern "system" fn resource1_create_shared_handle_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: u32,
    _p2: *const u8,
    p3: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    #[expect(
        unsafe_code,
        reason = "IDXGIResource1::CreateSharedHandle: pHandle, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pHandle` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p3, &[u8::MIN; 8]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGISurface::GetDesc` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGISurface::GetDesc, which the C side calls")]
pub unsafe extern "system" fn surface_get_desc<V: directx_graphics_infrastructure::Surface>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            if let Some(Recorded(recorded)) =
                found.part::<Recorded<directx_graphics_infrastructure::structures::SurfaceDesc>>()
            {
                let mut desc_bytes =
                    [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout>()];
                encode_windows_win32_graphics_dxgi_dxgi_surface_desc(recorded, &mut desc_bytes);
                return Ok((desc_bytes,));
            }
            let desc_value = found.value().get_desc()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_surface_desc(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISurface::GetDesc: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISurface::Unmap` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGISurface::Unmap, which the C side calls")]
pub unsafe extern "system" fn surface_unmap<V: directx_graphics_infrastructure::Surface>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let () = found.value().unmap()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISurface::Map` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(unsafe_code, reason = "IDXGISurface::Map, which the C side calls")]
pub(crate) unsafe extern "system" fn surface_map_refused(_this: Raw, p0: *mut u8, _p1: u32) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    #[expect(
        unsafe_code,
        reason = "IDXGISurface::Map: pLockedRect, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pLockedRect` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 16]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGISurface1::GetDC` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "IDXGISurface1::GetDC, which the C side calls")]
pub unsafe extern "system" fn surface1_get_dc<V: directx_graphics_infrastructure::Surface1>(
    this: Raw,
    discard: i32,
    phdc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if phdc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let discard = discard != 0;
            let phdc_value = found.value().get_dc(discard)?;
            drop(found);
            let mut phdc_bytes = [u8::MIN; 8];
            put_usize(&mut phdc_bytes, usize::MIN, phdc_value.0);
            Ok((phdc_bytes,))
        });
        let phdc_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 8], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISurface1::GetDC: phdc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `phdc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(phdc.cast::<u8>(), &phdc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISurface1::ReleaseDC` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISurface1::ReleaseDC, which the C side calls"
)]
pub unsafe extern "system" fn surface1_release_dc<V: directx_graphics_infrastructure::Surface1>(
    this: Raw,
    dirty_rect: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut dirty_rect_bytes = [u8::MIN; size_of::<WindowsWin32FoundationRectLayout>()];
            let dirty_rect_null = dirty_rect.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGISurface1::ReleaseDC: pDirtyRect, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDirtyRect` is `In` and `Const`: one `Rect` of 16 bytes, readable for the call.
            let dirty_rect_read = unsafe { read_bytes(dirty_rect, &mut dirty_rect_bytes) };
            let dirty_rect =
                dirty_rect_read.then(|| decode_windows_win32_foundation_rect(&dirty_rect_bytes));
            if !dirty_rect_read && !dirty_rect_null {
                return Err(E_INVALIDARG);
            }
            let () = found.value().release_dc(dirty_rect.as_ref())?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISurface2::GetResource` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISurface2::GetResource, which the C side calls"
)]
pub unsafe extern "system" fn surface2_get_resource<
    V: directx_graphics_infrastructure::Surface2,
>(
    this: Raw,
    riid: *const u8,
    parent_resource: *mut Raw,
    subresource_index: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if parent_resource.is_null() {
                return Err(E_POINTER);
            }
            if subresource_index.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGISurface2::GetResource: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let (parent_resource_value, subresource_index_value) =
                found.value().get_resource(&riid)?;
            drop(found);
            let parent_resource_answer = match parent_resource_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            let mut subresource_index_bytes = [u8::MIN; 4];
            put_u32(
                &mut subresource_index_bytes,
                usize::MIN,
                subresource_index_value,
            );
            Ok((
                (if parent_resource.is_null() {
                    0
                } else {
                    parent_resource_answer.into_address().get()
                })
                .to_ne_bytes(),
                subresource_index_bytes,
            ))
        });
        let parent_resource_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISurface2::GetResource: ppParentResource, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppParentResource` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(parent_resource.cast::<u8>(), &parent_resource_written) };
        let subresource_index_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGISurface2::GetResource: pSubresourceIndex, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pSubresourceIndex` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(subresource_index.cast::<u8>(), &subresource_index_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::Present` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::Present, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_present<V: directx_graphics_infrastructure::SwapChain>(
    this: Raw,
    sync_interval: u32,
    flags: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let sync_interval = sync_interval;
            let flags = directx_graphics_infrastructure::enumerations::Present(flags);
            let () = found.value().present(sync_interval, flags)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::GetBuffer` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::GetBuffer, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_get_buffer<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    buffer: u32,
    riid: *const u8,
    surface: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if surface.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let buffer = buffer;
            let mut riid_bytes = [u8::MIN; 16];
            #[expect(
                unsafe_code,
                reason = "IDXGISwapChain::GetBuffer: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one GUID, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            if !riid_read {
                return Err(E_INVALIDARG);
            }
            let riid = decode_guid(&riid_bytes);
            if let Some(Recorded(recorded)) =
                found.part::<Recorded<directx_graphics_infrastructure::structures::SwapChainDesc>>()
                && buffer >= recorded.buffer_count
            {
                return Err(E_INVALIDARG);
            }
            if let Some(recorded) = related_of(&found, "Buffer", buffer) {
                let answered = query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer>(KEY, recorded.address(), &riid).ok_or(E_NOINTERFACE)?;
                return Ok((answered.into_address().get().to_ne_bytes(),));
            }
            let surface_value = found.value().get_buffer(buffer)?;
            drop(found);
            let surface_interfaces = bound_buffer::<
                <V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer,
            >()
            .ok_or(E_NOINTERFACE)?;
            let surface_served =
                serve(KEY, surface_value, record(), surface_interfaces).ok_or(E_OUTOFMEMORY)?;
            relate_at::<V>(KEY, this.addr(), "Buffer", buffer, surface_served.weak());
            let surface_answer = query::<
                <V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer,
            >(KEY, surface_served.address(), &riid)
            .ok_or(E_NOINTERFACE)?;
            Ok(((if surface.is_null() {
                0
            } else {
                surface_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let surface_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetBuffer: ppSurface, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppSurface` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(surface.cast::<u8>(), &surface_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::SetFullscreenState` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::SetFullscreenState, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_set_fullscreen_state<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    fullscreen: i32,
    target: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let fullscreen = fullscreen != 0;
            let target_found = if target.is_null() {
                None
            } else {
                Some(object::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, target.addr()).ok_or(E_INVALIDARG)?)
            };
            let () = found
                .value()
                .set_fullscreen_state(fullscreen, target_found.as_ref().map(|held| held.value()))?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::GetFullscreenState` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::GetFullscreenState, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_get_fullscreen_state<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    fullscreen: *mut u8,
    target: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let (fullscreen_value, target_value) = found.value().get_fullscreen_state()?;
            let this_weak = found.weak();
            drop(found);
            let mut fullscreen_bytes = [u8::MIN; 4];
            put_i32(
                &mut fullscreen_bytes,
                usize::MIN,
                i32::from(fullscreen_value),
            );
            let target_record =
                this_weak.map_or_else(record, |weak| record_related("SwapChain", weak));
            let target_served = serve(KEY, target_value, target_record, output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
            Ok((
                fullscreen_bytes,
                (if target.is_null() {
                    0
                } else {
                    target_served.into_address().get()
                })
                .to_ne_bytes(),
            ))
        });
        let fullscreen_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetFullscreenState: pFullscreen, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pFullscreen` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(fullscreen.cast::<u8>(), &fullscreen_written) };
        let target_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetFullscreenState: ppTarget, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppTarget` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(target.cast::<u8>(), &target_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::GetDesc` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::GetDesc, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_get_desc<V: directx_graphics_infrastructure::SwapChain>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            if let Some(Recorded(recorded)) =
                found.part::<Recorded<directx_graphics_infrastructure::structures::SwapChainDesc>>()
            {
                let mut desc_bytes =
                    [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout>()];
                encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(recorded, &mut desc_bytes);
                return Ok((desc_bytes,));
            }
            let desc_value = found.value().get_desc()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetDesc: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::ResizeBuffers` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::ResizeBuffers, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_resize_buffers<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: i32,
    swap_chain_flags: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let buffer_count = buffer_count;
            let width = width;
            let height = height;
            let new_format = directx_graphics_infrastructure::enumerations::Format(new_format);
            let swap_chain_flags = swap_chain_flags;
            let () = found.value().resize_buffers(
                buffer_count,
                width,
                height,
                new_format,
                swap_chain_flags,
            )?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::ResizeTarget` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::ResizeTarget, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_resize_target<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    new_target_parameters: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut new_target_parameters_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>()];
            let new_target_parameters_null = new_target_parameters.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGISwapChain::ResizeTarget: pNewTargetParameters, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pNewTargetParameters` is `In` and `Const`: one `ModeDesc` of 28 bytes, readable for the call.
            let new_target_parameters_read =
                unsafe { read_bytes(new_target_parameters, &mut new_target_parameters_bytes) };
            let new_target_parameters = new_target_parameters_read.then(|| {
                decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
                    &new_target_parameters_bytes,
                )
            });
            let new_target_parameters = new_target_parameters.ok_or(E_INVALIDARG)?;
            let () = found.value().resize_target(&new_target_parameters)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::GetContainingOutput` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::GetContainingOutput, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_get_containing_output<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    output: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if output.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let output_value = found.value().get_containing_output()?;
            let this_weak = found.weak();
            drop(found);
            let output_record =
                this_weak.map_or_else(record, |weak| record_related("SwapChain", weak));
            let output_served = serve(KEY, output_value, output_record, output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
            Ok(((if output.is_null() {
                0
            } else {
                output_served.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let output_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetContainingOutput: ppOutput, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppOutput` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(output.cast::<u8>(), &output_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::GetFrameStatistics` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::GetFrameStatistics, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_get_frame_statistics<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    stats: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if stats.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let stats_value = found.value().get_frame_statistics()?;
            drop(found);
            let mut stats_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_frame_statistics(
                &stats_value,
                &mut stats_bytes,
            );
            Ok((stats_bytes,))
        });
        let stats_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetFrameStatistics: pStats, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pStats` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(stats.cast::<u8>(), &stats_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain::GetLastPresentCount` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain::GetLastPresentCount, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain_get_last_present_count<
    V: directx_graphics_infrastructure::SwapChain,
>(
    this: Raw,
    last_present_count: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if last_present_count.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let last_present_count_value = found.value().get_last_present_count()?;
            drop(found);
            let mut last_present_count_bytes = [u8::MIN; 4];
            put_u32(
                &mut last_present_count_bytes,
                usize::MIN,
                last_present_count_value,
            );
            Ok((last_present_count_bytes,))
        });
        let last_present_count_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetLastPresentCount: pLastPresentCount, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pLastPresentCount` is `Out`: one value of this size to write, or null, for the call.
        let _ =
            unsafe { write_bytes(last_present_count.cast::<u8>(), &last_present_count_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::GetDesc1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetDesc1, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_desc1<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_desc1()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>()];
            encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&desc_value, &mut desc_bytes);
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetDesc1: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::GetFullscreenDesc` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetFullscreenDesc, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_fullscreen_desc<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    desc: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if desc.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desc_value = found.value().get_fullscreen_desc()?;
            drop(found);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(
                &desc_value,
                &mut desc_bytes,
            );
            Ok((desc_bytes,))
        });
        let desc_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetFullscreenDesc: pDesc, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDesc` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(desc.cast::<u8>(), &desc_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::GetHwnd` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetHwnd, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_hwnd<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    hwnd: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if hwnd.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let hwnd_value = found.value().get_hwnd()?;
            drop(found);
            let mut hwnd_bytes = [u8::MIN; 8];
            put_usize(&mut hwnd_bytes, usize::MIN, hwnd_value.0);
            Ok((hwnd_bytes,))
        });
        let hwnd_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 8], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetHwnd: pHwnd, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pHwnd` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(hwnd.cast::<u8>(), &hwnd_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::GetCoreWindow` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetCoreWindow, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_core_window<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    refiid: *const u8,
    unk: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if unk.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut refiid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let refiid_null = refiid.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGISwapChain1::GetCoreWindow: refiid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `refiid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let refiid_read = unsafe { read_bytes(refiid, &mut refiid_bytes) };
            let refiid = refiid_read.then(|| decode_guid(&refiid_bytes));
            let refiid = refiid.ok_or(E_INVALIDARG)?;
            let unk_value = found.value().get_core_window(&refiid)?;
            drop(found);
            let unk_answer = match unk_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &refiid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if unk.is_null() {
                0
            } else {
                unk_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let unk_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetCoreWindow: ppUnk, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppUnk` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(unk.cast::<u8>(), &unk_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::IsTemporaryMonoSupported` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::IsTemporaryMonoSupported, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_is_temporary_mono_supported<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().is_temporary_mono_supported();
            drop(found);
            Some((i32::from(returned),))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGISwapChain1::GetRestrictToOutput` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetRestrictToOutput, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_restrict_to_output<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    restrict_to_output: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if restrict_to_output.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let restrict_to_output_value = found.value().get_restrict_to_output()?;
            drop(found);
            let restrict_to_output_served = match restrict_to_output_value { Some(value) => Some(serve(KEY, value, record(), output_interfaces::<<V::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?), None => None };
            Ok(((if restrict_to_output.is_null() {
                0
            } else {
                restrict_to_output_served.map_or(0, |served| served.into_address().get())
            })
            .to_ne_bytes(),))
        });
        let restrict_to_output_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetRestrictToOutput: ppRestrictToOutput, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppRestrictToOutput` is `Out`: one interface pointer to write, or null, for the call.
        let _ =
            unsafe { write_bytes(restrict_to_output.cast::<u8>(), &restrict_to_output_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::SetBackgroundColor` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::SetBackgroundColor, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_set_background_color<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    color: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut color_bytes = [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiRgbaLayout>()];
            let color_null = color.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGISwapChain1::SetBackgroundColor: pColor, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pColor` is `In` and `Const`: one `Rgba` of 16 bytes, readable for the call.
            let color_read = unsafe { read_bytes(color, &mut color_bytes) };
            let color =
                color_read.then(|| decode_windows_win32_graphics_dxgi_dxgi_rgba(&color_bytes));
            let color = color.ok_or(E_INVALIDARG)?;
            let () = found.value().set_background_color(&color)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::GetBackgroundColor` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetBackgroundColor, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_background_color<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    color: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if color.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let color_value = found.value().get_background_color()?;
            drop(found);
            let mut color_bytes = [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiRgbaLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_rgba(&color_value, &mut color_bytes);
            Ok((color_bytes,))
        });
        let color_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiRgbaLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetBackgroundColor: pColor, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pColor` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(color.cast::<u8>(), &color_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::SetRotation` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::SetRotation, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_set_rotation<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    rotation: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let rotation = directx_graphics_infrastructure::enumerations::ModeRotation(rotation);
            let () = found.value().set_rotation(rotation)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::GetRotation` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain1::GetRotation, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain1_get_rotation<
    V: directx_graphics_infrastructure::SwapChain1,
>(
    this: Raw,
    rotation: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if rotation.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let rotation_value = found.value().get_rotation()?;
            drop(found);
            let mut rotation_bytes = [u8::MIN; 4];
            put_i32(&mut rotation_bytes, usize::MIN, rotation_value.0);
            Ok((rotation_bytes,))
        });
        let rotation_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetRotation: pRotation, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pRotation` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(rotation.cast::<u8>(), &rotation_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain1::Present1` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn swap_chain1_present1_refused(
    _this: Raw,
    _p0: u32,
    _p1: u32,
    _p2: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGISwapChain2::SetSourceSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::SetSourceSize, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_set_source_size<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
    width: u32,
    height: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let width = width;
            let height = height;
            let () = found.value().set_source_size(width, height)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain2::GetSourceSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::GetSourceSize, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_get_source_size<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
    width: *mut u8,
    height: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if width.is_null() {
                return Err(E_POINTER);
            }
            if height.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let (width_value, height_value) = found.value().get_source_size()?;
            drop(found);
            let mut width_bytes = [u8::MIN; 4];
            put_u32(&mut width_bytes, usize::MIN, width_value);
            let mut height_bytes = [u8::MIN; 4];
            put_u32(&mut height_bytes, usize::MIN, height_value);
            Ok((width_bytes, height_bytes))
        });
        let width_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetSourceSize: pWidth, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pWidth` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(width.cast::<u8>(), &width_written) };
        let height_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetSourceSize: pHeight, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pHeight` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(height.cast::<u8>(), &height_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain2::SetMaximumFrameLatency` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::SetMaximumFrameLatency, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_set_maximum_frame_latency<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
    max_latency: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let max_latency = max_latency;
            let () = found.value().set_maximum_frame_latency(max_latency)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain2::GetMaximumFrameLatency` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::GetMaximumFrameLatency, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_get_maximum_frame_latency<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
    max_latency: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if max_latency.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let max_latency_value = found.value().get_maximum_frame_latency()?;
            drop(found);
            let mut max_latency_bytes = [u8::MIN; 4];
            put_u32(&mut max_latency_bytes, usize::MIN, max_latency_value);
            Ok((max_latency_bytes,))
        });
        let max_latency_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetMaximumFrameLatency: pMaxLatency, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pMaxLatency` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(max_latency.cast::<u8>(), &max_latency_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain2::GetFrameLatencyWaitableObject` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::GetFrameLatencyWaitableObject, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_get_frame_latency_waitable_object<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
) -> usize {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_frame_latency_waitable_object();
            drop(found);
            Some((returned.0,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGISwapChain2::SetMatrixTransform` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::SetMatrixTransform, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_set_matrix_transform<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
    matrix: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut matrix_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout>()];
            let matrix_null = matrix.is_null();
            #[expect(
                unsafe_code,
                reason = "IDXGISwapChain2::SetMatrixTransform: pMatrix, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pMatrix` is `In` and `Const`: one `Matrix3x2F` of 24 bytes, readable for the call.
            let matrix_read = unsafe { read_bytes(matrix, &mut matrix_bytes) };
            let matrix = matrix_read
                .then(|| decode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(&matrix_bytes));
            let matrix = matrix.ok_or(E_INVALIDARG)?;
            let () = found.value().set_matrix_transform(&matrix)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain2::GetMatrixTransform` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain2::GetMatrixTransform, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain2_get_matrix_transform<
    V: directx_graphics_infrastructure::SwapChain2,
>(
    this: Raw,
    matrix: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if matrix.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let matrix_value = found.value().get_matrix_transform()?;
            drop(found);
            let mut matrix_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(&matrix_value, &mut matrix_bytes);
            Ok((matrix_bytes,))
        });
        let matrix_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetMatrixTransform: pMatrix, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pMatrix` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(matrix.cast::<u8>(), &matrix_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain3::GetCurrentBackBufferIndex` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain3::GetCurrentBackBufferIndex, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain3_get_current_back_buffer_index<
    V: directx_graphics_infrastructure::SwapChain3,
>(
    this: Raw,
) -> u32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_current_back_buffer_index();
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `IDXGISwapChain3::CheckColorSpaceSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain3::CheckColorSpaceSupport, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain3_check_color_space_support<
    V: directx_graphics_infrastructure::SwapChain3,
>(
    this: Raw,
    color_space: i32,
    color_space_support: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if color_space_support.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let color_space =
                directx_graphics_infrastructure::enumerations::ColorSpaceType(color_space);
            let color_space_support_value = found.value().check_color_space_support(color_space)?;
            drop(found);
            let mut color_space_support_bytes = [u8::MIN; 4];
            put_u32(
                &mut color_space_support_bytes,
                usize::MIN,
                color_space_support_value,
            );
            Ok((color_space_support_bytes,))
        });
        let color_space_support_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain3::CheckColorSpaceSupport: pColorSpaceSupport, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pColorSpaceSupport` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe {
            write_bytes(
                color_space_support.cast::<u8>(),
                &color_space_support_written,
            )
        };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain3::SetColorSpace1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain3::SetColorSpace1, which the C side calls"
)]
pub unsafe extern "system" fn swap_chain3_set_color_space1<
    V: directx_graphics_infrastructure::SwapChain3,
>(
    this: Raw,
    color_space: i32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let color_space =
                directx_graphics_infrastructure::enumerations::ColorSpaceType(color_space);
            let () = found.value().set_color_space1(color_space)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChain3::ResizeBuffers1` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL. The call is counted (`refusals_reached`).
pub(crate) extern "system" fn swap_chain3_resize_buffers1_refused(
    _this: Raw,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    _p3: i32,
    _p4: u32,
    _p5: *mut u8,
    _p6: *mut u8,
) -> i32 {
    REFUSALS_REACHED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `IDXGISwapChain4::SetHDRMetaData` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChain4::SetHDRMetaData, which the C side calls"
)]
pub(crate) unsafe extern "system" fn swap_chain4_set_hdr_meta_data<
    V: directx_graphics_infrastructure::SwapChain4,
>(
    this: Raw,
    type_: i32,
    size: u32,
    meta_data: *const u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let type_ = directx_graphics_infrastructure::enumerations::HdrMetadataType(type_);
            let meta_data_absent = meta_data.is_null();
            let meta_data_count = usize::try_from(size)
                .ok()
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let meta_data_bytes = if meta_data_count == 0 || meta_data.is_null() {
                Some(Vec::new())
            } else {
                #[expect(
                    unsafe_code,
                    reason = "IDXGISwapChain4::SetHDRMetaData: pMetaData, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `Size` as the count of `pMetaData`: that many elements of 1 bytes, readable for the call.
                unsafe {
                    read_only_at(meta_data, meta_data_count)
                }
            };
            let meta_data: Vec<_> = meta_data_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<1>()
                .0
                .iter()
                .map(|element| get_u8(element, usize::MIN))
                .collect();
            let () = found
                .value()
                .set_hdr_meta_data(type_, (!meta_data_absent).then_some(&meta_data[..]))?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChainMedia::GetFrameStatisticsMedia` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChainMedia::GetFrameStatisticsMedia, which the C side calls"
)]
pub(crate) unsafe extern "system" fn swap_chain_media_get_frame_statistics_media<
    V: directx_graphics_infrastructure::SwapChainMedia,
>(
    this: Raw,
    stats: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if stats.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let stats_value = found.value().get_frame_statistics_media()?;
            drop(found);
            let mut stats_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout>()];
            encode_windows_win32_graphics_dxgi_dxgi_frame_statistics_media(
                &stats_value,
                &mut stats_bytes,
            );
            Ok((stats_bytes,))
        });
        let stats_written = answer.as_ref().ok().map_or(
            [u8::MIN; size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout>()],
            |written| written.0,
        );
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChainMedia::GetFrameStatisticsMedia: pStats, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pStats` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(stats.cast::<u8>(), &stats_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChainMedia::SetPresentDuration` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChainMedia::SetPresentDuration, which the C side calls"
)]
pub(crate) unsafe extern "system" fn swap_chain_media_set_present_duration<
    V: directx_graphics_infrastructure::SwapChainMedia,
>(
    this: Raw,
    duration: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let duration = duration;
            let () = found.value().set_present_duration(duration)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGISwapChainMedia::CheckPresentDurationSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGISwapChainMedia::CheckPresentDurationSupport, which the C side calls"
)]
pub(crate) unsafe extern "system" fn swap_chain_media_check_present_duration_support<
    V: directx_graphics_infrastructure::SwapChainMedia,
>(
    this: Raw,
    desired_present_duration: u32,
    closest_smaller_present_duration: *mut u8,
    closest_larger_present_duration: *mut u8,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if closest_smaller_present_duration.is_null() {
                return Err(E_POINTER);
            }
            if closest_larger_present_duration.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let desired_present_duration = desired_present_duration;
            let (closest_smaller_present_duration_value, closest_larger_present_duration_value) =
                found
                    .value()
                    .check_present_duration_support(desired_present_duration)?;
            drop(found);
            let mut closest_smaller_present_duration_bytes = [u8::MIN; 4];
            put_u32(
                &mut closest_smaller_present_duration_bytes,
                usize::MIN,
                closest_smaller_present_duration_value,
            );
            let mut closest_larger_present_duration_bytes = [u8::MIN; 4];
            put_u32(
                &mut closest_larger_present_duration_bytes,
                usize::MIN,
                closest_larger_present_duration_value,
            );
            Ok((
                closest_smaller_present_duration_bytes,
                closest_larger_present_duration_bytes,
            ))
        });
        let closest_smaller_present_duration_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChainMedia::CheckPresentDurationSupport: pClosestSmallerPresentDuration, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pClosestSmallerPresentDuration` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe {
            write_bytes(
                closest_smaller_present_duration.cast::<u8>(),
                &closest_smaller_present_duration_written,
            )
        };
        let closest_larger_present_duration_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; 4], |written| written.1);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChainMedia::CheckPresentDurationSupport: pClosestLargerPresentDuration, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pClosestLargerPresentDuration` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe {
            write_bytes(
                closest_larger_present_duration.cast::<u8>(),
                &closest_larger_present_duration_written,
            )
        };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `IDXGraphicsAnalysis::BeginCapture` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGraphicsAnalysis::BeginCapture, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_analysis_begin_capture<
    V: directx_graphics_infrastructure::GraphicsAnalysis,
>(
    this: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let () = found.value().begin_capture();
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `IDXGraphicsAnalysis::EndCapture` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "IDXGraphicsAnalysis::EndCapture, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_analysis_end_capture<
    V: directx_graphics_infrastructure::GraphicsAnalysis,
>(
    this: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let () = found.value().end_capture();
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `CreateDXGIFactory` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "CreateDXGIFactory, which the C side calls")]
pub(crate) unsafe extern "system" fn export_create_dxgi_factory<
    M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>(
    riid: *const u8,
    factory: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if factory.is_null() {
                return Err(E_POINTER);
            }
            let found = module::<M>().ok_or(E_FAIL)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "CreateDXGIFactory: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let factory_value = found.create_dxgi_factory(&riid)?;
            drop(found);
            let factory_answer = match factory_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if factory.is_null() {
                0
            } else {
                factory_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let factory_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory: ppFactory, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppFactory` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(factory.cast::<u8>(), &factory_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `CreateDXGIFactory1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "CreateDXGIFactory1, which the C side calls")]
pub(crate) unsafe extern "system" fn export_create_dxgi_factory1<
    M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>(
    riid: *const u8,
    factory: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if factory.is_null() {
                return Err(E_POINTER);
            }
            let found = module::<M>().ok_or(E_FAIL)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "CreateDXGIFactory1: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let factory_value = found.create_dxgi_factory1(&riid)?;
            drop(found);
            let factory_answer = match factory_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if factory.is_null() {
                0
            } else {
                factory_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let factory_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory1: ppFactory, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppFactory` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(factory.cast::<u8>(), &factory_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `CreateDXGIFactory2` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "CreateDXGIFactory2, which the C side calls")]
pub(crate) unsafe extern "system" fn export_create_dxgi_factory2<
    M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>(
    flags: u32,
    riid: *const u8,
    factory: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if factory.is_null() {
                return Err(E_POINTER);
            }
            let found = module::<M>().ok_or(E_FAIL)?;
            let flags = directx_graphics_infrastructure::enumerations::CreateFactoryFlags(flags);
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "CreateDXGIFactory2: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let factory_value = found.create_dxgi_factory2(flags, &riid)?;
            drop(found);
            let factory_answer = match factory_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if factory.is_null() {
                0
            } else {
                factory_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let factory_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory2: ppFactory, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppFactory` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(factory.cast::<u8>(), &factory_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `DXGIDeclareAdapterRemovalSupport` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "DXGIDeclareAdapterRemovalSupport, which the C side calls"
)]
pub(crate) unsafe extern "system" fn export_dxgi_declare_adapter_removal_support<
    M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>() -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = module::<M>().ok_or(E_FAIL)?;
            let () = found.dxgi_declare_adapter_removal_support()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `DXGIDisableVBlankVirtualization` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "DXGIDisableVBlankVirtualization, which the C side calls"
)]
pub(crate) unsafe extern "system" fn export_dxgi_disable_v_blank_virtualization<
    M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>() -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = module::<M>().ok_or(E_FAIL)?;
            let () = found.dxgi_disable_v_blank_virtualization()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `DXGIGetDebugInterface1` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "DXGIGetDebugInterface1, which the C side calls")]
pub(crate) unsafe extern "system" fn export_dxgi_get_debug_interface1<
    M: directx_graphics_infrastructure::DirectXGraphicsInfrastructure,
>(
    flags: u32,
    riid: *const u8,
    debug: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if debug.is_null() {
                return Err(E_POINTER);
            }
            let found = module::<M>().ok_or(E_FAIL)?;
            let flags = flags;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "DXGIGetDebugInterface1: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let debug_value = found.dxgi_get_debug_interface1(flags, &riid)?;
            drop(found);
            let debug_answer = match debug_value {
                directx_graphics_infrastructure::UnknownObject::Adapter(object) => {
                    let served = serve(KEY, object, record(), adapter_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Debug(object) => {
                    let served = serve(KEY, object, record(), debug_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Debug>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object) => {
                    let served = serve(KEY, object, record(), decode_swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Device(object) => {
                    let served = serve(KEY, object, record(), device_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Device>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::DisplayControl(object) => {
                    let served = serve(KEY, object, record(), display_control_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DisplayControl>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Factory(object) => {
                    let served = serve(KEY, object, record(), factory_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Factory>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::FactoryMedia(object) => {
                    let served = serve(KEY, object, record(), factory_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::FactoryMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::InfoQueue(object) => {
                    let served = serve(KEY, object, record(), info_queue_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::InfoQueue>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::KeyedMutex(object) => {
                    let served = serve(KEY, object, record(), keyed_mutex_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::KeyedMutex>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Output(object) => {
                    let served = serve(KEY, object, record(), output_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::OutputDuplication(object) => {
                    let served = serve(KEY, object, record(), output_duplication_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Resource(object) => {
                    let served = serve(KEY, object, record(), resource_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Surface(object) => {
                    let served = serve(KEY, object, record(), surface_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChain(object) => {
                    let served = serve(KEY, object, record(), swap_chain_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object) => {
                    let served = serve(KEY, object, record(), swap_chain_media_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChainMedia>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object) => {
                    let served = serve(KEY, object, record(), graphics_analysis_interfaces::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>).ok_or(E_OUTOFMEMORY)?;
                    query::<<M as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::GraphicsAnalysis>(KEY, served.address(), &riid).ok_or(E_NOINTERFACE)?
                }
                directx_graphics_infrastructure::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if debug.is_null() {
                0
            } else {
                debug_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let debug_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "DXGIGetDebugInterface1: pDebug, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pDebug` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(debug.cast::<u8>(), &debug_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}
