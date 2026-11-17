//! Command lists: what the program records, kept front-side until `ExecuteCommandLists` replays it
//! into the back end. Recording touches no back end, so it runs on the program's own thread.

use std::sync::Arc;

use crate::Error;
use crate::device::{PipelineId, ResourceId, RootId};

/// A render target view as a descriptor holds it (`D3D12_RENDER_TARGET_VIEW_DESC` resolved against
/// its resource): the program copies descriptors as bytes, so this is plain data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct TargetView {
    pub resource: ResourceId,
    /// The DXGI format the view reads the texels as.
    pub format: u32,
    pub mip: u32,
    pub first_layer: u32,
    pub layers: u32,
}

/// A shader resource or unordered access view as a descriptor holds it
/// (`D3D12_SHADER_RESOURCE_VIEW_DESC`, `D3D12_UNORDERED_ACCESS_VIEW_DESC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct ResourceView {
    pub resource: ResourceId,
    /// The DXGI format the view reads the texels as (the resource's own where the view says none).
    pub format: u32,
    /// `D3D12_SRV_DIMENSION` or `D3D12_UAV_DIMENSION`.
    pub dimension: u32,
    /// `Shader4ComponentMapping` (shader resource views).
    pub mapping: u32,
    /// Buffers: the first element.
    pub first_element: u64,
    /// Buffers: the element count; textures: the mip count.
    pub count: u32,
    /// Buffers: the structure stride, zero for typed and raw views.
    pub stride: u32,
    /// Textures: the first mip.
    pub mip: u32,
    pub first_layer: u32,
    pub layers: u32,
    /// Buffers: `D3D12_BUFFER_SRV_FLAGS` or `D3D12_BUFFER_UAV_FLAGS` (raw).
    pub flags: u32,
    /// Textures: `ResourceMinLODClamp`, as f32 bits.
    pub min_lod: u32,
}

/// `D3D12_SAMPLER_DESC`, floats as bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct SamplerView {
    pub filter: u32,
    pub address: [u32; 3],
    pub lod_bias: u32,
    pub anisotropy: u32,
    pub compare: u32,
    pub border: [u32; 4],
    pub min_lod: u32,
    pub max_lod: u32,
}

/// What one descriptor slot of a heap holds. At most [`crate::address::DESCRIPTOR_SIZE`] bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C, u32)]
pub enum Descriptor {
    #[default]
    Empty,
    RenderTarget(TargetView),
    DepthStencil {
        view: TargetView,
        /// `D3D12_DSV_FLAGS`: 1 depth read only, 2 stencil read only.
        read_only: u32,
    },
    /// A constant buffer view: a GPU virtual address and a size in bytes.
    ConstantBuffer {
        address: u64,
        size: u32,
    },
    ShaderResource(ResourceView),
    UnorderedAccess(ResourceView),
    Sampler(SamplerView),
}

const _: () = assert!(size_of::<Descriptor>() <= crate::address::DESCRIPTOR_SIZE as usize);

/// `D3D12_VERTEX_BUFFER_VIEW`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VertexBufferView {
    pub address: u64,
    pub size: u32,
    pub stride: u32,
}

/// `D3D12_INDEX_BUFFER_VIEW`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexBufferView {
    pub address: u64,
    pub size: u32,
    /// `DXGI_FORMAT_R16_UINT` or `DXGI_FORMAT_R32_UINT`.
    pub format: u32,
}

/// `D3D12_VIEWPORT`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

/// A `RECT`: left, top, right, bottom.
pub type Rect = [i32; 4];

/// `D3D12_TEXTURE_COPY_LOCATION`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyLocation {
    Subresource {
        resource: ResourceId,
        index: u32,
    },
    Footprint {
        resource: ResourceId,
        footprint: crate::resource::Footprint,
    },
}

/// A root parameter's argument (`SetGraphicsRoot*`, `SetComputeRoot*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootArgument {
    /// A descriptor table's first GPU descriptor handle.
    Table(u64),
    /// A root descriptor's GPU virtual address.
    Descriptor(u64),
}

/// One recorded command.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Pipeline(PipelineId),
    /// `SetGraphicsRootSignature` (`compute` false) or `SetComputeRootSignature`.
    RootSignature {
        compute: bool,
        root: Option<RootId>,
    },
    RootArgument {
        compute: bool,
        index: u32,
        argument: RootArgument,
    },
    /// `Set*Root32BitConstants`: `values` from the 32-bit value `offset` of the parameter.
    RootConstants {
        compute: bool,
        index: u32,
        offset: u32,
        values: Vec<u32>,
    },
    Topology(u32),
    VertexBuffers {
        start: u32,
        views: Vec<VertexBufferView>,
    },
    IndexBuffer(Option<IndexBufferView>),
    Viewports(Vec<ViewportF>),
    Scissors(Vec<Rect>),
    BlendFactor([f32; 4]),
    StencilRef(u32),
    Targets {
        colors: Vec<Descriptor>,
        depth: Option<Descriptor>,
    },
    ClearTarget {
        view: Descriptor,
        color: [f32; 4],
        rects: Vec<Rect>,
    },
    /// `ClearDepthStencilView`: None leaves that aspect.
    ClearDepth {
        view: Descriptor,
        depth: Option<f32>,
        stencil: Option<u8>,
        rects: Vec<Rect>,
    },
    Draw {
        vertices: u32,
        instances: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    DrawIndexed {
        indices: u32,
        instances: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    },
    CopyBuffer {
        dst: ResourceId,
        dst_offset: u64,
        src: ResourceId,
        src_offset: u64,
        size: u64,
    },
    CopyTexture {
        dst: CopyLocation,
        at: [u32; 3],
        src: CopyLocation,
        /// left, top, front, right, bottom, back.
        area: Option<[u32; 6]>,
    },
    Dispatch([u32; 3]),
    /// `ClearUnorderedAccessViewUint` (`float` false: the values are integers) or `...Float` (the
    /// values are f32 bits).
    ClearStorage {
        view: ResourceView,
        values: [u32; 4],
        float: bool,
        rects: Vec<Rect>,
    },
}

/// A graphics command list being recorded, or closed and ready to execute.
#[derive(Debug, Default)]
pub struct CommandList {
    commands: Vec<Command>,
    /// The first error since `Reset`, which `Close` reports.
    error: Option<Error>,
    closed: Option<Arc<[Command]>>,
}

impl CommandList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts recording again; the commands of an earlier `Close` stay with whoever holds them.
    pub fn reset(&mut self) {
        self.commands.clear();
        self.error = None;
        self.closed = None;
    }

    pub fn record(&mut self, command: Command) {
        if self.closed.is_some() {
            self.fail(Error::Invalid("recording into a closed command list"));
        } else {
            self.commands.push(command);
        }
    }

    /// Notes an error: a call the list cannot take, reported by `Close` as Direct3D 12 does.
    pub fn fail(&mut self, error: Error) {
        self.error.get_or_insert(error);
    }

    pub fn close(&mut self) -> Result<(), Error> {
        if self.closed.is_some() {
            return Err(Error::Invalid("closing a closed command list"));
        }
        if let Some(e) = self.error {
            return Err(e);
        }
        self.closed = Some(std::mem::take(&mut self.commands).into());
        Ok(())
    }

    /// The commands of a closed list.
    pub fn commands(&self) -> Option<&Arc<[Command]>> {
        self.closed.as_ref()
    }
}
