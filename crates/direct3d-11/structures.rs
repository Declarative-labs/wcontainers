//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

/// The header's `D3D11_AES_CTR_IV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_aes_ctr_iv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AesCtrIv {
    pub iv: u64,
    pub count: u64,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_ACCESSIBLE_ENCRYPTION_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_accessible_encryption_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureAccessibleEncryptionInput {
    pub parameters: crate::structures::AuthenticatedConfigureInput,
    pub encryption_guid: component_object_model::structures::Guid,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_CRYPTO_SESSION_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_crypto_session_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureCryptoSessionInput {
    pub parameters: crate::structures::AuthenticatedConfigureInput,
    pub decoder_handle: component_object_model::structures::Handle,
    pub crypto_session_handle: component_object_model::structures::Handle,
    pub device_handle: component_object_model::structures::Handle,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_INITIALIZE_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_initialize_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureInitializeInput {
    pub parameters: crate::structures::AuthenticatedConfigureInput,
    pub start_sequence_query: u32,
    pub start_sequence_configure: u32,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureInput {
    pub omac: crate::structures::Omac,
    pub configure_type: component_object_model::structures::Guid,
    pub h_channel: component_object_model::structures::Handle,
    pub sequence_number: u32,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureOutput {
    pub omac: crate::structures::Omac,
    pub configure_type: component_object_model::structures::Guid,
    pub h_channel: component_object_model::structures::Handle,
    pub sequence_number: u32,
    pub return_code: component_object_model::structures::HResult,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_PROTECTION_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_protection_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureProtectionInput {
    pub parameters: crate::structures::AuthenticatedConfigureInput,
    pub protections: crate::structures::AuthenticatedProtectionFlags,
}

/// The header's `D3D11_AUTHENTICATED_CONFIGURE_SHARED_RESOURCE_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_configure_shared_resource_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedConfigureSharedResourceInput {
    pub parameters: crate::structures::AuthenticatedConfigureInput,
    pub process_type: crate::enumerations::AuthenticatedProcessIdentifierType,
    pub process_handle: component_object_model::structures::Handle,
    pub allow_access: bool,
}

/// The header's `D3D11_AUTHENTICATED_PROTECTION_FLAGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_protection_flags).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedProtectionFlags {
    pub flags: crate::structures::AuthenticatedProtectionFlagsFlags,
    pub value: u32,
}

/// A nested struct of `D3D11_AUTHENTICATED_PROTECTION_FLAGS`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedProtectionFlagsFlags {
    pub _bitfield: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_ACCESSIBILITY_ENCRYPTION_GUID_COUNT_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_accessibility_encryption_guid_count_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryAccessibilityEncryptionGuidCountOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub encryption_guid_count: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_ACCESSIBILITY_ENCRYPTION_GUID_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_accessibility_encryption_guid_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryAccessibilityEncryptionGuidInput {
    pub input: crate::structures::AuthenticatedQueryInput,
    pub encryption_guid_index: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_ACCESSIBILITY_ENCRYPTION_GUID_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_accessibility_encryption_guid_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryAccessibilityEncryptionGuidOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub encryption_guid_index: u32,
    pub encryption_guid: component_object_model::structures::Guid,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_ACCESSIBILITY_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_accessibility_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryAccessibilityOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub bus_type: crate::enumerations::BusType,
    pub accessible_in_contiguous_blocks: bool,
    pub accessible_in_non_contiguous_blocks: bool,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_CHANNEL_TYPE_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_channel_type_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryChannelTypeOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub channel_type: crate::enumerations::AuthenticatedChannelType,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_CRYPTO_SESSION_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_crypto_session_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryCryptoSessionInput {
    pub input: crate::structures::AuthenticatedQueryInput,
    pub decoder_handle: component_object_model::structures::Handle,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_CRYPTO_SESSION_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_crypto_session_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryCryptoSessionOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub decoder_handle: component_object_model::structures::Handle,
    pub crypto_session_handle: component_object_model::structures::Handle,
    pub device_handle: component_object_model::structures::Handle,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_CURRENT_ACCESSIBILITY_ENCRYPTION_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_current_accessibility_encryption_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryCurrentAccessibilityEncryptionOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub encryption_guid: component_object_model::structures::Guid,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_DEVICE_HANDLE_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_device_handle_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryDeviceHandleOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub device_handle: component_object_model::structures::Handle,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryInput {
    pub query_type: component_object_model::structures::Guid,
    pub h_channel: component_object_model::structures::Handle,
    pub sequence_number: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryOutput {
    pub omac: crate::structures::Omac,
    pub query_type: component_object_model::structures::Guid,
    pub h_channel: component_object_model::structures::Handle,
    pub sequence_number: u32,
    pub return_code: component_object_model::structures::HResult,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT_ID_COUNT_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_output_id_count_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryOutputIdCountInput {
    pub input: crate::structures::AuthenticatedQueryInput,
    pub device_handle: component_object_model::structures::Handle,
    pub crypto_session_handle: component_object_model::structures::Handle,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT_ID_COUNT_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_output_id_count_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryOutputIdCountOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub device_handle: component_object_model::structures::Handle,
    pub crypto_session_handle: component_object_model::structures::Handle,
    pub output_id_count: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT_ID_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_output_id_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryOutputIdInput {
    pub input: crate::structures::AuthenticatedQueryInput,
    pub device_handle: component_object_model::structures::Handle,
    pub crypto_session_handle: component_object_model::structures::Handle,
    pub output_id_index: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_OUTPUT_ID_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_output_id_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryOutputIdOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub device_handle: component_object_model::structures::Handle,
    pub crypto_session_handle: component_object_model::structures::Handle,
    pub output_id_index: u32,
    pub output_id: u64,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_PROTECTION_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_protection_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryProtectionOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub protection_flags: crate::structures::AuthenticatedProtectionFlags,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS_COUNT_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_restricted_shared_resource_process_count_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryRestrictedSharedResourceProcessCountOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub restricted_shared_resource_process_count: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS_INPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_restricted_shared_resource_process_input).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryRestrictedSharedResourceProcessInput {
    pub input: crate::structures::AuthenticatedQueryInput,
    pub process_index: u32,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_RESTRICTED_SHARED_RESOURCE_PROCESS_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_restricted_shared_resource_process_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryRestrictedSharedResourceProcessOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub process_index: u32,
    pub process_identifier: crate::enumerations::AuthenticatedProcessIdentifierType,
    pub process_handle: component_object_model::structures::Handle,
}

/// The header's `D3D11_AUTHENTICATED_QUERY_UNRESTRICTED_PROTECTED_SHARED_RESOURCE_COUNT_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_authenticated_query_unrestricted_protected_shared_resource_count_output).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthenticatedQueryUnrestrictedProtectedSharedResourceCountOutput {
    pub output: crate::structures::AuthenticatedQueryOutput,
    pub unrestricted_protected_shared_resource_count: u32,
}

/// The header's `D3D11_BLEND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_blend_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendDesc {
    pub alpha_to_coverage_enable: bool,
    pub independent_blend_enable: bool,
    pub render_target: [crate::structures::RenderTargetBlendDesc;
        crate::constants::D3D11_BLEND_DESC_RENDERTARGET_LENGTH],
}

/// The header's `D3D11_BLEND_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_blend_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendDesc1 {
    pub alpha_to_coverage_enable: bool,
    pub independent_blend_enable: bool,
    pub render_target: [crate::structures::RenderTargetBlendDesc1;
        crate::constants::D3D11_BLEND_DESC1_RENDERTARGET_LENGTH],
}

/// The header's `D3D11_BOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box {
    pub left: u32,
    pub top: u32,
    pub front: u32,
    pub right: u32,
    pub bottom: u32,
    pub back: u32,
}

/// The header's `D3D11_BUFFEREX_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_bufferex_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferexSrv {
    pub first_element: u32,
    pub num_elements: u32,
    pub flags: u32,
}

/// The header's `D3D11_BUFFER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_buffer_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferDesc {
    pub byte_width: u32,
    pub usage: crate::enumerations::Usage,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
    pub structure_byte_stride: u32,
}

/// The header's `D3D11_BUFFER_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_buffer_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferRtv {
    pub anonymous1: crate::structures::BufferRtvAnonymous1,
    pub anonymous2: crate::structures::BufferRtvAnonymous2,
}

/// A C union of `D3D11_BUFFER_RTV`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferRtvAnonymous1 {
    pub first_element: u32,
    pub element_offset: u32,
}

/// A C union of `D3D11_BUFFER_RTV`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferRtvAnonymous2 {
    pub num_elements: u32,
    pub element_width: u32,
}

/// The header's `D3D11_BUFFER_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_buffer_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSrv {
    pub anonymous1: crate::structures::BufferSrvAnonymous1,
    pub anonymous2: crate::structures::BufferSrvAnonymous2,
}

/// A C union of `D3D11_BUFFER_SRV`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSrvAnonymous1 {
    pub first_element: u32,
    pub element_offset: u32,
}

/// A C union of `D3D11_BUFFER_SRV`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSrvAnonymous2 {
    pub num_elements: u32,
    pub element_width: u32,
}

/// The header's `D3D11_BUFFER_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_buffer_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferUav {
    pub first_element: u32,
    pub num_elements: u32,
    pub flags: u32,
}

/// The header's `D3D11_CLASS_INSTANCE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_class_instance_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassInstanceDesc {
    pub instance_id: u32,
    pub instance_index: u32,
    pub type_id: u32,
    pub constant_buffer: u32,
    pub base_constant_buffer_offset: u32,
    pub base_texture: u32,
    pub base_sampler: u32,
    pub created: bool,
}

/// The header's `D3D11_COMPUTE_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_compute_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputeShaderTraceDesc {
    pub invocation: u64,
    pub thread_id_in_group:
        [u32; crate::constants::D3D11_COMPUTE_SHADER_TRACE_DESC_THREADIDINGROUP_LENGTH],
    pub thread_group_id:
        [u32; crate::constants::D3D11_COMPUTE_SHADER_TRACE_DESC_THREADGROUPID_LENGTH],
}

/// The header's `D3D11_COUNTER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_counter_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CounterDesc {
    pub counter: crate::enumerations::CounterKind,
    pub misc_flags: u32,
}

/// The header's `D3D11_COUNTER_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_counter_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CounterInfo {
    pub last_device_dependent_counter: crate::enumerations::CounterKind,
    pub num_simultaneous_counters: u32,
    pub num_detectable_parallel_units: u8,
}

/// The header's `D3D11_DEPTH_STENCILOP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_depth_stencilop_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilopDesc {
    pub stencil_fail_op: crate::enumerations::StencilOp,
    pub stencil_depth_fail_op: crate::enumerations::StencilOp,
    pub stencil_pass_op: crate::enumerations::StencilOp,
    pub stencil_func: crate::enumerations::ComparisonFunc,
}

/// The header's `D3D11_DEPTH_STENCIL_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_depth_stencil_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilDesc {
    pub depth_enable: bool,
    pub depth_write_mask: crate::enumerations::DepthWriteMask,
    pub depth_func: crate::enumerations::ComparisonFunc,
    pub stencil_enable: bool,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
    pub front_face: crate::structures::DepthStencilopDesc,
    pub back_face: crate::structures::DepthStencilopDesc,
}

/// The header's `D3D11_DEPTH_STENCIL_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_depth_stencil_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::DsvDimension,
    pub flags: u32,
    pub anonymous: crate::structures::DepthStencilViewDescAnonymous,
}

/// A C union of `D3D11_DEPTH_STENCIL_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilViewDescAnonymous {
    pub texture_1d: crate::structures::Tex1DDsv,
    pub texture_1d_array: crate::structures::Tex1DArrayDsv,
    pub texture_2d: crate::structures::Tex2DDsv,
    pub texture_2d_array: crate::structures::Tex2DArrayDsv,
    pub texture_2dms: crate::structures::Tex2DmsDsv,
    pub texture_2dms_array: crate::structures::Tex2DmsArrayDsv,
}

/// The header's `D3D11_DOMAIN_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_domain_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DomainShaderTraceDesc {
    pub invocation: u64,
}

/// The header's `D3D11_DRAW_INDEXED_INSTANCED_INDIRECT_ARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_draw_indexed_instanced_indirect_args).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawIndexedInstancedIndirectArgs {
    pub index_count_per_instance: u32,
    pub instance_count: u32,
    pub start_index_location: u32,
    pub base_vertex_location: i32,
    pub start_instance_location: u32,
}

/// The header's `D3D11_DRAW_INSTANCED_INDIRECT_ARGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_draw_instanced_indirect_args).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawInstancedIndirectArgs {
    pub vertex_count_per_instance: u32,
    pub instance_count: u32,
    pub start_vertex_location: u32,
    pub start_instance_location: u32,
}

/// The header's `D3D11_ENCRYPTED_BLOCK_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_encrypted_block_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EncryptedBlockInfo {
    pub num_encrypted_bytes_at_beginning: u32,
    pub num_bytes_in_skip_pattern: u32,
    pub num_bytes_in_encrypt_pattern: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataArchitectureInfo {
    pub tile_based_deferred_renderer: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D10_X_HARDWARE_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d10_x_hardware_options).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D10XHardwareOptions {
    pub compute_shaders_plus_raw_and_structured_buffers_via_shader_4_x: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D11_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d11_options).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D11Options {
    pub output_merger_logic_op: bool,
    pub uav_only_rendering_forced_sample_count: bool,
    pub discard_ap_is_seen_by_driver: bool,
    pub flags_for_update_and_copy_seen_by_driver: bool,
    pub clear_view: bool,
    pub copy_with_overlap: bool,
    pub constant_buffer_partial_update: bool,
    pub constant_buffer_offsetting: bool,
    pub map_no_overwrite_on_dynamic_constant_buffer: bool,
    pub map_no_overwrite_on_dynamic_buffer_srv: bool,
    pub multisample_rtv_with_forced_sample_count_one: bool,
    pub sad4_shader_instructions: bool,
    pub extended_doubles_shader_instructions: bool,
    pub extended_resource_sharing: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D11_OPTIONS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d11_options1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D11Options1 {
    pub tiled_resources_tier: crate::enumerations::TiledResourcesTier,
    pub min_max_filtering: bool,
    pub clear_view_also_supports_depth_only_formats: bool,
    pub map_on_default_buffers: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D11_OPTIONS2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d11_options2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D11Options2 {
    pub ps_specified_stencil_ref_supported: bool,
    pub typed_uav_load_additional_formats: bool,
    pub ro_vs_supported: bool,
    pub conservative_rasterization_tier: crate::enumerations::ConservativeRasterizationTier,
    pub tiled_resources_tier: crate::enumerations::TiledResourcesTier,
    pub map_on_default_textures: bool,
    pub standard_swizzle: bool,
    pub unified_memory_architecture: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D11_OPTIONS3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d11_options3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D11Options3 {
    pub vp_and_rt_array_index_from_any_shader_feeding_rasterizer: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D11_OPTIONS4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ns-d3d11_4-d3d11_feature_data_d3d11_options4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D11Options4 {
    pub extended_nv12_shared_texture_supported: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D11_OPTIONS5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d11_options5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D11Options5 {
    pub shared_resource_tier: crate::enumerations::SharedResourceTier,
}

/// The header's `D3D11_FEATURE_DATA_D3D9_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d9_options).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D9Options {
    pub full_non_pow2_texture_support: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D9_OPTIONS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d9_options1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D9Options1 {
    pub full_non_pow2_texture_supported: bool,
    pub depth_as_texture_with_less_equal_comparison_filter_supported: bool,
    pub simple_instancing_supported: bool,
    pub texture_cube_face_render_target_with_non_cube_depth_stencil_supported: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D9_SHADOW_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d9_shadow_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D9ShadowSupport {
    pub supports_depth_as_texture_with_less_equal_comparison_filter: bool,
}

/// The header's `D3D11_FEATURE_DATA_D3D9_SIMPLE_INSTANCING_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_d3d9_simple_instancing_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D9SimpleInstancingSupport {
    pub simple_instancing_supported: bool,
}

/// The header's `D3D11_FEATURE_DATA_DISPLAYABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_displayable).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataDisplayable {
    pub displayable_texture: bool,
    pub shared_resource_tier: crate::enumerations::SharedResourceTier,
}

/// The header's `D3D11_FEATURE_DATA_DOUBLES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_doubles).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataDoubles {
    pub double_precision_float_shader_ops: bool,
}

/// The header's `D3D11_FEATURE_DATA_FORMAT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_format_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataFormatSupport {
    pub in_format: directx_graphics_infrastructure::enumerations::Format,
    pub out_format_support: u32,
}

/// The header's `D3D11_FEATURE_DATA_FORMAT_SUPPORT2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_format_support2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataFormatSupport2 {
    pub in_format: directx_graphics_infrastructure::enumerations::Format,
    pub out_format_support2: u32,
}

/// The header's `D3D11_FEATURE_DATA_GPU_VIRTUAL_ADDRESS_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_gpu_virtual_address_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataGpuVirtualAddressSupport {
    pub max_gpu_virtual_address_bits_per_resource: u32,
    pub max_gpu_virtual_address_bits_per_process: u32,
}

/// The header's `D3D11_FEATURE_DATA_MARKER_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_marker_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataMarkerSupport {
    pub profile: bool,
}

/// The header's `D3D11_FEATURE_DATA_SHADER_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_shader_cache).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataShaderCache {
    pub support_flags: u32,
}

/// The header's `D3D11_FEATURE_DATA_SHADER_MIN_PRECISION_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_shader_min_precision_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataShaderMinPrecisionSupport {
    pub pixel_shader_min_precision: u32,
    pub all_other_shader_stages_min_precision: u32,
}

/// The header's `D3D11_FEATURE_DATA_THREADING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_feature_data_threading).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataThreading {
    pub driver_concurrent_creates: bool,
    pub driver_command_lists: bool,
}

/// The header's `D3D11_FEATURE_DATA_VIDEO_DECODER_HISTOGRAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/ns-d3d11_4-d3d11_feature_data_video_decoder_histogram).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataVideoDecoderHistogram {
    pub decoder_desc: crate::structures::VideoDecoderDesc,
    pub components: crate::enumerations::VideoDecoderHistogramComponentFlags,
    pub bin_count: u32,
    pub counter_bit_depth: u32,
}

/// The header's `D3D11_FUNCTION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_function_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDesc {
    pub version: u32,
    pub creator: std::ffi::CString,
    pub flags: u32,
    pub constant_buffers: u32,
    pub bound_resources: u32,
    pub instruction_count: u32,
    pub temp_register_count: u32,
    pub temp_array_count: u32,
    pub def_count: u32,
    pub dcl_count: u32,
    pub texture_normal_instructions: u32,
    pub texture_load_instructions: u32,
    pub texture_comp_instructions: u32,
    pub texture_bias_instructions: u32,
    pub texture_gradient_instructions: u32,
    pub float_instruction_count: u32,
    pub int_instruction_count: u32,
    pub uint_instruction_count: u32,
    pub static_flow_control_count: u32,
    pub dynamic_flow_control_count: u32,
    pub macro_instruction_count: u32,
    pub array_instruction_count: u32,
    pub mov_instruction_count: u32,
    pub movc_instruction_count: u32,
    pub conversion_instruction_count: u32,
    pub bitwise_instruction_count: u32,
    pub min_feature_level: direct3d_common::enumerations::FeatureLevel,
    pub required_feature_flags: u64,
    pub name: std::ffi::CString,
    pub function_parameter_count: i32,
    pub has_return: bool,
    pub has10_level9_vertex_shader: bool,
    pub has10_level9_pixel_shader: bool,
}

/// The header's `D3D11_GEOMETRY_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_geometry_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryShaderTraceDesc {
    pub invocation: u64,
}

/// The header's `D3D11_HULL_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_hull_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullShaderTraceDesc {
    pub invocation: u64,
}

/// The header's `D3D11_INFO_QUEUE_FILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ns-d3d11sdklayers-d3d11_info_queue_filter).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueFilter {
    pub allow_list: crate::structures::InfoQueueFilterDesc,
    pub deny_list: crate::structures::InfoQueueFilterDesc,
}

/// The header's `D3D11_INFO_QUEUE_FILTER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ns-d3d11sdklayers-d3d11_info_queue_filter_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueFilterDesc {
    pub num_categories: u32,
    pub category_list: Vec<crate::enumerations::MessageCategory>,
    pub num_severities: u32,
    pub severity_list: Vec<crate::enumerations::MessageSeverity>,
    pub num_i_ds: u32,
    pub id_list: Vec<crate::enumerations::MessageId>,
}

/// The header's `D3D11_INPUT_ELEMENT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_input_element_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct InputElementDesc {
    pub semantic_name: std::ffi::CString,
    pub semantic_index: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub input_slot: u32,
    pub aligned_byte_offset: u32,
    pub input_slot_class: crate::enumerations::InputClassification,
    pub instance_data_step_rate: u32,
}

/// The header's `D3D11_KEY_EXCHANGE_HW_PROTECTION_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_key_exchange_hw_protection_data).
#[derive(Clone, Debug, PartialEq)]
pub struct KeyExchangeHwProtectionData {
    pub hw_protection_function_id: u32,
    pub input_data: Vec<crate::structures::KeyExchangeHwProtectionInputData>,
    pub output_data: Vec<crate::structures::KeyExchangeHwProtectionOutputData>,
    pub status: component_object_model::structures::HResult,
}

/// The header's `D3D11_KEY_EXCHANGE_HW_PROTECTION_INPUT_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_key_exchange_hw_protection_input_data).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyExchangeHwProtectionInputData {
    pub private_data_size: u32,
    pub hw_protection_data_size: u32,
    pub pb_input:
        [u8; crate::constants::D3D11_KEY_EXCHANGE_HW_PROTECTION_INPUT_DATA_PBINPUT_LENGTH],
}

/// The header's `D3D11_KEY_EXCHANGE_HW_PROTECTION_OUTPUT_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_key_exchange_hw_protection_output_data).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyExchangeHwProtectionOutputData {
    pub private_data_size: u32,
    pub max_hw_protection_data_size: u32,
    pub hw_protection_data_size: u32,
    pub transport_time: u64,
    pub execution_time: u64,
    pub pb_output:
        [u8; crate::constants::D3D11_KEY_EXCHANGE_HW_PROTECTION_OUTPUT_DATA_PBOUTPUT_LENGTH],
}

/// The header's `D3D11_LIBRARY_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_library_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryDesc {
    pub creator: std::ffi::CString,
    pub flags: u32,
    pub function_count: u32,
}

/// The header's `D3D11_MAPPED_SUBRESOURCE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_mapped_subresource).
#[derive(Clone, Debug, PartialEq)]
pub struct MappedSubresource<Bytes> {
    pub data: Bytes,
    pub row_pitch: u32,
    pub depth_pitch: u32,
}

/// The header's `D3D11_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/ns-d3d11sdklayers-d3d11_message).
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub category: crate::enumerations::MessageCategory,
    pub severity: crate::enumerations::MessageSeverity,
    pub id: crate::enumerations::MessageId,
    pub description: Vec<u8>,
    pub description_byte_length: usize,
}

/// The header's `D3D11_OMAC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_omac).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Omac {
    pub omac: [u8; crate::constants::D3D11_OMAC_OMAC_LENGTH],
}

/// The header's `D3D11_PACKED_MIP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ns-d3d11_2-d3d11_packed_mip_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedMipDesc {
    pub num_standard_mips: u8,
    pub num_packed_mips: u8,
    pub num_tiles_for_packed_mips: u32,
    pub start_tile_index_in_overall_resource: u32,
}

/// The header's `D3D11_PARAMETER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_parameter_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterDesc {
    pub name: std::ffi::CString,
    pub semantic_name: std::ffi::CString,
    pub type_: direct3d_common::enumerations::ShaderVariableType,
    pub class: direct3d_common::enumerations::ShaderVariableClass,
    pub rows: u32,
    pub columns: u32,
    pub interpolation_mode: direct3d_common::enumerations::InterpolationMode,
    pub flags: direct3d_common::enumerations::ParameterFlags,
    pub first_in_register: u32,
    pub first_in_component: u32,
    pub first_out_register: u32,
    pub first_out_component: u32,
}

/// The header's `D3D11_PIXEL_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_pixel_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelShaderTraceDesc {
    pub invocation: u64,
    pub x: i32,
    pub y: i32,
    pub sample_mask: u64,
}

/// The header's `D3D11_QUERY_DATA_PIPELINE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_query_data_pipeline_statistics).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDataPipelineStatistics {
    pub ia_vertices: u64,
    pub ia_primitives: u64,
    pub vs_invocations: u64,
    pub gs_invocations: u64,
    pub gs_primitives: u64,
    pub c_invocations: u64,
    pub c_primitives: u64,
    pub ps_invocations: u64,
    pub hs_invocations: u64,
    pub ds_invocations: u64,
    pub cs_invocations: u64,
}

/// The header's `D3D11_QUERY_DATA_SO_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_query_data_so_statistics).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDataSoStatistics {
    pub num_primitives_written: u64,
    pub primitives_storage_needed: u64,
}

/// The header's `D3D11_QUERY_DATA_TIMESTAMP_DISJOINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_query_data_timestamp_disjoint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDataTimestampDisjoint {
    pub frequency: u64,
    pub disjoint: bool,
}

/// The header's `D3D11_QUERY_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_query_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDesc {
    pub query: crate::enumerations::QueryKind,
    pub misc_flags: u32,
}

/// The header's `D3D11_QUERY_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_query_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDesc1 {
    pub query: crate::enumerations::QueryKind,
    pub misc_flags: u32,
    pub context_type: crate::enumerations::ContextType,
}

/// The header's `D3D11_RASTERIZER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_rasterizer_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerDesc {
    pub fill_mode: crate::enumerations::FillMode,
    pub cull_mode: crate::enumerations::CullMode,
    pub front_counter_clockwise: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub scissor_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
}

/// The header's `D3D11_RASTERIZER_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_rasterizer_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerDesc1 {
    pub fill_mode: crate::enumerations::FillMode,
    pub cull_mode: crate::enumerations::CullMode,
    pub front_counter_clockwise: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub scissor_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
    pub forced_sample_count: u32,
}

/// The header's `D3D11_RASTERIZER_DESC2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_rasterizer_desc2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerDesc2 {
    pub fill_mode: crate::enumerations::FillMode,
    pub cull_mode: crate::enumerations::CullMode,
    pub front_counter_clockwise: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub scissor_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
    pub forced_sample_count: u32,
    pub conservative_raster: crate::enumerations::ConservativeRasterizationMode,
}

/// The header's `D3D11_RENDER_TARGET_BLEND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_render_target_blend_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetBlendDesc {
    pub blend_enable: bool,
    pub src_blend: crate::enumerations::Blend,
    pub dest_blend: crate::enumerations::Blend,
    pub blend_op: crate::enumerations::BlendOp,
    pub src_blend_alpha: crate::enumerations::Blend,
    pub dest_blend_alpha: crate::enumerations::Blend,
    pub blend_op_alpha: crate::enumerations::BlendOp,
    pub render_target_write_mask: u8,
}

/// The header's `D3D11_RENDER_TARGET_BLEND_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_render_target_blend_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetBlendDesc1 {
    pub blend_enable: bool,
    pub logic_op_enable: bool,
    pub src_blend: crate::enumerations::Blend,
    pub dest_blend: crate::enumerations::Blend,
    pub blend_op: crate::enumerations::BlendOp,
    pub src_blend_alpha: crate::enumerations::Blend,
    pub dest_blend_alpha: crate::enumerations::Blend,
    pub blend_op_alpha: crate::enumerations::BlendOp,
    pub logic_op: crate::enumerations::LogicOp,
    pub render_target_write_mask: u8,
}

/// The header's `D3D11_RENDER_TARGET_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_render_target_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::RtvDimension,
    pub anonymous: crate::structures::RenderTargetViewDescAnonymous,
}

/// A C union of `D3D11_RENDER_TARGET_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetViewDescAnonymous {
    pub buffer: crate::structures::BufferRtv,
    pub texture_1d: crate::structures::Tex1DRtv,
    pub texture_1d_array: crate::structures::Tex1DArrayRtv,
    pub texture_2d: crate::structures::Tex2DRtv,
    pub texture_2d_array: crate::structures::Tex2DArrayRtv,
    pub texture_2dms: crate::structures::Tex2DmsRtv,
    pub texture_2dms_array: crate::structures::Tex2DmsArrayRtv,
    pub texture_3d: crate::structures::Tex3DRtv,
}

/// The header's `D3D11_RENDER_TARGET_VIEW_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_render_target_view_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetViewDesc1 {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::RtvDimension,
    pub anonymous: crate::structures::RenderTargetViewDesc1Anonymous,
}

/// A C union of `D3D11_RENDER_TARGET_VIEW_DESC1`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetViewDesc1Anonymous {
    pub buffer: crate::structures::BufferRtv,
    pub texture_1d: crate::structures::Tex1DRtv,
    pub texture_1d_array: crate::structures::Tex1DArrayRtv,
    pub texture_2d: crate::structures::Tex2DRtv1,
    pub texture_2d_array: crate::structures::Tex2DArrayRtv1,
    pub texture_2dms: crate::structures::Tex2DmsRtv,
    pub texture_2dms_array: crate::structures::Tex2DmsArrayRtv,
    pub texture_3d: crate::structures::Tex3DRtv,
}

/// The header's `D3D11_SAMPLER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_sampler_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDesc {
    pub filter: crate::enumerations::Filter,
    pub address_u: crate::enumerations::TextureAddressMode,
    pub address_v: crate::enumerations::TextureAddressMode,
    pub address_w: crate::enumerations::TextureAddressMode,
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison_func: crate::enumerations::ComparisonFunc,
    pub border_color: [f32; crate::constants::D3D11_SAMPLER_DESC_BORDERCOLOR_LENGTH],
    pub min_lod: f32,
    pub max_lod: f32,
}

/// The header's `D3D11_SHADER_BUFFER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_shader_buffer_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderBufferDesc {
    pub name: std::ffi::CString,
    pub type_: direct3d_common::enumerations::CbufferType,
    pub variables: u32,
    pub size: u32,
    pub u_flags: u32,
}

/// The header's `D3D11_SHADER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_shader_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderDesc {
    pub version: u32,
    pub creator: std::ffi::CString,
    pub flags: u32,
    pub constant_buffers: u32,
    pub bound_resources: u32,
    pub input_parameters: u32,
    pub output_parameters: u32,
    pub instruction_count: u32,
    pub temp_register_count: u32,
    pub temp_array_count: u32,
    pub def_count: u32,
    pub dcl_count: u32,
    pub texture_normal_instructions: u32,
    pub texture_load_instructions: u32,
    pub texture_comp_instructions: u32,
    pub texture_bias_instructions: u32,
    pub texture_gradient_instructions: u32,
    pub float_instruction_count: u32,
    pub int_instruction_count: u32,
    pub uint_instruction_count: u32,
    pub static_flow_control_count: u32,
    pub dynamic_flow_control_count: u32,
    pub macro_instruction_count: u32,
    pub array_instruction_count: u32,
    pub cut_instruction_count: u32,
    pub emit_instruction_count: u32,
    pub gs_output_topology: direct3d_common::enumerations::PrimitiveTopology,
    pub gs_max_output_vertex_count: u32,
    pub input_primitive: direct3d_common::enumerations::Primitive,
    pub patch_constant_parameters: u32,
    pub c_gs_instance_count: u32,
    pub c_control_points: u32,
    pub hs_output_primitive: direct3d_common::enumerations::TessellatorOutputPrimitive,
    pub hs_partitioning: direct3d_common::enumerations::TessellatorPartitioning,
    pub tessellator_domain: direct3d_common::enumerations::TessellatorDomain,
    pub c_barrier_instructions: u32,
    pub c_interlocked_instructions: u32,
    pub c_texture_store_instructions: u32,
}

/// The header's `D3D11_SHADER_INPUT_BIND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_shader_input_bind_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderInputBindDesc {
    pub name: std::ffi::CString,
    pub type_: direct3d_common::enumerations::ShaderInputType,
    pub bind_point: u32,
    pub bind_count: u32,
    pub u_flags: u32,
    pub return_type: direct3d_common::enumerations::ResourceReturnType,
    pub dimension: direct3d_common::enumerations::SrvDimension,
    pub num_samples: u32,
}

/// The header's `D3D11_SHADER_RESOURCE_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_shader_resource_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderResourceViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: direct3d_common::enumerations::SrvDimension,
    pub anonymous: crate::structures::ShaderResourceViewDescAnonymous,
}

/// A C union of `D3D11_SHADER_RESOURCE_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderResourceViewDescAnonymous {
    pub buffer: crate::structures::BufferSrv,
    pub texture_1d: crate::structures::Tex1DSrv,
    pub texture_1d_array: crate::structures::Tex1DArraySrv,
    pub texture_2d: crate::structures::Tex2DSrv,
    pub texture_2d_array: crate::structures::Tex2DArraySrv,
    pub texture_2dms: crate::structures::Tex2DmsSrv,
    pub texture_2dms_array: crate::structures::Tex2DmsArraySrv,
    pub texture_3d: crate::structures::Tex3DSrv,
    pub texture_cube: crate::structures::TexcubeSrv,
    pub texture_cube_array: crate::structures::TexcubeArraySrv,
    pub buffer_ex: crate::structures::BufferexSrv,
}

/// The header's `D3D11_SHADER_RESOURCE_VIEW_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_shader_resource_view_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderResourceViewDesc1 {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: direct3d_common::enumerations::SrvDimension,
    pub anonymous: crate::structures::ShaderResourceViewDesc1Anonymous,
}

/// A C union of `D3D11_SHADER_RESOURCE_VIEW_DESC1`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderResourceViewDesc1Anonymous {
    pub buffer: crate::structures::BufferSrv,
    pub texture_1d: crate::structures::Tex1DSrv,
    pub texture_1d_array: crate::structures::Tex1DArraySrv,
    pub texture_2d: crate::structures::Tex2DSrv1,
    pub texture_2d_array: crate::structures::Tex2DArraySrv1,
    pub texture_2dms: crate::structures::Tex2DmsSrv,
    pub texture_2dms_array: crate::structures::Tex2DmsArraySrv,
    pub texture_3d: crate::structures::Tex3DSrv,
    pub texture_cube: crate::structures::TexcubeSrv,
    pub texture_cube_array: crate::structures::TexcubeArraySrv,
    pub buffer_ex: crate::structures::BufferexSrv,
}

/// The header's `D3D11_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderTraceDesc {
    pub type_: crate::enumerations::ShaderType,
    pub flags: u32,
    pub anonymous: crate::structures::ShaderTraceDescAnonymous,
}

/// A C union of `D3D11_SHADER_TRACE_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderTraceDescAnonymous {
    pub vertex_shader_trace_desc: crate::structures::VertexShaderTraceDesc,
    pub hull_shader_trace_desc: crate::structures::HullShaderTraceDesc,
    pub domain_shader_trace_desc: crate::structures::DomainShaderTraceDesc,
    pub geometry_shader_trace_desc: crate::structures::GeometryShaderTraceDesc,
    pub pixel_shader_trace_desc: crate::structures::PixelShaderTraceDesc,
    pub compute_shader_trace_desc: crate::structures::ComputeShaderTraceDesc,
}

/// The header's `D3D11_SHADER_TYPE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_shader_type_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderTypeDesc {
    pub class: direct3d_common::enumerations::ShaderVariableClass,
    pub type_: direct3d_common::enumerations::ShaderVariableType,
    pub rows: u32,
    pub columns: u32,
    pub elements: u32,
    pub members: u32,
    pub offset: u32,
    pub name: std::ffi::CString,
}

/// The header's `D3D11_SHADER_VARIABLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_shader_variable_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderVariableDesc {
    pub name: std::ffi::CString,
    pub start_offset: u32,
    pub size: u32,
    pub u_flags: u32,
    pub default_value: Vec<u8>,
    pub start_texture: u32,
    pub texture_size: u32,
    pub start_sampler: u32,
    pub sampler_size: u32,
}

/// The header's `D3D11_SIGNATURE_PARAMETER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shader/ns-d3d11shader-d3d11_signature_parameter_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct SignatureParameterDesc {
    pub semantic_name: std::ffi::CString,
    pub semantic_index: u32,
    pub register: u32,
    pub system_value_type: direct3d_common::enumerations::Name,
    pub component_type: direct3d_common::enumerations::RegisterComponentType,
    pub mask: u8,
    pub read_write_mask: u8,
    pub stream: u32,
    pub min_precision: direct3d_common::enumerations::MinPrecision,
}

/// The header's `D3D11_SO_DECLARATION_ENTRY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_so_declaration_entry).
#[derive(Clone, Debug, PartialEq)]
pub struct SoDeclarationEntry {
    pub stream: u32,
    pub semantic_name: std::ffi::CString,
    pub semantic_index: u32,
    pub start_component: u8,
    pub component_count: u8,
    pub output_slot: u8,
}

/// The header's `D3D11_SUBRESOURCE_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_subresource_data).
#[derive(Clone, Debug, PartialEq)]
pub struct SubresourceData {
    pub sys_mem: Vec<u8>,
    pub sys_mem_pitch: u32,
    pub sys_mem_slice_pitch: u32,
}

/// The header's `D3D11_SUBRESOURCE_TILING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ns-d3d11_2-d3d11_subresource_tiling).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubresourceTiling {
    pub width_in_tiles: u32,
    pub height_in_tiles: u16,
    pub depth_in_tiles: u16,
    pub start_tile_index_in_overall_resource: u32,
}

/// The header's `D3D11_TEX1D_ARRAY_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_array_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArrayDsv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX1D_ARRAY_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_array_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArrayRtv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX1D_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArraySrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX1D_ARRAY_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_array_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArrayUav {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX1D_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DDsv {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX1D_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DRtv {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX1D_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
}

/// The header's `D3D11_TEX1D_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex1d_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DUav {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX2DMS_ARRAY_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2dms_array_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArrayDsv {
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2DMS_ARRAY_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2dms_array_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArrayRtv {
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2DMS_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2dms_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArraySrv {
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2DMS_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2dms_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsDsv {
    pub unused_field_nothing_to_define: u32,
}

/// The header's `D3D11_TEX2DMS_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2dms_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsRtv {
    pub unused_field_nothing_to_define: u32,
}

/// The header's `D3D11_TEX2DMS_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2dms_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsSrv {
    pub unused_field_nothing_to_define: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_array_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayDsv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_array_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayRtv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_RTV1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_tex2d_array_rtv1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayRtv1 {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub plane_slice: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArraySrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_SRV1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_tex2d_array_srv1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArraySrv1 {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub plane_slice: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_array_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayUav {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_UAV1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_tex2d_array_uav1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayUav1 {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub plane_slice: u32,
}

/// The header's `D3D11_TEX2D_ARRAY_VPOV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_array_vpov).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayVpov {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D11_TEX2D_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DDsv {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX2D_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DRtv {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX2D_RTV1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_tex2d_rtv1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DRtv1 {
    pub mip_slice: u32,
    pub plane_slice: u32,
}

/// The header's `D3D11_TEX2D_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
}

/// The header's `D3D11_TEX2D_SRV1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_tex2d_srv1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DSrv1 {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub plane_slice: u32,
}

/// The header's `D3D11_TEX2D_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DUav {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX2D_UAV1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_tex2d_uav1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DUav1 {
    pub mip_slice: u32,
    pub plane_slice: u32,
}

/// The header's `D3D11_TEX2D_VDOV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_vdov).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DVdov {
    pub array_slice: u32,
}

/// The header's `D3D11_TEX2D_VPIV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_vpiv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DVpiv {
    pub mip_slice: u32,
    pub array_slice: u32,
}

/// The header's `D3D11_TEX2D_VPOV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex2d_vpov).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DVpov {
    pub mip_slice: u32,
}

/// The header's `D3D11_TEX3D_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex3d_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex3DRtv {
    pub mip_slice: u32,
    pub first_w_slice: u32,
    pub w_size: u32,
}

/// The header's `D3D11_TEX3D_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex3d_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex3DSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
}

/// The header's `D3D11_TEX3D_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_tex3d_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex3DUav {
    pub mip_slice: u32,
    pub first_w_slice: u32,
    pub w_size: u32,
}

/// The header's `D3D11_TEXCUBE_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_texcube_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexcubeArraySrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_2d_array_face: u32,
    pub num_cubes: u32,
}

/// The header's `D3D11_TEXCUBE_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_texcube_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexcubeSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
}

/// The header's `D3D11_TEXTURE1D_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_texture1d_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture1DDesc {
    pub width: u32,
    pub mip_levels: u32,
    pub array_size: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub usage: crate::enumerations::Usage,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
}

/// The header's `D3D11_TEXTURE2D_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_texture2d_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture2DDesc {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub array_size: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub sample_desc: directx_graphics_infrastructure::structures::SampleDesc,
    pub usage: crate::enumerations::Usage,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
}

/// The header's `D3D11_TEXTURE2D_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_texture2d_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture2DDesc1 {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub array_size: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub sample_desc: directx_graphics_infrastructure::structures::SampleDesc,
    pub usage: crate::enumerations::Usage,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
    pub texture_layout: crate::enumerations::TextureLayout,
}

/// The header's `D3D11_TEXTURE3D_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_texture3d_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture3DDesc {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub usage: crate::enumerations::Usage,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
}

/// The header's `D3D11_TEXTURE3D_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_texture3d_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture3DDesc1 {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub usage: crate::enumerations::Usage,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
    pub texture_layout: crate::enumerations::TextureLayout,
}

/// The header's `D3D11_TILED_RESOURCE_COORDINATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ns-d3d11_2-d3d11_tiled_resource_coordinate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiledResourceCoordinate {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub subresource: u32,
}

/// The header's `D3D11_TILE_REGION_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ns-d3d11_2-d3d11_tile_region_size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRegionSize {
    pub num_tiles: u32,
    pub b_use_box: bool,
    pub width: u32,
    pub height: u16,
    pub depth: u16,
}

/// The header's `D3D11_TILE_SHAPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/ns-d3d11_2-d3d11_tile_shape).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileShape {
    pub width_in_texels: u32,
    pub height_in_texels: u32,
    pub depth_in_texels: u32,
}

/// The header's `D3D11_TRACE_REGISTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_trace_register).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceRegister {
    pub reg_type: crate::enumerations::TraceRegisterType,
    pub anonymous: crate::structures::TraceRegisterAnonymous,
    pub operand_index: u8,
    pub flags: u8,
}

/// A C union of `D3D11_TRACE_REGISTER`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceRegisterAnonymous {
    pub index_1d: u16,
    pub index_2d: [u16; crate::constants::D3D11_TRACE_REGISTER_ANONYMOUS_E_UNION_INDEX2D_LENGTH],
}

/// The header's `D3D11_TRACE_STATS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_trace_stats).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceStats {
    pub trace_desc: crate::structures::ShaderTraceDesc,
    pub num_invocations_in_stamp: u8,
    pub target_stamp_index: u8,
    pub num_trace_steps: u32,
    pub input_mask: [u8; crate::constants::D3D11_TRACE_STATS_INPUTMASK_LENGTH],
    pub output_mask: [u8; crate::constants::D3D11_TRACE_STATS_OUTPUTMASK_LENGTH],
    pub num_temps: u16,
    pub max_indexable_temp_index: u16,
    pub indexable_temp_size: [u16; crate::constants::D3D11_TRACE_STATS_INDEXABLETEMPSIZE_LENGTH],
    pub immediate_constant_buffer_size: u16,
    pub pixel_position: [u32; crate::constants::D3D11_TRACE_STATS_PIXELPOSITION_LENGTH],
    pub pixel_coverage_mask: [u64; crate::constants::D3D11_TRACE_STATS_PIXELCOVERAGEMASK_LENGTH],
    pub pixel_discarded_mask: [u64; crate::constants::D3D11_TRACE_STATS_PIXELDISCARDEDMASK_LENGTH],
    pub pixel_coverage_mask_after_shader:
        [u64; crate::constants::D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERSHADER_LENGTH],
    pub pixel_coverage_mask_after_a2_c_sample_mask:
        [u64; crate::constants::D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERA2CSAMPLEMASK_LENGTH],
    pub pixel_coverage_mask_after_a2_c_sample_mask_depth:
        [u64; crate::constants::D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERA2CSAMPLEMASKDEPTH_LENGTH],
    pub pixel_coverage_mask_after_a2_c_sample_mask_depth_stencil: [u64;
        crate::constants::D3D11_TRACE_STATS_PIXELCOVERAGEMASKAFTERA2CSAMPLEMASKDEPTHSTENCIL_LENGTH],
    pub ps_outputs_depth: bool,
    pub ps_outputs_mask: bool,
    pub gs_input_primitive: crate::enumerations::TraceGsInputPrimitive,
    pub gs_inputs_primitive_id: bool,
    pub hs_output_patch_constant_mask:
        [u8; crate::constants::D3D11_TRACE_STATS_HSOUTPUTPATCHCONSTANTMASK_LENGTH],
    pub ds_input_patch_constant_mask:
        [u8; crate::constants::D3D11_TRACE_STATS_DSINPUTPATCHCONSTANTMASK_LENGTH],
}

/// The header's `D3D11_TRACE_STEP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_trace_step).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceStep {
    pub id: u32,
    pub instruction_active: bool,
    pub num_registers_written: u8,
    pub num_registers_read: u8,
    pub misc_operations: u16,
    pub opcode_type: u32,
    pub current_global_cycle: u64,
}

/// The header's `D3D11_TRACE_VALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_trace_value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceValue {
    pub bits: [u32; crate::constants::D3D11_TRACE_VALUE_BITS_LENGTH],
    pub valid_mask: u8,
}

/// The header's `D3D11_UNORDERED_ACCESS_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_unordered_access_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnorderedAccessViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::UavDimension,
    pub anonymous: crate::structures::UnorderedAccessViewDescAnonymous,
}

/// A C union of `D3D11_UNORDERED_ACCESS_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnorderedAccessViewDescAnonymous {
    pub buffer: crate::structures::BufferUav,
    pub texture_1d: crate::structures::Tex1DUav,
    pub texture_1d_array: crate::structures::Tex1DArrayUav,
    pub texture_2d: crate::structures::Tex2DUav,
    pub texture_2d_array: crate::structures::Tex2DArrayUav,
    pub texture_3d: crate::structures::Tex3DUav,
}

/// The header's `D3D11_UNORDERED_ACCESS_VIEW_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/ns-d3d11_3-d3d11_unordered_access_view_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnorderedAccessViewDesc1 {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::UavDimension,
    pub anonymous: crate::structures::UnorderedAccessViewDesc1Anonymous,
}

/// A C union of `D3D11_UNORDERED_ACCESS_VIEW_DESC1`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnorderedAccessViewDesc1Anonymous {
    pub buffer: crate::structures::BufferUav,
    pub texture_1d: crate::structures::Tex1DUav,
    pub texture_1d_array: crate::structures::Tex1DArrayUav,
    pub texture_2d: crate::structures::Tex2DUav1,
    pub texture_2d_array: crate::structures::Tex2DArrayUav1,
    pub texture_3d: crate::structures::Tex3DUav,
}

/// The header's `D3D11_VERTEX_SHADER_TRACE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/ns-d3d11shadertracing-d3d11_vertex_shader_trace_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexShaderTraceDesc {
    pub invocation: u64,
}

/// The header's `D3D11_VIDEO_COLOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_color).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoColor {
    pub anonymous: crate::structures::VideoColorAnonymous,
}

/// A C union of `D3D11_VIDEO_COLOR`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoColorAnonymous {
    pub y_cb_cr: crate::structures::VideoColorYcbcra,
    pub rgba: crate::structures::VideoColorRgba,
}

/// The header's `D3D11_VIDEO_COLOR_RGBA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_color_rgba).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// The header's `D3D11_VIDEO_COLOR_YCbCrA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_color_ycbcra).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoColorYcbcra {
    pub y: f32,
    pub cb: f32,
    pub cr: f32,
    pub a: f32,
}

/// The header's `D3D11_VIDEO_CONTENT_PROTECTION_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_content_protection_caps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoContentProtectionCaps {
    pub caps: u32,
    pub key_exchange_type_count: u32,
    pub block_alignment_size: u32,
    pub protected_memory_size: u64,
}

/// The header's `D3D11_VIDEO_DECODER_BEGIN_FRAME_CRYPTO_SESSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_video_decoder_begin_frame_crypto_session).
pub struct VideoDecoderBeginFrameCryptoSession<A: crate::Direct3D11> {
    pub crypto_session: Option<<A as crate::Direct3D11>::CryptoSession>,
    pub blob_size: u32,
    pub blob: Vec<u8>,
    pub key_info_id: Vec<component_object_model::structures::Guid>,
    pub private_data_size: u32,
    pub private_data: Vec<u8>,
}

/// The header's `D3D11_VIDEO_DECODER_BUFFER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_decoder_buffer_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct VideoDecoderBufferDesc {
    pub buffer_type: crate::enumerations::VideoDecoderBufferType,
    pub buffer_index: u32,
    pub data_offset: u32,
    pub data_size: u32,
    pub first_m_baddress: u32,
    pub num_m_bs_in_buffer: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub reserved_bits: u32,
    pub iv: Vec<u8>,
    pub iv_size: u32,
    pub partial_encryption: bool,
    pub encrypted_block_info: crate::structures::EncryptedBlockInfo,
}

/// The header's `D3D11_VIDEO_DECODER_BUFFER_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_video_decoder_buffer_desc1).
#[derive(Clone, Debug, PartialEq)]
pub struct VideoDecoderBufferDesc1 {
    pub buffer_type: crate::enumerations::VideoDecoderBufferType,
    pub data_offset: u32,
    pub data_size: u32,
    pub iv: Vec<u8>,
    pub iv_size: u32,
    pub sub_sample_mapping_block: Vec<crate::structures::VideoDecoderSubSampleMappingBlock>,
    pub sub_sample_mapping_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoDecoderBufferDesc2 {
    pub buffer_type: crate::enumerations::VideoDecoderBufferType,
    pub data_offset: u32,
    pub data_size: u32,
    pub iv: Vec<u8>,
    pub iv_size: u32,
    pub sub_sample_mapping_block: Vec<crate::structures::VideoDecoderSubSampleMappingBlock>,
    pub sub_sample_mapping_count: u32,
    pub c_blocks_stripe_encrypted: u32,
    pub c_blocks_stripe_clear: u32,
}

/// The header's `D3D11_VIDEO_DECODER_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_decoder_config).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoDecoderConfig {
    pub guid_config_bitstream_encryption: component_object_model::structures::Guid,
    pub guid_config_m_bcontrol_encryption: component_object_model::structures::Guid,
    pub guid_config_resid_diff_encryption: component_object_model::structures::Guid,
    pub config_bitstream_raw: u32,
    pub config_m_bcontrol_raster_order: u32,
    pub config_resid_diff_host: u32,
    pub config_spatial_resid8: u32,
    pub config_resid8_subtraction: u32,
    pub config_spatial_host8or9_clipping: u32,
    pub config_spatial_resid_interleaved: u32,
    pub config_intra_resid_unsigned: u32,
    pub config_resid_diff_accelerator: u32,
    pub config_host_inverse_scan: u32,
    pub config_specific_idct: u32,
    pub config4_grouped_coefs: u32,
    pub config_min_render_target_buff_count: u16,
    pub config_decoder_specific: u16,
}

/// The header's `D3D11_VIDEO_DECODER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_decoder_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoDecoderDesc {
    pub guid: component_object_model::structures::Guid,
    pub sample_width: u32,
    pub sample_height: u32,
    pub output_format: directx_graphics_infrastructure::enumerations::Format,
}

/// The header's `D3D11_VIDEO_DECODER_EXTENSION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_decoder_extension).
pub struct VideoDecoderExtension<A: crate::Direct3D11> {
    pub function: u32,
    pub private_input_data: Vec<u8>,
    pub private_input_data_size: u32,
    pub private_output_data: Vec<u8>,
    pub private_output_data_size: u32,
    pub resource_count: u32,
    pub resource_list: Vec<Option<crate::ResourceObject<A>>>,
}

/// The header's `D3D11_VIDEO_DECODER_OUTPUT_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_decoder_output_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoDecoderOutputViewDesc {
    pub decode_profile: component_object_model::structures::Guid,
    pub view_dimension: crate::enumerations::VdovDimension,
    pub anonymous: crate::structures::VideoDecoderOutputViewDescAnonymous,
}

/// A C union of `D3D11_VIDEO_DECODER_OUTPUT_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoDecoderOutputViewDescAnonymous {
    pub texture_2d: crate::structures::Tex2DVdov,
}

/// The header's `D3D11_VIDEO_DECODER_SUB_SAMPLE_MAPPING_BLOCK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_video_decoder_sub_sample_mapping_block).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoDecoderSubSampleMappingBlock {
    pub clear_size: u32,
    pub encrypted_size: u32,
}

/// The header's `D3D11_VIDEO_PROCESSOR_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_caps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorCaps {
    pub device_caps: u32,
    pub feature_caps: u32,
    pub filter_caps: u32,
    pub input_format_caps: u32,
    pub auto_stream_caps: u32,
    pub stereo_caps: u32,
    pub rate_conversion_caps_count: u32,
    pub max_input_streams: u32,
    pub max_stream_states: u32,
}

/// The header's `D3D11_VIDEO_PROCESSOR_COLOR_SPACE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_color_space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorColorSpace {
    pub _bitfield: u32,
}

/// The header's `D3D11_VIDEO_PROCESSOR_CONTENT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_content_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorContentDesc {
    pub input_frame_format: crate::enumerations::VideoFrameFormat,
    pub input_frame_rate: directx_graphics_infrastructure::structures::Rational,
    pub input_width: u32,
    pub input_height: u32,
    pub output_frame_rate: directx_graphics_infrastructure::structures::Rational,
    pub output_width: u32,
    pub output_height: u32,
    pub usage: crate::enumerations::VideoUsage,
}

/// The header's `D3D11_VIDEO_PROCESSOR_CUSTOM_RATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_custom_rate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorCustomRate {
    pub custom_rate: directx_graphics_infrastructure::structures::Rational,
    pub output_frames: u32,
    pub input_interlaced: bool,
    pub input_frames_or_fields: u32,
}

/// The header's `D3D11_VIDEO_PROCESSOR_FILTER_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_filter_range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorFilterRange {
    pub minimum: i32,
    pub maximum: i32,
    pub default: i32,
    pub multiplier: f32,
}

/// The header's `D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_input_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorInputViewDesc {
    pub four_cc: u32,
    pub view_dimension: crate::enumerations::VpivDimension,
    pub anonymous: crate::structures::VideoProcessorInputViewDescAnonymous,
}

/// A C union of `D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorInputViewDescAnonymous {
    pub texture_2d: crate::structures::Tex2DVpiv,
}

/// The header's `D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_output_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorOutputViewDesc {
    pub view_dimension: crate::enumerations::VpovDimension,
    pub anonymous: crate::structures::VideoProcessorOutputViewDescAnonymous,
}

/// A C union of `D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorOutputViewDescAnonymous {
    pub texture_2d: crate::structures::Tex2DVpov,
    pub texture_2d_array: crate::structures::Tex2DArrayVpov,
}

/// The header's `D3D11_VIDEO_PROCESSOR_RATE_CONVERSION_CAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_rate_conversion_caps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorRateConversionCaps {
    pub past_frames: u32,
    pub future_frames: u32,
    pub processor_caps: u32,
    pub i_telecine_caps: u32,
    pub custom_rate_count: u32,
}

/// The header's `D3D11_VIDEO_PROCESSOR_STREAM`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_stream).
pub struct VideoProcessorStream<A: crate::Direct3D11> {
    pub enable: bool,
    pub output_index: u32,
    pub input_frame_or_field: u32,
    pub past_frames: u32,
    pub future_frames: u32,
    pub past_surfaces: Vec<Option<<A as crate::Direct3D11>::VideoProcessorInputView>>,
    pub input_surface: Option<<A as crate::Direct3D11>::VideoProcessorInputView>,
    pub future_surfaces: Vec<Option<<A as crate::Direct3D11>::VideoProcessorInputView>>,
    pub past_surfaces_right: Vec<Option<<A as crate::Direct3D11>::VideoProcessorInputView>>,
    pub input_surface_right: Option<<A as crate::Direct3D11>::VideoProcessorInputView>,
    pub future_surfaces_right: Vec<Option<<A as crate::Direct3D11>::VideoProcessorInputView>>,
}

/// The header's `D3D11_VIDEO_PROCESSOR_STREAM_BEHAVIOR_HINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_video_processor_stream_behavior_hint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProcessorStreamBehaviorHint {
    pub enable: bool,
    pub width: u32,
    pub height: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
}

/// The header's `D3D11_VIDEO_SAMPLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/ns-d3d11_1-d3d11_video_sample_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoSampleDesc {
    pub width: u32,
    pub height: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
}

/// The header's `D3D11_VIEWPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_viewport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

/// The header's `D3DX11_FFT_BUFFER_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ns-d3dcsx-d3dx11_fft_buffer_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3Dx11FftBufferInfo {
    pub num_temp_buffer_sizes: u32,
    pub temp_buffer_float_sizes:
        [u32; crate::constants::D3DX11_FFT_BUFFER_INFO_TEMPBUFFERFLOATSIZES_LENGTH],
    pub num_precompute_buffer_sizes: u32,
    pub precompute_buffer_float_sizes:
        [u32; crate::constants::D3DX11_FFT_BUFFER_INFO_PRECOMPUTEBUFFERFLOATSIZES_LENGTH],
}

/// The header's `D3DX11_FFT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcsx/ns-d3dcsx-d3dx11_fft_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D3Dx11FftDesc {
    pub num_dimensions: u32,
    pub element_lengths: [u32; crate::constants::D3DX11_FFT_DESC_ELEMENTLENGTHS_LENGTH],
    pub dimension_mask: u32,
    pub type_: crate::enumerations::D3Dx11FftDataType,
}
