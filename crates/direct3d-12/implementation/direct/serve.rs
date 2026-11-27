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
use wcontainers_direct3d_12 as direct3d_12;
use wcontainers_direct3d_common as direct3d_common;

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
pub(crate) struct Tables<V>(PhantomData<V>);

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::CommandAllocator> Tables<V> {
    /// `ID3D12CommandAllocator`'s table for objects of type `V`.
    pub(crate) const COMMAND_ALLOCATOR: &'static CommandAllocatorTable = &CommandAllocatorTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        command_allocator_reset: command_allocator_reset::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::CommandList> Tables<V> {
    /// `ID3D12CommandList`'s table for objects of type `V`.
    pub const COMMAND_LIST: &'static CommandListTable = &CommandListTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        command_list_get_type: command_list_get_type_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::CommandQueue> Tables<V> {
    /// `ID3D12CommandQueue`'s table for objects of type `V`.
    pub(crate) const COMMAND_QUEUE: &'static CommandQueueTable = &CommandQueueTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        command_queue_update_tile_mappings: command_queue_update_tile_mappings_refused,
        command_queue_copy_tile_mappings: command_queue_copy_tile_mappings_refused,
        command_queue_execute_command_lists: command_queue_execute_command_lists_refused,
        command_queue_set_marker: command_queue_set_marker_refused,
        command_queue_begin_event: command_queue_begin_event_refused,
        command_queue_end_event: command_queue_end_event_refused,
        command_queue_signal: command_queue_signal::<V>,
        command_queue_wait: command_queue_wait_refused,
        command_queue_get_timestamp_frequency: command_queue_get_timestamp_frequency_refused,
        command_queue_get_clock_calibration: command_queue_get_clock_calibration_refused,
        command_queue_get_desc: command_queue_get_desc_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::DescriptorHeap> Tables<V> {
    /// `ID3D12DescriptorHeap`'s table for objects of type `V`.
    pub(crate) const DESCRIPTOR_HEAP: &'static DescriptorHeapTable = &DescriptorHeapTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        descriptor_heap_get_desc: descriptor_heap_get_desc_refused,
        descriptor_heap_get_cpu_descriptor_handle_for_heap_start:
            descriptor_heap_get_cpu_descriptor_handle_for_heap_start_refused,
        descriptor_heap_get_gpu_descriptor_handle_for_heap_start:
            descriptor_heap_get_gpu_descriptor_handle_for_heap_start_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::Device> Tables<V> {
    /// `ID3D12Device`'s table for objects of type `V`.
    pub(crate) const DEVICE: &'static DeviceTable = &DeviceTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_get_node_count: device_get_node_count_refused,
        device_create_command_queue: device_create_command_queue::<V>,
        device_create_command_allocator: device_create_command_allocator::<V>,
        device_create_graphics_pipeline_state: device_create_graphics_pipeline_state_refused,
        device_create_compute_pipeline_state: device_create_compute_pipeline_state_refused,
        device_create_command_list: device_create_command_list::<V>,
        device_check_feature_support: device_check_feature_support_refused,
        device_create_descriptor_heap: device_create_descriptor_heap::<V>,
        device_get_descriptor_handle_increment_size: device_get_descriptor_handle_increment_size::<V>,
        device_create_root_signature: device_create_root_signature::<V>,
        device_create_constant_buffer_view: device_create_constant_buffer_view_refused,
        device_create_shader_resource_view: device_create_shader_resource_view_refused,
        device_create_unordered_access_view: device_create_unordered_access_view_refused,
        device_create_render_target_view: device_create_render_target_view_refused,
        device_create_depth_stencil_view: device_create_depth_stencil_view_refused,
        device_create_sampler: device_create_sampler_refused,
        device_copy_descriptors: device_copy_descriptors_refused,
        device_copy_descriptors_simple: device_copy_descriptors_simple_refused,
        device_get_resource_allocation_info: device_get_resource_allocation_info_refused,
        device_get_custom_heap_properties: device_get_custom_heap_properties_refused,
        device_create_committed_resource: device_create_committed_resource::<V>,
        device_create_heap: device_create_heap_refused,
        device_create_placed_resource: device_create_placed_resource_refused,
        device_create_reserved_resource: device_create_reserved_resource_refused,
        device_create_shared_handle: device_create_shared_handle_refused,
        device_open_shared_handle: device_open_shared_handle_refused,
        device_open_shared_handle_by_name: device_open_shared_handle_by_name_refused,
        device_make_resident: device_make_resident_refused,
        device_evict: device_evict_refused,
        device_create_fence: device_create_fence::<V>,
        device_get_device_removed_reason: device_get_device_removed_reason_refused,
        device_get_copyable_footprints: device_get_copyable_footprints::<V>,
        device_create_query_heap: device_create_query_heap::<V>,
        device_set_stable_power_state: device_set_stable_power_state_refused,
        device_create_command_signature: device_create_command_signature_refused,
        device_get_resource_tiling: device_get_resource_tiling_refused,
        device_get_adapter_luid: device_get_adapter_luid_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::DeviceChild> Tables<V> {
    /// `ID3D12DeviceChild`'s table for objects of type `V`.
    pub const DEVICE_CHILD: &'static DeviceChildTable = &DeviceChildTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::Fence> Tables<V> {
    /// `ID3D12Fence`'s table for objects of type `V`.
    pub(crate) const FENCE: &'static FenceTable = &FenceTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        fence_get_completed_value: fence_get_completed_value::<V>,
        fence_set_event_on_completion: fence_set_event_on_completion::<V>,
        fence_signal: fence_signal_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::GraphicsCommandList> Tables<V> {
    /// `ID3D12GraphicsCommandList`'s table for objects of type `V`.
    pub(crate) const GRAPHICS_COMMAND_LIST: &'static GraphicsCommandListTable =
        &GraphicsCommandListTable {
            unknown: unknown_table::<V>(),
            object_get_private_data: object_get_private_data_refused,
            object_set_private_data: object_set_private_data_refused,
            object_set_private_data_interface: object_set_private_data_interface_refused,
            object_set_name: object_set_name_refused,
            device_child_get_device: device_child_get_device_refused,
            command_list_get_type: command_list_get_type_refused,
            graphics_command_list_close: graphics_command_list_close::<V>,
            graphics_command_list_reset: graphics_command_list_reset::<V>,
            graphics_command_list_clear_state: graphics_command_list_clear_state_refused,
            graphics_command_list_draw_instanced: graphics_command_list_draw_instanced::<V>,
            graphics_command_list_draw_indexed_instanced:
                graphics_command_list_draw_indexed_instanced::<V>,
            graphics_command_list_dispatch: graphics_command_list_dispatch::<V>,
            graphics_command_list_copy_buffer_region: graphics_command_list_copy_buffer_region::<V>,
            graphics_command_list_copy_texture_region:
                graphics_command_list_copy_texture_region_refused,
            graphics_command_list_copy_resource: graphics_command_list_copy_resource_refused,
            graphics_command_list_copy_tiles: graphics_command_list_copy_tiles_refused,
            graphics_command_list_resolve_subresource:
                graphics_command_list_resolve_subresource_refused,
            graphics_command_list_ia_set_primitive_topology:
                graphics_command_list_ia_set_primitive_topology::<V>,
            graphics_command_list_rs_set_viewports: graphics_command_list_rs_set_viewports_refused,
            graphics_command_list_rs_set_scissor_rects:
                graphics_command_list_rs_set_scissor_rects_refused,
            graphics_command_list_om_set_blend_factor:
                graphics_command_list_om_set_blend_factor_refused,
            graphics_command_list_om_set_stencil_ref:
                graphics_command_list_om_set_stencil_ref_refused,
            graphics_command_list_set_pipeline_state: graphics_command_list_set_pipeline_state::<V>,
            graphics_command_list_resource_barrier: graphics_command_list_resource_barrier_refused,
            graphics_command_list_execute_bundle: graphics_command_list_execute_bundle_refused,
            graphics_command_list_set_descriptor_heaps: graphics_command_list_set_descriptor_heaps::<
                V,
            >,
            graphics_command_list_set_compute_root_signature:
                graphics_command_list_set_compute_root_signature::<V>,
            graphics_command_list_set_graphics_root_signature:
                graphics_command_list_set_graphics_root_signature::<V>,
            graphics_command_list_set_compute_root_descriptor_table:
                graphics_command_list_set_compute_root_descriptor_table_refused,
            graphics_command_list_set_graphics_root_descriptor_table:
                graphics_command_list_set_graphics_root_descriptor_table_refused,
            graphics_command_list_set_compute_root32_bit_constant:
                graphics_command_list_set_compute_root32_bit_constant_refused,
            graphics_command_list_set_graphics_root32_bit_constant:
                graphics_command_list_set_graphics_root32_bit_constant_refused,
            graphics_command_list_set_compute_root32_bit_constants:
                graphics_command_list_set_compute_root32_bit_constants_refused,
            graphics_command_list_set_graphics_root32_bit_constants:
                graphics_command_list_set_graphics_root32_bit_constants_refused,
            graphics_command_list_set_compute_root_constant_buffer_view:
                graphics_command_list_set_compute_root_constant_buffer_view::<V>,
            graphics_command_list_set_graphics_root_constant_buffer_view:
                graphics_command_list_set_graphics_root_constant_buffer_view::<V>,
            graphics_command_list_set_compute_root_shader_resource_view:
                graphics_command_list_set_compute_root_shader_resource_view_refused,
            graphics_command_list_set_graphics_root_shader_resource_view:
                graphics_command_list_set_graphics_root_shader_resource_view_refused,
            graphics_command_list_set_compute_root_unordered_access_view:
                graphics_command_list_set_compute_root_unordered_access_view::<V>,
            graphics_command_list_set_graphics_root_unordered_access_view:
                graphics_command_list_set_graphics_root_unordered_access_view_refused,
            graphics_command_list_ia_set_index_buffer: graphics_command_list_ia_set_index_buffer::<V>,
            graphics_command_list_ia_set_vertex_buffers:
                graphics_command_list_ia_set_vertex_buffers::<V>,
            graphics_command_list_so_set_targets: graphics_command_list_so_set_targets::<V>,
            graphics_command_list_om_set_render_targets:
                graphics_command_list_om_set_render_targets::<V>,
            graphics_command_list_clear_depth_stencil_view:
                graphics_command_list_clear_depth_stencil_view_refused,
            graphics_command_list_clear_render_target_view:
                graphics_command_list_clear_render_target_view_refused,
            graphics_command_list_clear_unordered_access_view_uint:
                graphics_command_list_clear_unordered_access_view_uint_refused,
            graphics_command_list_clear_unordered_access_view_float:
                graphics_command_list_clear_unordered_access_view_float_refused,
            graphics_command_list_discard_resource: graphics_command_list_discard_resource_refused,
            graphics_command_list_begin_query: graphics_command_list_begin_query::<V>,
            graphics_command_list_end_query: graphics_command_list_end_query::<V>,
            graphics_command_list_resolve_query_data: graphics_command_list_resolve_query_data::<V>,
            graphics_command_list_set_predication: graphics_command_list_set_predication_refused,
            graphics_command_list_set_marker: graphics_command_list_set_marker_refused,
            graphics_command_list_begin_event: graphics_command_list_begin_event_refused,
            graphics_command_list_end_event: graphics_command_list_end_event_refused,
            graphics_command_list_execute_indirect: graphics_command_list_execute_indirect_refused,
        };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::Object> Tables<V> {
    /// `ID3D12Object`'s table for objects of type `V`.
    pub const OBJECT: &'static ObjectTable = &ObjectTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::Pageable> Tables<V> {
    /// `ID3D12Pageable`'s table for objects of type `V`.
    pub const PAGEABLE: &'static PageableTable = &PageableTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::PipelineState> Tables<V> {
    /// `ID3D12PipelineState`'s table for objects of type `V`.
    pub(crate) const PIPELINE_STATE: &'static PipelineStateTable = &PipelineStateTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        pipeline_state_get_cached_blob: pipeline_state_get_cached_blob_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::QueryHeap> Tables<V> {
    /// `ID3D12QueryHeap`'s table for objects of type `V`.
    pub(crate) const QUERY_HEAP: &'static QueryHeapTable = &QueryHeapTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::Resource> Tables<V> {
    /// `ID3D12Resource`'s table for objects of type `V`.
    pub(crate) const RESOURCE: &'static ResourceTable = &ResourceTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
        resource_map: resource_map_refused,
        resource_unmap: resource_unmap::<V>,
        resource_get_desc: resource_get_desc_refused,
        resource_get_gpu_virtual_address: resource_get_gpu_virtual_address::<V>,
        resource_write_to_subresource: resource_write_to_subresource_refused,
        resource_read_from_subresource: resource_read_from_subresource_refused,
        resource_get_heap_properties: resource_get_heap_properties_refused,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_12::RootSignature> Tables<V> {
    /// `ID3D12RootSignature`'s table for objects of type `V`.
    pub(crate) const ROOT_SIGNATURE: &'static RootSignatureTable = &RootSignatureTable {
        unknown: unknown_table::<V>(),
        object_get_private_data: object_get_private_data_refused,
        object_set_private_data: object_set_private_data_refused,
        object_set_private_data_interface: object_set_private_data_interface_refused,
        object_set_name: object_set_name_refused,
        device_child_get_device: device_child_get_device_refused,
    };
}

/// The tables an object of the kind `CommandAllocator` answers, by interface; IUnknown's is the first.
pub fn command_allocator_interfaces<V: direct3d_12::CommandAllocator>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `CommandAllocatorTable` lays out `ID3D12CommandAllocator`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::COMMAND_ALLOCATOR) });
    }
    None
}

/// The tables an object of the kind `CommandQueue` answers, by interface; IUnknown's is the first.
pub fn command_queue_interfaces<V: direct3d_12::CommandQueue>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `CommandQueueTable` lays out `ID3D12CommandQueue`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::COMMAND_QUEUE) });
    }
    None
}

/// The tables an object of the kind `DescriptorHeap` answers, by interface; IUnknown's is the first.
pub fn descriptor_heap_interfaces<V: direct3d_12::DescriptorHeap>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `DescriptorHeapTable` lays out `ID3D12DescriptorHeap`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DESCRIPTOR_HEAP) });
    }
    None
}

/// The tables an object of the kind `Device` answers, by interface; IUnknown's is the first.
pub fn device_interfaces<V: direct3d_12::Device>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_DEVICE,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `DeviceTable` lays out `ID3D12Device`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DEVICE) });
    }
    None
}

/// The tables an object of the kind `Fence` answers, by interface; IUnknown's is the first.
pub fn fence_interfaces<V: direct3d_12::Fence>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_FENCE,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `FenceTable` lays out `ID3D12Fence`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::FENCE) });
    }
    None
}

/// The tables an object of the kind `GraphicsCommandList` answers, by interface; IUnknown's is the first.
pub fn graphics_command_list_interfaces<V: direct3d_12::GraphicsCommandList>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
            direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `GraphicsCommandListTable` lays out `ID3D12GraphicsCommandList`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::GRAPHICS_COMMAND_LIST) });
    }
    None
}

/// The tables an object of the kind `PipelineState` answers, by interface; IUnknown's is the first.
pub fn pipeline_state_interfaces<V: direct3d_12::PipelineState>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `PipelineStateTable` lays out `ID3D12PipelineState`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::PIPELINE_STATE) });
    }
    None
}

/// The tables an object of the kind `QueryHeap` answers, by interface; IUnknown's is the first.
pub fn query_heap_interfaces<V: direct3d_12::QueryHeap>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `QueryHeapTable` lays out `ID3D12QueryHeap`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::QUERY_HEAP) });
    }
    None
}

/// The tables an object of the kind `Resource` answers, by interface; IUnknown's is the first.
pub fn resource_interfaces<V: direct3d_12::Resource>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_RESOURCE,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `ResourceTable` lays out `ID3D12Resource`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::RESOURCE) });
    }
    None
}

/// The tables an object of the kind `RootSignature` answers, by interface; IUnknown's is the first.
pub fn root_signature_interfaces<V: direct3d_12::RootSignature>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [
            direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `RootSignatureTable` lays out `ID3D12RootSignature`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::ROOT_SIGNATURE) });
    }
    None
}

/// A `ID3D12CommandList*` the C side passed, found as one of our objects and held for the call.
pub enum CommandListRefFound<A: direct3d_12::Direct3D12> {
    GraphicsCommandList(Found<<A as direct3d_12::Direct3D12>::GraphicsCommandList>),
}

impl<A: direct3d_12::Direct3D12> CommandListRefFound<A> {
    /// Finds the object at this address, of any of the kinds, or none.
    pub fn find(address: usize) -> Option<CommandListRefFound<A>> {
        if let Some(found) =
            object::<<A as direct3d_12::Direct3D12>::GraphicsCommandList>(KEY, address)
        {
            return Some(CommandListRefFound::GraphicsCommandList(found));
        }
        None
    }

    /// The object as the trait takes it.
    pub fn get(&self) -> direct3d_12::CommandListRef<'_, A> {
        match self {
            CommandListRefFound::GraphicsCommandList(found) => {
                direct3d_12::CommandListRef::GraphicsCommandList(found.value())
            }
        }
    }

    /// The part of what the glue recorded about the object that is of type `R`, if any.
    pub fn part<R: Any>(&self) -> Option<&R> {
        match self {
            CommandListRefFound::GraphicsCommandList(found) => found.part::<R>(),
        }
    }
}

/// An `IUnknown*` the C side passed: one of our objects of a kind of this API, held for the call, or any other.
pub enum UnknownFound<A: direct3d_12::Direct3D12> {
    CommandAllocator(Found<<A as direct3d_12::Direct3D12>::CommandAllocator>),
    CommandQueue(Found<<A as direct3d_12::Direct3D12>::CommandQueue>),
    DescriptorHeap(Found<<A as direct3d_12::Direct3D12>::DescriptorHeap>),
    Device(Found<<A as direct3d_12::Direct3D12>::Device>),
    Fence(Found<<A as direct3d_12::Direct3D12>::Fence>),
    GraphicsCommandList(Found<<A as direct3d_12::Direct3D12>::GraphicsCommandList>),
    PipelineState(Found<<A as direct3d_12::Direct3D12>::PipelineState>),
    QueryHeap(Found<<A as direct3d_12::Direct3D12>::QueryHeap>),
    Resource(Found<<A as direct3d_12::Direct3D12>::Resource>),
    RootSignature(Found<<A as direct3d_12::Direct3D12>::RootSignature>),
    Unknown(Foreign),
}

impl<A: direct3d_12::Direct3D12> UnknownFound<A> {
    /// Finds the object at this address: one of ours, else the C side's, of which it takes a reference; none for null.
    ///
    /// # Safety
    /// `raw` is null or a live COM object, for the call.
    #[expect(unsafe_code, reason = "an IUnknown* the C side passes in")]
    pub unsafe fn find(raw: Raw) -> Option<UnknownFound<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            let address = raw.addr();
            if let Some(found) =
                object::<<A as direct3d_12::Direct3D12>::CommandAllocator>(KEY, address)
            {
                return Some(UnknownFound::CommandAllocator(found));
            }
            if let Some(found) =
                object::<<A as direct3d_12::Direct3D12>::CommandQueue>(KEY, address)
            {
                return Some(UnknownFound::CommandQueue(found));
            }
            if let Some(found) =
                object::<<A as direct3d_12::Direct3D12>::DescriptorHeap>(KEY, address)
            {
                return Some(UnknownFound::DescriptorHeap(found));
            }
            if let Some(found) = object::<<A as direct3d_12::Direct3D12>::Device>(KEY, address) {
                return Some(UnknownFound::Device(found));
            }
            if let Some(found) = object::<<A as direct3d_12::Direct3D12>::Fence>(KEY, address) {
                return Some(UnknownFound::Fence(found));
            }
            if let Some(found) =
                object::<<A as direct3d_12::Direct3D12>::GraphicsCommandList>(KEY, address)
            {
                return Some(UnknownFound::GraphicsCommandList(found));
            }
            if let Some(found) =
                object::<<A as direct3d_12::Direct3D12>::PipelineState>(KEY, address)
            {
                return Some(UnknownFound::PipelineState(found));
            }
            if let Some(found) = object::<<A as direct3d_12::Direct3D12>::QueryHeap>(KEY, address) {
                return Some(UnknownFound::QueryHeap(found));
            }
            if let Some(found) = object::<<A as direct3d_12::Direct3D12>::Resource>(KEY, address) {
                return Some(UnknownFound::Resource(found));
            }
            if let Some(found) =
                object::<<A as direct3d_12::Direct3D12>::RootSignature>(KEY, address)
            {
                return Some(UnknownFound::RootSignature(found));
            }
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
    pub fn get(&self) -> direct3d_12::UnknownRef<'_, A> {
        match self {
            UnknownFound::CommandAllocator(found) => {
                direct3d_12::UnknownRef::CommandAllocator(found.value())
            }
            UnknownFound::CommandQueue(found) => {
                direct3d_12::UnknownRef::CommandQueue(found.value())
            }
            UnknownFound::DescriptorHeap(found) => {
                direct3d_12::UnknownRef::DescriptorHeap(found.value())
            }
            UnknownFound::Device(found) => direct3d_12::UnknownRef::Device(found.value()),
            UnknownFound::Fence(found) => direct3d_12::UnknownRef::Fence(found.value()),
            UnknownFound::GraphicsCommandList(found) => {
                direct3d_12::UnknownRef::GraphicsCommandList(found.value())
            }
            UnknownFound::PipelineState(found) => {
                direct3d_12::UnknownRef::PipelineState(found.value())
            }
            UnknownFound::QueryHeap(found) => direct3d_12::UnknownRef::QueryHeap(found.value()),
            UnknownFound::Resource(found) => direct3d_12::UnknownRef::Resource(found.value()),
            UnknownFound::RootSignature(found) => {
                direct3d_12::UnknownRef::RootSignature(found.value())
            }
            UnknownFound::Unknown(foreign) => direct3d_12::UnknownRef::Unknown(foreign),
        }
    }
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
pub fn exports<M: direct3d_12::Direct3D12>(module: M) -> Library {
    {
        let mut modules = MODULES.lock().unwrap_or_else(PoisonError::into_inner);
        if !modules.iter().any(|held| held.is::<M>()) {
            modules.push(Arc::new(module));
        }
    }
    Library {
        d_3d12_create_device: export_d_3d12_create_device::<M>,
    }
}

/// The calling glue over the serving glue's own exports of `module`: `module` reached through both glues, as if
/// it were the system's library.
pub fn served<M: direct3d_12::Direct3D12>(module: M) -> crate::SystemDirect3D12 {
    let library = exports(module);
    #[expect(
        unsafe_code,
        reason = "our own serving glue's exports, which take what the metadata gives"
    )]
    // SAFETY: each entry of `library` is this crate's serving export of that name, written from the same metadata.
    unsafe {
        crate::SystemDirect3D12::new(library)
    }
}

/// `ID3D12CommandAllocator::Reset` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12CommandAllocator::Reset, which the C side calls"
)]
pub(crate) unsafe extern "system" fn command_allocator_reset<V: direct3d_12::CommandAllocator>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let () = found.value().reset()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12CommandList::GetType` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers zero.
pub(crate) extern "system" fn command_list_get_type_refused(_this: Raw) -> i32 {
    Default::default()
}

/// `ID3D12CommandQueue::Signal` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12CommandQueue::Signal, which the C side calls"
)]
pub(crate) unsafe extern "system" fn command_queue_signal<V: direct3d_12::CommandQueue>(
    this: Raw,
    fence: Raw,
    value: u64,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let fence_found =
                object::<<V::Api as direct3d_12::Direct3D12>::Fence>(KEY, fence.addr())
                    .ok_or(E_INVALIDARG)?;
            let value = value;
            let () = found.value().signal(fence_found.value(), value)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12CommandQueue::UpdateTileMappings` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn command_queue_update_tile_mappings_refused(
    _this: Raw,
    _p0: Raw,
    _p1: u32,
    _p2: *mut u8,
    _p3: *mut u8,
    _p4: Raw,
    _p5: u32,
    _p6: *mut u8,
    _p7: *mut u8,
    _p8: *mut u8,
    _p9: i32,
) {
}

/// `ID3D12CommandQueue::CopyTileMappings` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn command_queue_copy_tile_mappings_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
    _p2: Raw,
    _p3: *mut u8,
    _p4: *mut u8,
    _p5: i32,
) {
}

/// `ID3D12CommandQueue::ExecuteCommandLists` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn command_queue_execute_command_lists_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
) {
}

/// `ID3D12CommandQueue::SetMarker` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn command_queue_set_marker_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    _p2: u32,
) {
}

/// `ID3D12CommandQueue::BeginEvent` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn command_queue_begin_event_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    _p2: u32,
) {
}

/// `ID3D12CommandQueue::EndEvent` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn command_queue_end_event_refused(_this: Raw) {}

/// `ID3D12CommandQueue::Wait` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn command_queue_wait_refused(_this: Raw, _p0: Raw, _p1: u64) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12CommandQueue::GetTimestampFrequency` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12CommandQueue::GetTimestampFrequency, which the C side calls"
)]
pub(crate) unsafe extern "system" fn command_queue_get_timestamp_frequency_refused(
    _this: Raw,
    p0: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12CommandQueue::GetTimestampFrequency: pFrequency, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pFrequency` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 8]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12CommandQueue::GetClockCalibration` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12CommandQueue::GetClockCalibration, which the C side calls"
)]
pub(crate) unsafe extern "system" fn command_queue_get_clock_calibration_refused(
    _this: Raw,
    p0: *mut u8,
    p1: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12CommandQueue::GetClockCalibration: pGpuTimestamp, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pGpuTimestamp` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 8]) };
    #[expect(
        unsafe_code,
        reason = "ID3D12CommandQueue::GetClockCalibration: pCpuTimestamp, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pCpuTimestamp` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p1, &[u8::MIN; 8]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12CommandQueue::GetDesc` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 16 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12CommandQueue::GetDesc, which the C side calls"
)]
pub(crate) unsafe extern "system" fn command_queue_get_desc_refused(
    _this: Raw,
    p0: *mut u8,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12CommandQueue::GetDesc: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 16]) };
    p0
}

/// `ID3D12DescriptorHeap::GetDesc` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 16 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12DescriptorHeap::GetDesc, which the C side calls"
)]
pub(crate) unsafe extern "system" fn descriptor_heap_get_desc_refused(
    _this: Raw,
    p0: *mut u8,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12DescriptorHeap::GetDesc: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 16]) };
    p0
}

/// `ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 8 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart, which the C side calls"
)]
pub(crate) unsafe extern "system" fn descriptor_heap_get_cpu_descriptor_handle_for_heap_start_refused(
    _this: Raw,
    p0: *mut u8,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 8]) };
    p0
}

/// `ID3D12DescriptorHeap::GetGPUDescriptorHandleForHeapStart` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 8 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12DescriptorHeap::GetGPUDescriptorHandleForHeapStart, which the C side calls"
)]
pub(crate) unsafe extern "system" fn descriptor_heap_get_gpu_descriptor_handle_for_heap_start_refused(
    _this: Raw,
    p0: *mut u8,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12DescriptorHeap::GetGPUDescriptorHandleForHeapStart: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 8]) };
    p0
}

/// `ID3D12Device::CreateCommandQueue` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateCommandQueue, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_command_queue<V: direct3d_12::Device>(
    this: Raw,
    desc: *const u8,
    riid: *const u8,
    command_queue: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if command_queue.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommandQueue: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `CommandQueueDesc` of 16 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&desc_bytes)
            });
            let desc = desc.ok_or(E_INVALIDARG)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommandQueue: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let command_queue_value = found.value().create_command_queue(&desc, &riid)?;
            drop(found);
            let command_queue_answer = match command_queue_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if command_queue.is_null() {
                0
            } else {
                command_queue_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let command_queue_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandQueue: ppCommandQueue, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppCommandQueue` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(command_queue.cast::<u8>(), &command_queue_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::CreateCommandAllocator` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateCommandAllocator, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_command_allocator<V: direct3d_12::Device>(
    this: Raw,
    type_: i32,
    riid: *const u8,
    command_allocator: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if command_allocator.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let type_ = direct3d_12::enumerations::CommandListType(type_);
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommandAllocator: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let command_allocator_value = found.value().create_command_allocator(type_, &riid)?;
            drop(found);
            let command_allocator_answer = match command_allocator_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if command_allocator.is_null() {
                0
            } else {
                command_allocator_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let command_allocator_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandAllocator: ppCommandAllocator, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppCommandAllocator` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(command_allocator.cast::<u8>(), &command_allocator_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::CreateCommandList` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateCommandList, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_command_list<V: direct3d_12::Device>(
    this: Raw,
    node_mask: u32,
    type_: i32,
    command_allocator: Raw,
    initial_state: Raw,
    riid: *const u8,
    command_list: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if command_list.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let node_mask = node_mask;
            let type_ = direct3d_12::enumerations::CommandListType(type_);
            let command_allocator_found = object::<
                <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
            >(KEY, command_allocator.addr())
            .ok_or(E_INVALIDARG)?;
            let initial_state_found = if initial_state.is_null() {
                None
            } else {
                Some(
                    object::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        initial_state.addr(),
                    )
                    .ok_or(E_INVALIDARG)?,
                )
            };
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommandList: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let command_list_value = found.value().create_command_list(
                node_mask,
                type_,
                command_allocator_found.value(),
                initial_state_found.as_ref().map(|held| held.value()),
                &riid,
            )?;
            drop(found);
            let command_list_answer = match command_list_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if command_list.is_null() {
                0
            } else {
                command_list_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let command_list_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandList: ppCommandList, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppCommandList` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(command_list.cast::<u8>(), &command_list_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::CreateDescriptorHeap` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateDescriptorHeap, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_descriptor_heap<V: direct3d_12::Device>(
    this: Raw,
    descriptor_heap_desc: *const u8,
    riid: *const u8,
    ppv_heap: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if ppv_heap.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut descriptor_heap_desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout>()];
            let descriptor_heap_desc_null = descriptor_heap_desc.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateDescriptorHeap: pDescriptorHeapDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDescriptorHeapDesc` is `In` and `Const`: one `DescriptorHeapDesc` of 16 bytes, readable for the call.
            let descriptor_heap_desc_read =
                unsafe { read_bytes(descriptor_heap_desc, &mut descriptor_heap_desc_bytes) };
            let descriptor_heap_desc = descriptor_heap_desc_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
                    &descriptor_heap_desc_bytes,
                )
            });
            let descriptor_heap_desc = descriptor_heap_desc.ok_or(E_INVALIDARG)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateDescriptorHeap: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let ppv_heap_value = found
                .value()
                .create_descriptor_heap(&descriptor_heap_desc, &riid)?;
            drop(found);
            let ppv_heap_answer = match ppv_heap_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if ppv_heap.is_null() {
                0
            } else {
                ppv_heap_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let ppv_heap_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateDescriptorHeap: ppvHeap, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppvHeap` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(ppv_heap.cast::<u8>(), &ppv_heap_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::GetDescriptorHandleIncrementSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::GetDescriptorHandleIncrementSize, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_get_descriptor_handle_increment_size<
    V: direct3d_12::Device,
>(
    this: Raw,
    descriptor_heap_type: i32,
) -> u32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let descriptor_heap_type =
                direct3d_12::enumerations::DescriptorHeapType(descriptor_heap_type);
            let returned = found
                .value()
                .get_descriptor_handle_increment_size(descriptor_heap_type);
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `ID3D12Device::CreateRootSignature` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateRootSignature, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_root_signature<V: direct3d_12::Device>(
    this: Raw,
    node_mask: u32,
    blob_with_root_signature: *const u8,
    blob_length_in_bytes: usize,
    riid: *const u8,
    ppv_root_signature: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if ppv_root_signature.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let node_mask = node_mask;
            let blob_with_root_signature_count = Some(blob_length_in_bytes)
                .filter(|count| *count <= 1 << 16)
                .ok_or(E_INVALIDARG)?;
            let blob_with_root_signature_bytes = if blob_with_root_signature_count == 0 {
                Some(Vec::new())
            } else {
                #[expect(
                    unsafe_code,
                    reason = "ID3D12Device::CreateRootSignature: pBlobWithRootSignature, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `blobLengthInBytes` as the count of `pBlobWithRootSignature`: that many elements of 1 bytes, readable for the call.
                unsafe {
                    read_only_at(blob_with_root_signature, blob_with_root_signature_count)
                }
            };
            let blob_with_root_signature: Vec<_> = blob_with_root_signature_bytes
                .ok_or(E_INVALIDARG)?
                .as_chunks::<1>()
                .0
                .iter()
                .map(|element| get_u8(element, usize::MIN))
                .collect();
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateRootSignature: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let ppv_root_signature_value =
                found
                    .value()
                    .create_root_signature(node_mask, &blob_with_root_signature, &riid)?;
            drop(found);
            let ppv_root_signature_answer = match ppv_root_signature_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if ppv_root_signature.is_null() {
                0
            } else {
                ppv_root_signature_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let ppv_root_signature_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateRootSignature: ppvRootSignature, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppvRootSignature` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ =
            unsafe { write_bytes(ppv_root_signature.cast::<u8>(), &ppv_root_signature_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::CreateCommittedResource` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateCommittedResource, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_committed_resource<V: direct3d_12::Device>(
    this: Raw,
    heap_properties: *const u8,
    heap_flags: i32,
    desc: *const u8,
    initial_resource_state: i32,
    optimized_clear_value: *const u8,
    riid_resource: *const u8,
    ppv_resource: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut heap_properties_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout>()];
            let heap_properties_null = heap_properties.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommittedResource: pHeapProperties, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pHeapProperties` is `In` and `Const`: one `HeapProperties` of 20 bytes, readable for the call.
            let heap_properties_read =
                unsafe { read_bytes(heap_properties, &mut heap_properties_bytes) };
            let heap_properties = heap_properties_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(
                    &heap_properties_bytes,
                )
            });
            let heap_properties = heap_properties.ok_or(E_INVALIDARG)?;
            let heap_flags = direct3d_12::enumerations::HeapFlags(heap_flags);
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommittedResource: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `ResourceDesc` of 56 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&desc_bytes)
            });
            let desc = desc.ok_or(E_INVALIDARG)?;
            let initial_resource_state =
                direct3d_12::enumerations::ResourceStates(initial_resource_state);
            let mut optimized_clear_value_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout>()];
            let optimized_clear_value_null = optimized_clear_value.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommittedResource: pOptimizedClearValue, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pOptimizedClearValue` is `In` and `Const`: one `ClearValue` of 20 bytes, readable for the call.
            let optimized_clear_value_read =
                unsafe { read_bytes(optimized_clear_value, &mut optimized_clear_value_bytes) };
            let optimized_clear_value = optimized_clear_value_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_clear_value(
                    &optimized_clear_value_bytes,
                )
            });
            if !optimized_clear_value_read && !optimized_clear_value_null {
                return Err(E_INVALIDARG);
            }
            let mut riid_resource_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_resource_null = riid_resource.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateCommittedResource: riidResource, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riidResource` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_resource_read = unsafe { read_bytes(riid_resource, &mut riid_resource_bytes) };
            let riid_resource = riid_resource_read.then(|| decode_guid(&riid_resource_bytes));
            let riid_resource = riid_resource.ok_or(E_INVALIDARG)?;
            let ppv_resource_value = found.value().create_committed_resource(
                &heap_properties,
                heap_flags,
                &desc,
                initial_resource_state,
                optimized_clear_value.as_ref(),
                &riid_resource,
            )?;
            drop(found);
            let ppv_resource_answer = match ppv_resource_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid_resource,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if ppv_resource.is_null() {
                0
            } else {
                ppv_resource_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let ppv_resource_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommittedResource: ppvResource, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppvResource` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(ppv_resource.cast::<u8>(), &ppv_resource_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::CreateFence` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateFence, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_fence<V: direct3d_12::Device>(
    this: Raw,
    initial_value: u64,
    flags: i32,
    riid: *const u8,
    fence: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            if fence.is_null() {
                return Err(E_POINTER);
            }
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let initial_value = initial_value;
            let flags = direct3d_12::enumerations::FenceFlags(flags);
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateFence: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let fence_value = found.value().create_fence(initial_value, flags, &riid)?;
            drop(found);
            let fence_answer = match fence_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if fence.is_null() {
                0
            } else {
                fence_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let fence_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateFence: ppFence, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppFence` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(fence.cast::<u8>(), &fence_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::GetCopyableFootprints` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::GetCopyableFootprints, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_get_copyable_footprints<V: direct3d_12::Device>(
    this: Raw,
    resource_desc: *const u8,
    first_subresource: u32,
    num_subresources: u32,
    base_offset: u64,
    layouts: *mut u8,
    num_rows: *mut u8,
    row_size_in_bytes: *mut u8,
    total_bytes: *mut u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let layouts_length: Option<usize> = usize::try_from(num_subresources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 32);
        let num_rows_length: Option<usize> = usize::try_from(num_subresources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 4);
        let row_size_in_bytes_length: Option<usize> = usize::try_from(num_subresources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 8);
        let answer = guarded(None, move || {
            let layouts_length = layouts_length?;
            let num_rows_length = num_rows_length?;
            let row_size_in_bytes_length = row_size_in_bytes_length?;
            let found = object::<V>(KEY, this.addr())?;
            let mut resource_desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>()];
            let resource_desc_null = resource_desc.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::GetCopyableFootprints: pResourceDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pResourceDesc` is `In` and `Const`: one `ResourceDesc` of 56 bytes, readable for the call.
            let resource_desc_read = unsafe { read_bytes(resource_desc, &mut resource_desc_bytes) };
            let resource_desc = resource_desc_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&resource_desc_bytes)
            });
            let resource_desc = resource_desc?;
            let first_subresource = first_subresource;
            let num_subresources = num_subresources;
            let base_offset = base_offset;
            let (layouts_value, num_rows_value, row_size_in_bytes_value, total_bytes_value) =
                found.value().get_copyable_footprints(
                    &resource_desc,
                    first_subresource,
                    num_subresources,
                    base_offset,
                );
            drop(found);
            let mut layouts_bytes = vec![u8::MIN; layouts_length];
            for (value, bytes) in layouts_value
                .iter()
                .zip(layouts_bytes.as_chunks_mut::<32>().0.iter_mut())
            {
                encode_windows_win32_graphics_direct_3d12_d_3d12_placed_subresource_footprint(
                    value, bytes,
                );
            }
            let mut num_rows_bytes = vec![u8::MIN; num_rows_length];
            for (value, bytes) in num_rows_value
                .iter()
                .zip(num_rows_bytes.as_chunks_mut::<4>().0.iter_mut())
            {
                put_u32(bytes, usize::MIN, *value);
            }
            let mut row_size_in_bytes_bytes = vec![u8::MIN; row_size_in_bytes_length];
            for (value, bytes) in row_size_in_bytes_value
                .iter()
                .zip(row_size_in_bytes_bytes.as_chunks_mut::<8>().0.iter_mut())
            {
                put_u64(bytes, usize::MIN, *value);
            }
            let mut total_bytes_bytes = [u8::MIN; 8];
            put_u64(&mut total_bytes_bytes, usize::MIN, total_bytes_value);
            Some((
                layouts_bytes,
                num_rows_bytes,
                row_size_in_bytes_bytes,
                total_bytes_bytes,
            ))
        });
        if let Some(length) = layouts_length {
            let layouts_written = answer
                .as_ref()
                .map_or_else(|| vec![u8::MIN; length], |written| written.0.clone());
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::GetCopyableFootprints: pLayouts, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `NumSubresources` as the count of `pLayouts`, which is `Out`: that many elements of 32 bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(layouts.cast::<u8>(), &layouts_written) };
        }
        if let Some(length) = num_rows_length {
            let num_rows_written = answer
                .as_ref()
                .map_or_else(|| vec![u8::MIN; length], |written| written.1.clone());
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::GetCopyableFootprints: pNumRows, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `NumSubresources` as the count of `pNumRows`, which is `Out`: that many elements of 4 bytes (`length`), writable for the call, or null.
            let _ = unsafe { write_bytes(num_rows.cast::<u8>(), &num_rows_written) };
        }
        if let Some(length) = row_size_in_bytes_length {
            let row_size_in_bytes_written = answer
                .as_ref()
                .map_or_else(|| vec![u8::MIN; length], |written| written.2.clone());
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::GetCopyableFootprints: pRowSizeInBytes, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `NativeArrayInfo` names `NumSubresources` as the count of `pRowSizeInBytes`, which is `Out`: that many elements of 8 bytes (`length`), writable for the call, or null.
            let _ =
                unsafe { write_bytes(row_size_in_bytes.cast::<u8>(), &row_size_in_bytes_written) };
        }
        let total_bytes_written = answer
            .as_ref()
            .as_ref()
            .map_or([u8::MIN; 8], |written| written.3);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::GetCopyableFootprints: pTotalBytes, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `pTotalBytes` is `Out`: one value of this size to write, or null, for the call.
        let _ = unsafe { write_bytes(total_bytes.cast::<u8>(), &total_bytes_written) };
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12Device::CreateQueryHeap` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateQueryHeap, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_query_heap<V: direct3d_12::Device>(
    this: Raw,
    desc: *const u8,
    riid: *const u8,
    ppv_heap: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let mut desc_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout>()];
            let desc_null = desc.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateQueryHeap: pDesc, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDesc` is `In` and `Const`: one `QueryHeapDesc` of 12 bytes, readable for the call.
            let desc_read = unsafe { read_bytes(desc, &mut desc_bytes) };
            let desc = desc_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&desc_bytes)
            });
            let desc = desc.ok_or(E_INVALIDARG)?;
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Device::CreateQueryHeap: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let ppv_heap_value = found.value().create_query_heap(&desc, &riid)?;
            drop(found);
            let ppv_heap_answer = match ppv_heap_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            command_queue_interfaces::<
                                <V::Api as direct3d_12::Direct3D12>::CommandQueue,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        descriptor_heap_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::DescriptorHeap,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<V::Api as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Device>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<V::Api as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Fence>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::PipelineState,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<V::Api as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<
                            <V::Api as direct3d_12::Direct3D12>::RootSignature,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
                    return Err(E_NOINTERFACE);
                }
            };
            Ok(((if ppv_heap.is_null() {
                0
            } else {
                ppv_heap_answer.into_address().get()
            })
            .to_ne_bytes(),))
        });
        let ppv_heap_written = answer
            .as_ref()
            .ok()
            .map_or([u8::MIN; size_of::<usize>()], |written| written.0);
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateQueryHeap: ppvHeap, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppvHeap` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(ppv_heap.cast::<u8>(), &ppv_heap_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Device::GetNodeCount` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers zero.
pub(crate) extern "system" fn device_get_node_count_refused(_this: Raw) -> u32 {
    Default::default()
}

/// `ID3D12Device::CreateGraphicsPipelineState` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateGraphicsPipelineState, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_graphics_pipeline_state_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreateGraphicsPipelineState: ppPipelineState, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppPipelineState` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CreateComputePipelineState` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateComputePipelineState, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_compute_pipeline_state_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreateComputePipelineState: ppPipelineState, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppPipelineState` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CheckFeatureSupport` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn device_check_feature_support_refused(
    _this: Raw,
    _p0: i32,
    _p1: *mut u8,
    _p2: u32,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CreateConstantBufferView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_create_constant_buffer_view_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: u64,
) {
}

/// `ID3D12Device::CreateShaderResourceView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_create_shader_resource_view_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
    _p2: u64,
) {
}

/// `ID3D12Device::CreateUnorderedAccessView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_create_unordered_access_view_refused(
    _this: Raw,
    _p0: Raw,
    _p1: Raw,
    _p2: *mut u8,
    _p3: u64,
) {
}

/// `ID3D12Device::CreateRenderTargetView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_create_render_target_view_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
    _p2: u64,
) {
}

/// `ID3D12Device::CreateDepthStencilView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_create_depth_stencil_view_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
    _p2: u64,
) {
}

/// `ID3D12Device::CreateSampler` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_create_sampler_refused(_this: Raw, _p0: *mut u8, _p1: u64) {}

/// `ID3D12Device::CopyDescriptors` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_copy_descriptors_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    _p2: *mut u8,
    _p3: u32,
    _p4: *mut u8,
    _p5: *mut u8,
    _p6: i32,
) {
}

/// `ID3D12Device::CopyDescriptorsSimple` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn device_copy_descriptors_simple_refused(
    _this: Raw,
    _p0: u32,
    _p1: u64,
    _p2: u64,
    _p3: i32,
) {
}

/// `ID3D12Device::GetResourceAllocationInfo` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 16 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::GetResourceAllocationInfo, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_get_resource_allocation_info_refused(
    _this: Raw,
    p0: *mut u8,
    _p1: u32,
    _p2: u32,
    _p3: *mut u8,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::GetResourceAllocationInfo: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 16]) };
    p0
}

/// `ID3D12Device::GetCustomHeapProperties` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 20 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::GetCustomHeapProperties, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_get_custom_heap_properties_refused(
    _this: Raw,
    p0: *mut u8,
    _p1: u32,
    _p2: i32,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::GetCustomHeapProperties: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 20]) };
    p0
}

/// `ID3D12Device::CreateHeap` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateHeap, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_heap_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreateHeap: ppvHeap, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvHeap` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CreatePlacedResource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreatePlacedResource, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_placed_resource_refused(
    _this: Raw,
    _p0: Raw,
    _p1: u64,
    _p2: *mut u8,
    _p3: i32,
    _p4: *mut u8,
    _p5: *mut u8,
    p6: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreatePlacedResource: ppvResource, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvResource` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p6, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CreateReservedResource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateReservedResource, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_reserved_resource_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: i32,
    _p2: *mut u8,
    _p3: *mut u8,
    p4: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreateReservedResource: ppvResource, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvResource` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p4, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CreateSharedHandle` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateSharedHandle, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_shared_handle_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
    _p2: u32,
    _p3: *const u8,
    p4: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreateSharedHandle: pHandle, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pHandle` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p4, &[u8::MIN; 8]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::OpenSharedHandle` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::OpenSharedHandle, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_open_shared_handle_refused(
    _this: Raw,
    _p0: usize,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::OpenSharedHandle: ppvObj, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvObj` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::OpenSharedHandleByName` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::OpenSharedHandleByName, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_open_shared_handle_by_name_refused(
    _this: Raw,
    _p0: *const u8,
    _p1: u32,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::OpenSharedHandleByName: pNTHandle, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pNTHandle` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; 8]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::MakeResident` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn device_make_resident_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::Evict` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn device_evict_refused(_this: Raw, _p0: u32, _p1: *mut u8) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::GetDeviceRemovedReason` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn device_get_device_removed_reason_refused(_this: Raw) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::SetStablePowerState` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn device_set_stable_power_state_refused(_this: Raw, _p0: i32) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::CreateCommandSignature` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::CreateCommandSignature, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_create_command_signature_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: Raw,
    _p2: *mut u8,
    p3: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::CreateCommandSignature: ppvCommandSignature, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvCommandSignature` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p3, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Device::GetResourceTiling` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::GetResourceTiling, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_get_resource_tiling_refused(
    _this: Raw,
    _p0: Raw,
    p1: *mut u8,
    p2: *mut u8,
    p3: *mut u8,
    _p4: *mut u8,
    _p5: u32,
    _p6: *mut u8,
) {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::GetResourceTiling: pNumTilesForEntireResource, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pNumTilesForEntireResource` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p1, &[u8::MIN; 4]) };
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::GetResourceTiling: pPackedMipDesc, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pPackedMipDesc` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; 12]) };
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::GetResourceTiling: pStandardTileShapeForNonPackedMips, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pStandardTileShapeForNonPackedMips` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p3, &[u8::MIN; 12]) };
}

/// `ID3D12Device::GetAdapterLuid` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 8 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Device::GetAdapterLuid, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_get_adapter_luid_refused(
    _this: Raw,
    p0: *mut u8,
) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Device::GetAdapterLuid: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 8]) };
    p0
}

/// `ID3D12DeviceChild::GetDevice` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12DeviceChild::GetDevice, which the C side calls"
)]
pub(crate) unsafe extern "system" fn device_child_get_device_refused(
    _this: Raw,
    _p0: *mut u8,
    p1: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12DeviceChild::GetDevice: ppvDevice, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppvDevice` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p1, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Fence::GetCompletedValue` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Fence::GetCompletedValue, which the C side calls"
)]
pub(crate) unsafe extern "system" fn fence_get_completed_value<V: direct3d_12::Fence>(
    this: Raw,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_completed_value();
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `ID3D12Fence::SetEventOnCompletion` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Fence::SetEventOnCompletion, which the C side calls"
)]
pub(crate) unsafe extern "system" fn fence_set_event_on_completion<V: direct3d_12::Fence>(
    this: Raw,
    value: u64,
    h_event: usize,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let value = value;
            let h_event = component_object_model::structures::Handle(h_event);
            let () = found.value().set_event_on_completion(value, h_event)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12Fence::Signal` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn fence_signal_refused(_this: Raw, _p0: u64) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12GraphicsCommandList::Close` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::Close, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_close<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let () = found.value().close()?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12GraphicsCommandList::Reset` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::Reset, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_reset<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    allocator: Raw,
    initial_state: Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let allocator_found = object::<<V::Api as direct3d_12::Direct3D12>::CommandAllocator>(
                KEY,
                allocator.addr(),
            )
            .ok_or(E_INVALIDARG)?;
            let initial_state_found = if initial_state.is_null() {
                None
            } else {
                Some(
                    object::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        initial_state.addr(),
                    )
                    .ok_or(E_INVALIDARG)?,
                )
            };
            let () = found.value().reset(
                allocator_found.value(),
                initial_state_found.as_ref().map(|held| held.value()),
            )?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3D12GraphicsCommandList::DrawInstanced` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::DrawInstanced, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_draw_instanced<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    vertex_count_per_instance: u32,
    instance_count: u32,
    start_vertex_location: u32,
    start_instance_location: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let vertex_count_per_instance = vertex_count_per_instance;
            let instance_count = instance_count;
            let start_vertex_location = start_vertex_location;
            let start_instance_location = start_instance_location;
            let () = found.value().draw_instanced(
                vertex_count_per_instance,
                instance_count,
                start_vertex_location,
                start_instance_location,
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::DrawIndexedInstanced` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::DrawIndexedInstanced, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_draw_indexed_instanced<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    index_count_per_instance: u32,
    instance_count: u32,
    start_index_location: u32,
    base_vertex_location: i32,
    start_instance_location: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let index_count_per_instance = index_count_per_instance;
            let instance_count = instance_count;
            let start_index_location = start_index_location;
            let base_vertex_location = base_vertex_location;
            let start_instance_location = start_instance_location;
            let () = found.value().draw_indexed_instanced(
                index_count_per_instance,
                instance_count,
                start_index_location,
                base_vertex_location,
                start_instance_location,
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::Dispatch` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::Dispatch, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_dispatch<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    thread_group_count_x: u32,
    thread_group_count_y: u32,
    thread_group_count_z: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let thread_group_count_x = thread_group_count_x;
            let thread_group_count_y = thread_group_count_y;
            let thread_group_count_z = thread_group_count_z;
            let () = found.value().dispatch(
                thread_group_count_x,
                thread_group_count_y,
                thread_group_count_z,
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::CopyBufferRegion` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::CopyBufferRegion, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_copy_buffer_region<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    dst_buffer: Raw,
    dst_offset: u64,
    src_buffer: Raw,
    src_offset: u64,
    num_bytes: u64,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let dst_buffer_found =
                object::<<V::Api as direct3d_12::Direct3D12>::Resource>(KEY, dst_buffer.addr())?;
            let dst_offset = dst_offset;
            let src_buffer_found =
                object::<<V::Api as direct3d_12::Direct3D12>::Resource>(KEY, src_buffer.addr())?;
            let src_offset = src_offset;
            let num_bytes = num_bytes;
            let () = found.value().copy_buffer_region(
                dst_buffer_found.value(),
                dst_offset,
                src_buffer_found.value(),
                src_offset,
                num_bytes,
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::IASetPrimitiveTopology` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::IASetPrimitiveTopology, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_ia_set_primitive_topology<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    primitive_topology: i32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let primitive_topology =
                direct3d_common::enumerations::PrimitiveTopology(primitive_topology);
            let () = found.value().ia_set_primitive_topology(primitive_topology);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetPipelineState` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetPipelineState, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_pipeline_state<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    pipeline_state: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let pipeline_state_found = object::<<V::Api as direct3d_12::Direct3D12>::PipelineState>(
                KEY,
                pipeline_state.addr(),
            )?;
            let () = found
                .value()
                .set_pipeline_state(pipeline_state_found.value());
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetDescriptorHeaps` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetDescriptorHeaps, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_descriptor_heaps<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    num_descriptor_heaps: u32,
    descriptor_heaps: *const Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let descriptor_heaps_count = usize::try_from(num_descriptor_heaps)
                .ok()
                .filter(|count| *count <= 1 << 16)?;
            let descriptor_heaps_bytes = if descriptor_heaps_count == 0 {
                Some(Vec::new())
            } else if descriptor_heaps.is_null() {
                None
            } else {
                #[expect(
                    unsafe_code,
                    reason = "ID3D12GraphicsCommandList::SetDescriptorHeaps: ppDescriptorHeaps, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumDescriptorHeaps` as the count of `ppDescriptorHeaps`: that many pointers, readable for the call.
                unsafe {
                    read_only_at(
                        descriptor_heaps.cast::<u8>(),
                        descriptor_heaps_count * size_of::<usize>(),
                    )
                }
            };
            let mut descriptor_heaps_found = Vec::new();
            for element in descriptor_heaps_bytes?
                .as_chunks::<{ size_of::<usize>() }>()
                .0
            {
                let address = get_usize(element, usize::MIN);
                descriptor_heaps_found.push(if address == 0 {
                    None
                } else {
                    Some(
                        object::<<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>(
                            KEY, address,
                        )?,
                    )
                });
            }
            let descriptor_heaps: Vec<
                Option<&<V::Api as direct3d_12::Direct3D12>::DescriptorHeap>,
            > = descriptor_heaps_found
                .iter()
                .map(|held| held.as_ref().map(|held| held.value()))
                .collect();
            let () = found.value().set_descriptor_heaps(&descriptor_heaps);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetComputeRootSignature` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetComputeRootSignature, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_compute_root_signature<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    root_signature: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let root_signature_found = if root_signature.is_null() {
                None
            } else {
                Some(
                    object::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        root_signature.addr(),
                    )?,
                )
            };
            let () = found
                .value()
                .set_compute_root_signature(root_signature_found.as_ref().map(|held| held.value()));
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetGraphicsRootSignature` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetGraphicsRootSignature, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_graphics_root_signature<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    root_signature: Raw,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let root_signature_found = if root_signature.is_null() {
                None
            } else {
                Some(
                    object::<<V::Api as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        root_signature.addr(),
                    )?,
                )
            };
            let () = found.value().set_graphics_root_signature(
                root_signature_found.as_ref().map(|held| held.value()),
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetComputeRootConstantBufferView` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetComputeRootConstantBufferView, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_compute_root_constant_buffer_view<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    root_parameter_index: u32,
    buffer_location: u64,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let root_parameter_index = root_parameter_index;
            let buffer_location = buffer_location;
            let () = found
                .value()
                .set_compute_root_constant_buffer_view(root_parameter_index, buffer_location);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetGraphicsRootConstantBufferView` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetGraphicsRootConstantBufferView, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_graphics_root_constant_buffer_view<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    root_parameter_index: u32,
    buffer_location: u64,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let root_parameter_index = root_parameter_index;
            let buffer_location = buffer_location;
            let () = found
                .value()
                .set_graphics_root_constant_buffer_view(root_parameter_index, buffer_location);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SetComputeRootUnorderedAccessView` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SetComputeRootUnorderedAccessView, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_set_compute_root_unordered_access_view<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    root_parameter_index: u32,
    buffer_location: u64,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let root_parameter_index = root_parameter_index;
            let buffer_location = buffer_location;
            let () = found
                .value()
                .set_compute_root_unordered_access_view(root_parameter_index, buffer_location);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::IASetIndexBuffer` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::IASetIndexBuffer, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_ia_set_index_buffer<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    view: *const u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let mut view_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12IndexBufferViewLayout>()];
            let view_null = view.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12GraphicsCommandList::IASetIndexBuffer: pView, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pView` is `In` and `Const`: one `IndexBufferView` of 16 bytes, readable for the call.
            let view_read = unsafe { read_bytes(view, &mut view_bytes) };
            let view = view_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_index_buffer_view(&view_bytes)
            });
            if !view_read && !view_null {
                return None;
            }
            let () = found.value().ia_set_index_buffer(view.as_ref());
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::IASetVertexBuffers` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::IASetVertexBuffers, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_ia_set_vertex_buffers<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    start_slot: u32,
    num_views: u32,
    views: *const u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let start_slot = start_slot;
            let views_count = usize::try_from(num_views)
                .ok()
                .filter(|count| *count <= 1 << 16)?;
            let views_bytes = if views_count == 0 || views.is_null() {
                Some(Vec::new())
            } else {
                #[expect(
                    unsafe_code,
                    reason = "ID3D12GraphicsCommandList::IASetVertexBuffers: pViews, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumViews` as the count of `pViews`: that many elements of 16 bytes, readable for the call.
                unsafe {
                    read_only_at(views, views_count * 16)
                }
            };
            let views: Vec<_> = views_bytes?
                .as_chunks::<16>()
                .0
                .iter()
                .map(|element| {
                    decode_windows_win32_graphics_direct_3d12_d_3d12_vertex_buffer_view(element)
                })
                .collect();
            let () = found.value().ia_set_vertex_buffers(start_slot, &views);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::SOSetTargets` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::SOSetTargets, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_so_set_targets<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    start_slot: u32,
    num_views: u32,
    views: *const u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let start_slot = start_slot;
            let views_count = usize::try_from(num_views)
                .ok()
                .filter(|count| *count <= 1 << 16)?;
            let views_bytes = if views_count == 0 || views.is_null() {
                Some(Vec::new())
            } else {
                #[expect(
                    unsafe_code,
                    reason = "ID3D12GraphicsCommandList::SOSetTargets: pViews, as the header declares it for the C side"
                )]
                // SAFETY: the metadata's `NativeArrayInfo` names `NumViews` as the count of `pViews`: that many elements of 24 bytes, readable for the call.
                unsafe {
                    read_only_at(views, views_count * 24)
                }
            };
            let views: Vec<_> = views_bytes?
                .as_chunks::<24>()
                .0
                .iter()
                .map(|element| {
                    decode_windows_win32_graphics_direct_3d12_d_3d12_stream_output_buffer_view(
                        element,
                    )
                })
                .collect();
            let () = found.value().so_set_targets(start_slot, &views);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::OMSetRenderTargets` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::OMSetRenderTargets, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_om_set_render_targets<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    num_render_target_descriptors: u32,
    render_target_descriptors: *const u8,
    r_ts_single_handle_to_descriptor_range: i32,
    depth_stencil_descriptor: *const u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let num_render_target_descriptors = num_render_target_descriptors;
            let mut render_target_descriptors_bytes = [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12D3d12CpuDescriptorHandleLayout>()];
            let render_target_descriptors_null = render_target_descriptors.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12GraphicsCommandList::OMSetRenderTargets: pRenderTargetDescriptors, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pRenderTargetDescriptors` is `In` and `Const`: one `CpuDescriptorHandle` of 8 bytes, readable for the call.
            let render_target_descriptors_read = unsafe {
                read_bytes(
                    render_target_descriptors,
                    &mut render_target_descriptors_bytes,
                )
            };
            let render_target_descriptors = render_target_descriptors_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
                    &render_target_descriptors_bytes,
                )
            });
            if !render_target_descriptors_read && !render_target_descriptors_null {
                return None;
            }
            let r_ts_single_handle_to_descriptor_range =
                r_ts_single_handle_to_descriptor_range != 0;
            let mut depth_stencil_descriptor_bytes = [u8::MIN;
                size_of::<WindowsWin32GraphicsDirect3d12D3d12CpuDescriptorHandleLayout>()];
            let depth_stencil_descriptor_null = depth_stencil_descriptor.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12GraphicsCommandList::OMSetRenderTargets: pDepthStencilDescriptor, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pDepthStencilDescriptor` is `In` and `Const`: one `CpuDescriptorHandle` of 8 bytes, readable for the call.
            let depth_stencil_descriptor_read = unsafe {
                read_bytes(
                    depth_stencil_descriptor,
                    &mut depth_stencil_descriptor_bytes,
                )
            };
            let depth_stencil_descriptor = depth_stencil_descriptor_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
                    &depth_stencil_descriptor_bytes,
                )
            });
            if !depth_stencil_descriptor_read && !depth_stencil_descriptor_null {
                return None;
            }
            let () = found.value().om_set_render_targets(
                num_render_target_descriptors,
                render_target_descriptors.as_ref(),
                r_ts_single_handle_to_descriptor_range,
                depth_stencil_descriptor.as_ref(),
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::BeginQuery` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::BeginQuery, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_begin_query<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    query_heap: Raw,
    type_: i32,
    index: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let query_heap_found =
                object::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(KEY, query_heap.addr())?;
            let type_ = direct3d_12::enumerations::QueryType(type_);
            let index = index;
            let () = found
                .value()
                .begin_query(query_heap_found.value(), type_, index);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::EndQuery` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::EndQuery, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_end_query<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    query_heap: Raw,
    type_: i32,
    index: u32,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let query_heap_found =
                object::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(KEY, query_heap.addr())?;
            let type_ = direct3d_12::enumerations::QueryType(type_);
            let index = index;
            let () = found
                .value()
                .end_query(query_heap_found.value(), type_, index);
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::ResolveQueryData` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12GraphicsCommandList::ResolveQueryData, which the C side calls"
)]
pub(crate) unsafe extern "system" fn graphics_command_list_resolve_query_data<
    V: direct3d_12::GraphicsCommandList,
>(
    this: Raw,
    query_heap: Raw,
    type_: i32,
    start_index: u32,
    num_queries: u32,
    destination_buffer: Raw,
    aligned_destination_buffer_offset: u64,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let query_heap_found =
                object::<<V::Api as direct3d_12::Direct3D12>::QueryHeap>(KEY, query_heap.addr())?;
            let type_ = direct3d_12::enumerations::QueryType(type_);
            let start_index = start_index;
            let num_queries = num_queries;
            let destination_buffer_found = object::<<V::Api as direct3d_12::Direct3D12>::Resource>(
                KEY,
                destination_buffer.addr(),
            )?;
            let aligned_destination_buffer_offset = aligned_destination_buffer_offset;
            let () = found.value().resolve_query_data(
                query_heap_found.value(),
                type_,
                start_index,
                num_queries,
                destination_buffer_found.value(),
                aligned_destination_buffer_offset,
            );
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12GraphicsCommandList::ClearState` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_clear_state_refused(_this: Raw, _p0: Raw) {}

/// `ID3D12GraphicsCommandList::CopyTextureRegion` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_copy_texture_region_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: u32,
    _p2: u32,
    _p3: u32,
    _p4: *mut u8,
    _p5: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::CopyResource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_copy_resource_refused(
    _this: Raw,
    _p0: Raw,
    _p1: Raw,
) {
}

/// `ID3D12GraphicsCommandList::CopyTiles` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_copy_tiles_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
    _p2: *mut u8,
    _p3: Raw,
    _p4: u64,
    _p5: i32,
) {
}

/// `ID3D12GraphicsCommandList::ResolveSubresource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_resolve_subresource_refused(
    _this: Raw,
    _p0: Raw,
    _p1: u32,
    _p2: Raw,
    _p3: u32,
    _p4: i32,
) {
}

/// `ID3D12GraphicsCommandList::RSSetViewports` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_rs_set_viewports_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::RSSetScissorRects` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_rs_set_scissor_rects_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::OMSetBlendFactor` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_om_set_blend_factor_refused(
    _this: Raw,
    _p0: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::OMSetStencilRef` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_om_set_stencil_ref_refused(
    _this: Raw,
    _p0: u32,
) {
}

/// `ID3D12GraphicsCommandList::ResourceBarrier` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_resource_barrier_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::ExecuteBundle` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_execute_bundle_refused(_this: Raw, _p0: Raw) {}

/// `ID3D12GraphicsCommandList::SetComputeRootDescriptorTable` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_compute_root_descriptor_table_refused(
    _this: Raw,
    _p0: u32,
    _p1: u64,
) {
}

/// `ID3D12GraphicsCommandList::SetGraphicsRootDescriptorTable` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_graphics_root_descriptor_table_refused(
    _this: Raw,
    _p0: u32,
    _p1: u64,
) {
}

/// `ID3D12GraphicsCommandList::SetComputeRoot32BitConstant` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_compute_root32_bit_constant_refused(
    _this: Raw,
    _p0: u32,
    _p1: u32,
    _p2: u32,
) {
}

/// `ID3D12GraphicsCommandList::SetGraphicsRoot32BitConstant` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_graphics_root32_bit_constant_refused(
    _this: Raw,
    _p0: u32,
    _p1: u32,
    _p2: u32,
) {
}

/// `ID3D12GraphicsCommandList::SetComputeRoot32BitConstants` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_compute_root32_bit_constants_refused(
    _this: Raw,
    _p0: u32,
    _p1: u32,
    _p2: *mut u8,
    _p3: u32,
) {
}

/// `ID3D12GraphicsCommandList::SetGraphicsRoot32BitConstants` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_graphics_root32_bit_constants_refused(
    _this: Raw,
    _p0: u32,
    _p1: u32,
    _p2: *mut u8,
    _p3: u32,
) {
}

/// `ID3D12GraphicsCommandList::SetComputeRootShaderResourceView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_compute_root_shader_resource_view_refused(
    _this: Raw,
    _p0: u32,
    _p1: u64,
) {
}

/// `ID3D12GraphicsCommandList::SetGraphicsRootShaderResourceView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_graphics_root_shader_resource_view_refused(
    _this: Raw,
    _p0: u32,
    _p1: u64,
) {
}

/// `ID3D12GraphicsCommandList::SetGraphicsRootUnorderedAccessView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_graphics_root_unordered_access_view_refused(
    _this: Raw,
    _p0: u32,
    _p1: u64,
) {
}

/// `ID3D12GraphicsCommandList::ClearDepthStencilView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_clear_depth_stencil_view_refused(
    _this: Raw,
    _p0: u64,
    _p1: i32,
    _p2: f32,
    _p3: u8,
    _p4: u32,
    _p5: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::ClearRenderTargetView` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_clear_render_target_view_refused(
    _this: Raw,
    _p0: u64,
    _p1: *mut u8,
    _p2: u32,
    _p3: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::ClearUnorderedAccessViewUint` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_clear_unordered_access_view_uint_refused(
    _this: Raw,
    _p0: u64,
    _p1: u64,
    _p2: Raw,
    _p3: *mut u8,
    _p4: u32,
    _p5: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::ClearUnorderedAccessViewFloat` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_clear_unordered_access_view_float_refused(
    _this: Raw,
    _p0: u64,
    _p1: u64,
    _p2: Raw,
    _p3: *mut u8,
    _p4: u32,
    _p5: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::DiscardResource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_discard_resource_refused(
    _this: Raw,
    _p0: Raw,
    _p1: *mut u8,
) {
}

/// `ID3D12GraphicsCommandList::SetPredication` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_predication_refused(
    _this: Raw,
    _p0: Raw,
    _p1: u64,
    _p2: i32,
) {
}

/// `ID3D12GraphicsCommandList::SetMarker` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_set_marker_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    _p2: u32,
) {
}

/// `ID3D12GraphicsCommandList::BeginEvent` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_begin_event_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    _p2: u32,
) {
}

/// `ID3D12GraphicsCommandList::EndEvent` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_end_event_refused(_this: Raw) {}

/// `ID3D12GraphicsCommandList::ExecuteIndirect` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers no result (nothing done).
pub(crate) extern "system" fn graphics_command_list_execute_indirect_refused(
    _this: Raw,
    _p0: Raw,
    _p1: u32,
    _p2: Raw,
    _p3: u64,
    _p4: Raw,
    _p5: u64,
) {
}

/// `ID3D12Object::GetPrivateData` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn object_get_private_data_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: *mut u8,
    _p2: *mut u8,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Object::SetPrivateData` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn object_set_private_data_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: u32,
    _p2: *mut u8,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Object::SetPrivateDataInterface` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn object_set_private_data_interface_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: Raw,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Object::SetName` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn object_set_name_refused(_this: Raw, _p0: *const u8) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12PipelineState::GetCachedBlob` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12PipelineState::GetCachedBlob, which the C side calls"
)]
pub(crate) unsafe extern "system" fn pipeline_state_get_cached_blob_refused(
    _this: Raw,
    p0: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12PipelineState::GetCachedBlob: ppBlob, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppBlob` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Resource::Unmap` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "ID3D12Resource::Unmap, which the C side calls")]
pub(crate) unsafe extern "system" fn resource_unmap<V: direct3d_12::Resource>(
    this: Raw,
    subresource: u32,
    written_range: *const u8,
) {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let subresource = subresource;
            let mut written_range_bytes =
                [u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12RangeLayout>()];
            let written_range_null = written_range.is_null();
            #[expect(
                unsafe_code,
                reason = "ID3D12Resource::Unmap: pWrittenRange, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pWrittenRange` is `In` and `Const`: one `Range` of 16 bytes, readable for the call.
            let written_range_read = unsafe { read_bytes(written_range, &mut written_range_bytes) };
            let written_range = written_range_read.then(|| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_range(&written_range_bytes)
            });
            if !written_range_read && !written_range_null {
                return None;
            }
            let () = found.value().unmap(subresource, written_range.as_ref());
            drop(found);
            Some(())
        });
        let _ = answer;
    };
    guarded((), body)
}

/// `ID3D12Resource::GetGPUVirtualAddress` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Resource::GetGPUVirtualAddress, which the C side calls"
)]
pub(crate) unsafe extern "system" fn resource_get_gpu_virtual_address<V: direct3d_12::Resource>(
    this: Raw,
) -> u64 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_gpu_virtual_address();
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `ID3D12Resource::Map` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(unsafe_code, reason = "ID3D12Resource::Map, which the C side calls")]
pub(crate) unsafe extern "system" fn resource_map_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Resource::Map: ppData, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `ppData` is `Out`: one pointer to write (COM's null on failure), or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; size_of::<usize>()]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Resource::GetDesc` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers the caller's pointer, 56 zero bytes written through it.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Resource::GetDesc, which the C side calls"
)]
pub(crate) unsafe extern "system" fn resource_get_desc_refused(_this: Raw, p0: *mut u8) -> *mut u8 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Resource::GetDesc: RetVal, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `RetVal` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 56]) };
    p0
}

/// `ID3D12Resource::WriteToSubresource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn resource_write_to_subresource_refused(
    _this: Raw,
    _p0: u32,
    _p1: *mut u8,
    _p2: *mut u8,
    _p3: u32,
    _p4: u32,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Resource::ReadFromSubresource` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
pub(crate) extern "system" fn resource_read_from_subresource_refused(
    _this: Raw,
    _p0: *mut u8,
    _p1: u32,
    _p2: u32,
    _p3: u32,
    _p4: *mut u8,
) -> i32 {
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `ID3D12Resource::GetHeapProperties` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3D12Resource::GetHeapProperties, which the C side calls"
)]
pub(crate) unsafe extern "system" fn resource_get_heap_properties_refused(
    _this: Raw,
    p0: *mut u8,
    p1: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3D12Resource::GetHeapProperties: pHeapProperties, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pHeapProperties` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p0, &[u8::MIN; 20]) };
    #[expect(
        unsafe_code,
        reason = "ID3D12Resource::GetHeapProperties: pHeapFlags, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pHeapFlags` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p1, &[u8::MIN; 4]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}

/// `D3D12CreateDevice` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(unsafe_code, reason = "D3D12CreateDevice, which the C side calls")]
pub(crate) unsafe extern "system" fn export_d_3d12_create_device<M: direct3d_12::Direct3D12>(
    adapter: Raw,
    minimum_feature_level: i32,
    riid: *const u8,
    device: *mut Raw,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = module::<M>().ok_or(E_FAIL)?;
            #[expect(
                unsafe_code,
                reason = "D3D12CreateDevice: pAdapter, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `pAdapter` is an `IUnknown*`: null or a live COM object, for the call.
            let adapter_found = unsafe { UnknownFound::<M>::find(adapter) };
            if adapter_found.is_none() && !adapter.is_null() {
                return Err(E_INVALIDARG);
            }
            let minimum_feature_level =
                direct3d_common::enumerations::FeatureLevel(minimum_feature_level);
            let mut riid_bytes = [u8::MIN; size_of::<GuidLayout>()];
            let riid_null = riid.is_null();
            #[expect(
                unsafe_code,
                reason = "D3D12CreateDevice: riid, as the header declares it for the C side"
            )]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one `Guid` of 16 bytes, readable for the call.
            let riid_read = unsafe { read_bytes(riid, &mut riid_bytes) };
            let riid = riid_read.then(|| decode_guid(&riid_bytes));
            let riid = riid.ok_or(E_INVALIDARG)?;
            let device_value = found.create_device(
                adapter_found.as_ref().map(|held| held.get()),
                minimum_feature_level,
                &riid,
            )?;
            drop(found);
            let device_answer = match device_value {
                direct3d_12::UnknownObject::CommandAllocator(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_allocator_interfaces::<
                            <M as direct3d_12::Direct3D12>::CommandAllocator,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::CommandAllocator>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::CommandQueue(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        command_queue_interfaces::<<M as direct3d_12::Direct3D12>::CommandQueue>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::CommandQueue>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::DescriptorHeap(object) => {
                    let served =
                        serve(
                            KEY,
                            object,
                            record(),
                            descriptor_heap_interfaces::<
                                <M as direct3d_12::Direct3D12>::DescriptorHeap,
                            >,
                        )
                        .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::DescriptorHeap>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Device(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        device_interfaces::<<M as direct3d_12::Direct3D12>::Device>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::Device>(KEY, served.address(), &riid)
                        .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Fence(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        fence_interfaces::<<M as direct3d_12::Direct3D12>::Fence>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::Fence>(KEY, served.address(), &riid)
                        .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::GraphicsCommandList(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        graphics_command_list_interfaces::<
                            <M as direct3d_12::Direct3D12>::GraphicsCommandList,
                        >,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::GraphicsCommandList>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::PipelineState(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        pipeline_state_interfaces::<<M as direct3d_12::Direct3D12>::PipelineState>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::PipelineState>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::QueryHeap(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        query_heap_interfaces::<<M as direct3d_12::Direct3D12>::QueryHeap>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::QueryHeap>(KEY, served.address(), &riid)
                        .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Resource(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        resource_interfaces::<<M as direct3d_12::Direct3D12>::Resource>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::Resource>(KEY, served.address(), &riid)
                        .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::RootSignature(object) => {
                    let served = serve(
                        KEY,
                        object,
                        record(),
                        root_signature_interfaces::<<M as direct3d_12::Direct3D12>::RootSignature>,
                    )
                    .ok_or(E_OUTOFMEMORY)?;
                    query::<<M as direct3d_12::Direct3D12>::RootSignature>(
                        KEY,
                        served.address(),
                        &riid,
                    )
                    .ok_or(E_NOINTERFACE)?
                }
                direct3d_12::UnknownObject::Unknown(_) => {
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
            reason = "D3D12CreateDevice: ppDevice, as the header declares it for the C side"
        )]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: one interface pointer to write, or null, for the call.
        let _ = unsafe { write_bytes(device.cast::<u8>(), &device_written) };
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}
