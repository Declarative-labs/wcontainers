//! Values and structures of Direct3D's headers that its versions share, named once for the fronts.

/// `D3D11_APPEND_ALIGNED_ELEMENT` and `D3D12_APPEND_ALIGNED_ELEMENT`: an input element placed right
/// after the one before it.
pub const APPEND_ALIGNED_ELEMENT: u32 = u32::MAX;

/// `D3D11_SO_NO_RASTERIZED_STREAM` and `D3D12_SO_NO_RASTERIZED_STREAM`.
pub const SO_NO_RASTERIZED_STREAM: u32 = u32::MAX;

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
