//! Command lists: what the program records, kept front-side until `ExecuteCommandLists` replays it
//! into the back end. Recording touches no back end, so it runs on the program's own thread.

use std::sync::Arc;

use crate::Error;
use crate::device::{PipelineId, QueryHeapId, ResourceId};

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
    /// `Shader4ComponentMapping` (shader resource views); unordered access views: the low word of
    /// their counter's GPU virtual address ([`ResourceView::counter`]).
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
    /// Textures: `ResourceMinLODClamp`, as f32 bits; unordered access views: the high word of
    /// their counter's address.
    pub min_lod: u32,
}

impl ResourceView {
    /// An unordered access view's counter: the GPU virtual address of its 32 bits, 0 for none.
    pub fn counter(&self) -> u64 {
        u64::from(self.mapping) | u64::from(self.min_lod) << 32
    }

    /// The view with its counter at GPU virtual address `address`.
    pub fn with_counter(self, address: u64) -> Self {
        ResourceView {
            mapping: address as u32,
            min_lod: (address >> 32) as u32,
            ..self
        }
    }
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

/// `D3D12_DSV_FLAG_READ_ONLY_DEPTH`: draws only read the view's depth.
pub const DSV_READ_ONLY_DEPTH: u32 = 1;
/// `D3D12_DSV_FLAG_READ_ONLY_STENCIL`: draws only read the view's stencil.
pub const DSV_READ_ONLY_STENCIL: u32 = 2;

/// What one descriptor slot of a heap holds. At most [`crate::address::DESCRIPTOR_SIZE`] bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C, u32)]
pub enum Descriptor {
    #[default]
    Empty,
    RenderTarget(TargetView),
    DepthStencil {
        view: TargetView,
        /// `D3D12_DSV_FLAGS`: [`DSV_READ_ONLY_DEPTH`], [`DSV_READ_ONLY_STENCIL`].
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

/// One `D3D12_STREAM_OUTPUT_BUFFER_VIEW`: where a stream output buffer starts, its size, and
/// where its 64-bit filled size lies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamView {
    pub address: u64,
    pub size: u64,
    pub filled: u64,
}

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

/// One argument of a command signature (`D3D12_INDIRECT_ARGUMENT_DESC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndirectArgument {
    Draw,
    DrawIndexed,
    Dispatch,
    VertexBuffer {
        slot: u32,
    },
    IndexBuffer,
    /// `count` root constants of root parameter `root` from the 32-bit value `offset`.
    Constants {
        root: u32,
        offset: u32,
        count: u32,
    },
    /// A root constant buffer, shader resource or unordered access view's address.
    RootView {
        root: u32,
    },
}

impl IndirectArgument {
    /// The bytes it takes in each command.
    pub fn size(self) -> u32 {
        match self {
            IndirectArgument::Draw => 16,
            IndirectArgument::DrawIndexed => 20,
            IndirectArgument::Dispatch => 12,
            IndirectArgument::VertexBuffer { .. } | IndirectArgument::IndexBuffer => 16,
            IndirectArgument::Constants { count, .. } => 4 * count,
            IndirectArgument::RootView { .. } => 8,
        }
    }
}

/// A command signature (`ID3D12CommandSignature`): each command of `ExecuteIndirect`'s argument
/// buffer is these arguments, packed in order, ending with its draw or dispatch, `stride` bytes
/// apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSignature {
    pub stride: u32,
    pub arguments: Vec<IndirectArgument>,
}

impl CommandSignature {
    /// Checks the description as `CreateCommandSignature` does. `root` says whether a root
    /// signature was given, which arguments that change root parameters need.
    pub fn new(stride: u32, arguments: Vec<IndirectArgument>, root: bool) -> Result<Self, Error> {
        use IndirectArgument::*;
        let calls = arguments
            .iter()
            .filter(|a| matches!(a, Draw | DrawIndexed | Dispatch))
            .count();
        if calls != 1 || !matches!(arguments.last(), Some(Draw | DrawIndexed | Dispatch)) {
            return Err(Error::Invalid(
                "a command signature must end with its one draw or dispatch",
            ));
        }
        let size: u32 = arguments.iter().map(|a| a.size()).sum();
        if stride < size || !stride.is_multiple_of(4) {
            return Err(Error::Invalid("a command signature's stride"));
        }
        let changes_root = arguments
            .iter()
            .any(|a| matches!(a, Constants { .. } | RootView { .. }));
        if changes_root && !root {
            return Err(Error::Invalid(
                "a command signature changing root parameters without a root signature",
            ));
        }
        if arguments.last() == Some(&Dispatch)
            && arguments
                .iter()
                .any(|a| matches!(a, VertexBuffer { .. } | IndexBuffer))
        {
            return Err(Error::Invalid(
                "vertex or index buffers in a dispatch's command signature",
            ));
        }
        Ok(CommandSignature { stride, arguments })
    }

    /// The draw or dispatch alone: commands the back end's indirect calls take as they are.
    pub fn call_only(&self) -> bool {
        self.arguments.len() == 1
    }

    /// Whether `max` commands from `offset` fit in an argument buffer of `width` bytes, whole
    /// strides included, as `ExecuteIndirect` asks.
    pub fn fits(&self, max: u32, offset: u64, width: u64) -> bool {
        offset.is_multiple_of(4)
            && offset
                .checked_add(u64::from(max) * u64::from(self.stride))
                .is_some_and(|end| end <= width)
    }

    pub fn dispatches(&self) -> bool {
        self.arguments.last() == Some(&IndirectArgument::Dispatch)
    }
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
    /// `ExecuteBundle`: a closed bundle's commands.
    Bundle(Arc<[Command]>),
    Pipeline(PipelineId),
    /// `SetGraphicsRootSignature` (`compute` false) or `SetComputeRootSignature`.
    RootSignature {
        compute: bool,
        root: Option<crate::root::RootSignature>,
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
    /// `SOSetTargets`.
    StreamTargets {
        start: u32,
        views: Vec<StreamView>,
    },
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
    /// `WriteBufferImmediate`: `value` written at GPU virtual address `address`, in order with
    /// the work around it.
    WriteImmediate {
        address: u64,
        value: u32,
    },
    /// `ResolveSubresource`: the samples of subresource `src_index` averaged as `format` into
    /// `dst_index`.
    Resolve {
        dst: ResourceId,
        dst_index: u32,
        src: ResourceId,
        src_index: u32,
        format: u32,
    },
    /// `BeginQuery` and `EndQuery`, `D3D12_QUERY_TYPE` in `kind`.
    BeginQuery {
        heap: QueryHeapId,
        kind: u32,
        index: u32,
    },
    EndQuery {
        heap: QueryHeapId,
        kind: u32,
        index: u32,
    },
    ResolveQueries {
        heap: QueryHeapId,
        kind: u32,
        first: u32,
        count: u32,
        dst: ResourceId,
        offset: u64,
    },
    /// `ExecuteIndirect`: up to `max` commands of `signature` from the argument buffer, as many as
    /// the 32-bit count in the count buffer says where one is given.
    ExecuteIndirect {
        signature: Arc<CommandSignature>,
        max: u32,
        args: ResourceId,
        args_offset: u64,
        count: Option<(ResourceId, u64)>,
    },
    /// `SetPredication`: the buffer, offset and `D3D12_PREDICATION_OP`, or none.
    Predication(Option<(ResourceId, u64, u32)>),
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
