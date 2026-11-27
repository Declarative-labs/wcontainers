//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The library d3d12.dll, and the debug layer's interfaces its objects answer.

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;

use wcontainers_component_object_model::structures::HResult;

pub mod constants;
pub mod enumerations;
pub mod structures;

/// The library d3d12.dll, and the debug layer's interfaces its objects answer.
pub trait Direct3D12: Sized + direct3d_common::Direct3DCommon {
    type CommandAllocator: CommandAllocator<Api = Self>;
    type CommandQueue: CommandQueue<Api = Self>;
    type DescriptorHeap: DescriptorHeap<Api = Self>;
    type Device: Device<Api = Self>;
    type Fence: Fence<Api = Self>;
    type GraphicsCommandList: GraphicsCommandList<Api = Self>;
    type PipelineState: PipelineState<Api = Self>;
    type QueryHeap: QueryHeap<Api = Self>;
    type Resource: Resource<Api = Self>;
    type RootSignature: RootSignature<Api = Self>;

    /// The header's `CreateDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-d3d12createdevice).
    fn create_device(
        &self,
        adapter: Option<UnknownRef<'_, Self>>,
        minimum_feature_level: direct3d_common::enumerations::FeatureLevel,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self>, HResult>;
}

/// The header's `ID3D12CommandAllocator`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12commandallocator).
pub trait CommandAllocator: Pageable {
    /// The header's `Reset`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12commandallocator-reset).
    fn reset(&self) -> Result<(), HResult>;
}

/// The header's `ID3D12CommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12commandlist).
pub trait CommandList: DeviceChild {}

/// The header's `ID3D12CommandQueue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12commandqueue).
pub trait CommandQueue: Pageable {
    /// The header's `ExecuteCommandLists`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12commandqueue-executecommandlists).
    fn execute_command_lists(&self, command_lists: &[Option<CommandListRef<'_, Self::Api>>]);
    /// The header's `Signal`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12commandqueue-signal).
    fn signal(&self, fence: &<Self::Api as Direct3D12>::Fence, value: u64) -> Result<(), HResult>;
}

/// The header's `ID3D12DescriptorHeap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12descriptorheap).
pub trait DescriptorHeap: Pageable {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12descriptorheap-getdesc).
    fn get_desc(&self) -> structures::DescriptorHeapDesc;
    /// The header's `GetCPUDescriptorHandleForHeapStart`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12descriptorheap-getcpudescriptorhandleforheapstart).
    fn get_cpu_descriptor_handle_for_heap_start(&self) -> structures::CpuDescriptorHandle;
    /// The header's `GetGPUDescriptorHandleForHeapStart`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12descriptorheap-getgpudescriptorhandleforheapstart).
    fn get_gpu_descriptor_handle_for_heap_start(&self) -> structures::GpuDescriptorHandle;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D12Device`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12device).
pub trait Device: Object {
    /// The header's `CreateCommandQueue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createcommandqueue).
    fn create_command_queue(
        &self,
        desc: &structures::CommandQueueDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateCommandAllocator`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createcommandallocator).
    fn create_command_allocator(
        &self,
        type_: enumerations::CommandListType,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateGraphicsPipelineState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-creategraphicspipelinestate).
    fn create_graphics_pipeline_state(
        &self,
        desc: &structures::GraphicsPipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateComputePipelineState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createcomputepipelinestate).
    fn create_compute_pipeline_state(
        &self,
        desc: &structures::ComputePipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateCommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createcommandlist).
    fn create_command_list(
        &self,
        node_mask: u32,
        type_: enumerations::CommandListType,
        command_allocator: &<Self::Api as Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as Direct3D12>::PipelineState>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateDescriptorHeap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createdescriptorheap).
    fn create_descriptor_heap(
        &self,
        descriptor_heap_desc: &structures::DescriptorHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `GetDescriptorHandleIncrementSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-getdescriptorhandleincrementsize).
    fn get_descriptor_handle_increment_size(
        &self,
        descriptor_heap_type: enumerations::DescriptorHeapType,
    ) -> u32;
    /// The header's `CreateRootSignature`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createrootsignature).
    fn create_root_signature(
        &self,
        node_mask: u32,
        blob_with_root_signature: &[u8],
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateShaderResourceView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createshaderresourceview).
    fn create_shader_resource_view(
        &self,
        resource: Option<&<Self::Api as Direct3D12>::Resource>,
        desc: Option<&structures::ShaderResourceViewDesc>,
        dest_descriptor: structures::CpuDescriptorHandle,
    );
    /// The header's `CreateRenderTargetView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createrendertargetview).
    fn create_render_target_view(
        &self,
        resource: Option<&<Self::Api as Direct3D12>::Resource>,
        desc: Option<&structures::RenderTargetViewDesc>,
        dest_descriptor: structures::CpuDescriptorHandle,
    );
    /// The header's `CreateCommittedResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createcommittedresource).
    fn create_committed_resource(
        &self,
        heap_properties: &structures::HeapProperties,
        heap_flags: enumerations::HeapFlags,
        desc: &structures::ResourceDesc,
        initial_resource_state: enumerations::ResourceStates,
        optimized_clear_value: Option<&structures::ClearValue>,
        riid_resource: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateFence`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createfence).
    fn create_fence(
        &self,
        initial_value: u64,
        flags: enumerations::FenceFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `GetCopyableFootprints`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-getcopyablefootprints).
    fn get_copyable_footprints(
        &self,
        resource_desc: &structures::ResourceDesc,
        first_subresource: u32,
        num_subresources: u32,
        base_offset: u64,
    ) -> (
        Vec<structures::PlacedSubresourceFootprint>,
        Vec<u32>,
        Vec<u64>,
        u64,
    );
    /// The header's `CreateQueryHeap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12device-createqueryheap).
    fn create_query_heap(
        &self,
        desc: &structures::QueryHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

/// The header's `ID3D12DeviceChild`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12devicechild).
pub trait DeviceChild: Object {}

/// The header's `ID3D12Fence`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12fence).
pub trait Fence: Pageable {
    /// The header's `GetCompletedValue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12fence-getcompletedvalue).
    fn get_completed_value(&self) -> u64;
    /// The header's `SetEventOnCompletion`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12fence-seteventoncompletion).
    fn set_event_on_completion(
        &self,
        value: u64,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D12GraphicsCommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12graphicscommandlist).
pub trait GraphicsCommandList: CommandList {
    /// The header's `Close`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-close).
    fn close(&self) -> Result<(), HResult>;
    /// The header's `Reset`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-reset).
    fn reset(
        &self,
        allocator: &<Self::Api as Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as Direct3D12>::PipelineState>,
    ) -> Result<(), HResult>;
    /// The header's `DrawInstanced`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-drawinstanced).
    fn draw_instanced(
        &self,
        vertex_count_per_instance: u32,
        instance_count: u32,
        start_vertex_location: u32,
        start_instance_location: u32,
    );
    /// The header's `DrawIndexedInstanced`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-drawindexedinstanced).
    fn draw_indexed_instanced(
        &self,
        index_count_per_instance: u32,
        instance_count: u32,
        start_index_location: u32,
        base_vertex_location: i32,
        start_instance_location: u32,
    );
    /// The header's `Dispatch`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-dispatch).
    fn dispatch(
        &self,
        thread_group_count_x: u32,
        thread_group_count_y: u32,
        thread_group_count_z: u32,
    );
    /// The header's `CopyBufferRegion`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-copybufferregion).
    fn copy_buffer_region(
        &self,
        dst_buffer: &<Self::Api as Direct3D12>::Resource,
        dst_offset: u64,
        src_buffer: &<Self::Api as Direct3D12>::Resource,
        src_offset: u64,
        num_bytes: u64,
    );
    /// The header's `CopyTextureRegion`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-copytextureregion).
    fn copy_texture_region(
        &self,
        dst: &structures::TextureCopyLocation<Self::Api>,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src: &structures::TextureCopyLocation<Self::Api>,
        src_box: Option<&structures::Box>,
    );
    /// The header's `IASetPrimitiveTopology`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-iasetprimitivetopology).
    fn ia_set_primitive_topology(
        &self,
        primitive_topology: direct3d_common::enumerations::PrimitiveTopology,
    );
    /// The header's `SetPipelineState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setpipelinestate).
    fn set_pipeline_state(&self, pipeline_state: &<Self::Api as Direct3D12>::PipelineState);
    /// The header's `ResourceBarrier`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-resourcebarrier).
    fn resource_barrier(&self, barriers: &[structures::ResourceBarrier<Self::Api>]);
    /// The header's `SetDescriptorHeaps`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setdescriptorheaps).
    fn set_descriptor_heaps(
        &self,
        descriptor_heaps: &[Option<&<Self::Api as Direct3D12>::DescriptorHeap>],
    );
    /// The header's `SetComputeRootSignature`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setcomputerootsignature).
    fn set_compute_root_signature(
        &self,
        root_signature: Option<&<Self::Api as Direct3D12>::RootSignature>,
    );
    /// The header's `SetGraphicsRootSignature`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setgraphicsrootsignature).
    fn set_graphics_root_signature(
        &self,
        root_signature: Option<&<Self::Api as Direct3D12>::RootSignature>,
    );
    /// The header's `SetGraphicsRootDescriptorTable`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setgraphicsrootdescriptortable).
    fn set_graphics_root_descriptor_table(
        &self,
        root_parameter_index: u32,
        base_descriptor: structures::GpuDescriptorHandle,
    );
    /// The header's `SetComputeRootConstantBufferView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setcomputerootconstantbufferview).
    fn set_compute_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    );
    /// The header's `SetGraphicsRootConstantBufferView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setgraphicsrootconstantbufferview).
    fn set_graphics_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    );
    /// The header's `SetComputeRootUnorderedAccessView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-setcomputerootunorderedaccessview).
    fn set_compute_root_unordered_access_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    );
    /// The header's `IASetIndexBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-iasetindexbuffer).
    fn ia_set_index_buffer(&self, view: Option<&structures::IndexBufferView>);
    /// The header's `IASetVertexBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-iasetvertexbuffers).
    fn ia_set_vertex_buffers(&self, start_slot: u32, views: &[structures::VertexBufferView]);
    /// The header's `SOSetTargets`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-sosettargets).
    fn so_set_targets(&self, start_slot: u32, views: &[structures::StreamOutputBufferView]);
    /// The header's `OMSetRenderTargets`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-omsetrendertargets).
    fn om_set_render_targets(
        &self,
        num_render_target_descriptors: u32,
        render_target_descriptors: Option<&structures::CpuDescriptorHandle>,
        r_ts_single_handle_to_descriptor_range: bool,
        depth_stencil_descriptor: Option<&structures::CpuDescriptorHandle>,
    );
    /// The header's `ClearRenderTargetView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-clearrendertargetview).
    fn clear_render_target_view(
        &self,
        render_target_view: structures::CpuDescriptorHandle,
        color_rgba: &[f32; constants::GRAPHICS_COMMAND_LIST_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH],
        rects: &[component_object_model::structures::Rect],
    );
    /// The header's `BeginQuery`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-beginquery).
    fn begin_query(
        &self,
        query_heap: &<Self::Api as Direct3D12>::QueryHeap,
        type_: enumerations::QueryType,
        index: u32,
    );
    /// The header's `EndQuery`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-endquery).
    fn end_query(
        &self,
        query_heap: &<Self::Api as Direct3D12>::QueryHeap,
        type_: enumerations::QueryType,
        index: u32,
    );
    /// The header's `ResolveQueryData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12graphicscommandlist-resolvequerydata).
    fn resolve_query_data(
        &self,
        query_heap: &<Self::Api as Direct3D12>::QueryHeap,
        type_: enumerations::QueryType,
        start_index: u32,
        num_queries: u32,
        destination_buffer: &<Self::Api as Direct3D12>::Resource,
        aligned_destination_buffer_offset: u64,
    );
}

/// The header's `ID3D12Object`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12object).
pub trait Object: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D12;
}

/// The header's `ID3D12Pageable`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12pageable).
pub trait Pageable: DeviceChild {}

/// The header's `ID3D12PipelineState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12pipelinestate).
pub trait PipelineState: Pageable {}

/// The header's `ID3D12QueryHeap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12queryheap).
pub trait QueryHeap: Pageable {}

/// The header's `ID3D12Resource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12resource).
pub trait Resource: Pageable {
    /// The header's `Map`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12resource-map).
    fn map(
        &self,
        subresource: u32,
        read_range: Option<&structures::Range>,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `Unmap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12resource-unmap).
    fn unmap(&self, subresource: u32, written_range: Option<&structures::Range>);
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/direct3d12/id3d12resource-getdesc).
    fn get_desc(&self) -> structures::ResourceDesc;
    /// The header's `GetGPUVirtualAddress`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nf-d3d12-id3d12resource-getgpuvirtualaddress).
    fn get_gpu_virtual_address(&self) -> u64;
}

/// The header's `ID3D12RootSignature`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d12/nn-d3d12-id3d12rootsignature).
pub trait RootSignature: DeviceChild {}

/// An `ID3D12CommandList*`: one of the kinds of object that are one.
pub enum CommandListRef<'a, A: Direct3D12> {
    GraphicsCommandList(&'a <A as Direct3D12>::GraphicsCommandList),
}

/// An `IUnknown*`: an object of one of this API's kinds, or of none.
pub enum UnknownRef<'a, A: Direct3D12> {
    CommandAllocator(&'a <A as Direct3D12>::CommandAllocator),
    CommandQueue(&'a <A as Direct3D12>::CommandQueue),
    DescriptorHeap(&'a <A as Direct3D12>::DescriptorHeap),
    Device(&'a <A as Direct3D12>::Device),
    Fence(&'a <A as Direct3D12>::Fence),
    GraphicsCommandList(&'a <A as Direct3D12>::GraphicsCommandList),
    PipelineState(&'a <A as Direct3D12>::PipelineState),
    QueryHeap(&'a <A as Direct3D12>::QueryHeap),
    Resource(&'a <A as Direct3D12>::Resource),
    RootSignature(&'a <A as Direct3D12>::RootSignature),
    Unknown(&'a component_object_model::structures::Foreign),
}

/// An `IUnknown*`: an object of one of this API's kinds, or of none.
pub enum UnknownObject<A: Direct3D12> {
    CommandAllocator(<A as Direct3D12>::CommandAllocator),
    CommandQueue(<A as Direct3D12>::CommandQueue),
    DescriptorHeap(<A as Direct3D12>::DescriptorHeap),
    Device(<A as Direct3D12>::Device),
    Fence(<A as Direct3D12>::Fence),
    GraphicsCommandList(<A as Direct3D12>::GraphicsCommandList),
    PipelineState(<A as Direct3D12>::PipelineState),
    QueryHeap(<A as Direct3D12>::QueryHeap),
    Resource(<A as Direct3D12>::Resource),
    RootSignature(<A as Direct3D12>::RootSignature),
    Unknown(component_object_model::structures::Foreign),
}
