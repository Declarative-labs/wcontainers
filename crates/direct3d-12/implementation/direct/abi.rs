//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The C tables of this API's interfaces, and its library's exports.

use wcontainers_component_object_model_direct::{Interface, Raw, UnknownTable};

/// `ID3D12CommandAllocator`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct CommandAllocatorTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) command_allocator_reset: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `CommandAllocatorTable` lays out `ID3D12CommandAllocator` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12CommandAllocator"
)]
unsafe impl Interface for CommandAllocatorTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12CommandList`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct CommandListTable {
    pub unknown: UnknownTable,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub command_list_get_type: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `CommandListTable` lays out `ID3D12CommandList` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12CommandList"
)]
unsafe impl Interface for CommandListTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12CommandQueue`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct CommandQueueTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) command_queue_update_tile_mappings: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *mut u8,
        *mut u8,
        Raw,
        u32,
        *mut u8,
        *mut u8,
        *mut u8,
        i32,
    ),
    pub(crate) command_queue_copy_tile_mappings:
        unsafe extern "system" fn(Raw, Raw, *mut u8, Raw, *mut u8, *mut u8, i32),
    pub(crate) command_queue_execute_command_lists: unsafe extern "system" fn(Raw, u32, *mut u8),
    pub(crate) command_queue_set_marker: unsafe extern "system" fn(Raw, u32, *mut u8, u32),
    pub(crate) command_queue_begin_event: unsafe extern "system" fn(Raw, u32, *mut u8, u32),
    pub(crate) command_queue_end_event: unsafe extern "system" fn(Raw),
    pub(crate) command_queue_signal: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
    pub(crate) command_queue_wait: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
    pub(crate) command_queue_get_timestamp_frequency:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) command_queue_get_clock_calibration:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) command_queue_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> *mut u8,
}

// SAFETY: `CommandQueueTable` lays out `ID3D12CommandQueue` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12CommandQueue"
)]
unsafe impl Interface for CommandQueueTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12DescriptorHeap`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DescriptorHeapTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) descriptor_heap_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> *mut u8,
    pub(crate) descriptor_heap_get_cpu_descriptor_handle_for_heap_start:
        unsafe extern "system" fn(Raw, *mut u8) -> *mut u8,
    pub(crate) descriptor_heap_get_gpu_descriptor_handle_for_heap_start:
        unsafe extern "system" fn(Raw, *mut u8) -> *mut u8,
}

// SAFETY: `DescriptorHeapTable` lays out `ID3D12DescriptorHeap` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12DescriptorHeap"
)]
unsafe impl Interface for DescriptorHeapTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12Device`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DeviceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_get_node_count: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) device_create_command_queue:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_command_allocator:
        unsafe extern "system" fn(Raw, i32, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_graphics_pipeline_state:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_compute_pipeline_state:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_command_list:
        unsafe extern "system" fn(Raw, u32, i32, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_check_feature_support:
        unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub(crate) device_create_descriptor_heap:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_get_descriptor_handle_increment_size:
        unsafe extern "system" fn(Raw, i32) -> u32,
    pub(crate) device_create_root_signature:
        unsafe extern "system" fn(Raw, u32, *const u8, usize, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_constant_buffer_view: unsafe extern "system" fn(Raw, *mut u8, u64),
    pub(crate) device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u64),
    pub(crate) device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *mut u8, u64),
    pub(crate) device_create_render_target_view: unsafe extern "system" fn(Raw, Raw, *mut u8, u64),
    pub(crate) device_create_depth_stencil_view: unsafe extern "system" fn(Raw, Raw, *mut u8, u64),
    pub(crate) device_create_sampler: unsafe extern "system" fn(Raw, *mut u8, u64),
    pub(crate) device_copy_descriptors:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8, u32, *mut u8, *mut u8, i32),
    pub(crate) device_copy_descriptors_simple: unsafe extern "system" fn(Raw, u32, u64, u64, i32),
    pub(crate) device_get_resource_allocation_info:
        unsafe extern "system" fn(Raw, *mut u8, u32, u32, *mut u8) -> *mut u8,
    pub(crate) device_get_custom_heap_properties:
        unsafe extern "system" fn(Raw, *mut u8, u32, i32) -> *mut u8,
    pub(crate) device_create_committed_resource: unsafe extern "system" fn(
        Raw,
        *const u8,
        i32,
        *const u8,
        i32,
        *const u8,
        *const u8,
        *mut Raw,
    ) -> i32,
    pub(crate) device_create_heap: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_placed_resource:
        unsafe extern "system" fn(Raw, Raw, u64, *mut u8, i32, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_reserved_resource:
        unsafe extern "system" fn(Raw, *mut u8, i32, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_shared_handle:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *const u8, *mut u8) -> i32,
    pub(crate) device_open_shared_handle:
        unsafe extern "system" fn(Raw, usize, *mut u8, *mut u8) -> i32,
    pub(crate) device_open_shared_handle_by_name:
        unsafe extern "system" fn(Raw, *const u8, u32, *mut u8) -> i32,
    pub(crate) device_make_resident: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) device_evict: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) device_create_fence:
        unsafe extern "system" fn(Raw, u64, i32, *const u8, *mut Raw) -> i32,
    pub(crate) device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_get_copyable_footprints: unsafe extern "system" fn(
        Raw,
        *const u8,
        u32,
        u32,
        u64,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
    ),
    pub(crate) device_create_query_heap:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_set_stable_power_state: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) device_create_command_signature:
        unsafe extern "system" fn(Raw, *mut u8, Raw, *mut u8, *mut u8) -> i32,
    pub(crate) device_get_resource_tiling:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, *mut u8, u32, *mut u8),
    pub(crate) device_get_adapter_luid: unsafe extern "system" fn(Raw, *mut u8) -> *mut u8,
}

// SAFETY: `DeviceTable` lays out `ID3D12Device` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12Device"
)]
unsafe impl Interface for DeviceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12DeviceChild`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceChildTable {
    pub unknown: UnknownTable,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
}

// SAFETY: `DeviceChildTable` lays out `ID3D12DeviceChild` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12DeviceChild"
)]
unsafe impl Interface for DeviceChildTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12Fence`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct FenceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) fence_get_completed_value: unsafe extern "system" fn(Raw) -> u64,
    pub(crate) fence_set_event_on_completion: unsafe extern "system" fn(Raw, u64, usize) -> i32,
    pub(crate) fence_signal: unsafe extern "system" fn(Raw, u64) -> i32,
}

// SAFETY: `FenceTable` lays out `ID3D12Fence` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12Fence"
)]
unsafe impl Interface for FenceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12GraphicsCommandList`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct GraphicsCommandListTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) command_list_get_type: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) graphics_command_list_close: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) graphics_command_list_reset: unsafe extern "system" fn(Raw, Raw, Raw) -> i32,
    pub(crate) graphics_command_list_clear_state: unsafe extern "system" fn(Raw, Raw),
    pub(crate) graphics_command_list_draw_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, u32),
    pub(crate) graphics_command_list_draw_indexed_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32),
    pub(crate) graphics_command_list_dispatch: unsafe extern "system" fn(Raw, u32, u32, u32),
    pub(crate) graphics_command_list_copy_buffer_region:
        unsafe extern "system" fn(Raw, Raw, u64, Raw, u64, u64),
    pub(crate) graphics_command_list_copy_texture_region:
        unsafe extern "system" fn(Raw, *mut u8, u32, u32, u32, *mut u8, *mut u8),
    pub(crate) graphics_command_list_copy_resource: unsafe extern "system" fn(Raw, Raw, Raw),
    pub(crate) graphics_command_list_copy_tiles:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, Raw, u64, i32),
    pub(crate) graphics_command_list_resolve_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, Raw, u32, i32),
    pub(crate) graphics_command_list_ia_set_primitive_topology: unsafe extern "system" fn(Raw, i32),
    pub(crate) graphics_command_list_rs_set_viewports: unsafe extern "system" fn(Raw, u32, *mut u8),
    pub(crate) graphics_command_list_rs_set_scissor_rects:
        unsafe extern "system" fn(Raw, u32, *mut u8),
    pub(crate) graphics_command_list_om_set_blend_factor: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) graphics_command_list_om_set_stencil_ref: unsafe extern "system" fn(Raw, u32),
    pub(crate) graphics_command_list_set_pipeline_state: unsafe extern "system" fn(Raw, Raw),
    pub(crate) graphics_command_list_resource_barrier: unsafe extern "system" fn(Raw, u32, *mut u8),
    pub(crate) graphics_command_list_execute_bundle: unsafe extern "system" fn(Raw, Raw),
    pub(crate) graphics_command_list_set_descriptor_heaps:
        unsafe extern "system" fn(Raw, u32, *const Raw),
    pub(crate) graphics_command_list_set_compute_root_signature:
        unsafe extern "system" fn(Raw, Raw),
    pub(crate) graphics_command_list_set_graphics_root_signature:
        unsafe extern "system" fn(Raw, Raw),
    pub(crate) graphics_command_list_set_compute_root_descriptor_table:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_graphics_root_descriptor_table:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_compute_root32_bit_constant:
        unsafe extern "system" fn(Raw, u32, u32, u32),
    pub(crate) graphics_command_list_set_graphics_root32_bit_constant:
        unsafe extern "system" fn(Raw, u32, u32, u32),
    pub(crate) graphics_command_list_set_compute_root32_bit_constants:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8, u32),
    pub(crate) graphics_command_list_set_graphics_root32_bit_constants:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8, u32),
    pub(crate) graphics_command_list_set_compute_root_constant_buffer_view:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_graphics_root_constant_buffer_view:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_compute_root_shader_resource_view:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_graphics_root_shader_resource_view:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_compute_root_unordered_access_view:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_set_graphics_root_unordered_access_view:
        unsafe extern "system" fn(Raw, u32, u64),
    pub(crate) graphics_command_list_ia_set_index_buffer: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) graphics_command_list_ia_set_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8),
    pub(crate) graphics_command_list_so_set_targets:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8),
    pub(crate) graphics_command_list_om_set_render_targets:
        unsafe extern "system" fn(Raw, u32, *const u8, i32, *const u8),
    pub(crate) graphics_command_list_clear_depth_stencil_view:
        unsafe extern "system" fn(Raw, u64, i32, f32, u8, u32, *mut u8),
    pub(crate) graphics_command_list_clear_render_target_view:
        unsafe extern "system" fn(Raw, u64, *mut u8, u32, *mut u8),
    pub(crate) graphics_command_list_clear_unordered_access_view_uint:
        unsafe extern "system" fn(Raw, u64, u64, Raw, *mut u8, u32, *mut u8),
    pub(crate) graphics_command_list_clear_unordered_access_view_float:
        unsafe extern "system" fn(Raw, u64, u64, Raw, *mut u8, u32, *mut u8),
    pub(crate) graphics_command_list_discard_resource: unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub(crate) graphics_command_list_begin_query: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub(crate) graphics_command_list_end_query: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub(crate) graphics_command_list_resolve_query_data:
        unsafe extern "system" fn(Raw, Raw, i32, u32, u32, Raw, u64),
    pub(crate) graphics_command_list_set_predication: unsafe extern "system" fn(Raw, Raw, u64, i32),
    pub(crate) graphics_command_list_set_marker: unsafe extern "system" fn(Raw, u32, *mut u8, u32),
    pub(crate) graphics_command_list_begin_event: unsafe extern "system" fn(Raw, u32, *mut u8, u32),
    pub(crate) graphics_command_list_end_event: unsafe extern "system" fn(Raw),
    pub(crate) graphics_command_list_execute_indirect:
        unsafe extern "system" fn(Raw, Raw, u32, Raw, u64, Raw, u64),
}

// SAFETY: `GraphicsCommandListTable` lays out `ID3D12GraphicsCommandList` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12GraphicsCommandList"
)]
unsafe impl Interface for GraphicsCommandListTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12Object`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct ObjectTable {
    pub unknown: UnknownTable,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
}

// SAFETY: `ObjectTable` lays out `ID3D12Object` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12Object"
)]
unsafe impl Interface for ObjectTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12Pageable`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct PageableTable {
    pub unknown: UnknownTable,
    pub object_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub object_set_private_data: unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub object_set_private_data_interface: unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
}

// SAFETY: `PageableTable` lays out `ID3D12Pageable` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12Pageable"
)]
unsafe impl Interface for PageableTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12PipelineState`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct PipelineStateTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) pipeline_state_get_cached_blob: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `PipelineStateTable` lays out `ID3D12PipelineState` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12PipelineState"
)]
unsafe impl Interface for PipelineStateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12QueryHeap`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct QueryHeapTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
}

// SAFETY: `QueryHeapTable` lays out `ID3D12QueryHeap` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12QueryHeap"
)]
unsafe impl Interface for QueryHeapTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12Resource`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ResourceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) resource_map: unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8) -> i32,
    pub(crate) resource_unmap: unsafe extern "system" fn(Raw, u32, *const u8),
    pub(crate) resource_get_desc: unsafe extern "system" fn(Raw, *mut u8) -> *mut u8,
    pub(crate) resource_get_gpu_virtual_address: unsafe extern "system" fn(Raw) -> u64,
    pub(crate) resource_write_to_subresource:
        unsafe extern "system" fn(Raw, u32, *mut u8, *mut u8, u32, u32) -> i32,
    pub(crate) resource_read_from_subresource:
        unsafe extern "system" fn(Raw, *mut u8, u32, u32, u32, *mut u8) -> i32,
    pub(crate) resource_get_heap_properties:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
}

// SAFETY: `ResourceTable` lays out `ID3D12Resource` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12Resource"
)]
unsafe impl Interface for ResourceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D12RootSignature`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct RootSignatureTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) object_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) object_set_private_data:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) object_set_private_data_interface:
        unsafe extern "system" fn(Raw, *mut u8, Raw) -> i32,
    pub(crate) object_set_name: unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
}

// SAFETY: `RootSignatureTable` lays out `ID3D12RootSignature` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D12RootSignature"
)]
unsafe impl Interface for RootSignatureTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// The library's exports, as C calls them.
#[derive(Clone, Copy)]
pub struct Library {
    pub d_3d12_create_device: unsafe extern "system" fn(Raw, i32, *const u8, *mut Raw) -> i32,
}
