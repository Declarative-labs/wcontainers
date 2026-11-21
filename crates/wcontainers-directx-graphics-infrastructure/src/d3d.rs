//! Values and structures of Direct3D's headers that its versions share, named once for the fronts.

/// `D3D11_APPEND_ALIGNED_ELEMENT` and `D3D12_APPEND_ALIGNED_ELEMENT`: an input element placed right
/// after the one before it.
pub const APPEND_ALIGNED_ELEMENT: u32 = u32::MAX;

/// `D3D11_SO_NO_RASTERIZED_STREAM` and `D3D12_SO_NO_RASTERIZED_STREAM`.
pub const SO_NO_RASTERIZED_STREAM: u32 = u32::MAX;

/// `D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT`: subresources in a buffer start at multiples of this.
pub const TEXTURE_DATA_PLACEMENT_ALIGNMENT: u64 = 1 << 9;

/// `D3D12_SMALL_RESOURCE_PLACEMENT_ALIGNMENT`.
pub const SMALL_RESOURCE_PLACEMENT_ALIGNMENT: u64 = 4 << 10;

/// `D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT`.
pub const DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT: u64 = 64 << 10;

/// `D3D12_DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT`.
pub const DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT: u64 = 4 << 20;

/// `D3D11_QUERY_DATA_PIPELINE_STATISTICS` and `D3D12_QUERY_DATA_PIPELINE_STATISTICS`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PipelineStatistics {
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

/// `D3D11_QUERY_DATA_SO_STATISTICS` and `D3D12_QUERY_DATA_SO_STATISTICS`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StreamOutputStatistics {
    pub primitives_written: u64,
    pub primitives_storage_needed: u64,
}

/// `D3D11_DRAW_INSTANCED_INDIRECT_ARGS` and `D3D12_DRAW_ARGUMENTS`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawArguments {
    pub vertex_count_per_instance: u32,
    pub instance_count: u32,
    pub start_vertex_location: u32,
    pub start_instance_location: u32,
}

/// `D3D11_DRAW_INDEXED_INSTANCED_INDIRECT_ARGS` and `D3D12_DRAW_INDEXED_ARGUMENTS`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndexedArguments {
    pub index_count_per_instance: u32,
    pub instance_count: u32,
    pub start_index_location: u32,
    pub base_vertex_location: i32,
    pub start_instance_location: u32,
}

/// `D3D12_DISPATCH_ARGUMENTS`, and the three group counts `DispatchIndirect` reads in Direct3D 11.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DispatchArguments {
    pub thread_group_count_x: u32,
    pub thread_group_count_y: u32,
    pub thread_group_count_z: u32,
}
