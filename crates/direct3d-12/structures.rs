//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

/// The header's `D3D12_AUTO_BREADCRUMB_NODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_auto_breadcrumb_node).
pub struct AutoBreadcrumbNode<A: crate::Direct3D12> {
    pub command_list_debug_name_a: Vec<u8>,
    pub command_list_debug_name_w: Vec<u16>,
    pub command_queue_debug_name_a: Vec<u8>,
    pub command_queue_debug_name_w: Vec<u16>,
    pub command_list: Option<<A as crate::Direct3D12>::GraphicsCommandList>,
    pub command_queue: Option<<A as crate::Direct3D12>::CommandQueue>,
    pub breadcrumb_count: u32,
    pub last_breadcrumb_value: Vec<u32>,
    pub command_history: Vec<crate::enumerations::AutoBreadcrumbOp>,
    pub next: Vec<crate::structures::AutoBreadcrumbNode<A>>,
}

pub struct AutoBreadcrumbNode1<A: crate::Direct3D12> {
    pub command_list_debug_name_a: Vec<u8>,
    pub command_list_debug_name_w: Vec<u16>,
    pub command_queue_debug_name_a: Vec<u8>,
    pub command_queue_debug_name_w: Vec<u16>,
    pub command_list: Option<<A as crate::Direct3D12>::GraphicsCommandList>,
    pub command_queue: Option<<A as crate::Direct3D12>::CommandQueue>,
    pub breadcrumb_count: u32,
    pub last_breadcrumb_value: Vec<u32>,
    pub command_history: Vec<crate::enumerations::AutoBreadcrumbOp>,
    pub next: Vec<crate::structures::AutoBreadcrumbNode1<A>>,
    pub breadcrumb_contexts_count: u32,
    pub breadcrumb_contexts: Vec<crate::structures::DredBreadcrumbContext>,
}

/// The header's `D3D12_BARRIER_GROUP`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_barrier_group).
pub struct BarrierGroup<A: crate::Direct3D12> {
    pub type_: crate::enumerations::BarrierType,
    pub num_barriers: u32,
    pub anonymous: crate::structures::BarrierGroupAnonymous<A>,
}

/// A C union of `D3D12_BARRIER_GROUP`: every member, each read from the same bytes.
pub struct BarrierGroupAnonymous<A: crate::Direct3D12> {
    pub global_barriers: Vec<crate::structures::GlobalBarrier>,
    pub texture_barriers: Vec<crate::structures::TextureBarrier<A>>,
    pub buffer_barriers: Vec<crate::structures::BufferBarrier<A>>,
}

/// The header's `D3D12_BARRIER_SUBRESOURCE_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_barrier_subresource_range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarrierSubresourceRange {
    pub index_or_first_mip_level: u32,
    pub num_mip_levels: u32,
    pub first_array_slice: u32,
    pub num_array_slices: u32,
    pub first_plane: u32,
    pub num_planes: u32,
}

/// The header's `D3D12_BLEND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_blend_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendDesc {
    pub alpha_to_coverage_enable: bool,
    pub independent_blend_enable: bool,
    pub render_target: [crate::structures::RenderTargetBlendDesc;
        crate::constants::D3D12_BLEND_DESC_RENDERTARGET_LENGTH],
}

/// The header's `D3D12_BOX`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box {
    pub left: u32,
    pub top: u32,
    pub front: u32,
    pub right: u32,
    pub bottom: u32,
    pub back: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BroadcastingLaunchOverrides {
    pub local_root_arguments_table_index: Vec<u32>,
    pub program_entry: Vec<bool>,
    pub new_name: Vec<crate::structures::NodeId>,
    pub share_input_of: Vec<crate::structures::NodeId>,
    pub dispatch_grid: Vec<u32>,
    pub max_dispatch_grid: Vec<u32>,
    pub num_output_overrides: u32,
    pub output_overrides: Vec<crate::structures::NodeOutputOverrides>,
}

/// The header's `D3D12_BUFFER_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_buffer_barrier).
pub struct BufferBarrier<A: crate::Direct3D12> {
    pub sync_before: crate::enumerations::BarrierSync,
    pub sync_after: crate::enumerations::BarrierSync,
    pub access_before: crate::enumerations::BarrierAccess,
    pub access_after: crate::enumerations::BarrierAccess,
    pub resource: Option<<A as crate::Direct3D12>::Resource>,
    pub offset: u64,
    pub size: u64,
}

/// The header's `D3D12_BUFFER_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_buffer_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferRtv {
    pub first_element: u64,
    pub num_elements: u32,
}

/// The header's `D3D12_BUFFER_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_buffer_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSrv {
    pub first_element: u64,
    pub num_elements: u32,
    pub structure_byte_stride: u32,
    pub flags: crate::enumerations::BufferSrvFlags,
}

/// The header's `D3D12_BUFFER_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_buffer_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferUav {
    pub first_element: u64,
    pub num_elements: u32,
    pub structure_byte_stride: u32,
    pub counter_offset_in_bytes: u64,
    pub flags: crate::enumerations::BufferUavFlags,
}

/// The header's `D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_build_raytracing_acceleration_structure_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct BuildRaytracingAccelerationStructureDesc {
    pub dest_acceleration_structure_data: u64,
    pub inputs: crate::structures::BuildRaytracingAccelerationStructureInputs,
    pub source_acceleration_structure_data: u64,
    pub scratch_acceleration_structure_data: u64,
}

/// The header's `D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_build_raytracing_acceleration_structure_inputs).
#[derive(Clone, Debug, PartialEq)]
pub struct BuildRaytracingAccelerationStructureInputs {
    pub type_: crate::enumerations::RaytracingAccelerationStructureType,
    pub flags: crate::enumerations::RaytracingAccelerationStructureBuildFlags,
    pub num_descs: u32,
    pub descs_layout: crate::enumerations::ElementsLayout,
    pub anonymous: crate::structures::BuildRaytracingAccelerationStructureInputsAnonymous,
}

/// A C union of `D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildRaytracingAccelerationStructureInputsAnonymous {
    pub instance_descs: u64,
    pub geometry_descs: Vec<crate::structures::RaytracingGeometryDesc>,
    pub geometry_descs_: Vec<u8>,
}

/// The header's `D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_TOOLS_VISUALIZATION_HEADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_build_raytracing_acceleration_structure_tools_visualization_header).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildRaytracingAccelerationStructureToolsVisualizationHeader {
    pub type_: crate::enumerations::RaytracingAccelerationStructureType,
    pub num_descs: u32,
}

/// The header's `D3D12_CACHED_PIPELINE_STATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_cached_pipeline_state).
#[derive(Clone, Debug, PartialEq)]
pub struct CachedPipelineState {
    pub cached_blob: Vec<u8>,
    pub cached_blob_size_in_bytes: usize,
}

/// The header's `D3D12_CLEAR_VALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_clear_value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClearValue {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub anonymous: crate::structures::ClearValueAnonymous,
}

/// A C union of `D3D12_CLEAR_VALUE`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClearValueAnonymous {
    pub color: [f32; crate::constants::D3D12_CLEAR_VALUE_ANONYMOUS_E_UNION_COLOR_LENGTH],
    pub depth_stencil: crate::structures::DepthStencilValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoalescingLaunchOverrides {
    pub local_root_arguments_table_index: Vec<u32>,
    pub program_entry: Vec<bool>,
    pub new_name: Vec<crate::structures::NodeId>,
    pub share_input_of: Vec<crate::structures::NodeId>,
    pub num_output_overrides: u32,
    pub output_overrides: Vec<crate::structures::NodeOutputOverrides>,
}

/// The header's `D3D12_COMMAND_QUEUE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_command_queue_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandQueueDesc {
    pub type_: crate::enumerations::CommandListType,
    pub priority: i32,
    pub flags: crate::enumerations::CommandQueueFlags,
    pub node_mask: u32,
}

/// The header's `D3D12_COMMAND_SIGNATURE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_command_signature_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSignatureDesc {
    pub byte_stride: u32,
    pub num_argument_descs: u32,
    pub argument_descs: Vec<crate::structures::IndirectArgumentDesc>,
    pub node_mask: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommonComputeNodeOverrides {
    pub local_root_arguments_table_index: Vec<u32>,
    pub program_entry: Vec<bool>,
    pub new_name: Vec<crate::structures::NodeId>,
    pub share_input_of: Vec<crate::structures::NodeId>,
    pub num_output_overrides: u32,
    pub output_overrides: Vec<crate::structures::NodeOutputOverrides>,
}

/// The header's `D3D12_COMPUTE_PIPELINE_STATE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_compute_pipeline_state_desc).
pub struct ComputePipelineStateDesc<A: crate::Direct3D12> {
    pub root_signature: Option<<A as crate::Direct3D12>::RootSignature>,
    pub cs: crate::structures::ShaderBytecode,
    pub node_mask: u32,
    pub cached_pso: crate::structures::CachedPipelineState,
    pub flags: crate::enumerations::PipelineStateFlags,
}

/// The header's `D3D12_CONSTANT_BUFFER_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_constant_buffer_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstantBufferViewDesc {
    pub buffer_location: u64,
    pub size_in_bytes: u32,
}

/// The header's `D3D12_CPU_DESCRIPTOR_HANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_cpu_descriptor_handle).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpuDescriptorHandle {
    pub ptr: usize,
}

/// The header's `D3D12_DEBUG_COMMAND_LIST_GPU_BASED_VALIDATION_SETTINGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ns-d3d12sdklayers-d3d12_debug_command_list_gpu_based_validation_settings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugCommandListGpuBasedValidationSettings {
    pub shader_patch_mode: crate::enumerations::GpuBasedValidationShaderPatchMode,
}

/// The header's `D3D12_DEBUG_DEVICE_GPU_BASED_VALIDATION_SETTINGS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ns-d3d12sdklayers-d3d12_debug_device_gpu_based_validation_settings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugDeviceGpuBasedValidationSettings {
    pub max_messages_per_command_list: u32,
    pub default_shader_patch_mode: crate::enumerations::GpuBasedValidationShaderPatchMode,
    pub pipeline_state_create_flags:
        crate::enumerations::GpuBasedValidationPipelineStateCreateFlags,
}

/// The header's `D3D12_DEBUG_DEVICE_GPU_SLOWDOWN_PERFORMANCE_FACTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ns-d3d12sdklayers-d3d12_debug_device_gpu_slowdown_performance_factor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugDeviceGpuSlowdownPerformanceFactor {
    pub slowdown_factor: f32,
}

/// The header's `D3D12_DEPTH_STENCILOP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_depth_stencilop_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilopDesc {
    pub stencil_fail_op: crate::enumerations::StencilOp,
    pub stencil_depth_fail_op: crate::enumerations::StencilOp,
    pub stencil_pass_op: crate::enumerations::StencilOp,
    pub stencil_func: crate::enumerations::ComparisonFunc,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilopDesc1 {
    pub stencil_fail_op: crate::enumerations::StencilOp,
    pub stencil_depth_fail_op: crate::enumerations::StencilOp,
    pub stencil_pass_op: crate::enumerations::StencilOp,
    pub stencil_func: crate::enumerations::ComparisonFunc,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
}

/// The header's `D3D12_DEPTH_STENCIL_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_depth_stencil_desc).
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

/// The header's `D3D12_DEPTH_STENCIL_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_depth_stencil_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilDesc1 {
    pub depth_enable: bool,
    pub depth_write_mask: crate::enumerations::DepthWriteMask,
    pub depth_func: crate::enumerations::ComparisonFunc,
    pub stencil_enable: bool,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
    pub front_face: crate::structures::DepthStencilopDesc,
    pub back_face: crate::structures::DepthStencilopDesc,
    pub depth_bounds_test_enable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilDesc2 {
    pub depth_enable: bool,
    pub depth_write_mask: crate::enumerations::DepthWriteMask,
    pub depth_func: crate::enumerations::ComparisonFunc,
    pub stencil_enable: bool,
    pub front_face: crate::structures::DepthStencilopDesc1,
    pub back_face: crate::structures::DepthStencilopDesc1,
    pub depth_bounds_test_enable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilFormat {
    pub depth_stencil_format: directx_graphics_infrastructure::enumerations::Format,
}

/// The header's `D3D12_DEPTH_STENCIL_VALUE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_depth_stencil_value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilValue {
    pub depth: f32,
    pub stencil: u8,
}

/// The header's `D3D12_DEPTH_STENCIL_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_depth_stencil_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::DsvDimension,
    pub flags: crate::enumerations::DsvFlags,
    pub anonymous: crate::structures::DepthStencilViewDescAnonymous,
}

/// A C union of `D3D12_DEPTH_STENCIL_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilViewDescAnonymous {
    pub texture_1d: crate::structures::Tex1DDsv,
    pub texture_1d_array: crate::structures::Tex1DArrayDsv,
    pub texture_2d: crate::structures::Tex2DDsv,
    pub texture_2d_array: crate::structures::Tex2DArrayDsv,
    pub texture_2dms: crate::structures::Tex2DmsDsv,
    pub texture_2dms_array: crate::structures::Tex2DmsArrayDsv,
}

/// The header's `D3D12_DESCRIPTOR_HEAP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_descriptor_heap_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptorHeapDesc {
    pub type_: crate::enumerations::DescriptorHeapType,
    pub num_descriptors: u32,
    pub flags: crate::enumerations::DescriptorHeapFlags,
    pub node_mask: u32,
}

/// The header's `D3D12_DESCRIPTOR_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_descriptor_range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptorRange {
    pub range_type: crate::enumerations::DescriptorRangeType,
    pub num_descriptors: u32,
    pub base_shader_register: u32,
    pub register_space: u32,
    pub offset_in_descriptors_from_table_start: u32,
}

/// The header's `D3D12_DESCRIPTOR_RANGE1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_descriptor_range1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptorRange1 {
    pub range_type: crate::enumerations::DescriptorRangeType,
    pub num_descriptors: u32,
    pub base_shader_register: u32,
    pub register_space: u32,
    pub flags: crate::enumerations::DescriptorRangeFlags,
    pub offset_in_descriptors_from_table_start: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceConfigurationDesc {
    pub flags: crate::enumerations::DeviceFlags,
    pub gpu_based_validation_flags: u32,
    pub sdk_version: u32,
    pub num_enabled_experimental_features: u32,
}

/// The header's `D3D12_DEVICE_REMOVED_EXTENDED_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_device_removed_extended_data).
pub struct DeviceRemovedExtendedData<A: crate::Direct3D12> {
    pub flags: crate::enumerations::DredFlags,
    pub head_auto_breadcrumb_node: Vec<crate::structures::AutoBreadcrumbNode<A>>,
}

/// The header's `D3D12_DEVICE_REMOVED_EXTENDED_DATA1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_device_removed_extended_data1).
pub struct DeviceRemovedExtendedData1<A: crate::Direct3D12> {
    pub device_removed_reason: component_object_model::structures::HResult,
    pub auto_breadcrumbs_output: crate::structures::DredAutoBreadcrumbsOutput<A>,
    pub page_fault_output: crate::structures::DredPageFaultOutput,
}

pub struct DeviceRemovedExtendedData2<A: crate::Direct3D12> {
    pub device_removed_reason: component_object_model::structures::HResult,
    pub auto_breadcrumbs_output: crate::structures::DredAutoBreadcrumbsOutput1<A>,
    pub page_fault_output: crate::structures::DredPageFaultOutput1<A>,
}

pub struct DeviceRemovedExtendedData3<A: crate::Direct3D12> {
    pub device_removed_reason: component_object_model::structures::HResult,
    pub auto_breadcrumbs_output: crate::structures::DredAutoBreadcrumbsOutput1<A>,
    pub page_fault_output: crate::structures::DredPageFaultOutput2<A>,
    pub device_state: crate::enumerations::DredDeviceState,
}

/// The header's `D3D12_DISCARD_REGION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_discard_region).
#[derive(Clone, Debug, PartialEq)]
pub struct DiscardRegion {
    pub num_rects: u32,
    pub rects: Vec<component_object_model::structures::Rect>,
    pub first_subresource: u32,
    pub num_subresources: u32,
}

/// The header's `D3D12_DISPATCH_ARGUMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dispatch_arguments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DispatchArguments {
    pub thread_group_count_x: u32,
    pub thread_group_count_y: u32,
    pub thread_group_count_z: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DispatchGraphDesc {
    pub mode: crate::enumerations::DispatchMode,
    pub anonymous: crate::structures::DispatchGraphDescAnonymous,
}

/// A C union of `D3D12_DISPATCH_GRAPH_DESC`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct DispatchGraphDescAnonymous {
    pub node_cpu_input: crate::structures::NodeCpuInput,
    pub node_gpu_input: u64,
    pub multi_node_cpu_input: crate::structures::MultiNodeCpuInput,
    pub multi_node_gpu_input: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DispatchMeshArguments {
    pub thread_group_count_x: u32,
    pub thread_group_count_y: u32,
    pub thread_group_count_z: u32,
}

/// The header's `D3D12_DISPATCH_RAYS_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dispatch_rays_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DispatchRaysDesc {
    pub ray_generation_shader_record: crate::structures::GpuVirtualAddressRange,
    pub miss_shader_table: crate::structures::GpuVirtualAddressRangeAndStride,
    pub hit_group_table: crate::structures::GpuVirtualAddressRangeAndStride,
    pub callable_shader_table: crate::structures::GpuVirtualAddressRangeAndStride,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

/// The header's `D3D12_DRAW_ARGUMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_draw_arguments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawArguments {
    pub vertex_count_per_instance: u32,
    pub instance_count: u32,
    pub start_vertex_location: u32,
    pub start_instance_location: u32,
}

/// The header's `D3D12_DRAW_INDEXED_ARGUMENTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_draw_indexed_arguments).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawIndexedArguments {
    pub index_count_per_instance: u32,
    pub instance_count: u32,
    pub start_index_location: u32,
    pub base_vertex_location: i32,
    pub start_instance_location: u32,
}

/// The header's `D3D12_DRED_ALLOCATION_NODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dred_allocation_node).
#[derive(Clone, Debug, PartialEq)]
pub struct DredAllocationNode {
    pub object_name_a: Vec<u8>,
    pub object_name_w: Vec<u16>,
    pub allocation_type: crate::enumerations::DredAllocationType,
    pub next: Vec<crate::structures::DredAllocationNode>,
}

pub struct DredAllocationNode1<A: crate::Direct3D12> {
    pub object_name_a: Vec<u8>,
    pub object_name_w: Vec<u16>,
    pub allocation_type: crate::enumerations::DredAllocationType,
    pub next: Vec<crate::structures::DredAllocationNode1<A>>,
    pub object: Option<crate::UnknownObject<A>>,
}

/// The header's `D3D12_DRED_AUTO_BREADCRUMBS_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dred_auto_breadcrumbs_output).
pub struct DredAutoBreadcrumbsOutput<A: crate::Direct3D12> {
    pub head_auto_breadcrumb_node: Vec<crate::structures::AutoBreadcrumbNode<A>>,
}

pub struct DredAutoBreadcrumbsOutput1<A: crate::Direct3D12> {
    pub head_auto_breadcrumb_node: Vec<crate::structures::AutoBreadcrumbNode1<A>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DredBreadcrumbContext {
    pub breadcrumb_index: u32,
    pub context_string: Vec<u16>,
}

/// The header's `D3D12_DRED_PAGE_FAULT_OUTPUT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dred_page_fault_output).
#[derive(Clone, Debug, PartialEq)]
pub struct DredPageFaultOutput {
    pub page_fault_va: u64,
    pub head_existing_allocation_node: Vec<crate::structures::DredAllocationNode>,
    pub head_recent_freed_allocation_node: Vec<crate::structures::DredAllocationNode>,
}

pub struct DredPageFaultOutput1<A: crate::Direct3D12> {
    pub page_fault_va: u64,
    pub head_existing_allocation_node: Vec<crate::structures::DredAllocationNode1<A>>,
    pub head_recent_freed_allocation_node: Vec<crate::structures::DredAllocationNode1<A>>,
}

pub struct DredPageFaultOutput2<A: crate::Direct3D12> {
    pub page_fault_va: u64,
    pub head_existing_allocation_node: Vec<crate::structures::DredAllocationNode1<A>>,
    pub head_recent_freed_allocation_node: Vec<crate::structures::DredAllocationNode1<A>>,
    pub page_fault_flags: crate::enumerations::DredPageFaultFlags,
}

/// The header's `D3D12_DXIL_LIBRARY_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dxil_library_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct DxilLibraryDesc {
    pub dxil_library: crate::structures::ShaderBytecode,
    pub num_exports: u32,
    pub exports: Vec<crate::structures::ExportDesc>,
}

/// The header's `D3D12_DXIL_SUBOBJECT_TO_EXPORTS_ASSOCIATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_dxil_subobject_to_exports_association).
#[derive(Clone, Debug, PartialEq)]
pub struct DxilSubobjectToExportsAssociation {
    pub subobject_to_associate: Vec<u16>,
    pub num_exports: u32,
    pub exports: Vec<Vec<u16>>,
}

/// The header's `D3D12_EXISTING_COLLECTION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_existing_collection_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ExistingCollectionDesc {
    pub existing_collection: u8,
    pub num_exports: u32,
    pub exports: Vec<crate::structures::ExportDesc>,
}

/// The header's `D3D12_EXPORT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_export_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ExportDesc {
    pub name: Vec<u16>,
    pub export_to_rename: Vec<u16>,
    pub flags: crate::enumerations::ExportFlags,
}

/// The header's `D3D12_FEATURE_DATA_ARCHITECTURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_architecture).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataArchitecture {
    pub node_index: u32,
    pub tile_based_renderer: bool,
    pub uma: bool,
    pub cache_coherent_uma: bool,
}

/// The header's `D3D12_FEATURE_DATA_ARCHITECTURE1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_architecture1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataArchitecture1 {
    pub node_index: u32,
    pub tile_based_renderer: bool,
    pub uma: bool,
    pub cache_coherent_uma: bool,
    pub isolated_mmu: bool,
}

/// The header's `D3D12_FEATURE_DATA_COMMAND_QUEUE_PRIORITY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_command_queue_priority).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataCommandQueuePriority {
    pub command_list_type: crate::enumerations::CommandListType,
    pub priority: u32,
    pub priority_for_type_is_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_CROSS_NODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_cross_node).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataCrossNode {
    pub sharing_tier: crate::enumerations::CrossNodeSharingTier,
    pub atomic_shader_instructions: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options {
    pub double_precision_float_shader_ops: bool,
    pub output_merger_logic_op: bool,
    pub min_precision_support: crate::enumerations::ShaderMinPrecisionSupport,
    pub tiled_resources_tier: crate::enumerations::TiledResourcesTier,
    pub resource_binding_tier: crate::enumerations::ResourceBindingTier,
    pub ps_specified_stencil_ref_supported: bool,
    pub typed_uav_load_additional_formats: bool,
    pub ro_vs_supported: bool,
    pub conservative_rasterization_tier: crate::enumerations::ConservativeRasterizationTier,
    pub max_gpu_virtual_address_bits_per_resource: u32,
    pub standard_swizzle64_kb_supported: bool,
    pub cross_node_sharing_tier: crate::enumerations::CrossNodeSharingTier,
    pub cross_adapter_row_major_texture_supported: bool,
    pub vp_and_rt_array_index_from_any_shader_feeding_rasterizer_supported_without_gs_emulation:
        bool,
    pub resource_heap_tier: crate::enumerations::ResourceHeapTier,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options1 {
    pub wave_ops: bool,
    pub wave_lane_count_min: u32,
    pub wave_lane_count_max: u32,
    pub total_lane_count: u32,
    pub expanded_compute_resource_states: bool,
    pub int64_shader_ops: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS10`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options10 {
    pub variable_rate_shading_sum_combiner_supported: bool,
    pub mesh_shader_per_primitive_shading_rate_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS11`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options11 {
    pub atomic_int64_on_descriptor_heap_resource_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS12`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options12 {
    pub ms_primitives_pipeline_statistic_includes_culled_primitives: crate::enumerations::TriState,
    pub enhanced_barriers_supported: bool,
    pub relaxed_format_casting_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS13`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options13 {
    pub unrestricted_buffer_texture_copy_pitch_supported: bool,
    pub unrestricted_vertex_element_alignment_supported: bool,
    pub inverted_viewport_height_flips_y_supported: bool,
    pub inverted_viewport_depth_flips_z_supported: bool,
    pub texture_copy_between_dimensions_supported: bool,
    pub alpha_blend_factor_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options14 {
    pub advanced_texture_ops_supported: bool,
    pub writeable_msaa_textures_supported: bool,
    pub independent_front_and_back_stencil_ref_mask_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options15 {
    pub triangle_fan_supported: bool,
    pub dynamic_index_buffer_strip_cut_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options16 {
    pub dynamic_depth_bias_supported: bool,
    pub gpu_upload_heap_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options17 {
    pub non_normalized_coordinate_samplers_supported: bool,
    pub manual_write_tracking_resource_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options18 {
    pub render_passes_valid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options19 {
    pub mismatching_output_dimensions_supported: bool,
    pub supported_sample_counts_with_no_outputs: u32,
    pub point_sampling_addresses_never_round_up: bool,
    pub rasterizer_desc2_supported: bool,
    pub narrow_quadrilateral_lines_supported: bool,
    pub aniso_filter_with_point_mip_supported: bool,
    pub max_sampler_descriptor_heap_size: u32,
    pub max_sampler_descriptor_heap_size_with_static_samplers: u32,
    pub max_view_descriptor_heap_size: u32,
    pub compute_only_custom_heap_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options2 {
    pub depth_bounds_test_supported: bool,
    pub programmable_sample_positions_tier: crate::enumerations::ProgrammableSamplePositionsTier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options20 {
    pub compute_only_write_watch_supported: bool,
    pub recreate_at_tier: crate::enumerations::RecreateAtTier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options21 {
    pub work_graphs_tier: crate::enumerations::WorkGraphsTier,
    pub execute_indirect_tier: crate::enumerations::ExecuteIndirectTier,
    pub sample_cmp_gradient_and_bias_supported: bool,
    pub extended_command_info_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options3 {
    pub copy_queue_timestamp_queries_supported: bool,
    pub casting_fully_typed_format_supported: bool,
    pub write_buffer_immediate_support_flags: crate::enumerations::CommandListSupportFlags,
    pub view_instancing_tier: crate::enumerations::ViewInstancingTier,
    pub barycentrics_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options4 {
    pub msaa64_kb_aligned_texture_supported: bool,
    pub shared_resource_compatibility_tier: crate::enumerations::SharedResourceCompatibilityTier,
    pub native16_bit_shader_ops_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options5 {
    pub srv_only_tiled_resource_tier3: bool,
    pub render_passes_tier: crate::enumerations::RenderPassTier,
    pub raytracing_tier: crate::enumerations::RaytracingTier,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS6`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options6 {
    pub additional_shading_rates_supported: bool,
    pub per_primitive_shading_rate_supported_with_viewport_indexing: bool,
    pub variable_shading_rate_tier: crate::enumerations::VariableShadingRateTier,
    pub shading_rate_image_tile_size: u32,
    pub background_processing_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS7`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options7 {
    pub mesh_shader_tier: crate::enumerations::MeshShaderTier,
    pub sampler_feedback_tier: crate::enumerations::SamplerFeedbackTier,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS8`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options8 {
    pub unaligned_block_textures_supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_D3D12_OPTIONS9`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_d3d12_options9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataD3D12Options9 {
    pub mesh_shader_pipeline_stats_supported: bool,
    pub mesh_shader_supports_full_range_render_target_array_index: bool,
    pub atomic_int64_on_typed_resource_supported: bool,
    pub atomic_int64_on_group_shared_supported: bool,
    pub derivatives_in_mesh_and_amplification_shaders_supported: bool,
    pub wave_mma_tier: crate::enumerations::WaveMmaTier,
}

/// The header's `D3D12_FEATURE_DATA_DISPLAYABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_displayable).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataDisplayable {
    pub displayable_texture: bool,
    pub shared_resource_compatibility_tier: crate::enumerations::SharedResourceCompatibilityTier,
}

/// The header's `D3D12_FEATURE_DATA_EXISTING_HEAPS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_existing_heaps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataExistingHeaps {
    pub supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_FEATURE_LEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_feature_levels).
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureDataFeatureLevels {
    pub num_feature_levels: u32,
    pub feature_levels_requested: Vec<direct3d_common::enumerations::FeatureLevel>,
    pub max_supported_feature_level: direct3d_common::enumerations::FeatureLevel,
}

/// The header's `D3D12_FEATURE_DATA_FORMAT_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_format_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataFormatInfo {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub plane_count: u8,
}

/// The header's `D3D12_FEATURE_DATA_FORMAT_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_format_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataFormatSupport {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub support1: crate::enumerations::FormatSupport1,
    pub support2: crate::enumerations::FormatSupport2,
}

/// The header's `D3D12_FEATURE_DATA_GPU_VIRTUAL_ADDRESS_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_gpu_virtual_address_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataGpuVirtualAddressSupport {
    pub max_gpu_virtual_address_bits_per_resource: u32,
    pub max_gpu_virtual_address_bits_per_process: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataHardwareCopy {
    pub supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_multisample_quality_levels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataMultisampleQualityLevels {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub sample_count: u32,
    pub flags: crate::enumerations::MultisampleQualityLevelFlags,
    pub num_quality_levels: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataPlacedResourceSupportInfo {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub dimension: crate::enumerations::ResourceDimension,
    pub dest_heap_properties: crate::structures::HeapProperties,
    pub supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataPredication {
    pub supported: bool,
}

/// The header's `D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_SUPPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_protected_resource_session_support).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataProtectedResourceSessionSupport {
    pub node_index: u32,
    pub support: crate::enumerations::ProtectedResourceSessionSupportFlags,
}

/// The header's `D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_TYPES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_protected_resource_session_types).
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureDataProtectedResourceSessionTypes {
    pub node_index: u32,
    pub count: u32,
    pub types: Vec<component_object_model::structures::Guid>,
}

/// The header's `D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_TYPE_COUNT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_protected_resource_session_type_count).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataProtectedResourceSessionTypeCount {
    pub node_index: u32,
    pub count: u32,
}

/// The header's `D3D12_FEATURE_DATA_QUERY_META_COMMAND`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_query_meta_command).
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureDataQueryMetaCommand {
    pub command_id: component_object_model::structures::Guid,
    pub node_mask: u32,
    pub query_input_data: Vec<u8>,
    pub query_input_data_size_in_bytes: usize,
    pub query_output_data: Vec<u8>,
    pub query_output_data_size_in_bytes: usize,
}

/// The header's `D3D12_FEATURE_DATA_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_root_signature).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataRootSignature {
    pub highest_version: crate::enumerations::RootSignatureVersion,
}

/// The header's `D3D12_FEATURE_DATA_SERIALIZATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_serialization).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataSerialization {
    pub node_index: u32,
    pub heap_serialization_tier: crate::enumerations::HeapSerializationTier,
}

/// The header's `D3D12_FEATURE_DATA_SHADER_CACHE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_shader_cache).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataShaderCache {
    pub support_flags: crate::enumerations::ShaderCacheSupportFlags,
}

/// The header's `D3D12_FEATURE_DATA_SHADER_MODEL`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_feature_data_shader_model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureDataShaderModel {
    pub highest_shader_model: crate::enumerations::ShaderModel,
}

/// The header's `D3D12_FUNCTION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_function_desc).
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

#[derive(Clone, Debug, PartialEq)]
pub struct GenericProgramDesc {
    pub program_name: Vec<u16>,
    pub num_exports: u32,
    pub exports: Vec<Vec<u16>>,
    pub num_subobjects: u32,
    pub subobjects: Vec<u8>,
}

/// The header's `D3D12_GLOBAL_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_global_barrier).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalBarrier {
    pub sync_before: crate::enumerations::BarrierSync,
    pub sync_after: crate::enumerations::BarrierSync,
    pub access_before: crate::enumerations::BarrierAccess,
    pub access_after: crate::enumerations::BarrierAccess,
}

/// The header's `D3D12_GLOBAL_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_global_root_signature).
pub struct GlobalRootSignature<A: crate::Direct3D12> {
    pub global_root_signature: Option<<A as crate::Direct3D12>::RootSignature>,
}

/// The header's `D3D12_GPU_DESCRIPTOR_HANDLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_gpu_descriptor_handle).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuDescriptorHandle {
    pub ptr: u64,
}

/// The header's `D3D12_GPU_VIRTUAL_ADDRESS_AND_STRIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_gpu_virtual_address_and_stride).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuVirtualAddressAndStride {
    pub start_address: u64,
    pub stride_in_bytes: u64,
}

/// The header's `D3D12_GPU_VIRTUAL_ADDRESS_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_gpu_virtual_address_range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuVirtualAddressRange {
    pub start_address: u64,
    pub size_in_bytes: u64,
}

/// The header's `D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_gpu_virtual_address_range_and_stride).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuVirtualAddressRangeAndStride {
    pub start_address: u64,
    pub size_in_bytes: u64,
    pub stride_in_bytes: u64,
}

/// The header's `D3D12_GRAPHICS_PIPELINE_STATE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_graphics_pipeline_state_desc).
pub struct GraphicsPipelineStateDesc<A: crate::Direct3D12> {
    pub root_signature: Option<<A as crate::Direct3D12>::RootSignature>,
    pub vs: crate::structures::ShaderBytecode,
    pub ps: crate::structures::ShaderBytecode,
    pub ds: crate::structures::ShaderBytecode,
    pub hs: crate::structures::ShaderBytecode,
    pub gs: crate::structures::ShaderBytecode,
    pub stream_output: crate::structures::StreamOutputDesc,
    pub blend_state: crate::structures::BlendDesc,
    pub sample_mask: u32,
    pub rasterizer_state: crate::structures::RasterizerDesc,
    pub depth_stencil_state: crate::structures::DepthStencilDesc,
    pub input_layout: crate::structures::InputLayoutDesc,
    pub ib_strip_cut_value: crate::enumerations::IndexBufferStripCutValue,
    pub primitive_topology_type: crate::enumerations::PrimitiveTopologyType,
    pub num_render_targets: u32,
    pub rtv_formats: [directx_graphics_infrastructure::enumerations::Format;
        crate::constants::D3D12_GRAPHICS_PIPELINE_STATE_DESC_RTVFORMATS_LENGTH],
    pub dsv_format: directx_graphics_infrastructure::enumerations::Format,
    pub sample_desc: directx_graphics_infrastructure::structures::SampleDesc,
    pub node_mask: u32,
    pub cached_pso: crate::structures::CachedPipelineState,
    pub flags: crate::enumerations::PipelineStateFlags,
}

/// The header's `D3D12_HEAP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_heap_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeapDesc {
    pub size_in_bytes: u64,
    pub properties: crate::structures::HeapProperties,
    pub alignment: u64,
    pub flags: crate::enumerations::HeapFlags,
}

/// The header's `D3D12_HEAP_PROPERTIES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_heap_properties).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeapProperties {
    pub type_: crate::enumerations::HeapType,
    pub cpu_page_property: crate::enumerations::CpuPageProperty,
    pub memory_pool_preference: crate::enumerations::MemoryPool,
    pub creation_node_mask: u32,
    pub visible_node_mask: u32,
}

/// The header's `D3D12_HIT_GROUP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_hit_group_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct HitGroupDesc {
    pub hit_group_export: Vec<u16>,
    pub type_: crate::enumerations::HitGroupType,
    pub any_hit_shader_import: Vec<u16>,
    pub closest_hit_shader_import: Vec<u16>,
    pub intersection_shader_import: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IbStripCutValue {
    pub index_buffer_strip_cut_value: crate::enumerations::IndexBufferStripCutValue,
}

/// The header's `D3D12_INDEX_BUFFER_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_index_buffer_view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexBufferView {
    pub buffer_location: u64,
    pub size_in_bytes: u32,
    pub format: directx_graphics_infrastructure::enumerations::Format,
}

/// The header's `D3D12_INDIRECT_ARGUMENT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_indirect_argument_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDesc {
    pub type_: crate::enumerations::IndirectArgumentType,
    pub anonymous: crate::structures::IndirectArgumentDescAnonymous,
}

/// A C union of `D3D12_INDIRECT_ARGUMENT_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymous {
    pub vertex_buffer: crate::structures::IndirectArgumentDescAnonymousVertexbuffer,
    pub constant: crate::structures::IndirectArgumentDescAnonymousConstant,
    pub constant_buffer_view: crate::structures::IndirectArgumentDescAnonymousConstantbufferview,
    pub shader_resource_view: crate::structures::IndirectArgumentDescAnonymousShaderresourceview,
    pub unordered_access_view: crate::structures::IndirectArgumentDescAnonymousUnorderedaccessview,
    pub incrementing_constant: crate::structures::IndirectArgumentDescAnonymousIncrementingconstant,
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymousVertexbuffer {
    pub slot: u32,
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymousConstant {
    pub root_parameter_index: u32,
    pub dest_offset_in32_bit_values: u32,
    pub num32_bit_values_to_set: u32,
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymousConstantbufferview {
    pub root_parameter_index: u32,
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymousShaderresourceview {
    pub root_parameter_index: u32,
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymousUnorderedaccessview {
    pub root_parameter_index: u32,
}

/// A nested struct of `_Anonymous_e__Union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndirectArgumentDescAnonymousIncrementingconstant {
    pub root_parameter_index: u32,
    pub dest_offset_in32_bit_values: u32,
}

/// The header's `D3D12_INFO_QUEUE_FILTER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ns-d3d12sdklayers-d3d12_info_queue_filter).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueFilter {
    pub allow_list: crate::structures::InfoQueueFilterDesc,
    pub deny_list: crate::structures::InfoQueueFilterDesc,
}

/// The header's `D3D12_INFO_QUEUE_FILTER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ns-d3d12sdklayers-d3d12_info_queue_filter_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoQueueFilterDesc {
    pub num_categories: u32,
    pub category_list: Vec<crate::enumerations::MessageCategory>,
    pub num_severities: u32,
    pub severity_list: Vec<crate::enumerations::MessageSeverity>,
    pub num_i_ds: u32,
    pub id_list: Vec<crate::enumerations::MessageId>,
}

/// The header's `D3D12_INPUT_ELEMENT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_input_element_desc).
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

/// The header's `D3D12_INPUT_LAYOUT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_input_layout_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct InputLayoutDesc {
    pub input_element_descs: Vec<crate::structures::InputElementDesc>,
    pub num_elements: u32,
}

/// The header's `D3D12_LIBRARY_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_library_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryDesc {
    pub creator: std::ffi::CString,
    pub flags: u32,
    pub function_count: u32,
}

/// The header's `D3D12_LOCAL_ROOT_SIGNATURE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_local_root_signature).
pub struct LocalRootSignature<A: crate::Direct3D12> {
    pub local_root_signature: Option<<A as crate::Direct3D12>::RootSignature>,
}

/// The header's `D3D12_MEMCPY_DEST`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_memcpy_dest).
#[derive(Clone, Debug, PartialEq)]
pub struct MemcpyDest {
    pub data: Vec<u8>,
    pub row_pitch: usize,
    pub slice_pitch: usize,
}

/// The header's `D3D12_MESSAGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12sdklayers/ns-d3d12sdklayers-d3d12_message).
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub category: crate::enumerations::MessageCategory,
    pub severity: crate::enumerations::MessageSeverity,
    pub id: crate::enumerations::MessageId,
    pub description: Vec<u8>,
    pub description_byte_length: usize,
}

/// The header's `D3D12_META_COMMAND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_meta_command_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct MetaCommandDesc {
    pub id: component_object_model::structures::Guid,
    pub name: Vec<u16>,
    pub initialization_dirty_state: crate::enumerations::GraphicsStates,
    pub execution_dirty_state: crate::enumerations::GraphicsStates,
}

/// The header's `D3D12_META_COMMAND_PARAMETER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_meta_command_parameter_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct MetaCommandParameterDesc {
    pub name: Vec<u16>,
    pub type_: crate::enumerations::MetaCommandParameterType,
    pub flags: crate::enumerations::MetaCommandParameterFlags,
    pub required_resource_state: crate::enumerations::ResourceStates,
    pub structure_offset: u32,
}

/// The header's `D3D12_MIP_REGION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_mip_region).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MipRegion {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultiNodeCpuInput {
    pub num_node_inputs: u32,
    pub node_inputs: Vec<crate::structures::NodeCpuInput>,
    pub node_input_stride_in_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultiNodeGpuInput {
    pub num_node_inputs: u32,
    pub node_inputs: crate::structures::GpuVirtualAddressAndStride,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub node_type: crate::enumerations::NodeType,
    pub anonymous: crate::structures::NodeAnonymous,
}

/// A C union of `D3D12_NODE`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeAnonymous {
    pub shader: crate::structures::ShaderNode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeCpuInput {
    pub entrypoint_index: u32,
    pub num_records: u32,
    pub records: Vec<u8>,
    pub record_stride_in_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeGpuInput {
    pub entrypoint_index: u32,
    pub num_records: u32,
    pub records: crate::structures::GpuVirtualAddressAndStride,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeId {
    pub name: Vec<u16>,
    pub array_index: u32,
}

/// The header's `D3D12_NODE_MASK`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_node_mask).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeMask {
    pub node_mask: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeOutputOverrides {
    pub output_index: u32,
    pub new_name: Vec<crate::structures::NodeId>,
    pub allow_sparse_nodes: Vec<bool>,
    pub max_records: Vec<u32>,
    pub max_records_shared_with_output_index: Vec<u32>,
}

/// The header's `D3D12_PACKED_MIP_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_packed_mip_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedMipInfo {
    pub num_standard_mips: u8,
    pub num_packed_mips: u8,
    pub num_tiles_for_packed_mips: u32,
    pub start_tile_index_in_overall_resource: u32,
}

/// The header's `D3D12_PARAMETER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_parameter_desc).
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

/// The header's `D3D12_PIPELINE_STATE_STREAM_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_pipeline_state_stream_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct PipelineStateStreamDesc {
    pub size_in_bytes: usize,
    pub pipeline_state_subobject_stream: Vec<u8>,
}

/// The header's `D3D12_PLACED_SUBRESOURCE_FOOTPRINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_placed_subresource_footprint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedSubresourceFootprint {
    pub offset: u64,
    pub footprint: crate::structures::SubresourceFootprint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimitiveTopologyDesc {
    pub primitive_topology: crate::enumerations::PrimitiveTopologyType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgramIdentifier {
    pub opaque_data: [u64; crate::constants::D3D12_PROGRAM_IDENTIFIER_OPAQUEDATA_LENGTH],
}

/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_protected_resource_session_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProtectedResourceSessionDesc {
    pub node_mask: u32,
    pub flags: crate::enumerations::ProtectedResourceSessionFlags,
}

/// The header's `D3D12_PROTECTED_RESOURCE_SESSION_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_protected_resource_session_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProtectedResourceSessionDesc1 {
    pub node_mask: u32,
    pub flags: crate::enumerations::ProtectedResourceSessionFlags,
    pub protection_type: component_object_model::structures::Guid,
}

/// The header's `D3D12_QUERY_DATA_PIPELINE_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_query_data_pipeline_statistics).
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDataPipelineStatistics1 {
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
    pub as_invocations: u64,
    pub ms_invocations: u64,
    pub ms_primitives: u64,
}

/// The header's `D3D12_QUERY_DATA_SO_STATISTICS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_query_data_so_statistics).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryDataSoStatistics {
    pub num_primitives_written: u64,
    pub primitives_storage_needed: u64,
}

/// The header's `D3D12_QUERY_HEAP_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_query_heap_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryHeapDesc {
    pub type_: crate::enumerations::QueryHeapType,
    pub count: u32,
    pub node_mask: u32,
}

/// The header's `D3D12_RANGE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub begin: usize,
    pub end: usize,
}

/// The header's `D3D12_RANGE_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_range_uint64).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeUint64 {
    pub begin: u64,
    pub end: u64,
}

/// The header's `D3D12_RASTERIZER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_rasterizer_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerDesc {
    pub fill_mode: crate::enumerations::FillMode,
    pub cull_mode: crate::enumerations::CullMode,
    pub front_counter_clockwise: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
    pub forced_sample_count: u32,
    pub conservative_raster: crate::enumerations::ConservativeRasterizationMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerDesc1 {
    pub fill_mode: crate::enumerations::FillMode,
    pub cull_mode: crate::enumerations::CullMode,
    pub front_counter_clockwise: bool,
    pub depth_bias: f32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
    pub forced_sample_count: u32,
    pub conservative_raster: crate::enumerations::ConservativeRasterizationMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerDesc2 {
    pub fill_mode: crate::enumerations::FillMode,
    pub cull_mode: crate::enumerations::CullMode,
    pub front_counter_clockwise: bool,
    pub depth_bias: f32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub line_rasterization_mode: crate::enumerations::LineRasterizationMode,
    pub forced_sample_count: u32,
    pub conservative_raster: crate::enumerations::ConservativeRasterizationMode,
}

/// The header's `D3D12_RAYTRACING_AABB`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_aabb).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAabb {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_COMPACTED_SIZE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_compacted_size_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructurePostbuildInfoCompactedSizeDesc {
    pub compacted_size_in_bytes: u64,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_CURRENT_SIZE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_current_size_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructurePostbuildInfoCurrentSizeDesc {
    pub current_size_in_bytes: u64,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructurePostbuildInfoDesc {
    pub dest_buffer: u64,
    pub info_type: crate::enumerations::RaytracingAccelerationStructurePostbuildInfoType,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_SERIALIZATION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_serialization_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructurePostbuildInfoSerializationDesc {
    pub serialized_size_in_bytes: u64,
    pub num_bottom_level_acceleration_structure_pointers: u64,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_POSTBUILD_INFO_TOOLS_VISUALIZATION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_postbuild_info_tools_visualization_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructurePostbuildInfoToolsVisualizationDesc {
    pub decoded_size_in_bytes: u64,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_prebuild_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructurePrebuildInfo {
    pub result_data_max_size_in_bytes: u64,
    pub scratch_data_size_in_bytes: u64,
    pub update_scratch_data_size_in_bytes: u64,
}

/// The header's `D3D12_RAYTRACING_ACCELERATION_STRUCTURE_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_acceleration_structure_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingAccelerationStructureSrv {
    pub location: u64,
}

/// The header's `D3D12_RAYTRACING_GEOMETRY_AABBS_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_geometry_aabbs_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingGeometryAabbsDesc {
    pub aabb_count: u64,
    pub aab_bs: crate::structures::GpuVirtualAddressAndStride,
}

/// The header's `D3D12_RAYTRACING_GEOMETRY_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_geometry_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingGeometryDesc {
    pub type_: crate::enumerations::RaytracingGeometryType,
    pub flags: crate::enumerations::RaytracingGeometryFlags,
    pub anonymous: crate::structures::RaytracingGeometryDescAnonymous,
}

/// A C union of `D3D12_RAYTRACING_GEOMETRY_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingGeometryDescAnonymous {
    pub triangles: crate::structures::RaytracingGeometryTrianglesDesc,
    pub aab_bs: crate::structures::RaytracingGeometryAabbsDesc,
}

/// The header's `D3D12_RAYTRACING_GEOMETRY_TRIANGLES_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_geometry_triangles_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingGeometryTrianglesDesc {
    pub transform3x4: u64,
    pub index_format: directx_graphics_infrastructure::enumerations::Format,
    pub vertex_format: directx_graphics_infrastructure::enumerations::Format,
    pub index_count: u32,
    pub vertex_count: u32,
    pub index_buffer: u64,
    pub vertex_buffer: crate::structures::GpuVirtualAddressAndStride,
}

/// The header's `D3D12_RAYTRACING_INSTANCE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_instance_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingInstanceDesc {
    pub transform: [f32; crate::constants::D3D12_RAYTRACING_INSTANCE_DESC_TRANSFORM_LENGTH],
    pub _bitfield1: u32,
    pub _bitfield2: u32,
    pub acceleration_structure: u64,
}

/// The header's `D3D12_RAYTRACING_PIPELINE_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_pipeline_config).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingPipelineConfig {
    pub max_trace_recursion_depth: u32,
}

/// The header's `D3D12_RAYTRACING_PIPELINE_CONFIG1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_pipeline_config1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingPipelineConfig1 {
    pub max_trace_recursion_depth: u32,
    pub flags: crate::enumerations::RaytracingPipelineFlags,
}

/// The header's `D3D12_RAYTRACING_SHADER_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_raytracing_shader_config).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaytracingShaderConfig {
    pub max_payload_size_in_bytes: u32,
    pub max_attribute_size_in_bytes: u32,
}

/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_beginning_access).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassBeginningAccess {
    pub type_: crate::enumerations::RenderPassBeginningAccessType,
    pub anonymous: crate::structures::RenderPassBeginningAccessAnonymous,
}

/// A C union of `D3D12_RENDER_PASS_BEGINNING_ACCESS`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassBeginningAccessAnonymous {
    pub clear: crate::structures::RenderPassBeginningAccessClearParameters,
    pub preserve_local: crate::structures::RenderPassBeginningAccessPreserveLocalParameters,
}

/// The header's `D3D12_RENDER_PASS_BEGINNING_ACCESS_CLEAR_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_beginning_access_clear_parameters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassBeginningAccessClearParameters {
    pub clear_value: crate::structures::ClearValue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassBeginningAccessPreserveLocalParameters {
    pub additional_width: u32,
    pub additional_height: u32,
}

/// The header's `D3D12_RENDER_PASS_DEPTH_STENCIL_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_depth_stencil_desc).
pub struct RenderPassDepthStencilDesc<A: crate::Direct3D12> {
    pub cpu_descriptor: crate::structures::CpuDescriptorHandle,
    pub depth_beginning_access: crate::structures::RenderPassBeginningAccess,
    pub stencil_beginning_access: crate::structures::RenderPassBeginningAccess,
    pub depth_ending_access: crate::structures::RenderPassEndingAccess<A>,
    pub stencil_ending_access: crate::structures::RenderPassEndingAccess<A>,
}

/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_ending_access).
pub struct RenderPassEndingAccess<A: crate::Direct3D12> {
    pub type_: crate::enumerations::RenderPassEndingAccessType,
    pub anonymous: crate::structures::RenderPassEndingAccessAnonymous<A>,
}

/// A C union of `D3D12_RENDER_PASS_ENDING_ACCESS`: every member, each read from the same bytes.
pub struct RenderPassEndingAccessAnonymous<A: crate::Direct3D12> {
    pub resolve: crate::structures::RenderPassEndingAccessResolveParameters<A>,
    pub preserve_local: crate::structures::RenderPassEndingAccessPreserveLocalParameters,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassEndingAccessPreserveLocalParameters {
    pub additional_width: u32,
    pub additional_height: u32,
}

/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_RESOLVE_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_ending_access_resolve_parameters).
pub struct RenderPassEndingAccessResolveParameters<A: crate::Direct3D12> {
    pub src_resource: Option<<A as crate::Direct3D12>::Resource>,
    pub dst_resource: Option<<A as crate::Direct3D12>::Resource>,
    pub subresource_count: u32,
    pub subresource_parameters:
        Vec<crate::structures::RenderPassEndingAccessResolveSubresourceParameters>,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub resolve_mode: crate::enumerations::ResolveMode,
    pub preserve_resolve_source: bool,
}

/// The header's `D3D12_RENDER_PASS_ENDING_ACCESS_RESOLVE_SUBRESOURCE_PARAMETERS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_ending_access_resolve_subresource_parameters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPassEndingAccessResolveSubresourceParameters {
    pub src_subresource: u32,
    pub dst_subresource: u32,
    pub dst_x: u32,
    pub dst_y: u32,
    pub src_rect: component_object_model::structures::Rect,
}

/// The header's `D3D12_RENDER_PASS_RENDER_TARGET_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_pass_render_target_desc).
pub struct RenderPassRenderTargetDesc<A: crate::Direct3D12> {
    pub cpu_descriptor: crate::structures::CpuDescriptorHandle,
    pub beginning_access: crate::structures::RenderPassBeginningAccess,
    pub ending_access: crate::structures::RenderPassEndingAccess<A>,
}

/// The header's `D3D12_RENDER_TARGET_BLEND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_target_blend_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetBlendDesc {
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

/// The header's `D3D12_RENDER_TARGET_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_render_target_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::RtvDimension,
    pub anonymous: crate::structures::RenderTargetViewDescAnonymous,
}

/// A C union of `D3D12_RENDER_TARGET_VIEW_DESC`: every member, each read from the same bytes.
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

/// The header's `D3D12_RESOURCE_ALIASING_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_aliasing_barrier).
pub struct ResourceAliasingBarrier<A: crate::Direct3D12> {
    pub resource_before: Option<<A as crate::Direct3D12>::Resource>,
    pub resource_after: Option<<A as crate::Direct3D12>::Resource>,
}

/// The header's `D3D12_RESOURCE_ALLOCATION_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_allocation_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceAllocationInfo {
    pub size_in_bytes: u64,
    pub alignment: u64,
}

/// The header's `D3D12_RESOURCE_ALLOCATION_INFO1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_allocation_info1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceAllocationInfo1 {
    pub offset: u64,
    pub alignment: u64,
    pub size_in_bytes: u64,
}

/// The header's `D3D12_RESOURCE_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_barrier).
pub struct ResourceBarrier<A: crate::Direct3D12> {
    pub type_: crate::enumerations::ResourceBarrierType,
    pub flags: crate::enumerations::ResourceBarrierFlags,
    pub anonymous: crate::structures::ResourceBarrierAnonymous<A>,
}

/// A C union of `D3D12_RESOURCE_BARRIER`: every member, each read from the same bytes.
pub struct ResourceBarrierAnonymous<A: crate::Direct3D12> {
    pub transition: crate::structures::ResourceTransitionBarrier<A>,
    pub aliasing: crate::structures::ResourceAliasingBarrier<A>,
    pub uav: crate::structures::ResourceUavBarrier<A>,
}

/// The header's `D3D12_RESOURCE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceDesc {
    pub dimension: crate::enumerations::ResourceDimension,
    pub alignment: u64,
    pub width: u64,
    pub height: u32,
    pub depth_or_array_size: u16,
    pub mip_levels: u16,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub sample_desc: directx_graphics_infrastructure::structures::SampleDesc,
    pub layout: crate::enumerations::TextureLayout,
    pub flags: crate::enumerations::ResourceFlags,
}

/// The header's `D3D12_RESOURCE_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_desc1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceDesc1 {
    pub dimension: crate::enumerations::ResourceDimension,
    pub alignment: u64,
    pub width: u64,
    pub height: u32,
    pub depth_or_array_size: u16,
    pub mip_levels: u16,
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub sample_desc: directx_graphics_infrastructure::structures::SampleDesc,
    pub layout: crate::enumerations::TextureLayout,
    pub flags: crate::enumerations::ResourceFlags,
    pub sampler_feedback_mip_region: crate::structures::MipRegion,
}

/// The header's `D3D12_RESOURCE_TRANSITION_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_transition_barrier).
pub struct ResourceTransitionBarrier<A: crate::Direct3D12> {
    pub resource: Option<<A as crate::Direct3D12>::Resource>,
    pub subresource: u32,
    pub state_before: crate::enumerations::ResourceStates,
    pub state_after: crate::enumerations::ResourceStates,
}

/// The header's `D3D12_RESOURCE_UAV_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_resource_uav_barrier).
pub struct ResourceUavBarrier<A: crate::Direct3D12> {
    pub resource: Option<<A as crate::Direct3D12>::Resource>,
}

/// The header's `D3D12_ROOT_CONSTANTS`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_constants).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootConstants {
    pub shader_register: u32,
    pub register_space: u32,
    pub num32_bit_values: u32,
}

/// The header's `D3D12_ROOT_DESCRIPTOR`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_descriptor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootDescriptor {
    pub shader_register: u32,
    pub register_space: u32,
}

/// The header's `D3D12_ROOT_DESCRIPTOR1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_descriptor1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootDescriptor1 {
    pub shader_register: u32,
    pub register_space: u32,
    pub flags: crate::enumerations::RootDescriptorFlags,
}

/// The header's `D3D12_ROOT_DESCRIPTOR_TABLE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_descriptor_table).
#[derive(Clone, Debug, PartialEq)]
pub struct RootDescriptorTable {
    pub num_descriptor_ranges: u32,
    pub descriptor_ranges: Vec<crate::structures::DescriptorRange>,
}

/// The header's `D3D12_ROOT_DESCRIPTOR_TABLE1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_descriptor_table1).
#[derive(Clone, Debug, PartialEq)]
pub struct RootDescriptorTable1 {
    pub num_descriptor_ranges: u32,
    pub descriptor_ranges: Vec<crate::structures::DescriptorRange1>,
}

/// The header's `D3D12_ROOT_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_parameter).
#[derive(Clone, Debug, PartialEq)]
pub struct RootParameter {
    pub parameter_type: crate::enumerations::RootParameterType,
    pub anonymous: crate::structures::RootParameterAnonymous,
    pub shader_visibility: crate::enumerations::ShaderVisibility,
}

/// A C union of `D3D12_ROOT_PARAMETER`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct RootParameterAnonymous {
    pub descriptor_table: crate::structures::RootDescriptorTable,
    pub constants: crate::structures::RootConstants,
    pub descriptor: crate::structures::RootDescriptor,
}

/// The header's `D3D12_ROOT_PARAMETER1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_parameter1).
#[derive(Clone, Debug, PartialEq)]
pub struct RootParameter1 {
    pub parameter_type: crate::enumerations::RootParameterType,
    pub anonymous: crate::structures::RootParameter1Anonymous,
    pub shader_visibility: crate::enumerations::ShaderVisibility,
}

/// A C union of `D3D12_ROOT_PARAMETER1`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct RootParameter1Anonymous {
    pub descriptor_table: crate::structures::RootDescriptorTable1,
    pub constants: crate::structures::RootConstants,
    pub descriptor: crate::structures::RootDescriptor1,
}

/// The header's `D3D12_ROOT_SIGNATURE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_signature_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct RootSignatureDesc {
    pub num_parameters: u32,
    pub parameters: Vec<crate::structures::RootParameter>,
    pub num_static_samplers: u32,
    pub static_samplers: Vec<crate::structures::StaticSamplerDesc>,
    pub flags: crate::enumerations::RootSignatureFlags,
}

/// The header's `D3D12_ROOT_SIGNATURE_DESC1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_root_signature_desc1).
#[derive(Clone, Debug, PartialEq)]
pub struct RootSignatureDesc1 {
    pub num_parameters: u32,
    pub parameters: Vec<crate::structures::RootParameter1>,
    pub num_static_samplers: u32,
    pub static_samplers: Vec<crate::structures::StaticSamplerDesc>,
    pub flags: crate::enumerations::RootSignatureFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RootSignatureDesc2 {
    pub num_parameters: u32,
    pub parameters: Vec<crate::structures::RootParameter1>,
    pub num_static_samplers: u32,
    pub static_samplers: Vec<crate::structures::StaticSamplerDesc1>,
    pub flags: crate::enumerations::RootSignatureFlags,
}

/// The header's `D3D12_RT_FORMAT_ARRAY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_rt_format_array).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RtFormatArray {
    pub rt_formats: [directx_graphics_infrastructure::enumerations::Format;
        crate::constants::D3D12_RT_FORMAT_ARRAY_RTFORMATS_LENGTH],
    pub num_render_targets: u32,
}

/// The header's `D3D12_SAMPLER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_sampler_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDesc {
    pub filter: crate::enumerations::Filter,
    pub address_u: crate::enumerations::TextureAddressMode,
    pub address_v: crate::enumerations::TextureAddressMode,
    pub address_w: crate::enumerations::TextureAddressMode,
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison_func: crate::enumerations::ComparisonFunc,
    pub border_color: [f32; crate::constants::D3D12_SAMPLER_DESC_BORDERCOLOR_LENGTH],
    pub min_lod: f32,
    pub max_lod: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDesc2 {
    pub filter: crate::enumerations::Filter,
    pub address_u: crate::enumerations::TextureAddressMode,
    pub address_v: crate::enumerations::TextureAddressMode,
    pub address_w: crate::enumerations::TextureAddressMode,
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison_func: crate::enumerations::ComparisonFunc,
    pub anonymous: crate::structures::SamplerDesc2Anonymous,
    pub min_lod: f32,
    pub max_lod: f32,
    pub flags: crate::enumerations::SamplerFlags,
}

/// A C union of `D3D12_SAMPLER_DESC2`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDesc2Anonymous {
    pub float_border_color:
        [f32; crate::constants::D3D12_SAMPLER_DESC2_ANONYMOUS_E_UNION_FLOATBORDERCOLOR_LENGTH],
    pub uint_border_color:
        [u32; crate::constants::D3D12_SAMPLER_DESC2_ANONYMOUS_E_UNION_UINTBORDERCOLOR_LENGTH],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleMask {
    pub sample_mask: u32,
}

/// The header's `D3D12_SAMPLE_POSITION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_sample_position).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplePosition {
    pub x: i8,
    pub y: i8,
}

/// The header's `D3D12_SERIALIZED_DATA_DRIVER_MATCHING_IDENTIFIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_serialized_data_driver_matching_identifier).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SerializedDataDriverMatchingIdentifier {
    pub driver_opaque_guid: component_object_model::structures::Guid,
    pub driver_opaque_versioning_data: [u8; crate::constants::D3D12_SERIALIZED_DATA_DRIVER_MATCHING_IDENTIFIER_DRIVEROPAQUEVERSIONINGDATA_LENGTH],
}

/// The header's `D3D12_SERIALIZED_RAYTRACING_ACCELERATION_STRUCTURE_HEADER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_serialized_raytracing_acceleration_structure_header).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SerializedRaytracingAccelerationStructureHeader {
    pub driver_matching_identifier: crate::structures::SerializedDataDriverMatchingIdentifier,
    pub serialized_size_in_bytes_including_header: u64,
    pub deserialized_size_in_bytes: u64,
    pub num_bottom_level_acceleration_structure_pointers_after_header: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetGenericPipelineDesc {
    pub program_identifier: crate::structures::ProgramIdentifier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetProgramDesc {
    pub type_: crate::enumerations::ProgramType,
    pub anonymous: crate::structures::SetProgramDescAnonymous,
}

/// A C union of `D3D12_SET_PROGRAM_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetProgramDescAnonymous {
    pub generic_pipeline: crate::structures::SetGenericPipelineDesc,
    pub raytracing_pipeline: crate::structures::SetRaytracingPipelineDesc,
    pub work_graph: crate::structures::SetWorkGraphDesc,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetRaytracingPipelineDesc {
    pub program_identifier: crate::structures::ProgramIdentifier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetWorkGraphDesc {
    pub program_identifier: crate::structures::ProgramIdentifier,
    pub flags: crate::enumerations::SetWorkGraphFlags,
    pub backing_memory: crate::structures::GpuVirtualAddressRange,
    pub node_local_root_arguments_table: crate::structures::GpuVirtualAddressRangeAndStride,
}

/// The header's `D3D12_SHADER_BUFFER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_shader_buffer_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderBufferDesc {
    pub name: std::ffi::CString,
    pub type_: direct3d_common::enumerations::CbufferType,
    pub variables: u32,
    pub size: u32,
    pub u_flags: u32,
}

/// The header's `D3D12_SHADER_BYTECODE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_shader_bytecode).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderBytecode {
    pub shader_bytecode: Vec<u8>,
    pub bytecode_length: usize,
}

/// The header's `D3D12_SHADER_CACHE_SESSION_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_shader_cache_session_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderCacheSessionDesc {
    pub identifier: component_object_model::structures::Guid,
    pub mode: crate::enumerations::ShaderCacheMode,
    pub flags: crate::enumerations::ShaderCacheFlags,
    pub maximum_in_memory_cache_size_bytes: u32,
    pub maximum_in_memory_cache_entries: u32,
    pub maximum_value_file_size_bytes: u32,
    pub version: u64,
}

/// The header's `D3D12_SHADER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_shader_desc).
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

/// The header's `D3D12_SHADER_INPUT_BIND_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_shader_input_bind_desc).
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
    pub space: u32,
    pub u_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderNode {
    pub shader: Vec<u16>,
    pub overrides_type: crate::enumerations::NodeOverridesType,
    pub anonymous: crate::structures::ShaderNodeAnonymous,
}

/// A C union of `D3D12_SHADER_NODE`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderNodeAnonymous {
    pub broadcasting_launch_overrides: Vec<crate::structures::BroadcastingLaunchOverrides>,
    pub coalescing_launch_overrides: Vec<crate::structures::CoalescingLaunchOverrides>,
    pub thread_launch_overrides: Vec<crate::structures::ThreadLaunchOverrides>,
    pub common_compute_node_overrides: Vec<crate::structures::CommonComputeNodeOverrides>,
}

/// The header's `D3D12_SHADER_RESOURCE_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_shader_resource_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderResourceViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::SrvDimension,
    pub shader4_component_mapping: u32,
    pub anonymous: crate::structures::ShaderResourceViewDescAnonymous,
}

/// A C union of `D3D12_SHADER_RESOURCE_VIEW_DESC`: every member, each read from the same bytes.
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
    pub raytracing_acceleration_structure: crate::structures::RaytracingAccelerationStructureSrv,
}

/// The header's `D3D12_SHADER_TYPE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_shader_type_desc).
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

/// The header's `D3D12_SHADER_VARIABLE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_shader_variable_desc).
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

/// The header's `D3D12_SIGNATURE_PARAMETER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12shader/ns-d3d12shader-d3d12_signature_parameter_desc).
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

/// The header's `D3D12_SO_DECLARATION_ENTRY`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_so_declaration_entry).
#[derive(Clone, Debug, PartialEq)]
pub struct SoDeclarationEntry {
    pub stream: u32,
    pub semantic_name: std::ffi::CString,
    pub semantic_index: u32,
    pub start_component: u8,
    pub component_count: u8,
    pub output_slot: u8,
}

/// The header's `D3D12_STATE_OBJECT_CONFIG`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_state_object_config).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateObjectConfig {
    pub flags: crate::enumerations::StateObjectFlags,
}

/// The header's `D3D12_STATE_OBJECT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_state_object_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct StateObjectDesc {
    pub type_: crate::enumerations::StateObjectType,
    pub num_subobjects: u32,
    pub subobjects: Vec<crate::structures::StateSubobject>,
}

/// The header's `D3D12_STATE_SUBOBJECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_state_subobject).
#[derive(Clone, Debug, PartialEq)]
pub struct StateSubobject {
    pub type_: crate::enumerations::StateSubobjectType,
    pub desc: Vec<u8>,
}

/// The header's `D3D12_STATIC_SAMPLER_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_static_sampler_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticSamplerDesc {
    pub filter: crate::enumerations::Filter,
    pub address_u: crate::enumerations::TextureAddressMode,
    pub address_v: crate::enumerations::TextureAddressMode,
    pub address_w: crate::enumerations::TextureAddressMode,
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison_func: crate::enumerations::ComparisonFunc,
    pub border_color: crate::enumerations::StaticBorderColor,
    pub min_lod: f32,
    pub max_lod: f32,
    pub shader_register: u32,
    pub register_space: u32,
    pub shader_visibility: crate::enumerations::ShaderVisibility,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticSamplerDesc1 {
    pub filter: crate::enumerations::Filter,
    pub address_u: crate::enumerations::TextureAddressMode,
    pub address_v: crate::enumerations::TextureAddressMode,
    pub address_w: crate::enumerations::TextureAddressMode,
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison_func: crate::enumerations::ComparisonFunc,
    pub border_color: crate::enumerations::StaticBorderColor,
    pub min_lod: f32,
    pub max_lod: f32,
    pub shader_register: u32,
    pub register_space: u32,
    pub shader_visibility: crate::enumerations::ShaderVisibility,
    pub flags: crate::enumerations::SamplerFlags,
}

/// The header's `D3D12_STREAM_OUTPUT_BUFFER_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_stream_output_buffer_view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamOutputBufferView {
    pub buffer_location: u64,
    pub size_in_bytes: u64,
    pub buffer_filled_size_location: u64,
}

/// The header's `D3D12_STREAM_OUTPUT_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_stream_output_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct StreamOutputDesc {
    pub so_declaration: Vec<crate::structures::SoDeclarationEntry>,
    pub num_entries: u32,
    pub buffer_strides: Vec<u32>,
    pub num_strides: u32,
    pub rasterized_stream: u32,
}

/// The header's `D3D12_SUBOBJECT_TO_EXPORTS_ASSOCIATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_subobject_to_exports_association).
#[derive(Clone, Debug, PartialEq)]
pub struct SubobjectToExportsAssociation {
    pub subobject_to_associate: Vec<crate::structures::StateSubobject>,
    pub num_exports: u32,
    pub exports: Vec<Vec<u16>>,
}

/// The header's `D3D12_SUBRESOURCE_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_subresource_data).
#[derive(Clone, Debug, PartialEq)]
pub struct SubresourceData {
    pub data: Vec<u8>,
    pub row_pitch: isize,
    pub slice_pitch: isize,
}

/// The header's `D3D12_SUBRESOURCE_FOOTPRINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_subresource_footprint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubresourceFootprint {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub row_pitch: u32,
}

/// The header's `D3D12_SUBRESOURCE_INFO`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_subresource_info).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubresourceInfo {
    pub offset: u64,
    pub row_pitch: u32,
    pub depth_pitch: u32,
}

/// The header's `D3D12_SUBRESOURCE_RANGE_UINT64`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_subresource_range_uint64).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubresourceRangeUint64 {
    pub subresource: u32,
    pub range: crate::structures::RangeUint64,
}

/// The header's `D3D12_SUBRESOURCE_TILING`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_subresource_tiling).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubresourceTiling {
    pub width_in_tiles: u32,
    pub height_in_tiles: u16,
    pub depth_in_tiles: u16,
    pub start_tile_index_in_overall_resource: u32,
}

/// The header's `D3D12_TEX1D_ARRAY_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_array_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArrayDsv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX1D_ARRAY_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_array_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArrayRtv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX1D_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArraySrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEX1D_ARRAY_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_array_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DArrayUav {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX1D_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DDsv {
    pub mip_slice: u32,
}

/// The header's `D3D12_TEX1D_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DRtv {
    pub mip_slice: u32,
}

/// The header's `D3D12_TEX1D_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEX1D_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex1d_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex1DUav {
    pub mip_slice: u32,
}

/// The header's `D3D12_TEX2DMS_ARRAY_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2dms_array_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArrayDsv {
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX2DMS_ARRAY_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2dms_array_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArrayRtv {
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX2DMS_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2dms_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArraySrv {
    pub first_array_slice: u32,
    pub array_size: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsArrayUav {
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX2DMS_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2dms_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsDsv {
    pub unused_field_nothing_to_define: u32,
}

/// The header's `D3D12_TEX2DMS_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2dms_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsRtv {
    pub unused_field_nothing_to_define: u32,
}

/// The header's `D3D12_TEX2DMS_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2dms_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsSrv {
    pub unused_field_nothing_to_define: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DmsUav {
    pub unused_field_nothing_to_define: u32,
}

/// The header's `D3D12_TEX2D_ARRAY_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_array_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayDsv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
}

/// The header's `D3D12_TEX2D_ARRAY_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_array_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayRtv {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub plane_slice: u32,
}

/// The header's `D3D12_TEX2D_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArraySrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub plane_slice: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEX2D_ARRAY_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_array_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DArrayUav {
    pub mip_slice: u32,
    pub first_array_slice: u32,
    pub array_size: u32,
    pub plane_slice: u32,
}

/// The header's `D3D12_TEX2D_DSV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_dsv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DDsv {
    pub mip_slice: u32,
}

/// The header's `D3D12_TEX2D_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DRtv {
    pub mip_slice: u32,
    pub plane_slice: u32,
}

/// The header's `D3D12_TEX2D_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub plane_slice: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEX2D_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex2d_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2DUav {
    pub mip_slice: u32,
    pub plane_slice: u32,
}

/// The header's `D3D12_TEX3D_RTV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex3d_rtv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex3DRtv {
    pub mip_slice: u32,
    pub first_w_slice: u32,
    pub w_size: u32,
}

/// The header's `D3D12_TEX3D_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex3d_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex3DSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEX3D_UAV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tex3d_uav).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex3DUav {
    pub mip_slice: u32,
    pub first_w_slice: u32,
    pub w_size: u32,
}

/// The header's `D3D12_TEXCUBE_ARRAY_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_texcube_array_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexcubeArraySrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub first_2d_array_face: u32,
    pub num_cubes: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEXCUBE_SRV`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_texcube_srv).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexcubeSrv {
    pub most_detailed_mip: u32,
    pub mip_levels: u32,
    pub resource_min_lod_clamp: f32,
}

/// The header's `D3D12_TEXTURE_BARRIER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_texture_barrier).
pub struct TextureBarrier<A: crate::Direct3D12> {
    pub sync_before: crate::enumerations::BarrierSync,
    pub sync_after: crate::enumerations::BarrierSync,
    pub access_before: crate::enumerations::BarrierAccess,
    pub access_after: crate::enumerations::BarrierAccess,
    pub layout_before: crate::enumerations::BarrierLayout,
    pub layout_after: crate::enumerations::BarrierLayout,
    pub resource: Option<<A as crate::Direct3D12>::Resource>,
    pub subresources: crate::structures::BarrierSubresourceRange,
    pub flags: crate::enumerations::TextureBarrierFlags,
}

/// The header's `D3D12_TEXTURE_COPY_LOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_texture_copy_location).
pub struct TextureCopyLocation<A: crate::Direct3D12> {
    pub resource: Option<<A as crate::Direct3D12>::Resource>,
    pub type_: crate::enumerations::TextureCopyType,
    pub anonymous: crate::structures::TextureCopyLocationAnonymous,
}

/// A C union of `D3D12_TEXTURE_COPY_LOCATION`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureCopyLocationAnonymous {
    pub placed_footprint: crate::structures::PlacedSubresourceFootprint,
    pub subresource_index: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThreadLaunchOverrides {
    pub local_root_arguments_table_index: Vec<u32>,
    pub program_entry: Vec<bool>,
    pub new_name: Vec<crate::structures::NodeId>,
    pub share_input_of: Vec<crate::structures::NodeId>,
    pub num_output_overrides: u32,
    pub output_overrides: Vec<crate::structures::NodeOutputOverrides>,
}

/// The header's `D3D12_TILED_RESOURCE_COORDINATE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tiled_resource_coordinate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiledResourceCoordinate {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub subresource: u32,
}

/// The header's `D3D12_TILE_REGION_SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tile_region_size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRegionSize {
    pub num_tiles: u32,
    pub use_box: bool,
    pub width: u32,
    pub height: u16,
    pub depth: u16,
}

/// The header's `D3D12_TILE_SHAPE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_tile_shape).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileShape {
    pub width_in_texels: u32,
    pub height_in_texels: u32,
    pub depth_in_texels: u32,
}

/// The header's `D3D12_UNORDERED_ACCESS_VIEW_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_unordered_access_view_desc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnorderedAccessViewDesc {
    pub format: directx_graphics_infrastructure::enumerations::Format,
    pub view_dimension: crate::enumerations::UavDimension,
    pub anonymous: crate::structures::UnorderedAccessViewDescAnonymous,
}

/// A C union of `D3D12_UNORDERED_ACCESS_VIEW_DESC`: every member, each read from the same bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnorderedAccessViewDescAnonymous {
    pub buffer: crate::structures::BufferUav,
    pub texture_1d: crate::structures::Tex1DUav,
    pub texture_1d_array: crate::structures::Tex1DArrayUav,
    pub texture_2d: crate::structures::Tex2DUav,
    pub texture_2d_array: crate::structures::Tex2DArrayUav,
    pub texture_2dms: crate::structures::Tex2DmsUav,
    pub texture_2dms_array: crate::structures::Tex2DmsArrayUav,
    pub texture_3d: crate::structures::Tex3DUav,
}

/// The header's `D3D12_VERSIONED_DEVICE_REMOVED_EXTENDED_DATA`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_versioned_device_removed_extended_data).
pub struct VersionedDeviceRemovedExtendedData<A: crate::Direct3D12> {
    pub version: crate::enumerations::DredVersion,
    pub anonymous: crate::structures::VersionedDeviceRemovedExtendedDataAnonymous<A>,
}

/// A C union of `D3D12_VERSIONED_DEVICE_REMOVED_EXTENDED_DATA`: every member, each read from the same bytes.
pub struct VersionedDeviceRemovedExtendedDataAnonymous<A: crate::Direct3D12> {
    pub dred_1_0: crate::structures::DeviceRemovedExtendedData<A>,
    pub dred_1_1: crate::structures::DeviceRemovedExtendedData1<A>,
    pub dred_1_2: crate::structures::DeviceRemovedExtendedData2<A>,
    pub dred_1_3: crate::structures::DeviceRemovedExtendedData3<A>,
}

/// The header's `D3D12_VERSIONED_ROOT_SIGNATURE_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_versioned_root_signature_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct VersionedRootSignatureDesc {
    pub version: crate::enumerations::RootSignatureVersion,
    pub anonymous: crate::structures::VersionedRootSignatureDescAnonymous,
}

/// A C union of `D3D12_VERSIONED_ROOT_SIGNATURE_DESC`: every member, each read from the same bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct VersionedRootSignatureDescAnonymous {
    pub desc_1_0: crate::structures::RootSignatureDesc,
    pub desc_1_1: crate::structures::RootSignatureDesc1,
    pub desc_1_2: crate::structures::RootSignatureDesc2,
}

/// The header's `D3D12_VERTEX_BUFFER_VIEW`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_vertex_buffer_view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexBufferView {
    pub buffer_location: u64,
    pub size_in_bytes: u32,
    pub stride_in_bytes: u32,
}

/// The header's `D3D12_VIEWPORT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_viewport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

/// The header's `D3D12_VIEW_INSTANCE_LOCATION`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_view_instance_location).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewInstanceLocation {
    pub viewport_array_index: u32,
    pub render_target_array_index: u32,
}

/// The header's `D3D12_VIEW_INSTANCING_DESC`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_view_instancing_desc).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewInstancingDesc {
    pub view_instance_count: u32,
    pub view_instance_locations: Vec<crate::structures::ViewInstanceLocation>,
    pub flags: crate::enumerations::ViewInstancingFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkGraphDesc {
    pub program_name: Vec<u16>,
    pub flags: crate::enumerations::WorkGraphFlags,
    pub num_entrypoints: u32,
    pub entrypoints: Vec<crate::structures::NodeId>,
    pub num_explicitly_defined_nodes: u32,
    pub explicitly_defined_nodes: Vec<crate::structures::Node>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkGraphMemoryRequirements {
    pub min_size_in_bytes: u64,
    pub max_size_in_bytes: u64,
    pub size_granularity_in_bytes: u32,
}

/// The header's `D3D12_WRITEBUFFERIMMEDIATE_PARAMETER`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/ns-d3d12-d3d12_writebufferimmediate_parameter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WritebufferimmediateParameter {
    pub dest: u64,
    pub value: u32,
}
