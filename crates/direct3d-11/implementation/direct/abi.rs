//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The C tables of this API's interfaces, and its library's exports.

use wcontainers_component_object_model_direct::{Interface, Raw, UnknownTable};

/// `ID3D11Asynchronous`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct AsynchronousTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub asynchronous_get_data_size: unsafe extern "system" fn(Raw) -> u32,
}

// SAFETY: `AsynchronousTable` lays out `ID3D11Asynchronous` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Asynchronous"
)]
unsafe impl Interface for AsynchronousTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11AuthenticatedChannel`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct AuthenticatedChannelTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) authenticated_channel_get_certificate_size:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) authenticated_channel_get_certificate:
        unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) authenticated_channel_get_channel_handle: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `AuthenticatedChannelTable` lays out `ID3D11AuthenticatedChannel` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11AuthenticatedChannel"
)]
unsafe impl Interface for AuthenticatedChannelTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11BlendState`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct BlendStateTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub blend_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `BlendStateTable` lays out `ID3D11BlendState` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11BlendState"
)]
unsafe impl Interface for BlendStateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11BlendState1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct BlendState1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) blend_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) blend_state1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `BlendState1Table` lays out `ID3D11BlendState1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11BlendState1"
)]
unsafe impl Interface for BlendState1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Buffer`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct BufferTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub(crate) resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) buffer_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `BufferTable` lays out `ID3D11Buffer` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Buffer"
)]
unsafe impl Interface for BufferTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ClassInstance`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ClassInstanceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) class_instance_get_class_linkage: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) class_instance_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) class_instance_get_instance_name: unsafe extern "system" fn(Raw, *const u8, *mut u8),
    pub(crate) class_instance_get_type_name: unsafe extern "system" fn(Raw, *const u8, *mut u8),
}

// SAFETY: `ClassInstanceTable` lays out `ID3D11ClassInstance` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ClassInstance"
)]
unsafe impl Interface for ClassInstanceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ClassLinkage`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ClassLinkageTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) class_linkage_get_class_instance:
        unsafe extern "system" fn(Raw, *const u8, u32, *mut Raw) -> i32,
    pub(crate) class_linkage_create_class_instance:
        unsafe extern "system" fn(Raw, *const u8, u32, u32, u32, u32, *mut Raw) -> i32,
}

// SAFETY: `ClassLinkageTable` lays out `ID3D11ClassLinkage` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ClassLinkage"
)]
unsafe impl Interface for ClassLinkageTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11CommandList`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct CommandListTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) command_list_get_context_flags: unsafe extern "system" fn(Raw) -> u32,
}

// SAFETY: `CommandListTable` lays out `ID3D11CommandList` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11CommandList"
)]
unsafe impl Interface for CommandListTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ComputeShader`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ComputeShaderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `ComputeShaderTable` lays out `ID3D11ComputeShader` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ComputeShader"
)]
unsafe impl Interface for ComputeShaderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Counter`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct CounterTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) asynchronous_get_data_size: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) counter_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `CounterTable` lays out `ID3D11Counter` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Counter"
)]
unsafe impl Interface for CounterTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11CryptoSession`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct CryptoSessionTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) crypto_session_get_crypto_type: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) crypto_session_get_decoder_profile: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) crypto_session_get_certificate_size: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) crypto_session_get_certificate: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) crypto_session_get_crypto_session_handle: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `CryptoSessionTable` lays out `ID3D11CryptoSession` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11CryptoSession"
)]
unsafe impl Interface for CryptoSessionTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Debug`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DebugTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) debug_set_feature_mask: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) debug_get_feature_mask: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) debug_set_present_per_render_op_delay: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) debug_get_present_per_render_op_delay: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) debug_set_swap_chain: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub(crate) debug_get_swap_chain: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub(crate) debug_validate_context: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub(crate) debug_report_live_device_objects: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) debug_validate_context_for_dispatch: unsafe extern "system" fn(Raw, Raw) -> i32,
}

// SAFETY: `DebugTable` lays out `ID3D11Debug` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Debug"
)]
unsafe impl Interface for DebugTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DepthStencilState`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DepthStencilStateTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) depth_stencil_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `DepthStencilStateTable` lays out `ID3D11DepthStencilState` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DepthStencilState"
)]
unsafe impl Interface for DepthStencilStateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DepthStencilView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DepthStencilViewTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) depth_stencil_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `DepthStencilViewTable` lays out `ID3D11DepthStencilView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DepthStencilView"
)]
unsafe impl Interface for DepthStencilViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Device`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceTable {
    pub unknown: UnknownTable,
    pub device_create_buffer: unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_1d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_2d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_3d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_input_layout:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8, usize, *mut u8) -> i32,
    pub device_create_vertex_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader_with_stream_output: unsafe extern "system" fn(
        Raw,
        *mut u8,
        usize,
        *mut u8,
        u32,
        *mut u8,
        u32,
        u32,
        Raw,
        *mut u8,
    ) -> i32,
    pub device_create_pixel_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_hull_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_domain_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_compute_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_class_linkage: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_blend_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_rasterizer_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_sampler_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_query: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub device_create_predicate: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_counter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_deferred_context: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device_open_shared_resource:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device_check_format_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub device_check_multisample_quality_levels:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8) -> i32,
    pub device_check_counter_info: unsafe extern "system" fn(Raw, *mut u8),
    pub device_check_counter: unsafe extern "system" fn(
        Raw,
        *mut u8,
        *mut u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
    ) -> i32,
    pub device_check_feature_support: unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub device_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_get_feature_level: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_immediate_context: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_set_exception_mode: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device_get_exception_mode: unsafe extern "system" fn(Raw) -> u32,
}

// SAFETY: `DeviceTable` lays out `ID3D11Device` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Device"
)]
unsafe impl Interface for DeviceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Device1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device1Table {
    pub unknown: UnknownTable,
    pub device_create_buffer: unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_1d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_2d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_3d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_input_layout:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8, usize, *mut u8) -> i32,
    pub device_create_vertex_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader_with_stream_output: unsafe extern "system" fn(
        Raw,
        *mut u8,
        usize,
        *mut u8,
        u32,
        *mut u8,
        u32,
        u32,
        Raw,
        *mut u8,
    ) -> i32,
    pub device_create_pixel_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_hull_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_domain_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_compute_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_class_linkage: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_blend_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_rasterizer_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_sampler_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_query: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub device_create_predicate: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_counter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_deferred_context: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device_open_shared_resource:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device_check_format_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub device_check_multisample_quality_levels:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8) -> i32,
    pub device_check_counter_info: unsafe extern "system" fn(Raw, *mut u8),
    pub device_check_counter: unsafe extern "system" fn(
        Raw,
        *mut u8,
        *mut u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
    ) -> i32,
    pub device_check_feature_support: unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub device_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_get_feature_level: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_immediate_context: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_set_exception_mode: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device_get_exception_mode: unsafe extern "system" fn(Raw) -> u32,
    pub device1_get_immediate_context1: unsafe extern "system" fn(Raw, *mut Raw),
    pub device1_create_deferred_context1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device1_create_blend_state1: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device1_create_rasterizer_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device1_create_device_context_state: unsafe extern "system" fn(
        Raw,
        u32,
        *const u8,
        u32,
        u32,
        *const u8,
        *mut u8,
        *mut Raw,
    ) -> i32,
    pub device1_open_shared_resource1:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device1_open_shared_resource_by_name:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8, *mut Raw) -> i32,
}

// SAFETY: `Device1Table` lays out `ID3D11Device1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Device1"
)]
unsafe impl Interface for Device1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Device2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device2Table {
    pub unknown: UnknownTable,
    pub device_create_buffer: unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_1d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_2d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_3d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_input_layout:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8, usize, *mut u8) -> i32,
    pub device_create_vertex_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader_with_stream_output: unsafe extern "system" fn(
        Raw,
        *mut u8,
        usize,
        *mut u8,
        u32,
        *mut u8,
        u32,
        u32,
        Raw,
        *mut u8,
    ) -> i32,
    pub device_create_pixel_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_hull_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_domain_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_compute_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_class_linkage: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_blend_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_rasterizer_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_sampler_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_query: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub device_create_predicate: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_counter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_deferred_context: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device_open_shared_resource:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device_check_format_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub device_check_multisample_quality_levels:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8) -> i32,
    pub device_check_counter_info: unsafe extern "system" fn(Raw, *mut u8),
    pub device_check_counter: unsafe extern "system" fn(
        Raw,
        *mut u8,
        *mut u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
    ) -> i32,
    pub device_check_feature_support: unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub device_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_get_feature_level: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_immediate_context: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_set_exception_mode: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device_get_exception_mode: unsafe extern "system" fn(Raw) -> u32,
    pub device1_get_immediate_context1: unsafe extern "system" fn(Raw, *mut Raw),
    pub device1_create_deferred_context1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device1_create_blend_state1: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device1_create_rasterizer_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device1_create_device_context_state: unsafe extern "system" fn(
        Raw,
        u32,
        *const u8,
        u32,
        u32,
        *const u8,
        *mut u8,
        *mut Raw,
    ) -> i32,
    pub device1_open_shared_resource1:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device1_open_shared_resource_by_name:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8, *mut Raw) -> i32,
    pub device2_get_immediate_context2: unsafe extern "system" fn(Raw, *mut Raw),
    pub device2_create_deferred_context2: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device2_get_resource_tiling:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, *mut u8, u32, *mut u8),
    pub device2_check_multisample_quality_levels1:
        unsafe extern "system" fn(Raw, i32, u32, u32, *mut u8) -> i32,
}

// SAFETY: `Device2Table` lays out `ID3D11Device2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Device2"
)]
unsafe impl Interface for Device2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Device3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Device3Table {
    pub unknown: UnknownTable,
    pub device_create_buffer: unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_1d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_2d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_texture_3d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device_create_input_layout:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8, usize, *mut u8) -> i32,
    pub device_create_vertex_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_geometry_shader_with_stream_output: unsafe extern "system" fn(
        Raw,
        *mut u8,
        usize,
        *mut u8,
        u32,
        *mut u8,
        u32,
        u32,
        Raw,
        *mut u8,
    ) -> i32,
    pub device_create_pixel_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_hull_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_domain_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_compute_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub device_create_class_linkage: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub device_create_blend_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_depth_stencil_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_rasterizer_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_sampler_state: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_query: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub device_create_predicate: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_counter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device_create_deferred_context: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device_open_shared_resource:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device_check_format_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub device_check_multisample_quality_levels:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8) -> i32,
    pub device_check_counter_info: unsafe extern "system" fn(Raw, *mut u8),
    pub device_check_counter: unsafe extern "system" fn(
        Raw,
        *mut u8,
        *mut u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
    ) -> i32,
    pub device_check_feature_support: unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub device_get_private_data: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_set_private_data: unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_set_private_data_interface: unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_get_feature_level: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub device_get_immediate_context: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_set_exception_mode: unsafe extern "system" fn(Raw, u32) -> i32,
    pub device_get_exception_mode: unsafe extern "system" fn(Raw) -> u32,
    pub device1_get_immediate_context1: unsafe extern "system" fn(Raw, *mut Raw),
    pub device1_create_deferred_context1: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device1_create_blend_state1: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device1_create_rasterizer_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device1_create_device_context_state: unsafe extern "system" fn(
        Raw,
        u32,
        *const u8,
        u32,
        u32,
        *const u8,
        *mut u8,
        *mut Raw,
    ) -> i32,
    pub device1_open_shared_resource1:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub device1_open_shared_resource_by_name:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8, *mut Raw) -> i32,
    pub device2_get_immediate_context2: unsafe extern "system" fn(Raw, *mut Raw),
    pub device2_create_deferred_context2: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device2_get_resource_tiling:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, *mut u8, u32, *mut u8),
    pub device2_check_multisample_quality_levels1:
        unsafe extern "system" fn(Raw, i32, u32, u32, *mut u8) -> i32,
    pub device3_create_texture_2d1:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device3_create_texture_3d1:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device3_create_rasterizer_state2:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device3_create_shader_resource_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device3_create_unordered_access_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device3_create_render_target_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub device3_create_query1: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub device3_get_immediate_context3: unsafe extern "system" fn(Raw, *mut Raw),
    pub device3_create_deferred_context3: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub device3_write_to_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub device3_read_from_subresource:
        unsafe extern "system" fn(Raw, *mut u8, u32, u32, Raw, u32, *mut u8),
}

// SAFETY: `Device3Table` lays out `ID3D11Device3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Device3"
)]
unsafe impl Interface for Device3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Device4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Device4Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_create_buffer:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_texture_1d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_texture_2d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_texture_3d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_input_layout:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8, usize, *mut u8) -> i32,
    pub(crate) device_create_vertex_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_geometry_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_geometry_shader_with_stream_output: unsafe extern "system" fn(
        Raw,
        *mut u8,
        usize,
        *mut u8,
        u32,
        *mut u8,
        u32,
        u32,
        Raw,
        *mut u8,
    )
        -> i32,
    pub(crate) device_create_pixel_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_hull_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_domain_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_compute_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_class_linkage: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub(crate) device_create_blend_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_depth_stencil_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_rasterizer_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_sampler_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_query: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_predicate: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_counter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_deferred_context: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device_open_shared_resource:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub(crate) device_check_format_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub(crate) device_check_multisample_quality_levels:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8) -> i32,
    pub(crate) device_check_counter_info: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) device_check_counter: unsafe extern "system" fn(
        Raw,
        *mut u8,
        *mut u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
    ) -> i32,
    pub(crate) device_check_feature_support:
        unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub(crate) device_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) device_get_feature_level: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_get_immediate_context: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_set_exception_mode: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) device_get_exception_mode: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) device1_get_immediate_context1: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device1_create_deferred_context1:
        unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device1_create_blend_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device1_create_rasterizer_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device1_create_device_context_state: unsafe extern "system" fn(
        Raw,
        u32,
        *const u8,
        u32,
        u32,
        *const u8,
        *mut u8,
        *mut Raw,
    ) -> i32,
    pub(crate) device1_open_shared_resource1:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub(crate) device1_open_shared_resource_by_name:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8, *mut Raw) -> i32,
    pub(crate) device2_get_immediate_context2: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device2_create_deferred_context2:
        unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device2_get_resource_tiling:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, *mut u8, u32, *mut u8),
    pub(crate) device2_check_multisample_quality_levels1:
        unsafe extern "system" fn(Raw, i32, u32, u32, *mut u8) -> i32,
    pub(crate) device3_create_texture_2d1:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device3_create_texture_3d1:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device3_create_rasterizer_state2:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_shader_resource_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_unordered_access_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_render_target_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_query1: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_get_immediate_context3: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device3_create_deferred_context3:
        unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device3_write_to_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub(crate) device3_read_from_subresource:
        unsafe extern "system" fn(Raw, *mut u8, u32, u32, Raw, u32, *mut u8),
    pub(crate) device4_register_device_removed_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) device4_unregister_device_removed: unsafe extern "system" fn(Raw, u32),
}

// SAFETY: `Device4Table` lays out `ID3D11Device4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Device4"
)]
unsafe impl Interface for Device4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Device5`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Device5Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_create_buffer:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_texture_1d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_texture_2d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_texture_3d:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_shader_resource_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_unordered_access_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_input_layout:
        unsafe extern "system" fn(Raw, *mut u8, u32, *mut u8, usize, *mut u8) -> i32,
    pub(crate) device_create_vertex_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_geometry_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_geometry_shader_with_stream_output: unsafe extern "system" fn(
        Raw,
        *mut u8,
        usize,
        *mut u8,
        u32,
        *mut u8,
        u32,
        u32,
        Raw,
        *mut u8,
    )
        -> i32,
    pub(crate) device_create_pixel_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_hull_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_domain_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_compute_shader:
        unsafe extern "system" fn(Raw, *const u8, usize, Raw, *mut Raw) -> i32,
    pub(crate) device_create_class_linkage: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    pub(crate) device_create_blend_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_depth_stencil_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_rasterizer_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_sampler_state:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_query: unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) device_create_predicate: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_counter: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device_create_deferred_context: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device_open_shared_resource:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub(crate) device_check_format_support: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub(crate) device_check_multisample_quality_levels:
        unsafe extern "system" fn(Raw, i32, u32, *mut u8) -> i32,
    pub(crate) device_check_counter_info: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) device_check_counter: unsafe extern "system" fn(
        Raw,
        *mut u8,
        *mut u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
        *const u8,
        *mut u8,
    ) -> i32,
    pub(crate) device_check_feature_support:
        unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub(crate) device_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) device_get_feature_level: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_get_creation_flags: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) device_get_device_removed_reason: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_get_immediate_context: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_set_exception_mode: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) device_get_exception_mode: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) device1_get_immediate_context1: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device1_create_deferred_context1:
        unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device1_create_blend_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device1_create_rasterizer_state1:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device1_create_device_context_state: unsafe extern "system" fn(
        Raw,
        u32,
        *const u8,
        u32,
        u32,
        *const u8,
        *mut u8,
        *mut Raw,
    ) -> i32,
    pub(crate) device1_open_shared_resource1:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub(crate) device1_open_shared_resource_by_name:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8, *mut Raw) -> i32,
    pub(crate) device2_get_immediate_context2: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device2_create_deferred_context2:
        unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device2_get_resource_tiling:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, *mut u8, u32, *mut u8),
    pub(crate) device2_check_multisample_quality_levels1:
        unsafe extern "system" fn(Raw, i32, u32, u32, *mut u8) -> i32,
    pub(crate) device3_create_texture_2d1:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device3_create_texture_3d1:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device3_create_rasterizer_state2:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_shader_resource_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_unordered_access_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_render_target_view1:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_create_query1: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) device3_get_immediate_context3: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device3_create_deferred_context3:
        unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32,
    pub(crate) device3_write_to_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub(crate) device3_read_from_subresource:
        unsafe extern "system" fn(Raw, *mut u8, u32, u32, Raw, u32, *mut u8),
    pub(crate) device4_register_device_removed_event:
        unsafe extern "system" fn(Raw, usize, *mut u8) -> i32,
    pub(crate) device4_unregister_device_removed: unsafe extern "system" fn(Raw, u32),
    pub(crate) device5_open_shared_fence:
        unsafe extern "system" fn(Raw, usize, *const u8, *mut Raw) -> i32,
    pub(crate) device5_create_fence:
        unsafe extern "system" fn(Raw, u64, i32, *const u8, *mut Raw) -> i32,
}

// SAFETY: `Device5Table` lays out `ID3D11Device5` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Device5"
)]
unsafe impl Interface for Device5Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DeviceChild`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceChildTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `DeviceChildTable` lays out `ID3D11DeviceChild` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DeviceChild"
)]
unsafe impl Interface for DeviceChildTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DeviceContext`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceContextTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_context_vs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ps_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_draw_indexed: unsafe extern "system" fn(Raw, u32, u32, i32),
    pub device_context_draw: unsafe extern "system" fn(Raw, u32, u32),
    pub device_context_map: unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *mut u8) -> i32,
    pub device_context_unmap: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_ps_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ia_set_input_layout: unsafe extern "system" fn(Raw, Raw),
    pub device_context_ia_set_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context_ia_set_index_buffer: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub device_context_draw_indexed_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32),
    pub device_context_draw_instanced: unsafe extern "system" fn(Raw, u32, u32, u32, u32),
    pub device_context_gs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ia_set_primitive_topology: unsafe extern "system" fn(Raw, i32),
    pub device_context_vs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_begin: unsafe extern "system" fn(Raw, Raw),
    pub device_context_end: unsafe extern "system" fn(Raw, Raw),
    pub device_context_get_data: unsafe extern "system" fn(Raw, Raw, *mut u8, u32, u32) -> i32,
    pub device_context_set_predication: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_gs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_om_set_render_targets: unsafe extern "system" fn(Raw, u32, *const Raw, Raw),
    pub device_context_om_set_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *const Raw, Raw, u32, u32, *const Raw, *const u8),
    pub device_context_om_set_blend_state: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub device_context_om_set_depth_stencil_state: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_so_set_targets: unsafe extern "system" fn(Raw, u32, *const Raw, *const u8),
    pub device_context_draw_auto: unsafe extern "system" fn(Raw),
    pub device_context_draw_indexed_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_draw_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_dispatch: unsafe extern "system" fn(Raw, u32, u32, u32),
    pub device_context_dispatch_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_rs_set_state: unsafe extern "system" fn(Raw, Raw),
    pub device_context_rs_set_viewports: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_rs_set_scissor_rects: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_copy_subresource_region:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8),
    pub device_context_copy_resource: unsafe extern "system" fn(Raw, Raw, Raw),
    pub device_context_update_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub device_context_copy_structure_count: unsafe extern "system" fn(Raw, Raw, u32, Raw),
    pub device_context_clear_render_target_view: unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_uint:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_float:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_depth_stencil_view: unsafe extern "system" fn(Raw, Raw, u32, f32, u8),
    pub device_context_generate_mips: unsafe extern "system" fn(Raw, Raw),
    pub device_context_set_resource_min_lod: unsafe extern "system" fn(Raw, Raw, f32),
    pub device_context_get_resource_min_lod: unsafe extern "system" fn(Raw, Raw) -> f32,
    pub device_context_resolve_subresource: unsafe extern "system" fn(Raw, Raw, u32, Raw, u32, i32),
    pub device_context_execute_command_list: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_hs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_hs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ds_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8),
    pub device_context_cs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_cs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ia_get_input_layout: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_ia_get_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context_ia_get_index_buffer:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_gs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ia_get_primitive_topology: unsafe extern "system" fn(Raw, *mut u8),
    pub device_context_vs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_get_predication: unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_gs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_om_get_render_targets:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw),
    pub device_context_om_get_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw, u32, u32, *mut Raw),
    pub device_context_om_get_blend_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_om_get_depth_stencil_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_so_get_targets: unsafe extern "system" fn(Raw, u32, *mut Raw),
    pub device_context_rs_get_state: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_rs_get_viewports: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_rs_get_scissor_rects: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_hs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_hs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ds_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_cs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_clear_state: unsafe extern "system" fn(Raw),
    pub device_context_flush: unsafe extern "system" fn(Raw),
    pub device_context_get_type: unsafe extern "system" fn(Raw) -> i32,
    pub device_context_get_context_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_context_finish_command_list: unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
}

// SAFETY: `DeviceContextTable` lays out `ID3D11DeviceContext` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DeviceContext"
)]
unsafe impl Interface for DeviceContextTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DeviceContext1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceContext1Table {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_context_vs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ps_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_draw_indexed: unsafe extern "system" fn(Raw, u32, u32, i32),
    pub device_context_draw: unsafe extern "system" fn(Raw, u32, u32),
    pub device_context_map: unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *mut u8) -> i32,
    pub device_context_unmap: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_ps_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ia_set_input_layout: unsafe extern "system" fn(Raw, Raw),
    pub device_context_ia_set_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context_ia_set_index_buffer: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub device_context_draw_indexed_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32),
    pub device_context_draw_instanced: unsafe extern "system" fn(Raw, u32, u32, u32, u32),
    pub device_context_gs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ia_set_primitive_topology: unsafe extern "system" fn(Raw, i32),
    pub device_context_vs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_begin: unsafe extern "system" fn(Raw, Raw),
    pub device_context_end: unsafe extern "system" fn(Raw, Raw),
    pub device_context_get_data: unsafe extern "system" fn(Raw, Raw, *mut u8, u32, u32) -> i32,
    pub device_context_set_predication: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_gs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_om_set_render_targets: unsafe extern "system" fn(Raw, u32, *const Raw, Raw),
    pub device_context_om_set_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *const Raw, Raw, u32, u32, *const Raw, *const u8),
    pub device_context_om_set_blend_state: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub device_context_om_set_depth_stencil_state: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_so_set_targets: unsafe extern "system" fn(Raw, u32, *const Raw, *const u8),
    pub device_context_draw_auto: unsafe extern "system" fn(Raw),
    pub device_context_draw_indexed_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_draw_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_dispatch: unsafe extern "system" fn(Raw, u32, u32, u32),
    pub device_context_dispatch_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_rs_set_state: unsafe extern "system" fn(Raw, Raw),
    pub device_context_rs_set_viewports: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_rs_set_scissor_rects: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_copy_subresource_region:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8),
    pub device_context_copy_resource: unsafe extern "system" fn(Raw, Raw, Raw),
    pub device_context_update_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub device_context_copy_structure_count: unsafe extern "system" fn(Raw, Raw, u32, Raw),
    pub device_context_clear_render_target_view: unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_uint:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_float:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_depth_stencil_view: unsafe extern "system" fn(Raw, Raw, u32, f32, u8),
    pub device_context_generate_mips: unsafe extern "system" fn(Raw, Raw),
    pub device_context_set_resource_min_lod: unsafe extern "system" fn(Raw, Raw, f32),
    pub device_context_get_resource_min_lod: unsafe extern "system" fn(Raw, Raw) -> f32,
    pub device_context_resolve_subresource: unsafe extern "system" fn(Raw, Raw, u32, Raw, u32, i32),
    pub device_context_execute_command_list: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_hs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_hs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ds_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8),
    pub device_context_cs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_cs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ia_get_input_layout: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_ia_get_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context_ia_get_index_buffer:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_gs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ia_get_primitive_topology: unsafe extern "system" fn(Raw, *mut u8),
    pub device_context_vs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_get_predication: unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_gs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_om_get_render_targets:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw),
    pub device_context_om_get_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw, u32, u32, *mut Raw),
    pub device_context_om_get_blend_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_om_get_depth_stencil_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_so_get_targets: unsafe extern "system" fn(Raw, u32, *mut Raw),
    pub device_context_rs_get_state: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_rs_get_viewports: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_rs_get_scissor_rects: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_hs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_hs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ds_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_cs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_clear_state: unsafe extern "system" fn(Raw),
    pub device_context_flush: unsafe extern "system" fn(Raw),
    pub device_context_get_type: unsafe extern "system" fn(Raw) -> i32,
    pub device_context_get_context_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_context_finish_command_list: unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub device_context1_copy_subresource_region1:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8, u32),
    pub device_context1_update_subresource1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32, u32),
    pub device_context1_discard_resource: unsafe extern "system" fn(Raw, Raw),
    pub device_context1_discard_view: unsafe extern "system" fn(Raw, Raw),
    pub device_context1_vs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_hs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_ds_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_gs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_ps_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_cs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_vs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_hs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_ds_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_gs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_ps_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_cs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_swap_device_context_state: unsafe extern "system" fn(Raw, Raw, *mut Raw),
    pub device_context1_clear_view: unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, u32),
    pub device_context1_discard_view1: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
}

// SAFETY: `DeviceContext1Table` lays out `ID3D11DeviceContext1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DeviceContext1"
)]
unsafe impl Interface for DeviceContext1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DeviceContext2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceContext2Table {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_context_vs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ps_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_draw_indexed: unsafe extern "system" fn(Raw, u32, u32, i32),
    pub device_context_draw: unsafe extern "system" fn(Raw, u32, u32),
    pub device_context_map: unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *mut u8) -> i32,
    pub device_context_unmap: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_ps_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ia_set_input_layout: unsafe extern "system" fn(Raw, Raw),
    pub device_context_ia_set_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context_ia_set_index_buffer: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub device_context_draw_indexed_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32),
    pub device_context_draw_instanced: unsafe extern "system" fn(Raw, u32, u32, u32, u32),
    pub device_context_gs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ia_set_primitive_topology: unsafe extern "system" fn(Raw, i32),
    pub device_context_vs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_begin: unsafe extern "system" fn(Raw, Raw),
    pub device_context_end: unsafe extern "system" fn(Raw, Raw),
    pub device_context_get_data: unsafe extern "system" fn(Raw, Raw, *mut u8, u32, u32) -> i32,
    pub device_context_set_predication: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_gs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_om_set_render_targets: unsafe extern "system" fn(Raw, u32, *const Raw, Raw),
    pub device_context_om_set_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *const Raw, Raw, u32, u32, *const Raw, *const u8),
    pub device_context_om_set_blend_state: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub device_context_om_set_depth_stencil_state: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_so_set_targets: unsafe extern "system" fn(Raw, u32, *const Raw, *const u8),
    pub device_context_draw_auto: unsafe extern "system" fn(Raw),
    pub device_context_draw_indexed_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_draw_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_dispatch: unsafe extern "system" fn(Raw, u32, u32, u32),
    pub device_context_dispatch_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_rs_set_state: unsafe extern "system" fn(Raw, Raw),
    pub device_context_rs_set_viewports: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_rs_set_scissor_rects: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_copy_subresource_region:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8),
    pub device_context_copy_resource: unsafe extern "system" fn(Raw, Raw, Raw),
    pub device_context_update_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub device_context_copy_structure_count: unsafe extern "system" fn(Raw, Raw, u32, Raw),
    pub device_context_clear_render_target_view: unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_uint:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_float:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_depth_stencil_view: unsafe extern "system" fn(Raw, Raw, u32, f32, u8),
    pub device_context_generate_mips: unsafe extern "system" fn(Raw, Raw),
    pub device_context_set_resource_min_lod: unsafe extern "system" fn(Raw, Raw, f32),
    pub device_context_get_resource_min_lod: unsafe extern "system" fn(Raw, Raw) -> f32,
    pub device_context_resolve_subresource: unsafe extern "system" fn(Raw, Raw, u32, Raw, u32, i32),
    pub device_context_execute_command_list: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_hs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_hs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ds_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8),
    pub device_context_cs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_cs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ia_get_input_layout: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_ia_get_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context_ia_get_index_buffer:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_gs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ia_get_primitive_topology: unsafe extern "system" fn(Raw, *mut u8),
    pub device_context_vs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_get_predication: unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_gs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_om_get_render_targets:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw),
    pub device_context_om_get_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw, u32, u32, *mut Raw),
    pub device_context_om_get_blend_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_om_get_depth_stencil_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_so_get_targets: unsafe extern "system" fn(Raw, u32, *mut Raw),
    pub device_context_rs_get_state: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_rs_get_viewports: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_rs_get_scissor_rects: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_hs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_hs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ds_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_cs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_clear_state: unsafe extern "system" fn(Raw),
    pub device_context_flush: unsafe extern "system" fn(Raw),
    pub device_context_get_type: unsafe extern "system" fn(Raw) -> i32,
    pub device_context_get_context_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_context_finish_command_list: unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub device_context1_copy_subresource_region1:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8, u32),
    pub device_context1_update_subresource1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32, u32),
    pub device_context1_discard_resource: unsafe extern "system" fn(Raw, Raw),
    pub device_context1_discard_view: unsafe extern "system" fn(Raw, Raw),
    pub device_context1_vs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_hs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_ds_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_gs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_ps_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_cs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_vs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_hs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_ds_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_gs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_ps_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_cs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_swap_device_context_state: unsafe extern "system" fn(Raw, Raw, *mut Raw),
    pub device_context1_clear_view: unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, u32),
    pub device_context1_discard_view1: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub device_context2_update_tile_mappings: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *const u8,
        *const u8,
        Raw,
        u32,
        *const u8,
        *const u8,
        *const u8,
        u32,
    ) -> i32,
    pub device_context2_copy_tile_mappings:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *const u8, *const u8, u32) -> i32,
    pub device_context2_copy_tiles:
        unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, Raw, u64, u32),
    pub device_context2_update_tiles:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, u32),
    pub device_context2_resize_tile_pool: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
    pub device_context2_tiled_resource_barrier: unsafe extern "system" fn(Raw, Raw, Raw),
    pub device_context2_is_annotation_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub device_context2_set_marker_int: unsafe extern "system" fn(Raw, *const u8, i32),
    pub device_context2_begin_event_int: unsafe extern "system" fn(Raw, *const u8, i32),
    pub device_context2_end_event: unsafe extern "system" fn(Raw),
}

// SAFETY: `DeviceContext2Table` lays out `ID3D11DeviceContext2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DeviceContext2"
)]
unsafe impl Interface for DeviceContext2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DeviceContext3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DeviceContext3Table {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub device_context_vs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ps_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ps_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_draw_indexed: unsafe extern "system" fn(Raw, u32, u32, i32),
    pub device_context_draw: unsafe extern "system" fn(Raw, u32, u32),
    pub device_context_map: unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *mut u8) -> i32,
    pub device_context_unmap: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_ps_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ia_set_input_layout: unsafe extern "system" fn(Raw, Raw),
    pub device_context_ia_set_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context_ia_set_index_buffer: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub device_context_draw_indexed_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32),
    pub device_context_draw_instanced: unsafe extern "system" fn(Raw, u32, u32, u32, u32),
    pub device_context_gs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ia_set_primitive_topology: unsafe extern "system" fn(Raw, i32),
    pub device_context_vs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_begin: unsafe extern "system" fn(Raw, Raw),
    pub device_context_end: unsafe extern "system" fn(Raw, Raw),
    pub device_context_get_data: unsafe extern "system" fn(Raw, Raw, *mut u8, u32, u32) -> i32,
    pub device_context_set_predication: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_gs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_gs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_om_set_render_targets: unsafe extern "system" fn(Raw, u32, *const Raw, Raw),
    pub device_context_om_set_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *const Raw, Raw, u32, u32, *const Raw, *const u8),
    pub device_context_om_set_blend_state: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub device_context_om_set_depth_stencil_state: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_so_set_targets: unsafe extern "system" fn(Raw, u32, *const Raw, *const u8),
    pub device_context_draw_auto: unsafe extern "system" fn(Raw),
    pub device_context_draw_indexed_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_draw_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_dispatch: unsafe extern "system" fn(Raw, u32, u32, u32),
    pub device_context_dispatch_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub device_context_rs_set_state: unsafe extern "system" fn(Raw, Raw),
    pub device_context_rs_set_viewports: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_rs_set_scissor_rects: unsafe extern "system" fn(Raw, u32, *const u8),
    pub device_context_copy_subresource_region:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8),
    pub device_context_copy_resource: unsafe extern "system" fn(Raw, Raw, Raw),
    pub device_context_update_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub device_context_copy_structure_count: unsafe extern "system" fn(Raw, Raw, u32, Raw),
    pub device_context_clear_render_target_view: unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_uint:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_unordered_access_view_float:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub device_context_clear_depth_stencil_view: unsafe extern "system" fn(Raw, Raw, u32, f32, u8),
    pub device_context_generate_mips: unsafe extern "system" fn(Raw, Raw),
    pub device_context_set_resource_min_lod: unsafe extern "system" fn(Raw, Raw, f32),
    pub device_context_get_resource_min_lod: unsafe extern "system" fn(Raw, Raw) -> f32,
    pub device_context_resolve_subresource: unsafe extern "system" fn(Raw, Raw, u32, Raw, u32, i32),
    pub device_context_execute_command_list: unsafe extern "system" fn(Raw, Raw, i32),
    pub device_context_hs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_hs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_hs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_ds_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_ds_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8),
    pub device_context_cs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub device_context_cs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_cs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub device_context_vs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ps_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ps_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ia_get_input_layout: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_ia_get_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context_ia_get_index_buffer:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_gs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ia_get_primitive_topology: unsafe extern "system" fn(Raw, *mut u8),
    pub device_context_vs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_vs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_get_predication: unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_gs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_gs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_om_get_render_targets:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw),
    pub device_context_om_get_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw, u32, u32, *mut Raw),
    pub device_context_om_get_blend_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub device_context_om_get_depth_stencil_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub device_context_so_get_targets: unsafe extern "system" fn(Raw, u32, *mut Raw),
    pub device_context_rs_get_state: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_context_rs_get_viewports: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_rs_get_scissor_rects: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub device_context_hs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_hs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_hs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_ds_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_ds_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader_resources: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_shader: unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub device_context_cs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_cs_get_constant_buffers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub device_context_clear_state: unsafe extern "system" fn(Raw),
    pub device_context_flush: unsafe extern "system" fn(Raw),
    pub device_context_get_type: unsafe extern "system" fn(Raw) -> i32,
    pub device_context_get_context_flags: unsafe extern "system" fn(Raw) -> u32,
    pub device_context_finish_command_list: unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub device_context1_copy_subresource_region1:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8, u32),
    pub device_context1_update_subresource1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32, u32),
    pub device_context1_discard_resource: unsafe extern "system" fn(Raw, Raw),
    pub device_context1_discard_view: unsafe extern "system" fn(Raw, Raw),
    pub device_context1_vs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_hs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_ds_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_gs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_ps_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_cs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub device_context1_vs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_hs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_ds_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_gs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_ps_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_cs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub device_context1_swap_device_context_state: unsafe extern "system" fn(Raw, Raw, *mut Raw),
    pub device_context1_clear_view: unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, u32),
    pub device_context1_discard_view1: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub device_context2_update_tile_mappings: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *const u8,
        *const u8,
        Raw,
        u32,
        *const u8,
        *const u8,
        *const u8,
        u32,
    ) -> i32,
    pub device_context2_copy_tile_mappings:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *const u8, *const u8, u32) -> i32,
    pub device_context2_copy_tiles:
        unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, Raw, u64, u32),
    pub device_context2_update_tiles:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, u32),
    pub device_context2_resize_tile_pool: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
    pub device_context2_tiled_resource_barrier: unsafe extern "system" fn(Raw, Raw, Raw),
    pub device_context2_is_annotation_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub device_context2_set_marker_int: unsafe extern "system" fn(Raw, *const u8, i32),
    pub device_context2_begin_event_int: unsafe extern "system" fn(Raw, *const u8, i32),
    pub device_context2_end_event: unsafe extern "system" fn(Raw),
    pub device_context3_flush1: unsafe extern "system" fn(Raw, i32, usize),
    pub device_context3_set_hardware_protection_state: unsafe extern "system" fn(Raw, i32),
    pub device_context3_get_hardware_protection_state: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `DeviceContext3Table` lays out `ID3D11DeviceContext3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DeviceContext3"
)]
unsafe impl Interface for DeviceContext3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DeviceContext4`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DeviceContext4Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) device_context_vs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_ps_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_ps_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub(crate) device_context_ps_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_vs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub(crate) device_context_draw_indexed: unsafe extern "system" fn(Raw, u32, u32, i32),
    pub(crate) device_context_draw: unsafe extern "system" fn(Raw, u32, u32),
    pub(crate) device_context_map:
        unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *mut u8) -> i32,
    pub(crate) device_context_unmap: unsafe extern "system" fn(Raw, Raw, u32),
    pub(crate) device_context_ps_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_ia_set_input_layout: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context_ia_set_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context_ia_set_index_buffer: unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub(crate) device_context_draw_indexed_instanced:
        unsafe extern "system" fn(Raw, u32, u32, u32, i32, u32),
    pub(crate) device_context_draw_instanced: unsafe extern "system" fn(Raw, u32, u32, u32, u32),
    pub(crate) device_context_gs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_gs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub(crate) device_context_ia_set_primitive_topology: unsafe extern "system" fn(Raw, i32),
    pub(crate) device_context_vs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_vs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_begin: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context_end: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context_get_data:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, u32) -> i32,
    pub(crate) device_context_set_predication: unsafe extern "system" fn(Raw, Raw, i32),
    pub(crate) device_context_gs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_gs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_om_set_render_targets:
        unsafe extern "system" fn(Raw, u32, *const Raw, Raw),
    pub(crate) device_context_om_set_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *const Raw, Raw, u32, u32, *const Raw, *const u8),
    pub(crate) device_context_om_set_blend_state:
        unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub(crate) device_context_om_set_depth_stencil_state: unsafe extern "system" fn(Raw, Raw, u32),
    pub(crate) device_context_so_set_targets:
        unsafe extern "system" fn(Raw, u32, *const Raw, *const u8),
    pub(crate) device_context_draw_auto: unsafe extern "system" fn(Raw),
    pub(crate) device_context_draw_indexed_instanced_indirect:
        unsafe extern "system" fn(Raw, Raw, u32),
    pub(crate) device_context_draw_instanced_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub(crate) device_context_dispatch: unsafe extern "system" fn(Raw, u32, u32, u32),
    pub(crate) device_context_dispatch_indirect: unsafe extern "system" fn(Raw, Raw, u32),
    pub(crate) device_context_rs_set_state: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context_rs_set_viewports: unsafe extern "system" fn(Raw, u32, *const u8),
    pub(crate) device_context_rs_set_scissor_rects: unsafe extern "system" fn(Raw, u32, *const u8),
    pub(crate) device_context_copy_subresource_region:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8),
    pub(crate) device_context_copy_resource: unsafe extern "system" fn(Raw, Raw, Raw),
    pub(crate) device_context_update_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32),
    pub(crate) device_context_copy_structure_count: unsafe extern "system" fn(Raw, Raw, u32, Raw),
    pub(crate) device_context_clear_render_target_view:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub(crate) device_context_clear_unordered_access_view_uint:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub(crate) device_context_clear_unordered_access_view_float:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub(crate) device_context_clear_depth_stencil_view:
        unsafe extern "system" fn(Raw, Raw, u32, f32, u8),
    pub(crate) device_context_generate_mips: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context_set_resource_min_lod: unsafe extern "system" fn(Raw, Raw, f32),
    pub(crate) device_context_get_resource_min_lod: unsafe extern "system" fn(Raw, Raw) -> f32,
    pub(crate) device_context_resolve_subresource:
        unsafe extern "system" fn(Raw, Raw, u32, Raw, u32, i32),
    pub(crate) device_context_execute_command_list: unsafe extern "system" fn(Raw, Raw, i32),
    pub(crate) device_context_hs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_hs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub(crate) device_context_hs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_hs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_ds_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_ds_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub(crate) device_context_ds_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_ds_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_cs_set_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_cs_set_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8),
    pub(crate) device_context_cs_set_shader: unsafe extern "system" fn(Raw, Raw, *const Raw, u32),
    pub(crate) device_context_cs_set_samplers: unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_cs_set_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw),
    pub(crate) device_context_vs_get_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_ps_get_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_ps_get_shader:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub(crate) device_context_ps_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_vs_get_shader:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub(crate) device_context_ps_get_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_ia_get_input_layout: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_context_ia_get_vertex_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context_ia_get_index_buffer:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context_gs_get_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_gs_get_shader:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub(crate) device_context_ia_get_primitive_topology: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) device_context_vs_get_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_vs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_get_predication: unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub(crate) device_context_gs_get_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_gs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_om_get_render_targets:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw),
    pub(crate) device_context_om_get_render_targets_and_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, *mut Raw, *mut Raw, u32, u32, *mut Raw),
    pub(crate) device_context_om_get_blend_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context_om_get_depth_stencil_state:
        unsafe extern "system" fn(Raw, *mut Raw, *mut u8),
    pub(crate) device_context_so_get_targets: unsafe extern "system" fn(Raw, u32, *mut Raw),
    pub(crate) device_context_rs_get_state: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_context_rs_get_viewports: unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub(crate) device_context_rs_get_scissor_rects:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8),
    pub(crate) device_context_hs_get_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_hs_get_shader:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub(crate) device_context_hs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_hs_get_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_ds_get_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_ds_get_shader:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub(crate) device_context_ds_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_ds_get_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_cs_get_shader_resources:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_cs_get_unordered_access_views:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_cs_get_shader:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8),
    pub(crate) device_context_cs_get_samplers: unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_cs_get_constant_buffers:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw),
    pub(crate) device_context_clear_state: unsafe extern "system" fn(Raw),
    pub(crate) device_context_flush: unsafe extern "system" fn(Raw),
    pub(crate) device_context_get_type: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_context_get_context_flags: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) device_context_finish_command_list:
        unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub(crate) device_context1_copy_subresource_region1:
        unsafe extern "system" fn(Raw, Raw, u32, u32, u32, u32, Raw, u32, *const u8, u32),
    pub(crate) device_context1_update_subresource1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, u32, u32, u32),
    pub(crate) device_context1_discard_resource: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context1_discard_view: unsafe extern "system" fn(Raw, Raw),
    pub(crate) device_context1_vs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context1_hs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context1_ds_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context1_gs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context1_ps_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context1_cs_set_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *const Raw, *const u8, *const u8),
    pub(crate) device_context1_vs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context1_hs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context1_ds_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context1_gs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context1_ps_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context1_cs_get_constant_buffers1:
        unsafe extern "system" fn(Raw, u32, u32, *mut Raw, *mut u8, *mut u8),
    pub(crate) device_context1_swap_device_context_state:
        unsafe extern "system" fn(Raw, Raw, *mut Raw),
    pub(crate) device_context1_clear_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, u32),
    pub(crate) device_context1_discard_view1: unsafe extern "system" fn(Raw, Raw, *const u8, u32),
    pub(crate) device_context2_update_tile_mappings: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *const u8,
        *const u8,
        Raw,
        u32,
        *const u8,
        *const u8,
        *const u8,
        u32,
    ) -> i32,
    pub(crate) device_context2_copy_tile_mappings:
        unsafe extern "system" fn(Raw, Raw, *const u8, Raw, *const u8, *const u8, u32) -> i32,
    pub(crate) device_context2_copy_tiles:
        unsafe extern "system" fn(Raw, Raw, *const u8, *const u8, Raw, u64, u32),
    pub(crate) device_context2_update_tiles:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8, *mut u8, u32),
    pub(crate) device_context2_resize_tile_pool: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
    pub(crate) device_context2_tiled_resource_barrier: unsafe extern "system" fn(Raw, Raw, Raw),
    pub(crate) device_context2_is_annotation_enabled: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) device_context2_set_marker_int: unsafe extern "system" fn(Raw, *const u8, i32),
    pub(crate) device_context2_begin_event_int: unsafe extern "system" fn(Raw, *const u8, i32),
    pub(crate) device_context2_end_event: unsafe extern "system" fn(Raw),
    pub(crate) device_context3_flush1: unsafe extern "system" fn(Raw, i32, usize),
    pub(crate) device_context3_set_hardware_protection_state: unsafe extern "system" fn(Raw, i32),
    pub(crate) device_context3_get_hardware_protection_state:
        unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) device_context4_signal: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
    pub(crate) device_context4_wait: unsafe extern "system" fn(Raw, Raw, u64) -> i32,
}

// SAFETY: `DeviceContext4Table` lays out `ID3D11DeviceContext4` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DeviceContext4"
)]
unsafe impl Interface for DeviceContext4Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11DomainShader`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DomainShaderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `DomainShaderTable` lays out `ID3D11DomainShader` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11DomainShader"
)]
unsafe impl Interface for DomainShaderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Fence`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct FenceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) fence_create_shared_handle:
        unsafe extern "system" fn(Raw, *mut u8, u32, *const u8, *mut u8) -> i32,
    pub(crate) fence_get_completed_value: unsafe extern "system" fn(Raw) -> u64,
    pub(crate) fence_set_event_on_completion: unsafe extern "system" fn(Raw, u64, usize) -> i32,
}

// SAFETY: `FenceTable` lays out `ID3D11Fence` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Fence"
)]
unsafe impl Interface for FenceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11GeometryShader`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct GeometryShaderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `GeometryShaderTable` lays out `ID3D11GeometryShader` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11GeometryShader"
)]
unsafe impl Interface for GeometryShaderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11HullShader`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct HullShaderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `HullShaderTable` lays out `ID3D11HullShader` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11HullShader"
)]
unsafe impl Interface for HullShaderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11InfoQueue`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct InfoQueueTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) info_queue_set_message_count_limit: unsafe extern "system" fn(Raw, u64) -> i32,
    pub(crate) info_queue_clear_stored_messages: unsafe extern "system" fn(Raw),
    pub(crate) info_queue_get_message: unsafe extern "system" fn(Raw, u64, *mut u8, *mut u8) -> i32,
    pub(crate) info_queue_get_num_messages_allowed_by_storage_filter:
        unsafe extern "system" fn(Raw) -> u64,
    pub(crate) info_queue_get_num_messages_denied_by_storage_filter:
        unsafe extern "system" fn(Raw) -> u64,
    pub(crate) info_queue_get_num_stored_messages: unsafe extern "system" fn(Raw) -> u64,
    pub(crate) info_queue_get_num_stored_messages_allowed_by_retrieval_filter:
        unsafe extern "system" fn(Raw) -> u64,
    pub(crate) info_queue_get_num_messages_discarded_by_message_count_limit:
        unsafe extern "system" fn(Raw) -> u64,
    pub(crate) info_queue_get_message_count_limit: unsafe extern "system" fn(Raw) -> u64,
    pub(crate) info_queue_add_storage_filter_entries:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) info_queue_get_storage_filter:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) info_queue_clear_storage_filter: unsafe extern "system" fn(Raw),
    pub(crate) info_queue_push_empty_storage_filter: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) info_queue_push_copy_of_storage_filter: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) info_queue_push_storage_filter: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) info_queue_pop_storage_filter: unsafe extern "system" fn(Raw),
    pub(crate) info_queue_get_storage_filter_stack_size: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) info_queue_add_retrieval_filter_entries:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) info_queue_get_retrieval_filter:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) info_queue_clear_retrieval_filter: unsafe extern "system" fn(Raw),
    pub(crate) info_queue_push_empty_retrieval_filter: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) info_queue_push_copy_of_retrieval_filter: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) info_queue_push_retrieval_filter: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) info_queue_pop_retrieval_filter: unsafe extern "system" fn(Raw),
    pub(crate) info_queue_get_retrieval_filter_stack_size: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) info_queue_add_message:
        unsafe extern "system" fn(Raw, i32, i32, i32, *const u8) -> i32,
    pub(crate) info_queue_add_application_message:
        unsafe extern "system" fn(Raw, i32, *const u8) -> i32,
    pub(crate) info_queue_set_break_on_category: unsafe extern "system" fn(Raw, i32, i32) -> i32,
    pub(crate) info_queue_set_break_on_severity: unsafe extern "system" fn(Raw, i32, i32) -> i32,
    pub(crate) info_queue_set_break_on_id: unsafe extern "system" fn(Raw, i32, i32) -> i32,
    pub(crate) info_queue_get_break_on_category: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) info_queue_get_break_on_severity: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) info_queue_get_break_on_id: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) info_queue_set_mute_debug_output: unsafe extern "system" fn(Raw, i32),
    pub(crate) info_queue_get_mute_debug_output: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `InfoQueueTable` lays out `ID3D11InfoQueue` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11InfoQueue"
)]
unsafe impl Interface for InfoQueueTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11InputLayout`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct InputLayoutTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `InputLayoutTable` lays out `ID3D11InputLayout` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11InputLayout"
)]
unsafe impl Interface for InputLayoutTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Multithread`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct MultithreadTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) multithread_enter: unsafe extern "system" fn(Raw),
    pub(crate) multithread_leave: unsafe extern "system" fn(Raw),
    pub(crate) multithread_set_multithread_protected: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) multithread_get_multithread_protected: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `MultithreadTable` lays out `ID3D11Multithread` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Multithread"
)]
unsafe impl Interface for MultithreadTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11PixelShader`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct PixelShaderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `PixelShaderTable` lays out `ID3D11PixelShader` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11PixelShader"
)]
unsafe impl Interface for PixelShaderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Predicate`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct PredicateTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) asynchronous_get_data_size: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) query_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `PredicateTable` lays out `ID3D11Predicate` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Predicate"
)]
unsafe impl Interface for PredicateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Query`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct QueryTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub asynchronous_get_data_size: unsafe extern "system" fn(Raw) -> u32,
    pub query_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `QueryTable` lays out `ID3D11Query` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Query"
)]
unsafe impl Interface for QueryTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Query1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Query1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) asynchronous_get_data_size: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) query_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) query1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `Query1Table` lays out `ID3D11Query1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Query1"
)]
unsafe impl Interface for Query1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RasterizerState`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct RasterizerStateTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub rasterizer_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `RasterizerStateTable` lays out `ID3D11RasterizerState` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RasterizerState"
)]
unsafe impl Interface for RasterizerStateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RasterizerState1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct RasterizerState1Table {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub rasterizer_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub rasterizer_state1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `RasterizerState1Table` lays out `ID3D11RasterizerState1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RasterizerState1"
)]
unsafe impl Interface for RasterizerState1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RasterizerState2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct RasterizerState2Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) rasterizer_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) rasterizer_state1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) rasterizer_state2_get_desc2: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `RasterizerState2Table` lays out `ID3D11RasterizerState2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RasterizerState2"
)]
unsafe impl Interface for RasterizerState2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RefDefaultTrackingOptions`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct RefDefaultTrackingOptionsTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) ref_default_tracking_options_set_tracking_options:
        unsafe extern "system" fn(Raw, u32, u32) -> i32,
}

// SAFETY: `RefDefaultTrackingOptionsTable` lays out `ID3D11RefDefaultTrackingOptions` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RefDefaultTrackingOptions"
)]
unsafe impl Interface for RefDefaultTrackingOptionsTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RefTrackingOptions`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct RefTrackingOptionsTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) ref_tracking_options_set_tracking_options:
        unsafe extern "system" fn(Raw, u32) -> i32,
}

// SAFETY: `RefTrackingOptionsTable` lays out `ID3D11RefTrackingOptions` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RefTrackingOptions"
)]
unsafe impl Interface for RefTrackingOptionsTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RenderTargetView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct RenderTargetViewTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub render_target_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `RenderTargetViewTable` lays out `ID3D11RenderTargetView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RenderTargetView"
)]
unsafe impl Interface for RenderTargetViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11RenderTargetView1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct RenderTargetView1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) render_target_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) render_target_view1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `RenderTargetView1Table` lays out `ID3D11RenderTargetView1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11RenderTargetView1"
)]
unsafe impl Interface for RenderTargetView1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Resource`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct ResourceTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
}

// SAFETY: `ResourceTable` lays out `ID3D11Resource` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Resource"
)]
unsafe impl Interface for ResourceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11SamplerState`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct SamplerStateTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) sampler_state_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `SamplerStateTable` lays out `ID3D11SamplerState` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11SamplerState"
)]
unsafe impl Interface for SamplerStateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ShaderResourceView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct ShaderResourceViewTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub shader_resource_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `ShaderResourceViewTable` lays out `ID3D11ShaderResourceView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ShaderResourceView"
)]
unsafe impl Interface for ShaderResourceViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ShaderResourceView1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ShaderResourceView1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) shader_resource_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) shader_resource_view1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `ShaderResourceView1Table` lays out `ID3D11ShaderResourceView1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ShaderResourceView1"
)]
unsafe impl Interface for ShaderResourceView1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ShaderTrace`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ShaderTraceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) shader_trace_trace_ready: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) shader_trace_reset_trace: unsafe extern "system" fn(Raw),
    pub(crate) shader_trace_get_trace_stats: unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) shader_trace_ps_select_stamp: unsafe extern "system" fn(Raw, u32) -> i32,
    pub(crate) shader_trace_get_initial_register_contents:
        unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub(crate) shader_trace_get_step: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) shader_trace_get_written_register:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8, *mut u8) -> i32,
    pub(crate) shader_trace_get_read_register:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8, *mut u8) -> i32,
}

// SAFETY: `ShaderTraceTable` lays out `ID3D11ShaderTrace` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ShaderTrace"
)]
unsafe impl Interface for ShaderTraceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11ShaderTraceFactory`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct ShaderTraceFactoryTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) shader_trace_factory_create_shader_trace:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
}

// SAFETY: `ShaderTraceFactoryTable` lays out `ID3D11ShaderTraceFactory` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11ShaderTraceFactory"
)]
unsafe impl Interface for ShaderTraceFactoryTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11SwitchToRef`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct SwitchToRefTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) switch_to_ref_set_use_ref: unsafe extern "system" fn(Raw, i32) -> i32,
    pub(crate) switch_to_ref_get_use_ref: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `SwitchToRefTable` lays out `ID3D11SwitchToRef` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11SwitchToRef"
)]
unsafe impl Interface for SwitchToRefTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Texture1D`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Texture1DTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub(crate) resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) texture_1d_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `Texture1DTable` lays out `ID3D11Texture1D` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Texture1D"
)]
unsafe impl Interface for Texture1DTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Texture2D`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Texture2DTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
    pub texture_2d_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `Texture2DTable` lays out `ID3D11Texture2D` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Texture2D"
)]
unsafe impl Interface for Texture2DTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Texture2D1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Texture2D1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub(crate) resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) texture_2d_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) texture_2d1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `Texture2D1Table` lays out `ID3D11Texture2D1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Texture2D1"
)]
unsafe impl Interface for Texture2D1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Texture3D`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct Texture3DTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
    pub texture_3d_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `Texture3DTable` lays out `ID3D11Texture3D` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Texture3D"
)]
unsafe impl Interface for Texture3DTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11Texture3D1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct Texture3D1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) resource_get_type: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) resource_set_eviction_priority: unsafe extern "system" fn(Raw, u32),
    pub(crate) resource_get_eviction_priority: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) texture_3d_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) texture_3d1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `Texture3D1Table` lays out `ID3D11Texture3D1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11Texture3D1"
)]
unsafe impl Interface for Texture3D1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11TracingDevice`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct TracingDeviceTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) tracing_device_set_shader_tracking_options_by_type:
        unsafe extern "system" fn(Raw, u32, u32) -> i32,
    pub(crate) tracing_device_set_shader_tracking_options:
        unsafe extern "system" fn(Raw, Raw, u32) -> i32,
}

// SAFETY: `TracingDeviceTable` lays out `ID3D11TracingDevice` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11TracingDevice"
)]
unsafe impl Interface for TracingDeviceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11UnorderedAccessView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct UnorderedAccessViewTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub unordered_access_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `UnorderedAccessViewTable` lays out `ID3D11UnorderedAccessView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11UnorderedAccessView"
)]
unsafe impl Interface for UnorderedAccessViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11UnorderedAccessView1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct UnorderedAccessView1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) unordered_access_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) unordered_access_view1_get_desc1: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `UnorderedAccessView1Table` lays out `ID3D11UnorderedAccessView1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11UnorderedAccessView1"
)]
unsafe impl Interface for UnorderedAccessView1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VertexShader`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VertexShaderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `VertexShaderTable` lays out `ID3D11VertexShader` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VertexShader"
)]
unsafe impl Interface for VertexShaderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoContext`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct VideoContextTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub video_context_get_decoder_buffer:
        unsafe extern "system" fn(Raw, Raw, i32, *mut u8, *mut u8) -> i32,
    pub video_context_release_decoder_buffer: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub video_context_decoder_begin_frame:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, *const u8) -> i32,
    pub video_context_decoder_end_frame: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub video_context_submit_decoder_buffers:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_decoder_extension: unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub video_context_video_processor_set_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub video_context_video_processor_set_output_background_color:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub video_context_video_processor_set_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub video_context_video_processor_set_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub video_context_video_processor_set_output_constriction:
        unsafe extern "system" fn(Raw, Raw, i32, u64),
    pub video_context_video_processor_set_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context_video_processor_set_output_extension:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_get_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_background_color:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context_video_processor_get_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_constriction:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context_video_processor_get_output_extension:
        unsafe extern "system" fn(Raw, Raw, *const u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_set_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context_video_processor_set_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8),
    pub video_context_video_processor_set_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, *const u8),
    pub video_context_video_processor_set_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub video_context_video_processor_set_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub video_context_video_processor_set_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32),
    pub video_context_video_processor_set_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *const u8),
    pub video_context_video_processor_set_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8, *const u8),
    pub video_context_video_processor_set_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32, f32),
    pub video_context_video_processor_set_stream_stereo_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32, i32, i32, i32),
    pub video_context_video_processor_set_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context_video_processor_set_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub video_context_video_processor_set_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_get_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *mut u8),
    pub video_context_video_processor_get_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_stereo_format: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
    ),
    pub video_context_video_processor_get_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, u32, *mut u8) -> i32,
    pub video_context_negotiate_crypto_session_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_encryption_blt: unsafe extern "system" fn(Raw, Raw, Raw, Raw, u32, *mut u8),
    pub video_context_decryption_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, Raw, *const u8, u32, *const u8, u32, *mut u8),
    pub video_context_start_session_key_refresh: unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_finish_session_key_refresh: unsafe extern "system" fn(Raw, Raw),
    pub video_context_get_encryption_blt_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_negotiate_authenticated_channel_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_query_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub video_context_configure_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub video_context_video_processor_set_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32),
    pub video_context_video_processor_get_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
}

// SAFETY: `VideoContextTable` lays out `ID3D11VideoContext` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoContext"
)]
unsafe impl Interface for VideoContextTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoContext1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct VideoContext1Table {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub video_context_get_decoder_buffer:
        unsafe extern "system" fn(Raw, Raw, i32, *mut u8, *mut u8) -> i32,
    pub video_context_release_decoder_buffer: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub video_context_decoder_begin_frame:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, *const u8) -> i32,
    pub video_context_decoder_end_frame: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub video_context_submit_decoder_buffers:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_decoder_extension: unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub video_context_video_processor_set_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub video_context_video_processor_set_output_background_color:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub video_context_video_processor_set_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub video_context_video_processor_set_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub video_context_video_processor_set_output_constriction:
        unsafe extern "system" fn(Raw, Raw, i32, u64),
    pub video_context_video_processor_set_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context_video_processor_set_output_extension:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_get_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_background_color:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context_video_processor_get_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_constriction:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context_video_processor_get_output_extension:
        unsafe extern "system" fn(Raw, Raw, *const u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_set_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context_video_processor_set_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8),
    pub video_context_video_processor_set_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, *const u8),
    pub video_context_video_processor_set_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub video_context_video_processor_set_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub video_context_video_processor_set_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32),
    pub video_context_video_processor_set_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *const u8),
    pub video_context_video_processor_set_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8, *const u8),
    pub video_context_video_processor_set_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32, f32),
    pub video_context_video_processor_set_stream_stereo_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32, i32, i32, i32),
    pub video_context_video_processor_set_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context_video_processor_set_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub video_context_video_processor_set_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_get_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *mut u8),
    pub video_context_video_processor_get_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_stereo_format: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
    ),
    pub video_context_video_processor_get_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, u32, *mut u8) -> i32,
    pub video_context_negotiate_crypto_session_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_encryption_blt: unsafe extern "system" fn(Raw, Raw, Raw, Raw, u32, *mut u8),
    pub video_context_decryption_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, Raw, *const u8, u32, *const u8, u32, *mut u8),
    pub video_context_start_session_key_refresh: unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_finish_session_key_refresh: unsafe extern "system" fn(Raw, Raw),
    pub video_context_get_encryption_blt_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_negotiate_authenticated_channel_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_query_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub video_context_configure_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub video_context_video_processor_set_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32),
    pub video_context_video_processor_get_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context1_submit_decoder_buffers1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context1_get_data_for_new_hardware_key:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub video_context1_check_crypto_session_status:
        unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub video_context1_decoder_enable_downsampling:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8, u32) -> i32,
    pub video_context1_decoder_update_downsampling:
        unsafe extern "system" fn(Raw, Raw, *const u8) -> i32,
    pub video_context1_video_processor_set_output_color_space1:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context1_video_processor_set_output_shader_usage:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context1_video_processor_get_output_color_space1:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context1_video_processor_get_output_shader_usage:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context1_video_processor_set_stream_color_space1:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context1_video_processor_set_stream_mirror:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub video_context1_video_processor_get_stream_color_space1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context1_video_processor_get_stream_mirror:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context1_video_processor_get_behavior_hints:
        unsafe extern "system" fn(Raw, Raw, u32, u32, i32, u32, *const u8, *mut u8) -> i32,
}

// SAFETY: `VideoContext1Table` lays out `ID3D11VideoContext1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoContext1"
)]
unsafe impl Interface for VideoContext1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoContext2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct VideoContext2Table {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub video_context_get_decoder_buffer:
        unsafe extern "system" fn(Raw, Raw, i32, *mut u8, *mut u8) -> i32,
    pub video_context_release_decoder_buffer: unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub video_context_decoder_begin_frame:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, *const u8) -> i32,
    pub video_context_decoder_end_frame: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub video_context_submit_decoder_buffers:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_decoder_extension: unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub video_context_video_processor_set_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub video_context_video_processor_set_output_background_color:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub video_context_video_processor_set_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub video_context_video_processor_set_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub video_context_video_processor_set_output_constriction:
        unsafe extern "system" fn(Raw, Raw, i32, u64),
    pub video_context_video_processor_set_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context_video_processor_set_output_extension:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_get_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_background_color:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context_video_processor_get_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_constriction:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub video_context_video_processor_get_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context_video_processor_get_output_extension:
        unsafe extern "system" fn(Raw, Raw, *const u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_set_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context_video_processor_set_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8),
    pub video_context_video_processor_set_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, *const u8),
    pub video_context_video_processor_set_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub video_context_video_processor_set_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub video_context_video_processor_set_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32),
    pub video_context_video_processor_set_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *const u8),
    pub video_context_video_processor_set_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8, *const u8),
    pub video_context_video_processor_set_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32, f32),
    pub video_context_video_processor_set_stream_stereo_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32, i32, i32, i32),
    pub video_context_video_processor_set_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context_video_processor_set_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub video_context_video_processor_set_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_get_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *mut u8),
    pub video_context_video_processor_get_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_stereo_format: unsafe extern "system" fn(
        Raw,
        Raw,
        u32,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
        *mut u8,
    ),
    pub video_context_video_processor_get_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_video_processor_get_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *mut u8, *mut u8),
    pub video_context_video_processor_get_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub video_context_video_processor_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, u32, *mut u8) -> i32,
    pub video_context_negotiate_crypto_session_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_encryption_blt: unsafe extern "system" fn(Raw, Raw, Raw, Raw, u32, *mut u8),
    pub video_context_decryption_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, Raw, *const u8, u32, *const u8, u32, *mut u8),
    pub video_context_start_session_key_refresh: unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context_finish_session_key_refresh: unsafe extern "system" fn(Raw, Raw),
    pub video_context_get_encryption_blt_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_negotiate_authenticated_channel_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context_query_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub video_context_configure_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub video_context_video_processor_set_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32),
    pub video_context_video_processor_get_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub video_context1_submit_decoder_buffers1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub video_context1_get_data_for_new_hardware_key:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub video_context1_check_crypto_session_status:
        unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub video_context1_decoder_enable_downsampling:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8, u32) -> i32,
    pub video_context1_decoder_update_downsampling:
        unsafe extern "system" fn(Raw, Raw, *const u8) -> i32,
    pub video_context1_video_processor_set_output_color_space1:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context1_video_processor_set_output_shader_usage:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub video_context1_video_processor_get_output_color_space1:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context1_video_processor_get_output_shader_usage:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub video_context1_video_processor_set_stream_color_space1:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub video_context1_video_processor_set_stream_mirror:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub video_context1_video_processor_get_stream_color_space1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub video_context1_video_processor_get_stream_mirror:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub video_context1_video_processor_get_behavior_hints:
        unsafe extern "system" fn(Raw, Raw, u32, u32, i32, u32, *const u8, *mut u8) -> i32,
    pub video_context2_video_processor_set_output_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, i32, u32, *const u8),
    pub video_context2_video_processor_get_output_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *mut u8),
    pub video_context2_video_processor_set_stream_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *const u8),
    pub video_context2_video_processor_get_stream_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, u32, *mut u8),
}

// SAFETY: `VideoContext2Table` lays out `ID3D11VideoContext2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoContext2"
)]
unsafe impl Interface for VideoContext2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoContext3`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoContext3Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) video_context_get_decoder_buffer:
        unsafe extern "system" fn(Raw, Raw, i32, *mut u8, *mut u8) -> i32,
    pub(crate) video_context_release_decoder_buffer:
        unsafe extern "system" fn(Raw, Raw, i32) -> i32,
    pub(crate) video_context_decoder_begin_frame:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, *const u8) -> i32,
    pub(crate) video_context_decoder_end_frame: unsafe extern "system" fn(Raw, Raw) -> i32,
    pub(crate) video_context_submit_decoder_buffers:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub(crate) video_context_decoder_extension: unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub(crate) video_context_video_processor_set_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub(crate) video_context_video_processor_set_output_background_color:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8),
    pub(crate) video_context_video_processor_set_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *const u8),
    pub(crate) video_context_video_processor_set_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, i32, u32),
    pub(crate) video_context_video_processor_set_output_constriction:
        unsafe extern "system" fn(Raw, Raw, i32, u64),
    pub(crate) video_context_video_processor_set_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub(crate) video_context_video_processor_set_output_extension:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *mut u8) -> i32,
    pub(crate) video_context_video_processor_get_output_target_rect:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_output_background_color:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_output_color_space:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub(crate) video_context_video_processor_get_output_alpha_fill_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_output_constriction:
        unsafe extern "system" fn(Raw, Raw, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_output_stereo_mode:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub(crate) video_context_video_processor_get_output_extension:
        unsafe extern "system" fn(Raw, Raw, *const u8, u32, *mut u8) -> i32,
    pub(crate) video_context_video_processor_set_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub(crate) video_context_video_processor_set_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8),
    pub(crate) video_context_video_processor_set_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, *const u8),
    pub(crate) video_context_video_processor_set_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub(crate) video_context_video_processor_set_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8),
    pub(crate) video_context_video_processor_set_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32),
    pub(crate) video_context_video_processor_set_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *const u8),
    pub(crate) video_context_video_processor_set_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *const u8, *const u8),
    pub(crate) video_context_video_processor_set_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, i32, f32, f32),
    pub(crate) video_context_video_processor_set_stream_stereo_format:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32, i32, i32, i32),
    pub(crate) video_context_video_processor_set_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub(crate) video_context_video_processor_set_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub(crate) video_context_video_processor_set_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, u32, *mut u8) -> i32,
    pub(crate) video_context_video_processor_get_stream_frame_format:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub(crate) video_context_video_processor_get_stream_color_space:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub(crate) video_context_video_processor_get_stream_output_rate:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_source_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_dest_rect:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_alpha:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_palette:
        unsafe extern "system" fn(Raw, Raw, u32, u32, *mut u8),
    pub(crate) video_context_video_processor_get_stream_pixel_aspect_ratio:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_luma_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_stereo_format:
        unsafe extern "system" fn(
            Raw,
            Raw,
            u32,
            *mut u8,
            *mut u8,
            *mut u8,
            *mut u8,
            *mut u8,
            *mut u8,
        ),
    pub(crate) video_context_video_processor_get_stream_auto_processing_mode:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub(crate) video_context_video_processor_get_stream_filter:
        unsafe extern "system" fn(Raw, Raw, u32, i32, *mut u8, *mut u8),
    pub(crate) video_context_video_processor_get_stream_extension:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub(crate) video_context_video_processor_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, u32, *mut u8) -> i32,
    pub(crate) video_context_negotiate_crypto_session_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub(crate) video_context_encryption_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, Raw, u32, *mut u8),
    pub(crate) video_context_decryption_blt:
        unsafe extern "system" fn(Raw, Raw, Raw, Raw, *const u8, u32, *const u8, u32, *mut u8),
    pub(crate) video_context_start_session_key_refresh:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub(crate) video_context_finish_session_key_refresh: unsafe extern "system" fn(Raw, Raw),
    pub(crate) video_context_get_encryption_blt_key:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub(crate) video_context_negotiate_authenticated_channel_key_exchange:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub(crate) video_context_query_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, u32, *mut u8) -> i32,
    pub(crate) video_context_configure_authenticated_channel:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub(crate) video_context_video_processor_set_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32),
    pub(crate) video_context_video_processor_get_stream_rotation:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8),
    pub(crate) video_context1_submit_decoder_buffers1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
    pub(crate) video_context1_get_data_for_new_hardware_key:
        unsafe extern "system" fn(Raw, Raw, u32, *const u8, *mut u8) -> i32,
    pub(crate) video_context1_check_crypto_session_status:
        unsafe extern "system" fn(Raw, Raw, *mut u8) -> i32,
    pub(crate) video_context1_decoder_enable_downsampling:
        unsafe extern "system" fn(Raw, Raw, i32, *const u8, u32) -> i32,
    pub(crate) video_context1_decoder_update_downsampling:
        unsafe extern "system" fn(Raw, Raw, *const u8) -> i32,
    pub(crate) video_context1_video_processor_set_output_color_space1:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub(crate) video_context1_video_processor_set_output_shader_usage:
        unsafe extern "system" fn(Raw, Raw, i32),
    pub(crate) video_context1_video_processor_get_output_color_space1:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub(crate) video_context1_video_processor_get_output_shader_usage:
        unsafe extern "system" fn(Raw, Raw, *mut u8),
    pub(crate) video_context1_video_processor_set_stream_color_space1:
        unsafe extern "system" fn(Raw, Raw, u32, i32),
    pub(crate) video_context1_video_processor_set_stream_mirror:
        unsafe extern "system" fn(Raw, Raw, u32, i32, i32, i32),
    pub(crate) video_context1_video_processor_get_stream_color_space1:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8),
    pub(crate) video_context1_video_processor_get_stream_mirror:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, *mut u8, *mut u8),
    pub(crate) video_context1_video_processor_get_behavior_hints:
        unsafe extern "system" fn(Raw, Raw, u32, u32, i32, u32, *const u8, *mut u8) -> i32,
    pub(crate) video_context2_video_processor_set_output_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, i32, u32, *const u8),
    pub(crate) video_context2_video_processor_get_output_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, *mut u8, u32, *mut u8),
    pub(crate) video_context2_video_processor_set_stream_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, u32, i32, u32, *const u8),
    pub(crate) video_context2_video_processor_get_stream_hdr_meta_data:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8, u32, *mut u8),
    pub(crate) video_context3_decoder_begin_frame1:
        unsafe extern "system" fn(Raw, Raw, Raw, u32, *const u8, u32, *const u8, *const Raw) -> i32,
    pub(crate) video_context3_submit_decoder_buffers2:
        unsafe extern "system" fn(Raw, Raw, u32, *mut u8) -> i32,
}

// SAFETY: `VideoContext3Table` lays out `ID3D11VideoContext3` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoContext3"
)]
unsafe impl Interface for VideoContext3Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoDecoder`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoDecoderTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) video_decoder_get_creation_parameters:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8) -> i32,
    pub(crate) video_decoder_get_driver_handle: unsafe extern "system" fn(Raw, *mut u8) -> i32,
}

// SAFETY: `VideoDecoderTable` lays out `ID3D11VideoDecoder` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoDecoder"
)]
unsafe impl Interface for VideoDecoderTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoDecoderOutputView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoDecoderOutputViewTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) video_decoder_output_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `VideoDecoderOutputViewTable` lays out `ID3D11VideoDecoderOutputView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoDecoderOutputView"
)]
unsafe impl Interface for VideoDecoderOutputViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoDevice`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct VideoDeviceTable {
    pub unknown: UnknownTable,
    pub video_device_create_video_decoder:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor:
        unsafe extern "system" fn(Raw, Raw, u32, *mut Raw) -> i32,
    pub video_device_create_authenticated_channel:
        unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub video_device_create_crypto_session:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_decoder_output_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor_input_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor_output_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor_enumerator:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub video_device_get_video_decoder_profile_count: unsafe extern "system" fn(Raw) -> u32,
    pub video_device_get_video_decoder_profile: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub video_device_check_video_decoder_format:
        unsafe extern "system" fn(Raw, *const u8, i32, *mut u8) -> i32,
    pub video_device_get_video_decoder_config_count:
        unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub video_device_get_video_decoder_config:
        unsafe extern "system" fn(Raw, *const u8, u32, *mut u8) -> i32,
    pub video_device_get_content_protection_caps:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut u8) -> i32,
    pub video_device_check_crypto_key_exchange:
        unsafe extern "system" fn(Raw, *const u8, *const u8, u32, *mut u8) -> i32,
    pub video_device_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub video_device_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `VideoDeviceTable` lays out `ID3D11VideoDevice` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoDevice"
)]
unsafe impl Interface for VideoDeviceTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoDevice1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct VideoDevice1Table {
    pub unknown: UnknownTable,
    pub video_device_create_video_decoder:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor:
        unsafe extern "system" fn(Raw, Raw, u32, *mut Raw) -> i32,
    pub video_device_create_authenticated_channel:
        unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub video_device_create_crypto_session:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_decoder_output_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor_input_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor_output_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub video_device_create_video_processor_enumerator:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub video_device_get_video_decoder_profile_count: unsafe extern "system" fn(Raw) -> u32,
    pub video_device_get_video_decoder_profile: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub video_device_check_video_decoder_format:
        unsafe extern "system" fn(Raw, *const u8, i32, *mut u8) -> i32,
    pub video_device_get_video_decoder_config_count:
        unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub video_device_get_video_decoder_config:
        unsafe extern "system" fn(Raw, *const u8, u32, *mut u8) -> i32,
    pub video_device_get_content_protection_caps:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut u8) -> i32,
    pub video_device_check_crypto_key_exchange:
        unsafe extern "system" fn(Raw, *const u8, *const u8, u32, *mut u8) -> i32,
    pub video_device_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub video_device_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub video_device1_get_crypto_session_private_data_size:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *const u8, *mut u8, *mut u8) -> i32,
    pub video_device1_get_video_decoder_caps: unsafe extern "system" fn(
        Raw,
        *const u8,
        u32,
        u32,
        *const u8,
        u32,
        *const u8,
        *mut u8,
    ) -> i32,
    pub video_device1_check_video_decoder_downsampling: unsafe extern "system" fn(
        Raw,
        *const u8,
        i32,
        *const u8,
        *const u8,
        *const u8,
        *mut u8,
        *mut u8,
    ) -> i32,
    pub video_device1_recommend_video_decoder_downsample_parameters:
        unsafe extern "system" fn(Raw, *const u8, i32, *const u8, *const u8, *mut u8) -> i32,
}

// SAFETY: `VideoDevice1Table` lays out `ID3D11VideoDevice1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoDevice1"
)]
unsafe impl Interface for VideoDevice1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoDevice2`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoDevice2Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) video_device_create_video_decoder:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) video_device_create_video_processor:
        unsafe extern "system" fn(Raw, Raw, u32, *mut Raw) -> i32,
    pub(crate) video_device_create_authenticated_channel:
        unsafe extern "system" fn(Raw, i32, *mut Raw) -> i32,
    pub(crate) video_device_create_crypto_session:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *const u8, *mut Raw) -> i32,
    pub(crate) video_device_create_video_decoder_output_view:
        unsafe extern "system" fn(Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) video_device_create_video_processor_input_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) video_device_create_video_processor_output_view:
        unsafe extern "system" fn(Raw, Raw, Raw, *const u8, *mut Raw) -> i32,
    pub(crate) video_device_create_video_processor_enumerator:
        unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub(crate) video_device_get_video_decoder_profile_count: unsafe extern "system" fn(Raw) -> u32,
    pub(crate) video_device_get_video_decoder_profile:
        unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) video_device_check_video_decoder_format:
        unsafe extern "system" fn(Raw, *const u8, i32, *mut u8) -> i32,
    pub(crate) video_device_get_video_decoder_config_count:
        unsafe extern "system" fn(Raw, *const u8, *mut u8) -> i32,
    pub(crate) video_device_get_video_decoder_config:
        unsafe extern "system" fn(Raw, *const u8, u32, *mut u8) -> i32,
    pub(crate) video_device_get_content_protection_caps:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *mut u8) -> i32,
    pub(crate) video_device_check_crypto_key_exchange:
        unsafe extern "system" fn(Raw, *const u8, *const u8, u32, *mut u8) -> i32,
    pub(crate) video_device_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) video_device_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) video_device1_get_crypto_session_private_data_size:
        unsafe extern "system" fn(Raw, *const u8, *const u8, *const u8, *mut u8, *mut u8) -> i32,
    pub(crate) video_device1_get_video_decoder_caps: unsafe extern "system" fn(
        Raw,
        *const u8,
        u32,
        u32,
        *const u8,
        u32,
        *const u8,
        *mut u8,
    ) -> i32,
    pub(crate) video_device1_check_video_decoder_downsampling: unsafe extern "system" fn(
        Raw,
        *const u8,
        i32,
        *const u8,
        *const u8,
        *const u8,
        *mut u8,
        *mut u8,
    )
        -> i32,
    pub(crate) video_device1_recommend_video_decoder_downsample_parameters:
        unsafe extern "system" fn(Raw, *const u8, i32, *const u8, *const u8, *mut u8) -> i32,
    pub(crate) video_device2_check_feature_support:
        unsafe extern "system" fn(Raw, i32, *mut u8, u32) -> i32,
    pub(crate) video_device2_negotiate_crypto_session_key_exchange_mt:
        unsafe extern "system" fn(Raw, Raw, i32, u32, *mut u8) -> i32,
}

// SAFETY: `VideoDevice2Table` lays out `ID3D11VideoDevice2` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoDevice2"
)]
unsafe impl Interface for VideoDevice2Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoProcessor`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoProcessorTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) video_processor_get_content_desc: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) video_processor_get_rate_conversion_caps: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `VideoProcessorTable` lays out `ID3D11VideoProcessor` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoProcessor"
)]
unsafe impl Interface for VideoProcessorTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoProcessorEnumerator`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct VideoProcessorEnumeratorTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub video_processor_enumerator_get_video_processor_content_desc:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub video_processor_enumerator_check_video_processor_format:
        unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub video_processor_enumerator_get_video_processor_caps:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub video_processor_enumerator_get_video_processor_rate_conversion_caps:
        unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub video_processor_enumerator_get_video_processor_custom_rate:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8) -> i32,
    pub video_processor_enumerator_get_video_processor_filter_range:
        unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
}

// SAFETY: `VideoProcessorEnumeratorTable` lays out `ID3D11VideoProcessorEnumerator` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoProcessorEnumerator"
)]
unsafe impl Interface for VideoProcessorEnumeratorTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoProcessorEnumerator1`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoProcessorEnumerator1Table {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) video_processor_enumerator_get_video_processor_content_desc:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) video_processor_enumerator_check_video_processor_format:
        unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub(crate) video_processor_enumerator_get_video_processor_caps:
        unsafe extern "system" fn(Raw, *mut u8) -> i32,
    pub(crate) video_processor_enumerator_get_video_processor_rate_conversion_caps:
        unsafe extern "system" fn(Raw, u32, *mut u8) -> i32,
    pub(crate) video_processor_enumerator_get_video_processor_custom_rate:
        unsafe extern "system" fn(Raw, u32, u32, *mut u8) -> i32,
    pub(crate) video_processor_enumerator_get_video_processor_filter_range:
        unsafe extern "system" fn(Raw, i32, *mut u8) -> i32,
    pub(crate) video_processor_enumerator1_check_video_processor_format_conversion:
        unsafe extern "system" fn(Raw, i32, i32, i32, i32, *mut u8) -> i32,
}

// SAFETY: `VideoProcessorEnumerator1Table` lays out `ID3D11VideoProcessorEnumerator1` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoProcessorEnumerator1"
)]
unsafe impl Interface for VideoProcessorEnumerator1Table {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoProcessorInputView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoProcessorInputViewTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) video_processor_input_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `VideoProcessorInputViewTable` lays out `ID3D11VideoProcessorInputView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoProcessorInputView"
)]
unsafe impl Interface for VideoProcessorInputViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11VideoProcessorOutputView`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct VideoProcessorOutputViewTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub(crate) view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
    pub(crate) video_processor_output_view_get_desc: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `VideoProcessorOutputViewTable` lays out `ID3D11VideoProcessorOutputView` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11VideoProcessorOutputView"
)]
unsafe impl Interface for VideoProcessorOutputViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3D11View`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct ViewTable {
    pub unknown: UnknownTable,
    pub device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
    pub view_get_resource: unsafe extern "system" fn(Raw, *mut u8),
}

// SAFETY: `ViewTable` lays out `ID3D11View` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3D11View"
)]
unsafe impl Interface for ViewTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3DDeviceContextState`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct DeviceContextStateTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) device_child_get_device: unsafe extern "system" fn(Raw, *mut Raw),
    pub(crate) device_child_get_private_data:
        unsafe extern "system" fn(Raw, *mut u8, *mut u8, *mut u8) -> i32,
    pub(crate) device_child_set_private_data:
        unsafe extern "system" fn(Raw, *const u8, u32, *const u8) -> i32,
    pub(crate) device_child_set_private_data_interface:
        unsafe extern "system" fn(Raw, *const u8, Raw) -> i32,
}

// SAFETY: `DeviceContextStateTable` lays out `ID3DDeviceContextState` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3DDeviceContextState"
)]
unsafe impl Interface for DeviceContextStateTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3DUserDefinedAnnotation`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct UserDefinedAnnotationTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) user_defined_annotation_begin_event:
        unsafe extern "system" fn(Raw, *const u8) -> i32,
    pub(crate) user_defined_annotation_end_event: unsafe extern "system" fn(Raw) -> i32,
    pub(crate) user_defined_annotation_set_marker: unsafe extern "system" fn(Raw, *const u8),
    pub(crate) user_defined_annotation_get_status: unsafe extern "system" fn(Raw) -> i32,
}

// SAFETY: `UserDefinedAnnotationTable` lays out `ID3DUserDefinedAnnotation` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3DUserDefinedAnnotation"
)]
unsafe impl Interface for UserDefinedAnnotationTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// The library's exports, as C calls them.
#[derive(Clone, Copy)]
pub struct Library {
    pub d_3d11_create_device: unsafe extern "system" fn(
        Raw,
        i32,
        usize,
        u32,
        *const u8,
        u32,
        u32,
        *mut Raw,
        *mut u8,
        *mut Raw,
    ) -> i32,
    pub d_3d11_create_device_and_swap_chain: unsafe extern "system" fn(
        Raw,
        i32,
        usize,
        u32,
        *const u8,
        u32,
        u32,
        *const u8,
        *mut Raw,
        *mut Raw,
        *mut u8,
        *mut Raw,
    ) -> i32,
}
