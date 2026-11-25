//! The Direct3D 12 device on one back end: its objects, the one queue order every guest queue shares,
//! the replay of command lists, and fences.

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use slab::Slab;
use wcontainers_directx_graphics_infrastructure::stage_registers::OUTPUT_REGISTERS;
use wcontainers_directx_graphics_infrastructure::{SwapChain, SwapChainDesc, SwapChainError};
use wcontainers_gpu::{
    AspectOps, Backend, BufferDesc, BufferUsage, ClearValue, ColorAttachment, ColorTarget,
    CopyBufferTexture, CountedDraw, DepthAttachment, DeviceStatus, DispatchCall, DispatchGroups,
    DrawBaseSource, DrawCall, DrawRange, EmptyPass, Extent, Features, Format, FormatKind,
    FormatUse, GpuError, HELPER_SET, HelperValue, INDEX_FORMATS, IndexBinding, IndexFormat,
    Interpolation, LinkedValues, LoadOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, MapIntent, Object,
    OcclusionCounter, Origin, PassDesc, PassTargets, PassTracker, PrepareDesc, Program, ReadSource,
    ReadState, STORAGE_WORD, SamplerDesc, ScissorRect, ShaderModule, ShaderStage, SlowPath,
    StageSet, StageValue, StepMode, StoreOp, SystemValue, Target, TargetInShader, TargetLayout,
    TexelBufferRoute, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
    Topology, VariantFlags, VariantKey, VertexAttribute, VertexBinding, VertexLayoutDesc,
    VertexStream, ViewComponent, ViewDimension, Viewport, WriteMode,
};
use wcontainers_shaders::{ConvertError, Converted, Source, refusal};
use zerocopy::FromZeros;

use crate::Error;
use crate::address::AddressMap;
use crate::list::{
    Command, CopyLocation, DSV_READ_ONLY_DEPTH, DSV_READ_ONLY_STENCIL, Descriptor, Rect,
    TargetView, UAV_DIMENSION_TEXTURE1D, UAV_DIMENSION_TEXTURE1DARRAY, UAV_DIMENSION_TEXTURE2D,
    UAV_DIMENSION_TEXTURE2DARRAY, UAV_DIMENSION_TEXTURE3D, VertexBufferView, ViewportF,
};
use crate::resource::{self, HeapType, ResourceDesc};
use crate::root::{RootSignature, RootSignatureDesc, deserialize};
use crate::state::{
    Blend, DepthStencil, FILL_MODE_WIREFRAME, InputElement, Raster, TOPOLOGY_TYPE_LINE,
    TOPOLOGY_TYPE_PATCH, TOPOLOGY_TYPE_POINT, TOPOLOGY_TYPE_TRIANGLE, TOPOLOGY_TYPE_UNDEFINED,
    resolve_offsets, vertex_format,
};
use pull::{Helpers, PulledKey, VertexProgram};
use wcontainers_directx_graphics_infrastructure::dxgi::R8G8B8A8_TYPELESS;

macro_rules! id {
    ($($(#[$m:meta])* $name:ident),*) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(pub u32);
    )*};
}
id!(
    /// A committed resource.
    ResourceId,
    /// A pipeline state object.
    PipelineId,
    /// A fence.
    FenceId,
    /// A swap chain.
    ChainId,
    /// A descriptor heap.
    HeapId,
    /// A command queue.
    QueueId,
    /// A query heap.
    QueryHeapId,
    /// A heap made by `CreateHeap`, holding placed resources.
    MemoryId
);

mod alias;
mod bind;
mod early;
mod formats;
mod geometry;
mod indirect;
mod null;
mod pull;
mod query;
mod statistics;
mod through;

/// Where GPU virtual addresses start, far from zero so a null address is never a buffer.
const ADDRESS_BASE: u64 = 1 << 32;

/// Buffers in default heaps can be anything to the GPU.
const DEFAULT_BUFFER: BufferUsage = BufferUsage::VERTEX
    .union(BufferUsage::INDEX)
    .union(BufferUsage::UNIFORM)
    .union(BufferUsage::STORAGE)
    .union(BufferUsage::INDIRECT)
    .union(BufferUsage::COPY_SRC)
    .union(BufferUsage::COPY_DST);
/// Upload buffers are read by the GPU and written from guest memory.
const UPLOAD_BUFFER: BufferUsage = BufferUsage::VERTEX
    .union(BufferUsage::INDEX)
    .union(BufferUsage::UNIFORM)
    .union(BufferUsage::STORAGE)
    .union(BufferUsage::INDIRECT)
    .union(BufferUsage::COPY_SRC)
    .union(BufferUsage::COPY_DST);
/// Readback buffers are only written by copies and read back.
const READBACK_BUFFER: BufferUsage = BufferUsage::COPY_DST.union(BufferUsage::READ_BACK);
/// Buffers in a custom heap the CPU sees are both.
const SHARED_BUFFER: BufferUsage = UPLOAD_BUFFER.union(BufferUsage::READ_BACK);

/// Guest-visible memory of an upload, readback or shared heap, shared by the buffers placed in it.
/// Cells, because the program writes it through `Map`'s pointer while the device holds it.
type HostMemory = Rc<Box<[Cell<u8>]>>;

/// The bytes `Map` hands out for an upload, readback or shared buffer: its own memory, or its part
/// of its heap's, which overlapping placed buffers share as the memory they alias.
struct Host {
    memory: HostMemory,
    offset: usize,
    len: usize,
}

impl Host {
    fn new(len: usize) -> Self {
        Host {
            // Zeroed by the allocator, so pages the program never touches stay untouched.
            memory: Rc::new(
                <[Cell<u8>]>::new_box_zeroed_with_elems(len).expect("memory for a mapped heap"),
            ),
            offset: 0,
            len,
        }
    }

    /// The bytes; `offset` and `len` lie inside the memory, as `create_placed` checked.
    fn cells(&self) -> &[Cell<u8>] {
        &self.memory[self.offset..][..self.len]
    }
}

enum Storage<B: Backend> {
    Buffer {
        buffer: B::Buffer,
        /// The bytes `Map` hands out, for upload, readback and shared heaps.
        host: Option<Host>,
    },
    Texture {
        texture: B::Texture,
        /// The texture as the program sees it; a multisampled array's layers are textures of one
        /// layer each.
        desc: TextureDesc,
        /// A multisampled array's layers after the first, each a texture of its own (the
        /// interface's multisampled textures have one layer); empty otherwise.
        layers: Vec<B::Texture>,
        /// Views made for passes and shaders, by what they view.
        views: Vec<(TextureViewDesc, B::TextureView)>,
        /// False for a swap chain's back buffer, whose texture the swap chain owns.
        owned: bool,
    },
}

impl<B: Backend> Storage<B> {
    /// The back end's texture holding array layer `layer`, and the layer's index in it.
    fn layer(&self, layer: u32) -> Option<(&B::Texture, u32)> {
        match self {
            Storage::Texture {
                texture, layers, ..
            } if layers.is_empty() => Some((texture, layer)),
            Storage::Texture {
                texture, layers, ..
            } => match layer {
                0 => Some((texture, 0)),
                n => layers.get(n as usize - 1).map(|t| (t, 0)),
            },
            Storage::Buffer { .. } => None,
        }
    }
}

struct Resource<B: Backend> {
    desc: ResourceDesc,
    heap: HeapType,
    storage: Storage<B>,
    address: u64,
    /// The heap a placed resource lies in, and its offset there.
    placed: Option<(MemoryId, u64)>,
    /// The placed resources whose bytes this one inherits where they overlap (`alias.rs`).
    inherits: Vec<ResourceId>,
    /// The stamp of the last GPU write recorded into it, and the stamp when it was last written or
    /// brought up to date.
    written: u64,
    synced: u64,
    /// Textures holding this typeless texture's bits in formats the back end cannot view them as,
    /// which inherit from it and from each other (`alias.rs`, `ReinterpretCopy`).
    twins: Vec<ResourceId>,
    /// Whether this is such a twin.
    twin: bool,
    /// The texture holding the chroma plane of an `NV12` texture (`resource::has_chroma_plane`),
    /// whose views, copies and inheritance name it in place of this one.
    chroma: Option<ResourceId>,
}

/// A heap of `CreateHeap`. Each placed resource in a default heap has storage of its own (plan
/// 5.2), so the heap keeps where they lie: offset, end and resource. Upload and readback heaps
/// have guest memory their buffers share.
struct Memory {
    size: u64,
    heap: HeapType,
    placed: Vec<(u64, u64, ResourceId)>,
    host: Option<HostMemory>,
}

/// What a pipeline state object holds.
struct Pipeline<B: Backend> {
    program: B::Program,
    /// Graphics pipelines' fixed state; none for compute pipelines.
    render: Option<Render<B>>,
    /// What the program's stages bind, and the size each stage's uniform blocks declare.
    bindings: Vec<bind::StageBinding>,
    /// A compute pipeline's threads in one group; zero for graphics pipelines.
    group_threads: u64,
    /// The stage whose typed view formats come from the bound views, if one does.
    formats: Option<Box<formats::Formats<B>>>,
    /// The helper block of the pixel (or compute) stage of `program`.
    helpers: Helpers,
    /// The slow paths its stages' programs take by themselves (`Reflection::slow_paths`),
    /// counted at each draw or dispatch.
    slow_paths: Vec<SlowPath>,
}

/// A graphics pipeline's fixed state.
struct Render<B: Backend> {
    /// The root signature, which sizes arrays its programs declare without a length.
    root: RootSignature,
    /// Depth clamped by the vertex program alone, the pipeline having no depth-stencil format.
    clamped_without_target: bool,
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    /// For a pipeline made with no depth-stencil format: the block its draws take while a
    /// depth-stencil view is bound, which Direct3D 12 tests and writes with the pipeline's state.
    depth_stencil_bound: Option<B::DepthStencil>,
    raster: B::Raster,
    /// Each input element and the program location it feeds.
    inputs: Vec<(InputElement, u32)>,
    targets: TargetLayout,
    /// `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE`: non-zero restarts strips.
    strip_cut: u32,
    /// The vertex shader reads `SV_VertexID` or `SV_InstanceID`, which leave out the draw's bases.
    reads_bases: bool,
    /// The vertex program's helper block.
    helpers: Helpers,
    /// For a pipeline reading the bases: the program whose vertex stage reads them from an
    /// indirect draw's arguments. None where it did not convert; such draws read their arguments
    /// back.
    arguments: Option<VertexProgram<B>>,
    /// Programs fetching some vertex inputs themselves, by the inputs and formats fetched and
    /// whether the bases come from indirect arguments; None where it did not convert.
    pulled: Vec<(PulledKey, Option<VertexProgram<B>>)>,
    /// The vertex shader's bytecode, the interpolation its outputs take, and the pixel program,
    /// from which the programs above are made.
    vs_code: Vec<u8>,
    interpolation: Vec<(u32, Interpolation)>,
    fragment: Option<ShaderModule>,
    /// The formats the vertex shader's typed views are converted with, and its typed buffers'
    /// route (`formats::typed_key`).
    typed_key: VariantKey,
    /// A vertex stage drawn directly (no geometry or domain stage after it), whose program is
    /// converted again for what is bound to it; and the programs made so, by what they are made
    /// for (`formats::bound_program`), None where the conversion failed.
    vertex_formats: Option<Box<formats::Formats<B>>>,
    bound: Vec<(formats::Combination, Option<formats::BoundProgram<B>>)>,
    /// For counted occlusion queries and pixel programs made later: the vertex stage drawn (the
    /// vertex program, the domain program or the stage helper's pass-through), the pixel shader's
    /// bytecode, and the counting program made from them at the first counted draw.
    vertex: ShaderModule,
    pixel: Vec<u8>,
    counting: Option<B::Program>,
    /// For pipeline statistics: whether the pixel shader runs once a sample, and the program the
    /// pixel counter draws with, made at the first draw a statistics query counts.
    per_sample: bool,
    pixel_counting: Option<B::Program>,
    writes_depth_stencil: bool,
    /// Where the pixel shader forces early depth and stencil tests, which change what it does
    /// (storage writes, or a discard while depth or stencil is written), and the pipeline tests
    /// depth or stencil: the tests, which its draws while a depth-stencil view is bound run in the
    /// program (`EarlyTestsInShader`).
    early_tests: Option<DepthStencil>,
    /// Where those tests matter while the pipeline writes depth or stencil, what its draws then
    /// draw each primitive with; `None` behind a geometry stage and for a pixel shader reading its
    /// primitive index, which a primitive drawn alone would read as zero.
    per_primitive: Option<early::PerPrimitive<B>>,
    alpha_to_coverage: bool,
    /// What the last vertex stage passes the pixel program; default where it passes nothing.
    linked: Link,
    /// Every per-instance element steps by one instance.
    instances_step_by_one: bool,
    /// The geometry stage, for pipelines with a geometry shader.
    geometry: Option<geometry::Geometry<B>>,
    /// Where the values between its drawn vertex stage and pixel program move, which every
    /// program made for it later follows.
    moves: pull::StageMoves,
}

/// `D3D12_GRAPHICS_PIPELINE_STATE_DESC`, the parts this front reads.
#[derive(Clone, Debug, Default)]
pub struct GraphicsPipelineDesc<'a> {
    pub root: Option<RootSignature>,
    pub vs: &'a [u8],
    pub hs: &'a [u8],
    pub ds: &'a [u8],
    pub gs: &'a [u8],
    pub ps: &'a [u8],
    pub blend: Blend,
    pub sample_mask: u32,
    pub raster: Raster,
    pub depth_stencil: DepthStencil,
    pub inputs: Vec<InputElement>,
    pub strip_cut: u32,
    /// `D3D12_PRIMITIVE_TOPOLOGY_TYPE`.
    pub topology_type: u32,
    /// A pipeline state stream gave no topology type: the draws' topologies decide.
    pub topology_from_draws: bool,
    pub rtv_formats: Vec<u32>,
    pub dsv_format: u32,
    pub samples: u32,
    /// The stream output declaration and `RasterizedStream`.
    pub stream_output: Option<(wcontainers_shaders::stage::StreamOutput, u32)>,
}

struct Fence<B: Backend> {
    /// The value reached by work known to be done, or set by the CPU.
    value: u64,
    /// Values queue signals will reach, with the back-end fence of the work before each.
    pending: Vec<(u64, B::Fence)>,
}

/// A read back into readback-heap memory, finished when its submission is done.
struct PendingRead<B: Backend> {
    submission: Option<B::Fence>,
    readback: B::Readback,
    resource: ResourceId,
    offset: usize,
    size: usize,
}

/// Work a queue holds behind a `Wait` not yet met, each with its place in the order of the calls.
enum Work {
    Execute(Vec<Arc<[Command]>>),
    Signal(FenceId, u64),
    Wait(FenceId, u64),
}

/// A descriptor heap: the slots CPU handles point into, and the first GPU handle.
struct Heap {
    /// Cells, because the program writes them through its CPU handles while the device holds them.
    slots: Box<[Cell<Descriptor>]>,
    gpu: u64,
}

/// Where GPU descriptor handles start, far from GPU virtual addresses.
const HANDLE_BASE: u64 = 1 << 56;

/// The warning for a draw or dispatch recorded with no pipeline state set, which Direct3D 12
/// leaves undefined: it is skipped.
const NO_PIPELINE_SKIPPED: &str =
    "warning: a draw or dispatch recorded with no pipeline state set was skipped";

/// An event waiting for fences: the fences and values, whether any one reaching its value is
/// enough, and the shell's token for the event.
type Waiter = (Vec<(FenceId, u64)>, bool, u64);

/// The device: every object and the replay of command lists, on one back end owned by one thread.
pub struct Device<B: Backend> {
    backend: B,
    /// The bound targets as the occlusion counter reads them, and the counter.
    passes: PassTracker<B>,
    counter: OcclusionCounter<B>,
    /// What pipeline statistics queries' counting draws are drawn with.
    counting: Option<statistics::Counting<B>>,
    resources: Slab<Resource<B>>,
    pipelines: Slab<Pipeline<B>>,
    fences: Slab<Fence<B>>,
    addresses: AddressMap<ResourceId>,
    heaps: Slab<Heap>,
    handles: AddressMap<HeapId>,
    samplers: HashMap<SamplerDesc, B::Sampler>,
    /// Upload buffers already written to the GPU in this `ExecuteCommandLists`.
    uploaded: Vec<ResourceId>,
    /// The bytes of an upload or read back on their way between the GPU and mapped memory.
    staging: Vec<u8>,
    layouts: HashMap<VertexLayoutDesc, B::VertexLayout>,
    reads: Vec<PendingRead<B>>,
    /// Events to set when fences reach values.
    waiters: Vec<Waiter>,
    chains: Slab<Chain<B>>,
    /// Each queue's held work, in order, numbered by the call that made it.
    queues: Slab<VecDeque<(u64, Work)>>,
    calls: u64,
    query_heaps: Slab<query::QueryHeap<B>>,
    memory: Slab<Memory>,
    /// How many resources inherit bytes from others; while none do, uses are not noted.
    aliased: usize,
    /// The resources the command being replayed uses, and whether it writes each, while `aliased`.
    touched: Vec<(ResourceId, bool)>,
    /// The last write stamp given.
    stamp: u64,
    programs: query::Programs<B>,
    nulls: null::Nulls<B>,
    /// Messages not yet shown, such as why a shader did not convert.
    messages: Vec<String>,
    /// The typed views the last binding sets made bind: API stage, slot and format.
    bound_formats: Vec<(u32, formats::Slot, Format)>,
    /// The helper values the last binding sets made give: API stage, value and its words.
    bound_helpers: Vec<(u32, HelperValue, pull::Words)>,
    /// The draw being bound runs its pixel stage's forced early tests in the program
    /// (`early::`), so its pixel program is the one converted to.
    early_draw: bool,
    /// The samplers the last binding sets made bind whose sampling the program applies: API
    /// stage, register, and whether it adds the bias and applies the addressing
    /// (`state::in_program`).
    bound_sampling: Vec<(u32, u32, bool, bool)>,
    /// The storage textures the last binding sets made read through buffers: API stage and
    /// register; and the copies around their draw or dispatch.
    bound_through: Vec<(u32, u32)>,
    /// The API stages a storage view of their own slots is bound to at an offset the back end
    /// cannot bind (`VariantFlags::STORAGE_OFFSET`).
    bound_offsets: Vec<u32>,
    /// The textures the last binding sets made bind under a minimum level-of-detail clamp (API
    /// stage, register, clamp in the view's levels), and every sampler they bind (API stage,
    /// register, descriptor), from which a clamped program's merged samplers are made.
    bound_clamps: Vec<(u32, u32, f32)>,
    bound_samplers: Vec<(u32, u32, crate::list::SamplerView)>,
    /// The textures the last binding sets made bind in a format the back end cannot filter: API
    /// stage and register.
    bound_unfiltered: Vec<(u32, u32)>,
    /// The component mappings the last binding sets made bind that the program applies: API
    /// stage, register and mapping.
    bound_swizzles: Vec<(u32, u32, [ViewComponent; 4])>,
    /// The multisampled array views the last binding sets made bind: API stage, register, the
    /// back end's stages reading it, and the views of its layers after the first.
    bound_layers: Vec<(u32, u32, StageSet, Vec<B::TextureView>)>,
    through: Vec<through::Through<B>>,
    /// The error that removed the device, if one did.
    removed: Option<Error>,
}

/// A swap chain and the resources standing for its back buffers.
struct Chain<B: Backend> {
    chain: SwapChain<B>,
    buffers: Vec<ResourceId>,
    /// The token of the frame latency waitable object, and the retired frames it was released for.
    waitable: Option<(u64, u64)>,
}

impl<B: Backend> Device<B> {
    pub fn new(backend: B) -> Self {
        Device {
            backend,
            resources: Slab::new(),
            pipelines: Slab::new(),
            fences: Slab::new(),
            addresses: AddressMap::new(ADDRESS_BASE),
            heaps: Slab::new(),
            handles: AddressMap::new(HANDLE_BASE),
            samplers: HashMap::new(),
            uploaded: Vec::new(),
            staging: Vec::new(),
            layouts: HashMap::new(),
            reads: Vec::new(),
            waiters: Vec::new(),
            chains: Slab::new(),
            queues: Slab::new(),
            calls: 0,
            query_heaps: Slab::new(),
            memory: Slab::new(),
            aliased: 0,
            touched: Vec::new(),
            stamp: 0,
            programs: query::Programs::default(),
            passes: PassTracker::default(),
            counter: OcclusionCounter::default(),
            counting: None,
            nulls: null::Nulls::default(),
            messages: Vec::new(),
            bound_formats: Vec::new(),
            bound_helpers: Vec::new(),
            early_draw: false,
            bound_sampling: Vec::new(),
            bound_through: Vec::new(),
            bound_offsets: Vec::new(),
            bound_clamps: Vec::new(),
            bound_unfiltered: Vec::new(),
            bound_samplers: Vec::new(),
            bound_swizzles: Vec::new(),
            bound_layers: Vec::new(),
            through: Vec::new(),
            removed: None,
        }
    }

    pub fn backend(&mut self) -> &mut B {
        &mut self.backend
    }

    /// The error that removed the device, if one did (`GetDeviceRemovedReason`).
    pub fn removed(&mut self) -> Option<Error> {
        self.poll_lost();
        self.removed
    }

    /// Removes the device where the back end lost its own (an error it recorded, a reset).
    fn poll_lost(&mut self) {
        if self.removed.is_none()
            && let DeviceStatus::Lost(_) = self.backend.status()
        {
            self.removed = Some(Error::DeviceRemoved);
        }
    }

    /// Removes the device for `e`, as Direct3D 12 does for a call it cannot carry out; the first
    /// error stays the reason.
    pub fn remove(&mut self, e: Error) {
        self.removed.get_or_insert(e);
    }

    /// Warnings not yet shown: each slow path the first time it is taken, and why a call failed
    /// where the error code cannot say.
    pub fn take_warnings(&mut self) -> Vec<String> {
        let mut out: Vec<String> = self
            .backend
            .slow_paths()
            .take_warnings()
            .into_iter()
            .map(|p| p.reason().to_string())
            .collect();
        out.append(&mut self.messages);
        out
    }

    fn live(&self) -> Result<(), Error> {
        match self.removed {
            Some(_) => Err(Error::DeviceRemoved),
            None => Ok(()),
        }
    }

    // ---- Resources ----

    /// `CreateCommittedResource`.
    pub fn create_committed(
        &mut self,
        heap: HeapType,
        desc: &ResourceDesc,
    ) -> Result<ResourceId, Error> {
        self.create(heap, desc, None)
    }

    /// A resource with storage of its own; an upload or readback buffer maps `host`, or memory of
    /// its own without.
    fn create(
        &mut self,
        heap: HeapType,
        desc: &ResourceDesc,
        host: Option<Host>,
    ) -> Result<ResourceId, Error> {
        self.live()?;
        let id = ResourceId(self.resources.vacant_key() as u32);
        let (storage, address) = if desc.dimension == resource::DIMENSION_BUFFER {
            if desc.width == 0 {
                return Err(Error::Invalid("a buffer of zero bytes"));
            }
            if desc.width > self.backend.caps().limits.max_buffer_size {
                return Err(Error::OutOfMemory);
            }
            let usage = match heap {
                HeapType::Default => DEFAULT_BUFFER,
                HeapType::Upload => UPLOAD_BUFFER,
                HeapType::Readback => READBACK_BUFFER,
                HeapType::Shared => SHARED_BUFFER,
            };
            let buffer = self.backend.create_buffer(&BufferDesc {
                size: stored_size(desc.width, self.backend.caps().limits.max_buffer_size),
                usage,
                intent: if heap.uploads() {
                    MapIntent::Often
                } else {
                    MapIntent::Rarely
                },
            })?;
            let host = (heap != HeapType::Default)
                .then(|| host.unwrap_or_else(|| Host::new(desc.width as usize)));
            let address = self.addresses.insert(desc.width, id);
            (Storage::Buffer { buffer, host }, address)
        } else {
            if heap.cpu_only() {
                return Err(Error::Invalid("textures live in default or custom heaps"));
            }
            let (format, typeless) = desc.texture_format()?;
            let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
            if desc.flags & resource::FLAG_DENY_SHADER_RESOURCE == 0 {
                usage |= TextureUsage::SAMPLE;
            }
            // Multisampled textures, and single-sampled 2D ones a resolve may write, are made to
            // draw into whatever their flags say: the back end multisamples and resolves only into
            // textures drawn into, which Direct3D 12 does not require.
            let resolves_into = desc.dimension == resource::DIMENSION_TEXTURE2D
                && self
                    .backend
                    .caps()
                    .formats
                    .get(format)
                    .contains(FormatUse::RESOLVE);
            if desc.flags & (resource::FLAG_RENDER_TARGET | resource::FLAG_DEPTH_STENCIL) != 0
                || desc.sample_count > 1
                || resolves_into
            {
                usage |= TextureUsage::RENDER;
            }
            // Formats the back end cannot store into are read and written through a buffer of
            // their texels (`StorageFormatThroughBuffer`).
            let stores = self
                .backend
                .caps()
                .formats
                .get(format)
                .intersects(FormatUse::STORAGE_READ | FormatUse::STORAGE_WRITE);
            if desc.flags & resource::FLAG_UNORDERED_ACCESS != 0 {
                if stores {
                    usage |= TextureUsage::STORAGE;
                }
                // `ClearUnorderedAccessView*` clears it by a pass where the format renders.
                if self
                    .backend
                    .caps()
                    .formats
                    .get(format)
                    .contains(FormatUse::RENDER)
                {
                    usage |= TextureUsage::RENDER;
                }
            }
            let three_d = desc.dimension == resource::DIMENSION_TEXTURE3D;
            let tdesc = TextureDesc {
                dimension: if three_d {
                    TextureDimension::D3
                } else {
                    TextureDimension::D2
                },
                format,
                width: u32::try_from(desc.stored_width())
                    .map_err(|_| Error::Invalid("texture too wide"))?,
                height: desc.height.max(1),
                depth_or_layers: u32::from(desc.depth_or_array_size).max(1),
                // Direct3D 12 takes more levels than the chain has; the texture keeps the chain.
                mip_levels: desc.mips().min(desc.full_chain()),
                samples: desc.sample_count.max(1),
                usage,
                cube: false,
                reinterpret: typeless,
            };
            // The interface's multisampled textures have one layer (WebGPU's rule): a
            // multisampled array is a texture per layer.
            let (texture, layers) = if tdesc.samples > 1 && tdesc.depth_or_layers > 1 {
                let one = TextureDesc {
                    depth_or_layers: 1,
                    ..tdesc
                };
                let mut all = Vec::with_capacity(tdesc.depth_or_layers as usize);
                for _ in 0..tdesc.depth_or_layers {
                    match self.backend.create_texture(&one) {
                        Ok(t) => all.push(t),
                        Err(e) => {
                            for t in all {
                                self.backend.destroy(Object::Texture(t));
                            }
                            return Err(e.into());
                        }
                    }
                }
                let first = all.remove(0);
                (first, all)
            } else {
                (self.backend.create_texture(&tdesc)?, Vec::new())
            };
            (
                Storage::Texture {
                    texture,
                    desc: tdesc,
                    layers,
                    views: Vec::new(),
                    owned: true,
                },
                0,
            )
        };
        self.resources.insert(Resource {
            desc: *desc,
            heap,
            storage,
            address,
            placed: None,
            inherits: Vec::new(),
            written: 0,
            synced: 0,
            twins: Vec::new(),
            twin: false,
            chroma: None,
        });
        if let Some(chroma) = desc.chroma() {
            match self.create(heap, &chroma, None) {
                Ok(c) => self.resources[id.0 as usize].chroma = Some(c),
                Err(e) => {
                    self.destroy_resource(id);
                    return Err(e);
                }
            }
        }
        Ok(id)
    }

    /// `CreateHeap`.
    pub fn create_heap(
        &mut self,
        size: u64,
        alignment: u64,
        heap: HeapType,
        flags: u32,
    ) -> Result<MemoryId, Error> {
        self.live()?;
        resource::check_heap(alignment, heap, flags)?;
        if size == 0 {
            return Err(Error::Invalid("a heap of zero bytes"));
        }
        let host = (heap != HeapType::Default)
            .then(|| Host::new(usize::try_from(size).unwrap_or(usize::MAX)).memory);
        Ok(MemoryId(self.memory.insert(Memory {
            size,
            heap,
            placed: Vec::new(),
            host,
        }) as u32))
    }

    /// The heap's release; resources placed in it keep their own storage.
    pub fn destroy_heap(&mut self, id: MemoryId) {
        self.memory.try_remove(id.0 as usize);
    }

    /// `CreatePlacedResource`. A placed resource in a default heap has storage of its own and
    /// inherits the bytes of the live ones it overlaps by copies (`alias.rs`); a buffer in an upload
    /// or readback heap maps its part of the heap's memory.
    pub fn create_placed(
        &mut self,
        memory: MemoryId,
        offset: u64,
        desc: &ResourceDesc,
    ) -> Result<ResourceId, Error> {
        self.live()?;
        let (padded, align) = desc.allocation()?;
        // A buffer takes its own bytes of the heap; a texture its whole allocation.
        let size = if desc.dimension == resource::DIMENSION_BUFFER {
            desc.width
        } else {
            padded
        };
        let m = self
            .memory
            .get(memory.0 as usize)
            .ok_or(Error::Invalid("no such heap"))?;
        if !offset.is_multiple_of(align) || offset.checked_add(size).is_none_or(|e| e > m.size) {
            return Err(Error::Invalid(
                "a placed resource outside its heap or misaligned",
            ));
        }
        let end = offset + size;
        let heap = m.heap;
        let host = m.host.clone().map(|memory| Host {
            memory,
            offset: offset as usize,
            len: desc.width as usize,
        });
        let overlaps: Vec<ResourceId> = m
            .placed
            .iter()
            .filter(|&&(o, e, _)| o < end && offset < e)
            .map(|p| p.2)
            .collect();
        let id = self.create(heap, desc, host)?;
        self.resources[id.0 as usize].placed = Some((memory, offset));
        self.memory[memory.0 as usize]
            .placed
            .push((offset, end, id));
        // Buffers in heaps the CPU sees share its memory; everything else inherits by copies.
        if heap == HeapType::Default || desc.dimension != resource::DIMENSION_BUFFER {
            self.inherit(id, &overlaps);
        }
        Ok(id)
    }

    /// The texture and subresource index holding subresource `index` of texture `id`: an `NV12`
    /// texture's chroma plane is a texture of its own.
    fn plane_holder(&self, id: ResourceId, index: u32) -> (ResourceId, u32) {
        match self.resources.get(id.0 as usize) {
            Some(Resource {
                chroma: Some(chroma),
                desc,
                ..
            }) if index >= desc.mips() * desc.layers() => {
                (*chroma, index - desc.mips() * desc.layers())
            }
            _ => (id, index),
        }
    }

    /// A box at `origin` of `extent` pixels of texture `id` in its stored texels (`YUY2`'s pixel
    /// pairs are one texel each).
    fn in_texels(&self, id: ResourceId, origin: Origin, extent: Extent) -> (Origin, Extent) {
        let n = self
            .resources
            .get(id.0 as usize)
            .map_or(1, |r| resource::pixels_per_texel(r.desc.format));
        (
            Origin {
                x: origin.x / n,
                ..origin
            },
            Extent {
                width: extent.width.div_ceil(n),
                ..extent
            },
        )
    }

    /// Copy location `l` naming the texture holding its subresource (`plane_holder`), and the
    /// pixels each of its texels holds (`in_texels`); a `YUY2` footprint's pixel pairs are
    /// `R8G8B8A8` texels.
    fn texel_location(&self, l: &CopyLocation) -> (CopyLocation, u32) {
        match *l {
            CopyLocation::Subresource { resource, index } => {
                let (resource, index) = self.plane_holder(resource, index);
                let n = self
                    .resources
                    .get(resource.0 as usize)
                    .map_or(1, |r| resource::pixels_per_texel(r.desc.format));
                (CopyLocation::Subresource { resource, index }, n)
            }
            CopyLocation::Footprint {
                resource,
                footprint,
            } => {
                let n = resource::pixels_per_texel(footprint.format);
                let footprint = match n {
                    1 => footprint,
                    _ => resource::Footprint {
                        format: R8G8B8A8_TYPELESS,
                        width: footprint.width.div_ceil(n),
                        ..footprint
                    },
                };
                (
                    CopyLocation::Footprint {
                        resource,
                        footprint,
                    },
                    n,
                )
            }
        }
    }

    /// The region of subresource `sub` of texture `id` at `origin`, for slice `z` of a box.
    fn subresource_region(
        &self,
        id: ResourceId,
        sub: u32,
        origin: Origin,
        z: u32,
    ) -> Result<TextureRegion<'_, B>, Error> {
        let (_, desc) = self.texture(id)?;
        let mips = desc.mip_levels;
        let three_d = desc.dimension == TextureDimension::D3;
        let (texture, layer) = self.texture_layer(id, if three_d { 0 } else { sub / mips })?;
        Ok(TextureRegion {
            texture,
            mip: sub % mips,
            layer,
            origin: Origin {
                z: origin.z + z,
                ..origin
            },
            stencil: false,
        })
    }

    /// `WriteToSubresource`: the box at `origin` of `extent` texels of subresource `sub`, from
    /// `data` with rows and slices the pitches' bytes apart.
    pub fn write_subresource(
        &mut self,
        id: ResourceId,
        sub: u32,
        origin: Origin,
        extent: Extent,
        data: &[u8],
        (row_pitch, depth_pitch): (u32, u32),
    ) -> Result<(), Error> {
        self.live()?;
        let (id, sub) = self.plane_holder(id, sub);
        let (origin, extent) = self.in_texels(id, origin, extent);
        let rows = extent.height.div_ceil(self.texture(id)?.1.format.block().1);
        for z in 0..extent.depth_or_layers {
            let start = (z * depth_pitch) as usize;
            let region = self.subresource_region(id, sub, origin, z)?;
            let (texture, region) = (region.texture.clone(), region);
            let region = TextureRegion {
                texture: &texture,
                ..region
            };
            self.backend.write_texture(
                &region,
                Extent {
                    depth_or_layers: 1,
                    ..extent
                },
                &data[start..],
                row_pitch,
                rows,
                WriteMode::Ordered,
            );
        }
        Ok(())
    }

    /// `ReadFromSubresource`: the box of subresource `sub` into `data`, laid out as for
    /// `write_subresource`, once the work recorded so far is done (`ReadBackWait`).
    pub fn read_subresource(
        &mut self,
        id: ResourceId,
        sub: u32,
        origin: Origin,
        extent: Extent,
        data: &mut [u8],
        (row_pitch, depth_pitch): (u32, u32),
    ) -> Result<(), Error> {
        self.live()?;
        let (id, sub) = self.plane_holder(id, sub);
        let (origin, extent) = self.in_texels(id, origin, extent);
        let (bw, bh, bytes) = self.texture(id)?.1.format.block();
        let (rows, row) = (
            extent.height.div_ceil(bh) as usize,
            (extent.width.div_ceil(bw) * bytes) as usize,
        );
        let mut tight = vec![0; rows * row];
        for z in 0..extent.depth_or_layers {
            let region = self.subresource_region(id, sub, origin, z)?;
            let (texture, region) = (region.texture.clone(), region);
            self.backend.read_back_now(
                &ReadSource::Texture {
                    region: TextureRegion {
                        texture: &texture,
                        ..region
                    },
                    extent: Extent {
                        depth_or_layers: 1,
                        ..extent
                    },
                },
                &mut tight,
            )?;
            for (r, src) in tight.chunks(row).enumerate() {
                let at = (z * depth_pitch) as usize + r * row_pitch as usize;
                data[at..at + row].copy_from_slice(src);
            }
        }
        Ok(())
    }

    /// `GetGPUVirtualAddress`: zero for textures.
    pub fn address(&self, id: ResourceId) -> u64 {
        self.resources.get(id.0 as usize).map_or(0, |r| r.address)
    }

    pub fn resource_desc(&self, id: ResourceId) -> Option<ResourceDesc> {
        self.resources.get(id.0 as usize).map(|r| r.desc)
    }

    /// `Map` of an upload or readback buffer: the bytes the program reads and writes, which stay
    /// where they are for the resource's life. Before handing out readback memory, read backs whose
    /// work is done are finished into it.
    pub fn map(&mut self, id: ResourceId) -> Result<&[Cell<u8>], Error> {
        self.finish_reads(false);
        match self
            .resources
            .get_mut(id.0 as usize)
            .map(|r| &mut r.storage)
        {
            Some(Storage::Buffer {
                host: Some(host), ..
            }) => Ok(host.cells()),
            Some(_) => Err(Error::Invalid("only upload and readback buffers map")),
            None => Err(Error::Invalid("no such resource")),
        }
    }

    /// The resource's release: the back end frees it when work using it is done.
    pub fn destroy_resource(&mut self, id: ResourceId) {
        let Some(r) = self.resources.try_remove(id.0 as usize) else {
            return;
        };
        if r.address != 0 {
            self.addresses.remove(r.address);
        }
        if let Some(m) = r
            .placed
            .and_then(|(m, _)| self.memory.get_mut(m.0 as usize))
        {
            m.placed.retain(|p| p.2 != id);
        }
        self.disinherit(id, &r.inherits);
        for t in r.twins.into_iter().chain(r.chroma) {
            self.destroy_resource(t);
        }
        self.reads.retain(|p| p.resource != id);
        match r.storage {
            Storage::Buffer { buffer, .. } => self.backend.destroy(Object::Buffer(buffer)),
            Storage::Texture {
                texture,
                layers,
                views,
                owned,
                ..
            } => {
                for (_, v) in views {
                    self.backend.destroy(Object::TextureView(v));
                }
                if owned {
                    for t in std::iter::once(texture).chain(layers) {
                        self.backend.destroy(Object::Texture(t));
                    }
                }
            }
        }
    }

    // ---- Descriptor heaps ----

    /// `CreateDescriptorHeap`: `count` empty slots, which the program writes through the returned
    /// pointer (its CPU handles) and names in tables by the returned GPU handle. The slots stay where
    /// they are until the heap is destroyed.
    pub fn create_descriptor_heap(&mut self, count: u32) -> (HeapId, *mut Descriptor, u64) {
        let count = count.max(1);
        let id = HeapId(self.heaps.vacant_key() as u32);
        let gpu = self
            .handles
            .insert(u64::from(count) * crate::address::DESCRIPTOR_SIZE, id);
        let slots: Box<[Cell<Descriptor>]> = (0..count).map(|_| Cell::default()).collect();
        let cpu = slots.as_ptr().cast::<Descriptor>().cast_mut();
        self.heaps.insert(Heap { slots, gpu });
        (id, cpu, gpu)
    }

    /// The slots of descriptor heap `id`, which a caller in Rust writes in place of the pointer
    /// `create_descriptor_heap` returns.
    pub fn descriptor_slots(&self, id: HeapId) -> Option<&[Cell<Descriptor>]> {
        self.heaps.get(id.0 as usize).map(|h| &*h.slots)
    }

    pub fn destroy_descriptor_heap(&mut self, id: HeapId) {
        if let Some(h) = self.heaps.try_remove(id.0 as usize) {
            self.handles.remove(h.gpu);
        }
    }

    /// The descriptor a GPU handle names, as the heap holds it now.
    fn descriptor_at(&self, handle: u64) -> Result<Descriptor, Error> {
        let (heap, offset) = self
            .handles
            .find(handle)
            .ok_or(Error::Invalid("a GPU descriptor handle of no heap"))?;
        let heap = &self.heaps[heap.0 as usize];
        let index = (offset / crate::address::DESCRIPTOR_SIZE) as usize;
        Ok(heap.slots[index].get())
    }

    // ---- Root signatures and pipelines ----

    /// `CreateRootSignature` from a serialized blob. Pipelines and recorded lists hold the
    /// signature they name, so it outlives the program's last reference while they need it.
    pub fn create_root_signature(&mut self, blob: &[u8]) -> Result<RootSignature, Error> {
        self.live()?;
        let desc = deserialize(blob).map_err(Error::Invalid)?;
        desc.validate().map_err(Error::Invalid)?;
        Ok(Arc::new(desc))
    }

    /// `CreateGraphicsPipelineState`: the blocks and program are made now and the combination is
    /// prepared with vertex streams packed as the input layout lays them out, the common case, so
    /// the back end builds it before the first draw.
    pub fn create_graphics_pipeline(
        &mut self,
        desc: &GraphicsPipelineDesc<'_>,
    ) -> Result<PipelineId, Error> {
        self.live()?;
        let root = self.pipeline_root(
            desc.root.as_ref(),
            &[desc.vs, desc.hs, desc.ds, desc.gs, desc.ps],
        )?;
        let tessellated = !desc.hs.is_empty();
        // Hull and domain shaders come together, with patches.
        if tessellated != !desc.ds.is_empty()
            || tessellated != (desc.topology_type == TOPOLOGY_TYPE_PATCH)
        {
            return Err(Error::Invalid(
                "hull and domain shaders without each other or without patches",
            ));
        }
        // A vertex stage feeding another stage, not the pixel stage.
        let has_geometry = !desc.gs.is_empty() || tessellated;
        // `GetRenderTargetSampleCount` reads the targets' sample count.
        let pixel_samples = desc.samples.clamp(1, 32) as u8;
        let mut vs_typed = formats::typed_key(desc.vs, self.texel_route(), &root, bind::VERTEX).0;
        let ps = if desc.ps.is_empty() {
            None
        } else {
            let (typed, through) =
                formats::typed_key(desc.ps, self.texel_route(), &root, bind::PIXEL);
            // Dual-source blending reads the second output as the blend's second source.
            let mut key = VariantKey {
                flags: if desc.blend.dual_source() {
                    VariantFlags::DUAL_SOURCE
                } else {
                    VariantFlags::empty()
                },
                samples: pixel_samples,
                ..typed
            };
            let converted =
                self.convert_stage(desc.ps, ShaderStage::Fragment, &mut key, through)?;
            Some((converted, key))
        };
        let (mut ps, mut ps_key) = match ps {
            Some((c, k)) => (Some(c), Some(k)),
            None => (None, None),
        };
        // Alpha-antialiased lines (multisampling off) are drawn as the stage helper's covered
        // rectangles, the pixel program multiplying target 0's alpha by the coverage
        // (`AntialiasedLinesAsQuads`).
        let antialiased = desc.raster.antialiased_line && !desc.raster.multisample;
        let antialiased_lines = antialiased
            && desc.topology_type == TOPOLOGY_TYPE_LINE
            && !has_geometry
            && desc.stream_output.is_none();
        // WebGPU and GLSL ES require the stages to agree on interpolation, which in Direct3D the
        // pixel shader alone chooses: the vertex shader writes its outputs so.
        let mut interpolation: Vec<(u32, Interpolation)> = ps
            .iter()
            .flat_map(|p| &p.reflection.inputs)
            .filter(|i| i.system.is_none() && i.interpolation != Interpolation::Perspective)
            .map(|i| (i.location, i.interpolation))
            .collect();
        interpolation.sort_by_key(|i| i.0);
        interpolation.dedup_by_key(|i| i.0);
        // With a geometry stage, the vertex stage's outputs go to it, not to the pixel stage.
        let mut vs_key = VariantKey {
            output_interpolation: match has_geometry {
                true => Vec::new(),
                false => interpolation.clone(),
            },
            // `SV_VertexID` and `SV_InstanceID` leave out the draw's bases, which the program
            // then reads from its helper block.
            draw_base: DrawBaseSource::Helpers,
            ..vs_typed.clone()
        };
        let mut vs = self.convert(desc.vs, ShaderStage::Vertex, vs_key.clone())?;
        // Cull distances, which the back ends lack, are drawn through the stage helper.
        let culled = !has_geometry && geometry::writes_cull_distances(&vs);
        // The vertex stage alone runs as compute and the stage helper's pass-through draws.
        let vertex_alone = culled || antialiased_lines;
        let lines_key = match antialiased_lines {
            true => {
                let (Some(p), Some(key)) = (&mut ps, &mut ps_key) else {
                    return Err(Error::NotImplemented(
                        "alpha-antialiased lines without a pixel shader",
                    ));
                };
                key.flags |= VariantFlags::LINE_COVERAGE;
                key.linked = LinkedValues {
                    location: linked_location(
                        vs.reflection.outputs.iter().chain(&p.reflection.inputs),
                    ),
                    clip_distances: 0,
                };
                *p = self.convert(desc.ps, ShaderStage::Fragment, key.clone())?;
                Some(key.clone())
            }
            false => None,
        };
        let staged = has_geometry || desc.stream_output.is_some() || vertex_alone;
        // Clip distances the back end lacks: the vertex program passes them past both programs'
        // values and the pixel program applies them (`ClipDistanceInShader`). A program the
        // stage helper makes is checked where it is drawn, below.
        let clip_distances = clip_distances(&vs.reflection.outputs);
        let mut slow_paths = Vec::new();
        let drawn_directly = !has_geometry && !vertex_alone;
        // A pixel shader reading its clip distances reads them as values the vertex program
        // passes.
        let pixel_reads_clip = ps
            .iter()
            .flat_map(|p| &p.reflection.inputs)
            .any(|i| i.system == Some(SystemValue::ClipDistance));
        let clipped_in_shader = clip_distances != 0
            && drawn_directly
            && (!self
                .backend
                .caps()
                .features
                .contains(Features::CLIP_DISTANCE)
                || pixel_reads_clip);
        // Depth clamping the back end lacks (`DepthClampInShader`): the vertex program keeps z
        // inside the clip volume; with a depth target it also passes z past both programs' values
        // and the pixel program writes it clamped to the viewport's depth range.
        let clamped_in_shader = !desc.raster.depth_clip
            && !self.backend.caps().features.contains(Features::DEPTH_CLAMP);
        let clamped_in_pixels = clamped_in_shader && desc.dsv_format != 0;
        // Behind the stage helper the last vertex stage clamps: a geometry shader's pass-through,
        // the domain program, or the vertex program the cull route runs.
        let clamped_in_stages = clamped_in_shader && !drawn_directly;
        if clamped_in_stages && antialiased_lines {
            return Err(Error::NotImplemented(CLAMP_ANTIALIASED_LINES));
        }
        if clamped_in_stages && desc.stream_output.is_some() && desc.gs.is_empty() {
            return Err(Error::NotImplemented(CLAMP_STREAMED));
        }
        if clamped_in_shader {
            slow_paths.push(SlowPath::DepthClampInShader);
        }
        if drawn_directly && (clipped_in_shader || clamped_in_shader) {
            let mut flags = VariantFlags::empty();
            if clipped_in_shader {
                flags |= VariantFlags::CLIP_DISTANCE;
                slow_paths.push(SlowPath::ClipDistanceInShader);
            }
            let pixel_flags = match clamped_in_pixels {
                true => flags | VariantFlags::DEPTH_CLAMP,
                false => flags,
            };
            let pixel_reads = clipped_in_shader || clamped_in_pixels;
            let pixel_inputs = ps.iter().flat_map(|p| &p.reflection.inputs);
            let linked = LinkedValues {
                location: linked_location(vs.reflection.outputs.iter().chain(pixel_inputs)),
                clip_distances: if clipped_in_shader { clip_distances } else { 0 },
            };
            if pixel_reads {
                let link = Link {
                    flags: pixel_flags,
                    values: linked,
                };
                self.link_pixel_program(desc, pixel_samples, &mut ps, &mut ps_key, link)?;
            }
            if clamped_in_shader {
                flags |= VariantFlags::DEPTH_CLAMP;
            }
            for k in [&mut vs_typed, &mut vs_key] {
                k.flags |= flags;
                k.linked = linked;
            }
            vs = self.convert(desc.vs, ShaderStage::Vertex, vs_key.clone())?;
        }
        let vertex_formats = match staged {
            true => None,
            false => {
                formats::Formats::of(bind::VERTEX, desc.vs, &vs_key, &vs.reflection.sampled_pairs)
            }
        };
        let mut inputs = desc.inputs.clone();
        resolve_offsets(&mut inputs)?;
        let reads = &vs.reflection.inputs;
        for i in reads.iter().filter(|i| i.system.is_none()) {
            if !inputs.iter().any(|e| {
                e.semantic.eq_ignore_ascii_case(&i.semantic) && e.semantic_index == i.semantic_index
            }) {
                return Err(Error::Invalid(
                    "the input layout lacks an input the vertex shader reads",
                ));
            }
        }
        // Elements the vertex shader does not read are allowed and fetch nothing.
        let inputs: Vec<(InputElement, u32)> = inputs
            .into_iter()
            .filter_map(|e| {
                reads
                    .iter()
                    .find(|i| {
                        i.system.is_none()
                            && i.semantic.eq_ignore_ascii_case(&e.semantic)
                            && i.semantic_index == e.semantic_index
                    })
                    .map(|i| (e, i.location))
            })
            .collect();
        let mut made = match staged {
            true => Some(self.geometry_stage(
                desc.vs,
                desc.hs,
                desc.ds,
                desc.gs,
                &inputs,
                &interpolation,
                desc.stream_output.as_ref(),
                ps.as_ref().map(|p| p.reflection.inputs.as_slice()),
                lines_key.as_ref(),
                clamped_in_stages,
                &root,
            )?),
            false => None,
        };
        // Without a depth target the last vertex stage only keeps z inside the clip volume; the
        // pixel program writes no depth.
        if let Some(m) = &mut made
            && !clamped_in_pixels
            && m.linked.flags.contains(VariantFlags::DEPTH_CLAMP)
        {
            if m.linked.flags.contains(VariantFlags::LAYER_FROM_STAGE) {
                return Err(Error::NotImplemented(CLAMP_NAMED_LAYER_WITHOUT_DEPTH));
            }
            m.linked.flags.remove(VariantFlags::DEPTH_CLAMP);
            if m.linked.flags.is_empty() {
                m.linked = Link::default();
            }
        }
        // What the last vertex stage behind the stage helper passes the pixel program: clip
        // distances the back end lacks, the unclamped depth, and the layer a geometry shader names.
        if let Some(m) = &made
            && m.linked != Link::default()
        {
            let link = m.linked;
            self.link_pixel_program(desc, pixel_samples, &mut ps, &mut ps_key, link)?;
            if link.flags.contains(VariantFlags::CLIP_DISTANCE) {
                slow_paths.push(SlowPath::ClipDistanceInShader);
            }
        }
        let reads_bases = !has_geometry
            && vs
                .reflection
                .helpers
                .iter()
                .any(|h| h.value == HelperValue::DrawBase);
        let instances_step_by_one = inputs
            .iter()
            .all(|(e, _)| !e.per_instance || e.step_rate == 1);
        let mut colors = [None; MAX_COLOR_TARGETS];
        // A slot of format UNKNOWN has no target: its writes are dropped.
        for (slot, f) in colors.iter_mut().zip(&desc.rtv_formats) {
            *slot = (*f != 0).then(|| dxgi(*f)).transpose()?;
        }
        let targets = TargetLayout {
            colors,
            depth_stencil: (desc.dsv_format != 0)
                .then(|| dxgi(desc.dsv_format))
                .transpose()?,
            samples: desc.samples.max(1),
        };
        let mut bindings = match &made {
            Some(m) => {
                // Without a geometry stage the draw after the stream output binds the vertex
                // stage's sets too; after culling, the pass-through draws.
                let stages = match m.shader.is_some() || vertex_alone {
                    true => StageSet::COMPUTE,
                    false => StageSet::COMPUTE | StageSet::VERTEX,
                };
                let mut b = bind::stage_bindings(&m.vertex, bind::VERTEX, stages);
                if let Some(shader) = &m.shader {
                    b.extend(bind::stage_bindings(
                        shader,
                        m.geometry.middle.0,
                        StageSet::COMPUTE,
                    ));
                }
                if let Some(domain) = &m.domain {
                    // A domain stage run as compute; drawn too unless a geometry stage follows.
                    let stages = match (&m.geometry.after_domain, &m.late) {
                        (None, _) => StageSet::VERTEX,
                        (Some(_), Some(_)) => StageSet::COMPUTE,
                        (Some(_), None) => StageSet::COMPUTE | StageSet::VERTEX,
                    };
                    b.extend(bind::stage_bindings(domain, geometry::DOMAIN, stages));
                }
                if let Some(late) = &m.late {
                    b.extend(bind::stage_bindings(
                        late,
                        bind::GEOMETRY,
                        StageSet::COMPUTE,
                    ));
                }
                b
            }
            None => bind::stage_bindings(&vs, bind::VERTEX, StageSet::VERTEX),
        };
        if let Some(ps) = &ps {
            bindings.extend(bind::stage_bindings(ps, bind::PIXEL, StageSet::FRAGMENT));
        }
        bind::check(&root, &bindings)?;
        let early_tests =
            early_tests_matter(ps.as_ref(), &desc.depth_stencil).then_some(desc.depth_stencil);
        let mut nulls = bind::null_shapes(&bindings);
        if early_tests.is_some() {
            nulls.push((early::stencil_stand_in(targets.samples), false));
        }
        self.prepare_nulls(&nulls)?;
        let mut stages = vec![&vs];
        if let Some(m) = &made {
            stages.extend([&m.vertex]);
            stages.extend(m.shader.iter().chain(&m.domain).chain(&m.late));
        }
        stages.extend(&ps);
        slow_paths.extend(programs_slow_paths(&stages));
        let caps = self.backend.caps();
        let blend = desc.blend.block(caps, desc.rtv_formats.len())?;
        // Direct3D 12 refuses a description of topology type `UNDEFINED` (the own case
        // `undefined-topology`) and takes a pipeline state stream without a topology type
        // (vkd3d's test_create_pipeline_state): then the draws' topologies decide, and nothing
        // is built ahead. A geometry stage draws the primitives it makes.
        let topology = match desc.topology_type {
            TOPOLOGY_TYPE_UNDEFINED if desc.topology_from_draws => None,
            TOPOLOGY_TYPE_UNDEFINED => {
                return Err(Error::Invalid(
                    "a pipeline description with no topology type",
                ));
            }
            TOPOLOGY_TYPE_POINT => Some(Topology::PointList),
            TOPOLOGY_TYPE_LINE => Some(Topology::LineList),
            TOPOLOGY_TYPE_TRIANGLE => Some(Topology::TriangleList),
            // The domain stage's output, below.
            TOPOLOGY_TYPE_PATCH => None,
            _ => return Err(Error::Invalid("unknown primitive topology type")),
        };
        let topology = made.as_ref().and_then(|m| m.output).or(topology);
        let mut raster = desc.raster.block(
            caps,
            desc.sample_mask,
            targets.samples,
            match antialiased_lines {
                true => Some(false),
                false => {
                    topology.map(|t| matches!(t, Topology::TriangleList | Topology::TriangleStrip))
                }
            },
        )?;
        // Lines are not culled, and their rectangles are filled.
        if antialiased_lines {
            raster.cull = wcontainers_gpu::CullMode::None;
            raster.fill = wcontainers_gpu::FillMode::Solid;
        }
        let depth_stencil = desc.depth_stencil.block(targets.depth_stencil.is_some())?;
        let depth_stencil_bound = match targets.depth_stencil {
            None => Some(desc.depth_stencil.block(true)?),
            Some(_) => None,
        };
        // Other alpha-antialiased lines the stage helper's line route does not take.
        let draws_lines = !matches!(
            topology,
            Some(Topology::TriangleList | Topology::TriangleStrip | Topology::PointList)
        ) || desc.raster.fill == FILL_MODE_WIREFRAME;
        if draws_lines && antialiased && !antialiased_lines {
            return Err(Error::NotImplemented(
                "alpha-antialiased lines after a geometry or tessellation stage or stream output, \
                 or of wireframe triangles",
            ));
        }
        // Direct3D 12 writes the bits of an output of another type than its target, which the
        // back ends' pipelines do not take: the program writes them as the target's type. After
        // the description's own checks, which Direct3D 12 makes first.
        if let (Some(p), Some(key)) = (&mut ps, &mut ps_key) {
            let retyped = retyped_targets(p, &colors);
            if !retyped.is_empty() {
                key.targets_in_shader = retyped;
                *p = self.convert(desc.ps, ShaderStage::Fragment, key.clone())?;
            }
        }
        // The vertex stage drawn when not the vertex program: the domain program, or the stage
        // helper's pass-through.
        let staged_last = match made.as_ref().filter(|m| m.shader.is_some() || vertex_alone) {
            Some(geometry::Made {
                domain: Some(domain),
                late: None,
                ..
            }) => Some(&domain.modules[0]),
            Some(m) => Some(
                m.geometry
                    .stages
                    .as_ref()
                    .and_then(wcontainers_shaders::stage::Stages::passthrough)
                    .expect("a geometry stage's pass-through"),
            ),
            None => None,
        };
        let moves = self.stage_moves(
            staged_last.unwrap_or(&vs.modules[0]),
            ps.as_ref().map(|p| &p.modules[0]),
        )?;
        let staged_last = staged_last
            .map(|m| pull::moved_module(m.clone(), &moves.outputs))
            .transpose()?;
        if staged_last.is_none() {
            vs = pull::moved(vs, &moves.outputs)?;
        }
        let ps = ps.map(|p| pull::moved(p, &moves.inputs)).transpose()?;
        let vs_shader = staged_last.as_ref().unwrap_or(&vs.modules[0]);
        if wcontainers_shaders::capabilities(&vs_shader.module)
            .contains(naga::valid::Capabilities::CLIP_DISTANCES)
            && !self
                .backend
                .caps()
                .features
                .contains(Features::CLIP_DISTANCE)
        {
            return Err(Error::NotImplemented(
                "clip distances from a geometry, domain, culling or antialiased line stage without \
                 the back end's (ClipDistanceInShader)",
            ));
        }
        if ps.as_ref().is_some_and(|p| {
            wcontainers_shaders::capabilities(&p.modules[0].module)
                .contains(naga::valid::Capabilities::PRIMITIVE_INDEX)
        }) && !self
            .backend
            .caps()
            .features
            .contains(Features::PRIMITIVE_INDEX)
        {
            return Err(Error::NotImplemented(
                "the primitive index without the back end's (PrimitiveIdAsGeometry)",
            ));
        }
        let vs_drawn = vs_shader.clone();
        let vs_shader = self.backend.create_shader(vs_shader)?;
        let ps_shader = match &ps {
            Some(ps) => match self.backend.create_shader(&ps.modules[0]) {
                Ok(s) => Some(s),
                Err(e) => {
                    self.backend.destroy(Object::Shader(vs_shader));
                    return Err(e.into());
                }
            },
            None => None,
        };
        let program = self.backend.create_program(&Program::Render {
            vertex: &vs_shader,
            fragment: ps_shader.as_ref(),
        });
        self.backend.destroy(Object::Shader(vs_shader));
        if let Some(s) = ps_shader {
            self.backend.destroy(Object::Shader(s));
        }
        let program = program?;
        // Indirect draws of such a program read the bases from their arguments on the GPU.
        let fragment = ps.as_ref().map(|p| p.modules[0].clone());
        let arguments = if reads_bases && self.backend.caps().features.contains(Features::INDIRECT)
        {
            let key = VariantKey {
                output_interpolation: interpolation.clone(),
                draw_base: DrawBaseSource::Arguments,
                ..vs_typed.clone()
            };
            self.vertex_program(desc.vs, key, fragment.as_ref(), &moves.outputs)
                .ok()
                .filter(|p| {
                    p.helpers
                        .slots
                        .iter()
                        .any(|h| h.value == HelperValue::DrawArguments)
                })
        } else {
            None
        };
        let writes = depth_stencil.depth_write
            || depth_stencil.stencil.is_some() && depth_stencil.stencil_write_mask != [0; 2];
        let reads_primitive = ps.as_ref().is_some_and(|p| {
            p.reflection
                .inputs
                .iter()
                .any(|i| i.system == Some(wcontainers_gpu::SystemValue::PrimitiveId))
        });
        let per_primitive =
            match early_tests.is_some() && writes && made.is_none() && !reads_primitive {
                true => Some(self.per_primitive(&depth_stencil, &raster, &vs.modules[0])?),
                false => None,
            };
        let linked = match (&made, &lines_key) {
            (_, Some(key)) => Link {
                flags: VariantFlags::LINE_COVERAGE,
                values: key.linked,
            },
            (Some(m), None) if m.shader.is_some() || vertex_alone => m.linked,
            _ => Link {
                flags: vs_typed.flags & VariantFlags::CLIP_DISTANCE
                    | ps_key.as_ref().map_or(VariantFlags::empty(), |k| {
                        k.flags & VariantFlags::DEPTH_CLAMP
                    }),
                values: vs_typed.linked,
            },
        };
        let render = Render {
            root: root.clone(),
            clamped_without_target: clamped_in_shader && !clamped_in_pixels,
            blend: self.backend.create_blend(&blend)?,
            depth_stencil: self.backend.create_depth_stencil(&depth_stencil)?,
            depth_stencil_bound: depth_stencil_bound
                .map(|d| self.backend.create_depth_stencil(&d))
                .transpose()?,
            raster: self.backend.create_raster(&raster)?,
            inputs,
            targets,
            strip_cut: desc.strip_cut,
            reads_bases,
            helpers: pull::Helpers::of(&vs),
            arguments,
            pulled: Vec::new(),
            vs_code: desc.vs.to_vec(),
            interpolation,
            fragment,
            typed_key: vs_typed,
            vertex_formats,
            bound: Vec::new(),
            vertex: vs_drawn,
            pixel: desc.ps.to_vec(),
            counting: None,
            per_sample: ps
                .as_ref()
                .is_some_and(|p| statistics::per_sample(&p.reflection)),
            pixel_counting: None,
            writes_depth_stencil: desc.dsv_format != 0
                && (desc.depth_stencil.depth_enable && desc.depth_stencil.depth_write_all
                    || desc.depth_stencil.stencil_enable
                        && desc.depth_stencil.stencil_write_mask != 0),
            early_tests,
            per_primitive,
            alpha_to_coverage: desc.blend.alpha_to_coverage,
            linked,
            instances_step_by_one,
            geometry: made.map(|m| m.geometry),
            moves,
        };
        // Built ahead with the strides of packed elements, where the back end fetches them so.
        let packed = packed_strides(&render.inputs);
        let fetched = pull::pulled_streams(&render.inputs, &packed, self.backend.caps())
            .iter()
            .all(Option::is_none);
        if let Some(topology) = topology
            && (fetched || render.geometry.is_some())
        {
            let layout = match render.geometry.as_ref().and_then(|g| g.layout.as_ref()) {
                Some(l) => l.clone(),
                None => self.vertex_layout(&render.inputs, &packed)?,
            };
            self.backend.prepare(&PrepareDesc {
                program: &program,
                blend: &render.blend,
                depth_stencil: &render.depth_stencil,
                raster: &render.raster,
                vertex_layout: &layout,
                targets,
                topology,
                layouts: Default::default(),
            });
        }
        Ok(PipelineId(
            self.pipelines.insert(Pipeline {
                program,
                render: Some(render),
                bindings,
                group_threads: 0,
                formats: ps
                    .as_ref()
                    .zip(ps_key)
                    .filter(|_| !desc.ps.is_empty())
                    .and_then(|(c, k)| {
                        formats::Formats::of(bind::PIXEL, desc.ps, &k, &c.reflection.sampled_pairs)
                    }),
                helpers: ps.as_ref().map(Helpers::of).unwrap_or_default(),
                slow_paths,
            }) as u32,
        ))
    }

    /// `CreateComputePipelineState`.
    pub fn create_compute_pipeline(
        &mut self,
        root: Option<&RootSignature>,
        cs: &[u8],
    ) -> Result<PipelineId, Error> {
        self.live()?;
        let root = self.pipeline_root(root, &[cs])?;
        if !self.backend.caps().features.contains(Features::COMPUTE) {
            return Err(Error::NotImplemented("compute shaders on this back end"));
        }
        let (mut key, through) = formats::typed_key(cs, self.texel_route(), &root, bind::ALL);
        let code = cs;
        let cs = self.convert_stage(code, ShaderStage::Compute, &mut key, through)?;
        let bindings = bind::stage_bindings(&cs, 0, StageSet::COMPUTE);
        bind::check(&root, &bindings)?;
        self.prepare_nulls(&bind::null_shapes(&bindings))?;
        let group_threads = match cs.reflection.extras {
            wcontainers_gpu::StageExtras::Compute { workgroup_size } => {
                workgroup_size.iter().map(|&n| u64::from(n)).product()
            }
            _ => 1,
        };
        let shader = self.backend.create_shader(&cs.modules[0])?;
        let program = self.backend.create_program(&Program::Compute(&shader));
        self.backend.destroy(Object::Shader(shader));
        Ok(PipelineId(self.pipelines.insert(Pipeline {
            program: program?,
            render: None,
            bindings,
            group_threads,
            formats: formats::Formats::of(0, code, &key, &cs.reflection.sampled_pairs),
            helpers: Helpers::of(&cs),
            slow_paths: programs_slow_paths(&[&cs]),
        }) as u32))
    }

    /// Counts the slow paths pipeline `pid`'s programs take by themselves.
    fn pipeline_slow_paths(&mut self, pid: PipelineId) {
        for &p in &self.pipelines[pid.0 as usize].slow_paths {
            self.backend.slow_paths().hit(p);
        }
    }

    /// The root signature a pipeline is made with: the one given, else one a shader carries.
    fn pipeline_root(
        &self,
        root: Option<&RootSignature>,
        shaders: &[&[u8]],
    ) -> Result<RootSignature, Error> {
        if let Some(r) = root {
            return Ok(r.clone());
        }
        let code = shaders
            .iter()
            .find(|c| container_has(c, *b"RTS0"))
            .ok_or(Error::Invalid("a pipeline needs its root signature"))?;
        Ok(Arc::new(deserialize(code).map_err(Error::Invalid)?))
    }

    /// A DXIL or DXBC shader in the common form. Shaders reading helper constants wait for the
    /// variants that need them; the converter's own refusals are kept as messages.
    fn convert(
        &mut self,
        code: &[u8],
        stage: ShaderStage,
        variant: VariantKey,
    ) -> Result<Converted, Error> {
        let converted = self.convert_any(code, &format!("{stage:?}"), variant)?;
        if converted.modules.len() != 1 || converted.modules[0].stage != stage {
            return Err(Error::Invalid("a shader of another stage"));
        }
        Ok(converted)
    }

    /// A pixel or compute shader in the common form with `key`; where the converter refuses
    /// atomics on storage textures, with its typed texture views (the slots `through`, from
    /// `formats::typed_key`) read through buffers (`AtomicsThroughBuffer`), and where a pixel
    /// shader reads its clip or cull distances, with them linked; which `key` then says.
    fn convert_stage(
        &mut self,
        code: &[u8],
        stage: ShaderStage,
        key: &mut VariantKey,
        through: u64,
    ) -> Result<Converted, Error> {
        let source = match container_has(code, *b"DXIL") {
            true => Source::Dxil(code),
            false => Source::Dxbc(code),
        };
        if key.texels_through_buffer == 0
            && through != 0
            && let Err(ConvertError::Unsupported("StorageImageAtomics")) =
                crate::cache::convert(source, key)
        {
            key.texels_through_buffer = through;
        }
        // A pixel shader reading its clip or cull distances reads them as values the last vertex
        // stage passes, past every register: converted so first, for its reflection, and linked
        // to that stage's values later (`link_pixel_program`). One reading both is refused for
        // each in turn.
        let mut result = crate::cache::convert(source, key);
        loop {
            let reads = match result {
                Err(ConvertError::Unsupported(refusal::CLIP_DISTANCE)) => {
                    VariantFlags::CLIP_DISTANCE
                }
                Err(ConvertError::Unsupported(refusal::CULL_DISTANCE)) => {
                    VariantFlags::CULL_DISTANCE_LINKED
                }
                _ => break,
            };
            if key.flags.contains(reads) {
                break;
            }
            key.flags |= reads;
            key.linked = LinkedValues {
                location: OUTPUT_REGISTERS,
                clip_distances: MOST_DISTANCES,
            };
            result = crate::cache::convert(source, key);
        }
        let converted = self.checked(result, &format!("{stage:?}"))?;
        if converted.modules.len() != 1 || converted.modules[0].stage != stage {
            return Err(Error::Invalid("a shader of another stage"));
        }
        Ok(converted)
    }

    /// A geometry, hull or domain shader in the common form, as the stage helper takes it.
    fn convert_staged(
        &mut self,
        code: &[u8],
        stage: wcontainers_gpu::SourceStage,
        variant: VariantKey,
    ) -> Result<Converted, Error> {
        let converted = self.convert_any(code, &format!("{stage:?}"), variant)?;
        if converted.reflection.stage != stage {
            return Err(Error::Invalid("a shader of another stage"));
        }
        Ok(converted)
    }

    /// `code` in the common form, of whichever stage; `stage` names it in messages.
    fn convert_any(
        &mut self,
        code: &[u8],
        stage: &str,
        variant: VariantKey,
    ) -> Result<Converted, Error> {
        let source = if container_has(code, *b"DXIL") {
            Source::Dxil(code)
        } else {
            Source::Dxbc(code)
        };
        self.checked(crate::cache::convert(source, &variant), stage)
    }

    /// A conversion's result of a shader of `stage` (named in messages): the converter's refusal as
    /// the front's, and kept as a message; refused where it reads helper constants the front does
    /// not write.
    fn checked(
        &mut self,
        result: Result<Converted, ConvertError>,
        stage: &str,
    ) -> Result<Converted, Error> {
        let converted = result.map_err(|e| {
            let error = match e {
                ConvertError::Rejected(_) => Error::Invalid("the shader bytecode was refused"),
                ConvertError::Unsupported(_) | ConvertError::Reader(_) => {
                    Error::NotImplemented("a shader feature the converter lacks")
                }
                ConvertError::Malformed(_) | ConvertError::Invalid(_) => {
                    Error::NotImplemented("a shader the converter got wrong")
                }
            };
            self.messages
                .push(format!("a {stage} shader did not convert: {e}"));
            error
        })?;
        if let Some(h) = converted.reflection.helpers.iter().find(|h| {
            !matches!(
                h.value,
                HelperValue::DrawBase
                    | HelperValue::DrawArguments
                    | HelperValue::PulledStream { .. }
                    | HelperValue::TexelBufferView { .. }
                    | HelperValue::LodBias { .. }
                    | HelperValue::BorderColor { .. }
                    | HelperValue::SamplerState { .. }
                    | HelperValue::TextureComponents { .. }
                    | HelperValue::TexelLayout { .. }
                    | HelperValue::MinLod { .. }
                    | HelperValue::Layer
                    | HelperValue::Viewport
                    | HelperValue::EarlyTests
                    | HelperValue::StorageOffset { .. }
                    | HelperValue::DepthRange
            )
        }) {
            self.messages.push(format!(
                "a {stage} shader reads helper constants the front does not write: {:?}",
                h.value
            ));
            return Err(Error::NotImplemented(
                "shaders reading helper constants the front does not write",
            ));
        }
        Ok(converted)
    }

    /// The pixel program of a pipeline without a pixel shader whose variant `key` needs one: the
    /// shaders crate's program writing nothing, with `key`'s rewrites.
    fn pixel_program_for_variant(&mut self, key: &VariantKey) -> Result<Converted, Error> {
        wcontainers_shaders::builtin::pixel_program_for_variant(key).map_err(|e| {
            self.messages.push(format!(
                "the pixel program for a variant did not convert: {e}"
            ));
            Error::NotImplemented("a shader feature the converter lacks")
        })
    }

    /// Converts pipeline `desc`'s pixel shader again with `link`, which the last vertex stage
    /// passes; without a pixel shader, the pixel program for the variant where `link` asks the
    /// pixel stage to apply anything.
    fn link_pixel_program(
        &mut self,
        desc: &GraphicsPipelineDesc<'_>,
        samples: u8,
        ps: &mut Option<Converted>,
        ps_key: &mut Option<VariantKey>,
        link: Link,
    ) -> Result<(), Error> {
        if let (Some(p), Some(key)) = (ps.as_mut(), ps_key.as_mut()) {
            link.apply(key);
            *p = self.convert(desc.ps, ShaderStage::Fragment, key.clone())?;
            return Ok(());
        }
        let link = link.unshaded();
        if link.flags.is_empty() {
            return Ok(());
        }
        let mut key = VariantKey {
            samples,
            ..VariantKey::default()
        };
        link.apply(&mut key);
        *ps = Some(self.pixel_program_for_variant(&key)?);
        *ps_key = Some(key);
        Ok(())
    }

    /// How programs read typed buffers on this back end.
    fn texel_route(&self) -> TexelBufferRoute {
        match self
            .backend
            .caps()
            .features
            .contains(Features::TEXEL_BUFFERS)
        {
            true => TexelBufferRoute::Native,
            false => TexelBufferRoute::Storage,
        }
    }

    /// A pipeline's counting program (its pixel shader writing 1.0 to target 0 and nothing
    /// else, `VariantFlags::COUNTING`), made at its first counted draw.
    fn counting_program(&mut self, pid: PipelineId) -> Result<B::Program, Error> {
        let render = self.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        if let Some(p) = &render.counting {
            return Ok(p.clone());
        }
        let (vertex, pixel) = (render.vertex.clone(), render.pixel.clone());
        let moves = render.moves.inputs.clone();
        let root = render.root.clone();
        let samples = render.targets.samples.clamp(1, 32) as u8;
        let linked = render.linked;
        let fragment = if pixel.is_empty() {
            let mut key = VariantKey {
                flags: VariantFlags::COUNTING,
                samples,
                ..VariantKey::default()
            };
            linked.unshaded().apply(&mut key);
            self.pixel_program_for_variant(&key)?.modules.swap_remove(0)
        } else {
            let mut key = VariantKey {
                flags: VariantFlags::COUNTING,
                samples,
                ..formats::typed_key(&pixel, self.texel_route(), &root, bind::PIXEL).0
            };
            linked.apply(&mut key);
            pull::moved(self.convert(&pixel, ShaderStage::Fragment, key)?, &moves)?
                .modules
                .swap_remove(0)
        };
        let vertex = self.backend.create_shader(&vertex)?;
        let fragment = match self.backend.create_shader(&fragment) {
            Ok(f) => f,
            Err(e) => {
                self.backend.destroy(Object::Shader(vertex));
                return Err(e.into());
            }
        };
        let program = self.backend.create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(vertex));
        self.backend.destroy(Object::Shader(fragment));
        let program = program?;
        self.render_mut(pid).counting = Some(program.clone());
        Ok(program)
    }

    pub fn destroy_pipeline(&mut self, id: PipelineId) {
        if let Some(p) = self.pipelines.try_remove(id.0 as usize) {
            self.backend.destroy(Object::Program(p.program));
            if let Some(f) = p.formats {
                f.destroy(&mut self.backend);
            }
            if let Some(r) = p.render {
                let arguments = r.arguments.map(|a| a.program);
                let pulled = r.pulled.into_iter().filter_map(|p| p.1.map(|p| p.program));
                let bound = r.bound.into_iter().filter_map(|b| b.1.map(|b| b.0));
                for c in [r.counting, r.pixel_counting, arguments]
                    .into_iter()
                    .flatten()
                    .chain(pulled)
                    .chain(bound)
                {
                    self.backend.destroy(Object::Program(c));
                }
                if let Some(f) = r.vertex_formats {
                    f.destroy(&mut self.backend);
                }
                if let Some(stages) = r.geometry.and_then(|g| g.stages) {
                    stages.destroy(&mut self.backend);
                }
                self.backend.destroy(Object::Blend(r.blend));
                self.backend.destroy(Object::DepthStencil(r.depth_stencil));
                if let Some(d) = r.depth_stencil_bound {
                    self.backend.destroy(Object::DepthStencil(d));
                }
                self.backend.destroy(Object::Raster(r.raster));
                if let Some(p) = r.per_primitive {
                    self.backend.destroy(Object::DepthStencil(p.tested));
                    self.backend.destroy(Object::Raster(p.flipped));
                    self.backend.destroy(Object::Program(p.written));
                }
            }
        }
    }

    /// The vertex layout block for `inputs` with the bound strides, made once per combination.
    fn vertex_layout(
        &mut self,
        inputs: &[(InputElement, u32)],
        strides: &[u32; MAX_VERTEX_STREAMS],
    ) -> Result<B::VertexLayout, Error> {
        let mut desc = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        let rates = self
            .backend
            .caps()
            .features
            .contains(Features::INSTANCE_STEP_RATE);
        for (i, (e, location)) in inputs.iter().enumerate() {
            let slot = e.slot as usize;
            desc.streams[slot] = Some(match (e.per_instance, e.step_rate) {
                // Never stepping: every vertex and instance reads one element, which the draw
                // binds at its first instance's.
                (true, 0) if !rates => VertexStream {
                    stride: 0,
                    step: StepMode::Vertex,
                },
                (true, rate) => VertexStream {
                    stride: strides[slot],
                    step: StepMode::Instance(rate),
                },
                (false, _) => VertexStream {
                    stride: strides[slot],
                    step: StepMode::Vertex,
                },
            });
            *desc
                .attributes
                .get_mut(i)
                .ok_or(Error::Invalid("too many input elements"))? = Some(VertexAttribute {
                location: *location,
                stream: e.slot as u8,
                offset: e.offset,
                format: vertex_format(e.format)?.0,
            });
        }
        self.layout(desc)
    }

    /// The vertex layout block of `desc`, made once.
    fn layout(&mut self, desc: VertexLayoutDesc) -> Result<B::VertexLayout, Error> {
        if let Some(l) = self.layouts.get(&desc) {
            return Ok(l.clone());
        }
        let layout = self.backend.create_vertex_layout(&desc)?;
        self.layouts.insert(desc, layout.clone());
        Ok(layout)
    }

    /// A graphics pipeline's fixed state, for a pipeline the draw has checked is one.
    fn render_mut(&mut self, pid: PipelineId) -> &mut Render<B> {
        self.pipelines[pid.0 as usize]
            .render
            .as_mut()
            .expect("a graphics pipeline")
    }

    // ---- Fences ----

    pub fn create_fence(&mut self, initial: u64) -> FenceId {
        FenceId(self.fences.insert(Fence {
            value: initial,
            pending: Vec::new(),
        }) as u32)
    }

    pub fn destroy_fence(&mut self, id: FenceId) {
        self.fences.try_remove(id.0 as usize);
        self.waiters.retain(|w| w.0.iter().all(|f| f.0 != id));
    }

    /// `SetEventOnCompletion` with an event: `token` comes back from [`Device::reached_events`]
    /// once the fence reaches `value`. Nothing waits here.
    pub fn add_waiter(&mut self, id: FenceId, value: u64, token: u64) {
        self.add_waiter_many(vec![(id, value)], false, token);
    }

    /// `SetEventOnMultipleFenceCompletion`: `token` comes back once every fence (or, with `any`,
    /// one of them) reaches its value.
    pub fn add_waiter_many(&mut self, fences: Vec<(FenceId, u64)>, any: bool, token: u64) {
        self.waiters.push((fences, any, token));
    }

    pub fn has_waiters(&self) -> bool {
        !self.waiters.is_empty()
            || self
                .chains
                .iter()
                .any(|(_, c)| c.waitable.is_some() && c.chain.frames_in_flight())
    }

    /// Gives swap chain `id` a frame latency waitable object, `token`, released once for each frame
    /// that finishes from now on.
    pub fn set_waitable(&mut self, id: ChainId, token: u64) {
        if let Some(c) = self.chains.get_mut(id.0 as usize) {
            let retired = c.chain.frames_retired(&mut self.backend);
            c.waitable = Some((token, retired));
        }
    }

    /// The waitable objects of swap chains whose frames finished since the last call, and how many
    /// frames finished for each.
    pub fn frames_finished(&mut self) -> Vec<(u64, u32)> {
        let mut out = Vec::new();
        for (_, c) in &mut self.chains {
            if let Some((token, released)) = &mut c.waitable {
                let retired = c.chain.frames_retired(&mut self.backend);
                if retired > *released {
                    out.push((*token, (retired - *released) as u32));
                    *released = retired;
                }
            }
        }
        out
    }

    /// The tokens of events whose fence has reached its value, asking the back end without waiting.
    pub fn reached_events(&mut self) -> Vec<u64> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.waiters.len() {
            let (fences, any, token) = std::mem::take(&mut self.waiters[i]);
            let mut reached = fences
                .iter()
                .map(|&(id, value)| self.completed(id) >= value);
            let done = if any {
                reached.any(|r| r)
            } else {
                reached.all(|r| r)
            };
            self.waiters[i] = (fences, any, token);
            if done {
                out.push(token);
                self.waiters.swap_remove(i);
            } else {
                i += 1;
            }
        }
        out
    }

    // ---- Queues ----

    /// `CreateCommandQueue`: every queue shares the back end's one submission order.
    pub fn create_queue(&mut self) -> Result<QueueId, Error> {
        self.live()?;
        Ok(QueueId(self.queues.insert(VecDeque::new()) as u32))
    }

    /// A queue's release; work still held behind a wait never runs, as on the GPU.
    pub fn destroy_queue(&mut self, id: QueueId) {
        self.queues.try_remove(id.0 as usize);
    }

    /// Holds `work` if the queue already holds some; else gives it back to be done now.
    fn hold(&mut self, queue: QueueId, work: Work) -> Result<Option<Work>, Error> {
        self.calls += 1;
        let held = self
            .queues
            .get_mut(queue.0 as usize)
            .ok_or(Error::Invalid("no such queue"))?;
        if held.is_empty() {
            return Ok(Some(work));
        }
        held.push_back((self.calls, work));
        Ok(None)
    }

    /// `ExecuteCommandLists`: replayed and submitted now, unless the queue waits.
    pub fn execute(&mut self, queue: QueueId, lists: &[Arc<[Command]>]) -> Result<(), Error> {
        self.poll_lost();
        self.live()?;
        if let Some(Work::Execute(lists)) = self.hold(queue, Work::Execute(lists.to_vec()))? {
            self.replay(&lists)?;
        }
        Ok(())
    }

    /// `ID3D12CommandQueue::Signal`: the fence reaches `value` when the work submitted before it is
    /// done.
    pub fn queue_signal(&mut self, queue: QueueId, id: FenceId, value: u64) -> Result<(), Error> {
        self.live()?;
        if !self.fences.contains(id.0 as usize) {
            return Err(Error::Invalid("no such fence"));
        }
        if self.hold(queue, Work::Signal(id, value))?.is_some() {
            self.signal_now(id, value);
            self.release();
        }
        Ok(())
    }

    /// `ID3D12CommandQueue::Wait`: the queue's later work is held until the fence's value is
    /// reached by work already replayed or by the CPU.
    pub fn queue_wait(&mut self, queue: QueueId, id: FenceId, value: u64) -> Result<(), Error> {
        self.live()?;
        if !self.fences.contains(id.0 as usize) {
            return Err(Error::Invalid("no such fence"));
        }
        if self.hold(queue, Work::Wait(id, value))?.is_some() && self.reached(id) < value {
            self.queues[queue.0 as usize].push_back((self.calls, Work::Wait(id, value)));
        }
        Ok(())
    }

    fn signal_now(&mut self, id: FenceId, value: u64) {
        let fence = self.backend.submit();
        if let Some(f) = self.fences.get_mut(id.0 as usize) {
            f.pending.push((value, fence));
        }
    }

    /// The value the fence will hold once the work replayed so far is done.
    fn reached(&self, id: FenceId) -> u64 {
        self.fences.get(id.0 as usize).map_or(0, |f| {
            f.pending.last().map_or(f.value, |p| p.0.max(f.value))
        })
    }

    /// Does held work whose waits are now met, earliest call first, until none is left to do.
    fn release(&mut self) {
        loop {
            let next = self
                .queues
                .iter()
                .filter_map(|(q, held)| {
                    let (call, work) = held.front()?;
                    match *work {
                        Work::Wait(f, v) if self.reached(f) < v => None,
                        _ => Some((*call, q)),
                    }
                })
                .min();
            let Some((_, q)) = next else {
                return;
            };
            match self.queues[q].pop_front().map(|w| w.1) {
                Some(Work::Execute(lists)) if self.removed.is_none() => {
                    // A failure removes the device, which every later call reports.
                    let _ = self.replay(&lists);
                }
                Some(Work::Signal(f, v)) => self.signal_now(f, v),
                _ => {}
            }
        }
    }

    /// `ID3D12Fence::Signal` from the CPU; queues waiting for it go on.
    pub fn cpu_signal(&mut self, id: FenceId, value: u64) {
        // Queue signals already done come first; those still to come land after it.
        self.completed(id);
        if let Some(f) = self.fences.get_mut(id.0 as usize) {
            f.value = value;
        }
        self.release();
    }

    /// `GetCompletedValue`. Read backs whose work is done are in readback memory first (waiting only
    /// for their bytes to arrive), so the program sees the bytes when it sees the value.
    pub fn completed(&mut self, id: FenceId) -> u64 {
        // A removed device's fences report every value reached, as Direct3D 12's do.
        if self.removed.is_some() {
            return u64::MAX;
        }
        let Some(f) = self.fences.get_mut(id.0 as usize) else {
            return 0;
        };
        let done = f
            .pending
            .iter()
            .take_while(|(_, g)| self.backend.fence_done(g))
            .count();
        if let Some((value, _)) = f.pending.drain(..done).next_back() {
            f.value = value;
        }
        let value = f.value;
        self.finish_reads(true);
        value
    }

    /// Waits until the fence reaches `value` through work already submitted; false when only work
    /// not yet submitted (or a CPU signal) could reach it, so waiting now would never end.
    pub fn wait(&mut self, id: FenceId, value: u64) -> bool {
        if self.completed(id) >= value {
            return true;
        }
        let Some(f) = self.fences.get_mut(id.0 as usize) else {
            return false;
        };
        let Some(at) = f.pending.iter().position(|(v, _)| *v >= value) else {
            return false;
        };
        let target = f.pending[at].1.clone();
        self.backend.wait_fence(&target);
        self.completed(id);
        self.finish_reads(true);
        true
    }

    /// Copies finished read backs into readback memory; with `all`, also those whose submission is
    /// done but whose bytes are still on their way, waiting for them.
    fn finish_reads(&mut self, all: bool) {
        let mut i = 0;
        while i < self.reads.len() {
            let r = &self.reads[i];
            let submitted_done = r
                .submission
                .as_ref()
                .is_some_and(|f| self.backend.fence_done(f));
            let ready = self.backend.read_state(&r.readback) == ReadState::Ready;
            if !(submitted_done && (ready || all)) {
                i += 1;
                continue;
            }
            self.finish_read(i);
        }
    }

    /// Finishes read back `i` into its buffer's memory, waiting for it if it is not ready. Later
    /// read backs keep their order, so overlapping ones land as the GPU wrote them.
    fn finish_read(&mut self, i: usize) {
        let r = self.reads.remove(i);
        if self.backend.read_state(&r.readback) != ReadState::Ready {
            self.backend.wait_read(&r.readback);
        }
        if let Some(Resource {
            storage: Storage::Buffer {
                host: Some(host), ..
            },
            ..
        }) = self.resources.get(r.resource.0 as usize)
        {
            self.staging.clear();
            self.staging.resize(r.size, 0);
            self.backend.finish_read(r.readback, &mut self.staging);
            let cells = &host.cells()[r.offset..r.offset + r.size];
            cells.iter().zip(&self.staging).for_each(|(c, b)| c.set(*b));
        }
    }

    // ---- Swap chains ----

    /// `CreateSwapChainForHwnd` and the others, on a queue of this device: a swap chain on the host
    /// window `window`, with a resource standing for each back buffer the program may get.
    pub fn create_swap_chain(
        &mut self,
        window: u32,
        desc: &SwapChainDesc,
    ) -> Result<ChainId, SwapChainError> {
        self.live().map_err(|_| GpuError::DeviceLost)?;
        let chain = SwapChain::new(&mut self.backend, window, desc)?;
        let buffers = adopt_buffers(&mut self.resources, &chain);
        Ok(ChainId(self.chains.insert(Chain {
            chain,
            buffers,
            waitable: None,
        }) as u32))
    }

    /// The resource standing for back buffer `index` (`GetBuffer`).
    pub fn chain_buffer(&self, id: ChainId, index: u32) -> Option<ResourceId> {
        self.chains
            .get(id.0 as usize)?
            .buffers
            .get(index as usize)
            .copied()
    }

    pub fn chain(&self, id: ChainId) -> Option<&SwapChain<B>> {
        self.chains.get(id.0 as usize).map(|c| &c.chain)
    }

    pub fn chain_mut(&mut self, id: ChainId) -> Option<&mut SwapChain<B>> {
        self.chains.get_mut(id.0 as usize).map(|c| &mut c.chain)
    }

    /// `Present`: the work executed so far is submitted with the copy into the window.
    pub fn present(&mut self, id: ChainId, sync_interval: u32) -> Result<(), SwapChainError> {
        self.poll_lost();
        self.live().map_err(|_| GpuError::DeviceLost)?;
        let c = self
            .chains
            .get_mut(id.0 as usize)
            .ok_or(SwapChainError::InvalidCall("no such swap chain"))?;
        c.chain.present(&mut self.backend, sync_interval)
    }

    /// Whether a present now would wait for frame pacing (`DXGI_PRESENT_DO_NOT_WAIT`).
    pub fn would_wait(&mut self, id: ChainId) -> bool {
        match self.chains.get_mut(id.0 as usize) {
            Some(c) => c.chain.would_wait(&mut self.backend),
            None => false,
        }
    }

    /// `ResizeBuffers`, once the shell has seen that the program holds no back buffer.
    pub fn resize_swap_chain(
        &mut self,
        id: ChainId,
        buffers: u32,
        width: u32,
        height: u32,
        format: u32,
    ) -> Result<(), SwapChainError> {
        let old = std::mem::take(
            &mut self
                .chains
                .get_mut(id.0 as usize)
                .ok_or(SwapChainError::InvalidCall("no such swap chain"))?
                .buffers,
        );
        for r in old {
            self.destroy_resource(r);
        }
        let c = &mut self.chains[id.0 as usize];
        let result = c
            .chain
            .resize(&mut self.backend, buffers, width, height, format);
        c.buffers = adopt_buffers(&mut self.resources, &c.chain);
        result
    }

    pub fn destroy_swap_chain(&mut self, id: ChainId) {
        if let Some(c) = self.chains.try_remove(id.0 as usize) {
            for r in c.buffers {
                self.destroy_resource(r);
            }
            c.chain.destroy(&mut self.backend);
        }
    }

    // ---- Execution ----

    /// `ExecuteCommandLists`: upload memory the lists read is written to the GPU, the lists are
    /// replayed into the back end in order, and the work is submitted.
    /// Replays command lists into the back end and submits them.
    fn replay(&mut self, lists: &[Arc<[Command]>]) -> Result<(), Error> {
        self.live()?;
        self.uploaded.clear();
        let mut uploads: Vec<ResourceId> = Vec::new();
        for c in lists.iter().flat_map(|l| l.iter()) {
            self.uploads_read(c, &mut uploads);
        }
        for id in uploads {
            self.upload(id)?;
        }
        let first_read = self.reads.len();
        let result = lists.iter().try_for_each(|l| {
            let mut replay = Replay::new();
            for (i, c) in l.iter().enumerate() {
                match c {
                    Command::Predication(p) => replay.set_predicate(self, *p, &l[i + 1..])?,
                    c => replay.run(self, c)?,
                }
            }
            replay.finish(self)
        });
        let result = result.and_then(|()| self.zero_written_nulls());
        let submission = self.backend.submit();
        for r in &mut self.reads[first_read..] {
            r.submission = Some(submission.clone());
        }
        if let Err(e) = result {
            self.remove(e);
            return Err(Error::DeviceRemoved);
        }
        Ok(())
    }

    /// Adds the upload resources command `c` reads.
    fn uploads_read(&self, c: &Command, out: &mut Vec<ResourceId>) {
        if let Command::Bundle(commands) = c {
            commands.iter().for_each(|c| self.uploads_read(c, out));
            return;
        }
        let mut add = |id: ResourceId| {
            if self
                .resources
                .get(id.0 as usize)
                .is_some_and(|r| r.heap.uploads())
            {
                out.push(id);
            }
        };
        match c {
            Command::VertexBuffers { views, .. } => views
                .iter()
                .filter_map(|v| self.addresses.find(v.address))
                .for_each(|(id, _)| add(id)),
            Command::IndexBuffer(Some(v)) => {
                if let Some((id, _)) = self.addresses.find(v.address) {
                    add(id)
                }
            }
            Command::CopyBuffer { src, .. } => add(*src),
            Command::CopyTexture {
                src: CopyLocation::Footprint { resource, .. },
                ..
            } => add(*resource),
            _ => {}
        }
    }

    /// `WriteBufferImmediate`: four bytes written by the GPU, in order with the work around them,
    /// into a buffer the GPU may write (not one in an upload heap).
    fn write_immediate(&mut self, address: u64, value: u32) -> Result<(), Error> {
        let (id, offset) = self
            .addresses
            .find(address)
            .filter(|_| address.is_multiple_of(4))
            .ok_or(Error::Invalid("an immediate write to no buffer"))?;
        let (buffer, heap) = self.buffer(id)?;
        if heap == HeapType::Upload {
            return Err(Error::Invalid("an immediate write into an upload heap"));
        }
        let buffer = buffer.clone();
        self.backend
            .clear_buffer(&buffer, offset, 4, &value.to_le_bytes());
        if heap.reads_back() {
            self.read_into(id, offset, 4)?;
        }
        Ok(())
    }

    /// `ResolveSubresource`: a multisampled subresource averaged into a single-sampled one of the
    /// same size, read as `format`, which must be the source's stored format or its sRGB twin.
    fn resolve_subresource(
        &mut self,
        dst: ResourceId,
        dst_index: u32,
        src: ResourceId,
        src_index: u32,
        format: u32,
    ) -> Result<(), Error> {
        let format = dxgi(format)?;
        let region = |d: &Self, id: ResourceId, index: u32| {
            let (_, desc) = d.texture(id)?;
            let r = &d.resources[id.0 as usize].desc;
            let (mips, layers) = (r.mips(), r.layers());
            if index >= mips * layers {
                return Err(Error::Invalid("a resolve's subresource"));
            }
            let mip = index % mips;
            let size = ((desc.width >> mip).max(1), (desc.height >> mip).max(1));
            let (texture, layer) = d.texture_layer(id, index / mips)?;
            Ok((texture.clone(), desc.format, desc.samples, mip, layer, size))
        };
        let (s, stored, samples, smip, slayer, ssize) = region(self, src, src_index)?;
        let (t, _, tsamples, tmip, tlayer, tsize) = region(self, dst, dst_index)?;
        if samples < 2 || tsamples != 1 || ssize != tsize {
            return Err(Error::Invalid(
                "a resolve from a multisampled texture into a single-sampled one of its size",
            ));
        }
        if format.linear() != stored.linear() {
            return Err(Error::NotImplemented(
                "a resolve in a format reinterpreting the stored bits",
            ));
        }
        let at = |texture, mip, layer| TextureRegion {
            texture,
            mip,
            layer,
            origin: Origin::default(),
            stencil: false,
        };
        self.backend
            .resolve(&at(&s, smip, slayer), &at(&t, tmip, tlayer), format);
        Ok(())
    }

    fn buffer(&self, id: ResourceId) -> Result<(&B::Buffer, HeapType), Error> {
        match self.resources.get(id.0 as usize) {
            Some(Resource {
                storage: Storage::Buffer { buffer, .. },
                heap,
                ..
            }) => Ok((buffer, *heap)),
            _ => Err(Error::Invalid("not a buffer")),
        }
    }

    fn texture(&self, id: ResourceId) -> Result<(&B::Texture, &TextureDesc), Error> {
        match self.resources.get(id.0 as usize) {
            Some(Resource {
                storage: Storage::Texture { texture, desc, .. },
                ..
            }) => Ok((texture, desc)),
            _ => Err(Error::Invalid("not a texture")),
        }
    }

    /// The back end's texture holding layer `layer` of texture `id`, and the layer's index in it.
    fn texture_layer(&self, id: ResourceId, layer: u32) -> Result<(&B::Texture, u32), Error> {
        self.resources
            .get(id.0 as usize)
            .and_then(|r| r.storage.layer(layer))
            .ok_or(Error::Invalid("a texture's layer"))
    }

    /// The back end's view for a render target or depth view, made once per resource and view.
    fn view(&mut self, v: &TargetView) -> Result<B::TextureView, Error> {
        self.view_at(v, 0)
    }

    /// As `view`, of the view's `layer`th layer past its first.
    fn view_at(&mut self, v: &TargetView, layer: u32) -> Result<B::TextureView, Error> {
        let three_d = matches!(
            self.resources.get(v.resource.0 as usize).map(|r| &r.storage),
            Some(Storage::Texture { desc, .. }) if desc.dimension == TextureDimension::D3
        );
        // Draws write one slice: the view's first, or the run's of a draw split per layer. Clears
        // of every slice go by `clear_texture`.
        self.texture_view(
            v.resource,
            TextureViewDesc {
                format: dxgi(v.format)?,
                dimension: if three_d {
                    ViewDimension::D3
                } else {
                    ViewDimension::D2
                },
                base_mip: v.mip,
                mip_count: 1,
                base_layer: if three_d { 0 } else { v.first_layer + layer },
                layer_count: 1,
                stencil: false,
            },
        )
    }

    /// One view to draw into for each array slice `v` covers (one for a 3D texture's level).
    fn slice_views(&mut self, v: &TargetView) -> Result<Vec<B::TextureView>, Error> {
        let three_d = matches!(
            self.resources.get(v.resource.0 as usize).map(|r| &r.storage),
            Some(Storage::Texture { desc, .. }) if desc.dimension == TextureDimension::D3
        );
        let slices = if three_d { 1 } else { v.layers.max(1) };
        (v.first_layer..v.first_layer.saturating_add(slices))
            .map(|layer| {
                self.view(&TargetView {
                    first_layer: layer,
                    ..*v
                })
            })
            .collect()
    }

    /// The back end's view `desc` of texture `resource`, made once.
    fn texture_view(
        &mut self,
        resource: ResourceId,
        desc: TextureViewDesc,
    ) -> Result<B::TextureView, Error> {
        let Some(Resource {
            storage:
                Storage::Texture {
                    texture,
                    layers,
                    views,
                    desc: tdesc,
                    ..
                },
            ..
        }) = self.resources.get_mut(resource.0 as usize)
        else {
            return Err(Error::Invalid("a texture view of no texture"));
        };
        // A multisampled array's layer is viewed in its own texture.
        let (texture, at) = match desc.base_layer {
            _ if layers.is_empty() => (&*texture, desc.base_layer),
            _ if desc.layer_count != 1 => {
                return Err(Error::NotImplemented(
                    "a view of several layers of a multisampled texture array",
                ));
            }
            0 => (&*texture, 0),
            n => (
                layers
                    .get(n as usize - 1)
                    .ok_or(Error::Invalid("a view past a texture's layers"))?,
                0,
            ),
        };
        let stored = tdesc.format;
        let mut desc = desc;
        if ResourceDesc::depth_read_as(desc.format) == Some(stored) {
            desc.format = stored;
        }
        if let Some((_, view)) = views.iter().find(|(k, _)| *k == desc) {
            return Ok(view.clone());
        }
        if stored.linear() != desc.format.linear()
            && !stored.is_depth_stencil()
            && !self
                .backend
                .caps()
                .features
                .contains(Features::FORMAT_REINTERPRET)
        {
            return Err(Error::NotImplemented(
                "a view reinterpreting a texture's bits without its twin",
            ));
        }
        let view = self.backend.create_view(
            texture,
            &TextureViewDesc {
                base_layer: at,
                ..desc
            },
        )?;
        views.push((desc, view.clone()));
        Ok(view)
    }

    /// Records a read back of `size` bytes at `offset` of a readback buffer, finished into its
    /// memory when the submission is done.
    /// `ClearUnorderedAccessViewUint` and `...Float`: the integer values are the view's bits with no
    /// conversion; the float values are converted to the view's format.
    fn clear_storage(
        &mut self,
        v: &crate::list::ResourceView,
        values: [u32; 4],
        float: bool,
        rects: &[Rect],
    ) -> Result<(), Error> {
        match self
            .resources
            .get(v.resource.0 as usize)
            .map(|r| &r.storage)
        {
            Some(Storage::Buffer { buffer, .. }) => {
                let buffer = buffer.clone();
                let raw = v.flags & 1 != 0;
                let (element, pattern) = if raw || v.stride != 0 {
                    if float {
                        return Err(Error::NotImplemented(
                            "float clears of raw and structured buffer views",
                        ));
                    }
                    let element = if raw { 4 } else { u64::from(v.stride) };
                    (element, values[0].to_le_bytes().to_vec())
                } else {
                    let format = dxgi(v.format)?;
                    let bytes = storage_texel(format, values, float).ok_or(
                        Error::NotImplemented("clearing a typed buffer view of this format"),
                    )?;
                    (u64::from(format.block().2), bytes)
                };
                if !element.is_multiple_of(pattern.len() as u64) {
                    return Err(Error::NotImplemented(
                        "clearing a structured buffer view whose stride is no multiple of 4",
                    ));
                }
                // A buffer view is one row of its elements; each rectangle clears the elements
                // from its left to its right edge where it covers that row.
                let count = u64::from(v.count);
                let ranges: Vec<(u64, u64)> = if rects.is_empty() {
                    vec![(0, count)]
                } else {
                    rects
                        .iter()
                        .filter(|r| r[1] <= 0 && r[3] > 0)
                        .map(|r| {
                            let at = |x: i32| (x.max(0) as u64).min(count);
                            (at(r[0]), at(r[2]))
                        })
                        .filter(|(l, r)| l < r)
                        .collect()
                };
                for (left, right) in ranges {
                    self.backend.clear_buffer(
                        &buffer,
                        (v.first_element + left) * element,
                        (right - left) * element,
                        &pattern,
                    );
                }
            }
            Some(Storage::Texture { texture, desc, .. }) => {
                let (texture, desc) = (texture.clone(), *desc);
                let format = dxgi(v.format)?;
                let value = storage_clear_value(format, values, float).ok_or(
                    Error::NotImplemented("clearing an unordered access view of this format"),
                )?;
                // One-dimensional textures are stored as 2D ones of one row.
                let layers = match v.dimension {
                    UAV_DIMENSION_TEXTURE1D | UAV_DIMENSION_TEXTURE2D => 0..1,
                    UAV_DIMENSION_TEXTURE1DARRAY
                    | UAV_DIMENSION_TEXTURE2DARRAY
                    | UAV_DIMENSION_TEXTURE3D => {
                        v.first_layer..v.first_layer.saturating_add(v.layers)
                    }
                    _ => return Err(Error::Invalid("an unordered access view's dimension")),
                };
                if !rects.is_empty() {
                    if desc.dimension == TextureDimension::D3 {
                        return self.clear_volume_rects(
                            &texture,
                            &desc,
                            format,
                            v,
                            ClearValue::Color(value),
                            rects,
                        );
                    }
                    let layers = layers.start..layers.end.min(desc.depth_or_layers);
                    let target = TargetView {
                        resource: v.resource,
                        format: v.format,
                        mip: v.mip,
                        first_layer: layers.start,
                        layers: layers.len() as u32,
                    };
                    let rects: Vec<_> = rects.iter().map(scissor).collect();
                    for view in self.slice_views(&target)? {
                        self.backend.clear_rects(
                            &view,
                            format,
                            ClearValue::Color(value),
                            &rects,
                        )?;
                    }
                    return Ok(());
                }
                self.backend.clear_texture(
                    &texture,
                    &desc,
                    format,
                    v.mip..v.mip + 1,
                    layers,
                    ClearValue::Color(value),
                )?;
            }
            None => return Err(Error::Invalid("clearing a view of no resource")),
        }
        Ok(())
    }

    /// Rectangles of a 3D texture's unordered access view, cleared in each of its depth slices: a
    /// scratch texture of the level's size is cleared and its rectangles copied in.
    fn clear_volume_rects(
        &mut self,
        texture: &B::Texture,
        desc: &TextureDesc,
        format: Format,
        v: &crate::list::ResourceView,
        value: ClearValue,
        rects: &[Rect],
    ) -> Result<(), Error> {
        if format.linear() != desc.format.linear() {
            return Err(Error::NotImplemented(
                "clearing rectangles of a 3D texture's view in another format",
            ));
        }
        let (width, height) = ((desc.width >> v.mip).max(1), (desc.height >> v.mip).max(1));
        let depth = (desc.depth_or_layers >> v.mip).max(1);
        let slices = v.first_layer.min(depth)..v.first_layer.saturating_add(v.layers).min(depth);
        let clipped: Vec<ScissorRect> = rects
            .iter()
            .map(scissor)
            .filter_map(|r| {
                let (x, y) = (r.x.min(width), r.y.min(height));
                let (w, h) = (
                    (r.x + r.width).min(width) - x,
                    (r.y + r.height).min(height) - y,
                );
                (w > 0 && h > 0).then_some(ScissorRect {
                    x,
                    y,
                    width: w,
                    height: h,
                })
            })
            .collect();
        if clipped.is_empty() || slices.is_empty() {
            return Ok(());
        }
        let scratch_desc = TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width,
            height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        };
        let scratch = self.backend.create_texture(&scratch_desc)?;
        let result = self
            .backend
            .clear_texture(&scratch, &scratch_desc, format, 0..1, 0..1, value);
        if result.is_ok() {
            for r in &clipped {
                for z in slices.clone() {
                    self.backend.copy_texture(
                        &TextureRegion {
                            texture: &scratch,
                            mip: 0,
                            layer: 0,
                            origin: Origin {
                                x: r.x,
                                y: r.y,
                                z: 0,
                            },
                            stencil: false,
                        },
                        &TextureRegion {
                            texture,
                            mip: v.mip,
                            layer: 0,
                            origin: Origin { x: r.x, y: r.y, z },
                            stencil: false,
                        },
                        Extent {
                            width: r.width,
                            height: r.height,
                            depth_or_layers: 1,
                        },
                    );
                }
            }
        }
        self.backend.destroy(Object::Texture(scratch));
        Ok(result?)
    }

    fn read_into(&mut self, id: ResourceId, offset: u64, size: u64) -> Result<(), Error> {
        let buffer = self.buffer(id)?.0.clone();
        let readback = self.backend.read_back(&ReadSource::Buffer {
            buffer: &buffer,
            offset,
            size,
        })?;
        self.reads.push(PendingRead {
            submission: None,
            readback,
            resource: id,
            offset: offset as usize,
            size: size as usize,
        });
        Ok(())
    }
}

/// Resources standing for the back buffers of `chain`, whose textures the swap chain keeps.
/// `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFF`.
const STRIP_CUT_0XFFFF: u32 = 1;

/// The interface's index format of an index buffer view's DXGI format.
pub(super) fn index_format(format: u32) -> Option<IndexFormat> {
    match format {
        wcontainers_directx_graphics_infrastructure::dxgi::R16_UINT => Some(IndexFormat::U16),
        wcontainers_directx_graphics_infrastructure::dxgi::R32_UINT => Some(IndexFormat::U32),
        _ => None,
    }
}

/// Whether strips of `format` indices drawn with the pipeline's `strip_cut` restart at another value
/// than WebGPU's, which restarts at the format's largest value always (`RestartRewrite`).
pub(super) fn restart_rewritten(format: IndexFormat, strip_cut: u32) -> bool {
    match format {
        IndexFormat::U16 => strip_cut != STRIP_CUT_0XFFFF,
        IndexFormat::U32 => strip_cut == STRIP_CUT_0XFFFF,
        IndexFormat::U8 => false,
    }
}

fn adopt_buffers<B: Backend>(
    resources: &mut Slab<Resource<B>>,
    chain: &SwapChain<B>,
) -> Vec<ResourceId> {
    let tdesc = chain.buffer_desc();
    let desc = ResourceDesc {
        dimension: resource::DIMENSION_TEXTURE2D,
        width: u64::from(tdesc.width),
        height: tdesc.height,
        depth_or_array_size: 1,
        mip_levels: 1,
        format: chain.desc().format,
        sample_count: 1,
        flags: resource::FLAG_RENDER_TARGET
            | if tdesc.usage.contains(TextureUsage::STORAGE) {
                resource::FLAG_UNORDERED_ACCESS
            } else {
                0
            },
        ..Default::default()
    };
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(texture) = chain.buffer(i) {
        let id = resources.insert(Resource {
            desc,
            heap: HeapType::Default,
            storage: Storage::Texture {
                texture: texture.clone(),
                desc: tdesc,
                layers: Vec::new(),
                views: Vec::new(),
                owned: false,
            },
            address: 0,
            placed: None,
            inherits: Vec::new(),
            written: 0,
            synced: 0,
            twins: Vec::new(),
            twin: false,
            chroma: None,
        });
        out.push(ResourceId(id as u32));
        i += 1;
    }
    out
}

/// What a pass does with one aspect of its depth-stencil target: clears it or loads it, and keeps
/// it read only where the view says so and nothing clears it.
fn aspect_ops<V>(clear: Option<V>, read_only: bool) -> AspectOps<V> {
    AspectOps {
        read_only: read_only && clear.is_none(),
        load: clear.map_or(LoadOp::Load, LoadOp::Clear),
        store: StoreOp::Store,
    }
}

/// Whether a depth-stencil format has a depth and a stencil aspect.
fn aspects(format: wcontainers_gpu::Format) -> (bool, bool) {
    use wcontainers_gpu::Format::*;
    match format {
        Stencil8 => (false, true),
        Depth24PlusStencil8 | Depth32FloatStencil8 => (true, true),
        _ => (true, false),
    }
}

/// Whether the DXBC container `code` holds a part named `fourcc`.
fn container_has(code: &[u8], fourcc: [u8; 4]) -> bool {
    crate::dxbc::chunk(code, fourcc).is_some()
}

/// The interface format of a DXGI format a view or target names.
fn dxgi(value: u32) -> Result<wcontainers_gpu::Format, Error> {
    wcontainers_directx_graphics_infrastructure::dxgi_format(value)
        .map(|(f, _)| f)
        .ok_or(Error::Invalid("a format the front cannot store"))
}

/// Strides of tightly packed streams: each slot's elements end to end.
fn packed_strides(inputs: &[(InputElement, u32)]) -> [u32; MAX_VERTEX_STREAMS] {
    let mut strides = [0; MAX_VERTEX_STREAMS];
    for (e, _) in inputs {
        let end = e.offset + vertex_format(e.format).map_or(0, |f| f.1);
        let s = &mut strides[e.slot as usize];
        *s = (*s).max(end);
    }
    strides
}

impl From<GpuError> for Error {
    fn from(e: GpuError) -> Self {
        match e {
            GpuError::OutOfMemory => Error::OutOfMemory,
            GpuError::DeviceLost => Error::DeviceRemoved,
            GpuError::Absent(_) => Error::NotImplemented("a feature the back end lacks"),
            GpuError::Invalid(s) => Error::Invalid(s),
        }
    }
}

/// The state a command list's replay carries from command to command. A list starts with none of
/// the previous list's state, as Direct3D 12 defines.
struct Replay<B: Backend> {
    pipeline: Option<PipelineId>,
    topology: Topology,
    /// `D3D_PRIMITIVE_TOPOLOGY` as set, adjacency included, which only a geometry stage takes.
    api_topology: u32,
    vertex_buffers: [VertexBufferView; MAX_VERTEX_STREAMS],
    index: Option<crate::list::IndexBufferView>,
    viewports: Vec<Viewport>,
    scissors: Vec<ScissorRect>,
    blend_constant: [u32; 4],
    stencil_reference: u8,
    colors: Vec<Descriptor>,
    depth: Option<Descriptor>,
    pass_open: bool,
    /// The open pass has no targets.
    empty_pass: bool,
    /// The open pass, or the next one, holds its depth-stencil attachment read only, for draws
    /// running their early tests in the program.
    pass_early: bool,
    /// The open pass, or the next one, has no colour targets: the depth and stencil writes of a
    /// primitive drawn alone (`early::EarlyRun::Written`).
    pass_written: bool,
    /// The run of a primitive drawn alone the draw being made is (`early::Early::PerPrimitive`).
    early_run: Option<early::EarlyRun>,
    /// The layer of the bound target views the next pass draws into, past each view's first: a
    /// run of a draw split per layer (`LayerSplit`).
    pass_layer: u32,
    graphics: bind::RootState,
    compute: bind::RootState,
    /// Whole-view clears not yet done: each becomes the next pass's `Clear` load, or a clear of its
    /// own before anything else reads the texture.
    clears: Vec<(TargetView, ClearValue)>,
    /// The occlusion query begun and not ended: heap, index, and whether it counts in the open
    /// pass, or counted in a pass already ended.
    query: Option<(QueryHeapId, u32, QueryState)>,
    /// The heap whose query set the open pass counts in.
    pass_heap: Option<QueryHeapId>,
    /// The counted occlusion query begun and not ended: heap and index.
    counted: Option<(QueryHeapId, u32)>,
    /// The pipeline statistics query begun and not ended, and the stream output statistics
    /// queries begun and not ended.
    statistics: Option<statistics::Running>,
    /// Each: heap, index, stream, and the primitives written and needed so far.
    streams: Vec<(
        QueryHeapId,
        u32,
        u32,
        wcontainers_directx_graphics_infrastructure::d3d::StreamOutputStatistics,
    )>,
    /// The bound stream output buffers.
    stream_views: [crate::list::StreamView; 4],
    /// The pass tracker holds the bound targets, for the occlusion and pixel counters.
    counted_targets: bool,
    /// The occlusion counter has the depth-stencil target as the kept draws began from it.
    counted_copy: bool,
    predicate: Option<query::Predicate<B>>,
    /// Buffers of the front's that draws of this list read, released at its end, after the
    /// counters that may draw them again.
    scratch: Vec<B::Buffer>,
}

/// A predicated draw's or dispatch's argument record: buffer and offset.
type Record<B> = Option<(<B as Backend>::Buffer, u64)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueryState {
    Waiting,
    Running,
    Ended,
}

impl<B: Backend> Replay<B> {
    /// Gives the depth range a pixel program clamping depth reads (`DepthClampInShader`) to the
    /// pixel stage's bound helper values.
    fn bind_depth_range(&self, d: &mut Device<B>) {
        let depth_range = self
            .viewports
            .first()
            .map_or([0; 2], |v| [v.min_depth, v.max_depth]);
        d.bound_helpers.push((
            bind::PIXEL,
            HelperValue::DepthRange,
            pull::words(&depth_range),
        ));
    }

    fn new() -> Self {
        Replay {
            pipeline: None,
            topology: Topology::TriangleList,
            api_topology: 4,
            vertex_buffers: [VertexBufferView::default(); MAX_VERTEX_STREAMS],
            index: None,
            viewports: Vec::new(),
            // A list that never sets scissors draws unscissored, as real Direct3D 12 on the
            // reference GPU does (Direct3D 12 leaves it undefined); the back end clips the
            // rectangle to the pass.
            scissors: vec![ScissorRect {
                x: 0,
                y: 0,
                width: u32::MAX,
                height: u32::MAX,
            }],
            blend_constant: [0; 4],
            stencil_reference: 0,
            colors: Vec::new(),
            depth: None,
            pass_open: false,
            empty_pass: false,
            pass_early: false,
            pass_written: false,
            early_run: None,
            pass_layer: 0,
            graphics: bind::RootState::default(),
            compute: bind::RootState::default(),
            clears: Vec::new(),
            query: None,
            pass_heap: None,
            counted: None,
            statistics: None,
            streams: Vec::new(),
            stream_views: Default::default(),
            counted_targets: false,
            counted_copy: false,
            predicate: None,
            scratch: Vec::new(),
        }
    }

    /// Ends the open pass; while a counted occlusion query runs, the occlusion counter then counts
    /// the draws it kept, before anything changes what they read or drew on.
    fn end_pass(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        if self.pass_open {
            if let Some((_, _, state @ QueryState::Running)) = &mut self.query {
                d.backend.end_query();
                *state = QueryState::Ended;
            }
            d.backend.end_pass();
            self.pass_open = false;
            self.empty_pass = false;
            self.pass_heap = None;
        }
        if self.counted.is_some() {
            d.counter.flush(&mut d.backend, &mut d.passes)?;
            self.counted_copy = false;
        }
        self.counted_targets = false;
        Ok(())
    }

    /// Ends any pass and does the clears still waiting, so copies see them.
    fn settle(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        self.end_pass(d)?;
        let clears = std::mem::take(&mut self.clears);
        Self::clear_now(d, clears)
    }

    /// Before a pass opens, does the clears waiting for views it does not draw into, so its draws
    /// read them; the pass's own targets take theirs as it opens.
    fn settle_unbound(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        let (colors, depth) = (&self.colors, &self.depth);
        let bound = |v: &TargetView| {
            colors
                .iter()
                .any(|c| matches!(c, Descriptor::RenderTarget(t) if t == v))
                || matches!(depth, Some(Descriptor::DepthStencil { view, .. }) if view == v)
        };
        let others: Vec<_> = self.clears.extract_if(.., |(v, _)| !bound(v)).collect();
        Self::clear_now(d, others)
    }

    fn clear_now(d: &mut Device<B>, clears: Vec<(TargetView, ClearValue)>) -> Result<(), Error> {
        for (v, value) in clears {
            let (texture, desc) = d.texture(v.resource)?;
            let (texture, desc) = (texture.clone(), *desc);
            let layers = v.first_layer..v.first_layer.saturating_add(v.layers.max(1));
            if desc.samples > 1 && desc.depth_or_layers > 1 {
                // A multisampled array's layers are textures of their own.
                let one = TextureDesc {
                    depth_or_layers: 1,
                    ..desc
                };
                for layer in layers.start..layers.end.min(desc.depth_or_layers) {
                    let (texture, _) = d.texture_layer(v.resource, layer)?;
                    let texture = texture.clone();
                    d.backend.clear_texture(
                        &texture,
                        &one,
                        dxgi(v.format)?,
                        v.mip..v.mip + 1,
                        0..1,
                        value,
                    )?;
                }
                continue;
            }
            d.backend.clear_texture(
                &texture,
                &desc,
                dxgi(v.format)?,
                v.mip..v.mip + 1,
                layers,
                value,
            )?;
        }
        Ok(())
    }

    fn finish(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        if let Some(p) = self.predicate.take() {
            d.drop_predicate(p);
        }
        self.settle(d)?;
        // A counted query a list leaves running counts nothing more.
        if self.counted.take().is_some() {
            d.counter.end(&mut d.backend, &mut d.passes)?;
        }
        self.statistics = None;
        for b in self.scratch.drain(..) {
            d.backend.destroy(Object::Buffer(b));
        }
        Ok(())
    }

    /// Whether a predicate skips a clear, copy or resolve, which have no indirect form.
    fn skipped(&mut self, d: &mut Device<B>) -> Result<bool, Error> {
        if self.predicate.is_none() {
            return Ok(false);
        }
        self.end_pass(d)?;
        d.skips(self.predicate.as_mut().expect("a predicate"))
    }

    /// `SetPredication`, with the commands that follow it in the list.
    fn set_predicate(
        &mut self,
        d: &mut Device<B>,
        predicate: Option<(ResourceId, u64, u32)>,
        rest: &[Command],
    ) -> Result<(), Error> {
        self.settle(d)?;
        if let Some(p) = self.predicate.take() {
            d.drop_predicate(p);
        }
        if let Some((resource, offset, op)) = predicate {
            d.touched.clear();
            d.note(resource, false);
            d.alias_settle()?;
            let end = rest
                .iter()
                .position(|c| matches!(c, Command::Predication(_)))
                .unwrap_or(rest.len());
            self.predicate = Some(d.predicate(resource, offset, op, &rest[..end])?);
        }
        Ok(())
    }

    /// For a draw or dispatch under a predicate: its argument record, or whether it is skipped.
    /// None when it is skipped.
    fn predicated(&mut self, d: &mut Device<B>) -> Result<Option<Record<B>>, Error> {
        let Some(p) = &mut self.predicate else {
            return Ok(Some(None));
        };
        let record = Device::<B>::next_record(p);
        // A statistics query counts only the work that runs, so its predicate is read back.
        if self.statistics.is_none()
            && let Some(record) = record
        {
            return Ok(Some(Some(record)));
        }
        if p.value_unknown() {
            self.end_pass(d)?;
        }
        let p = self.predicate.as_mut().expect("a predicate");
        if d.skips(p)? {
            return Ok(None);
        }
        Ok(Some(None))
    }

    fn run(&mut self, d: &mut Device<B>, c: &Command) -> Result<(), Error> {
        // A bundle's commands run in the calling list's state, and what they set stays set.
        if let Command::Bundle(commands) = c {
            return commands.iter().try_for_each(|c| self.run(d, c));
        }
        d.touched.clear();
        if d.aliased != 0 {
            self.copies_due(d, c)?;
        }
        match c {
            // Run at the top.
            Command::Bundle(_) => {}
            Command::Pipeline(p) => {
                if !d.pipelines.contains(p.0 as usize) {
                    return Err(Error::Invalid(
                        "a pipeline state released before its list ran",
                    ));
                }
                self.pipeline = Some(*p);
            }
            Command::RootSignature { compute, root } => {
                self.state(*compute).set_root(root.clone());
            }
            Command::RootArgument {
                compute,
                index,
                argument,
            } => {
                *self
                    .state(*compute)
                    .arguments
                    .get_mut(*index as usize)
                    .ok_or(Error::Invalid("a root parameter index out of range"))? =
                    Some(*argument);
            }
            Command::RootConstants {
                compute,
                index,
                offset,
                values,
            } => {
                let at = *offset as usize;
                self.state(*compute)
                    .constants
                    .get_mut(*index as usize)
                    .and_then(|c| c.get_mut(at..at + values.len()))
                    .ok_or(Error::Invalid("root constants out of range"))?
                    .copy_from_slice(values);
            }
            Command::Topology(t) => {
                self.api_topology = *t;
                if let Some((topology, _)) = crate::state::adjacency_dropped(*t, 0, 0) {
                    self.topology = topology;
                } else if !crate::state::staged_only(*t) {
                    self.topology = crate::state::topology(*t)?;
                }
            }
            Command::VertexBuffers { start, views } => {
                let at = *start as usize;
                self.vertex_buffers
                    .get_mut(at..at + views.len())
                    .ok_or(Error::Invalid("vertex buffer slot out of range"))?
                    .copy_from_slice(views);
            }
            Command::IndexBuffer(v) => self.index = *v,
            Command::StreamTargets { start, views } => {
                let at = *start as usize;
                self.stream_views
                    .get_mut(at..at + views.len())
                    .ok_or(Error::Invalid("stream output slot out of range"))?
                    .copy_from_slice(views);
            }
            Command::Viewports(v) => {
                // A pass with no targets is as large as the viewports it began with.
                if self.empty_pass {
                    self.end_pass(d)?;
                }
                let fractional = d
                    .backend
                    .caps()
                    .features
                    .contains(Features::FRACTIONAL_VIEWPORT);
                self.viewports = v
                    .iter()
                    .map(|v| viewport(v, fractional))
                    .collect::<Result<_, _>>()?;
            }
            Command::Scissors(r) => self.scissors = r.iter().map(scissor).collect(),
            Command::BlendFactor(f) => self.blend_constant = f.map(f32::to_bits),
            Command::StencilRef(r) => self.stencil_reference = *r as u8,
            Command::Targets { colors, depth } => {
                if *colors != self.colors || *depth != self.depth {
                    self.end_pass(d)?;
                    self.colors.clone_from(colors);
                    self.depth = *depth;
                }
            }
            Command::ClearTarget { .. }
            | Command::ClearDepth { .. }
            | Command::CopyBuffer { .. }
            | Command::CopyTexture { .. }
            | Command::Resolve { .. }
            | Command::ClearStorage { .. }
                if self.skipped(d)? => {}
            Command::ClearTarget { view, color, rects } => {
                let Descriptor::RenderTarget(v) = view else {
                    return Err(Error::Invalid(
                        "clearing a descriptor that is no render target",
                    ));
                };
                let value = clear_bits(dxgi(v.format)?, *color);
                if rects.is_empty() {
                    // A later whole clear of the view replaces one still waiting.
                    self.end_pass(d)?;
                    self.clears.retain(|(k, _)| k != v);
                    self.clears.push((*v, ClearValue::Color(value)));
                } else {
                    // Clears still waiting come first, so the rectangles land on them.
                    self.settle(d)?;
                    let rects: Vec<_> = rects.iter().map(scissor).collect();
                    for view in d.slice_views(v)? {
                        d.backend.clear_rects(
                            &view,
                            dxgi(v.format)?,
                            ClearValue::Color(value),
                            &rects,
                        )?;
                    }
                }
            }
            Command::ClearDepth {
                view,
                depth,
                stencil,
                rects,
            } => {
                let Descriptor::DepthStencil { view: v, .. } = view else {
                    return Err(Error::Invalid(
                        "clearing a descriptor that is no depth-stencil view",
                    ));
                };
                let format = dxgi(v.format)?;
                let (has_depth, has_stencil) = aspects(format);
                let depth = depth.filter(|_| has_depth).map(f32::to_bits);
                let stencil = stencil.filter(|_| has_stencil);
                if rects.is_empty() {
                    // A whole clear joins one still waiting for the view: the newer aspects win.
                    self.end_pass(d)?;
                    let (mut old_depth, mut old_stencil) = (None, None);
                    if let Some(i) = self.clears.iter().position(|(k, _)| k == v)
                        && let (_, ClearValue::DepthStencil { depth, stencil }) =
                            self.clears.swap_remove(i)
                    {
                        (old_depth, old_stencil) = (depth, stencil);
                    }
                    self.clears.push((
                        *v,
                        ClearValue::DepthStencil {
                            depth: depth.or(old_depth),
                            stencil: stencil.or(old_stencil),
                        },
                    ));
                } else {
                    self.settle(d)?;
                    let rects: Vec<_> = rects.iter().map(scissor).collect();
                    for view in d.slice_views(v)? {
                        d.backend.clear_rects(
                            &view,
                            format,
                            ClearValue::DepthStencil { depth, stencil },
                            &rects,
                        )?;
                    }
                }
            }
            Command::Draw {
                vertices,
                instances,
                first_vertex,
                first_instance,
            } => {
                if let Some(record) = self.predicated(d)? {
                    let range = DrawRange::Vertices {
                        first: *first_vertex,
                        count: *vertices,
                        first_instance: *first_instance,
                        instances: *instances,
                    };
                    self.draw(d, range, record.as_ref())?;
                }
            }
            Command::DrawIndexed {
                indices,
                instances,
                first_index,
                base_vertex,
                first_instance,
            } => {
                if let Some(record) = self.predicated(d)? {
                    let range = DrawRange::Indexed {
                        first: *first_index,
                        count: *indices,
                        base_vertex: *base_vertex,
                        first_instance: *first_instance,
                        instances: *instances,
                    };
                    self.draw(d, range, record.as_ref())?;
                }
            }
            Command::CopyBuffer {
                dst,
                dst_offset,
                src,
                src_offset,
                size,
            } => {
                self.settle(d)?;
                let (s, _) = d.buffer(*src)?;
                let s = s.clone();
                let (t, heap) = d.buffer(*dst)?;
                let t = t.clone();
                d.backend
                    .copy_buffer(&s, *src_offset, &t, *dst_offset, *size);
                if heap.reads_back() {
                    d.read_into(*dst, *dst_offset, *size)?;
                }
            }
            Command::CopyTexture { dst, at, src, area } => {
                self.settle(d)?;
                copy_texture(d, dst, *at, src, *area)?;
            }
            Command::Resolve {
                dst,
                dst_index,
                src,
                src_index,
                format,
            } => {
                self.settle(d)?;
                d.resolve_subresource(*dst, *dst_index, *src, *src_index, *format)?;
            }
            Command::WriteImmediate { address, value } => {
                self.settle(d)?;
                d.write_immediate(*address, *value)?;
            }
            Command::Dispatch(groups) => {
                if let Some(record) = self.predicated(d)? {
                    self.dispatch(d, *groups, record.as_ref())?;
                }
            }
            Command::BeginQuery { heap, kind, index } => {
                self.begin_query(d, *heap, *kind, *index)?
            }
            Command::EndQuery { heap, kind, index } => self.end_query(d, *heap, *kind, *index)?,
            Command::ResolveQueries {
                heap,
                kind,
                first,
                count,
                dst,
                offset,
            } => {
                self.settle(d)?;
                d.resolve_query_data(*heap, *kind, *first, *count, *dst, *offset)?;
            }
            Command::Predication(_) => {
                return Err(Error::Invalid("a predicate outside a list's replay"));
            }
            Command::ExecuteIndirect {
                signature,
                max,
                args,
                args_offset,
                count,
            } => self.execute_indirect(d, signature, *max, *args, *args_offset, *count)?,
            Command::ClearStorage {
                view,
                values,
                float,
                rects,
            } => {
                self.settle(d)?;
                d.clear_storage(view, *values, *float, rects)?;
            }
        }
        Ok(())
    }

    /// Notes what copies, clears and resolves use, and copies inherited bytes into them first.
    fn copies_due(&mut self, d: &mut Device<B>, c: &Command) -> Result<(), Error> {
        let location = |d: &Device<B>, l: &CopyLocation| match *l {
            CopyLocation::Subresource { resource, index } => d.plane_holder(resource, index).0,
            CopyLocation::Footprint { resource, .. } => resource,
        };
        match c {
            Command::CopyBuffer { dst, src, .. } => {
                d.note(*src, false);
                d.note(*dst, true);
            }
            Command::CopyTexture { dst, src, .. } => {
                d.note(location(d, src), false);
                d.note(location(d, dst), true);
            }
            Command::Resolve { dst, src, .. } => {
                d.note(*src, false);
                d.note(*dst, true);
            }
            Command::ClearTarget {
                view: Descriptor::RenderTarget(v),
                ..
            }
            | Command::ClearDepth {
                view: Descriptor::DepthStencil { view: v, .. },
                ..
            } => d.note(v.resource, true),
            Command::ClearStorage { view, .. } => d.note(view.resource, true),
            Command::WriteImmediate { address, .. } => {
                if let Some((id, _)) = d.addresses.find(*address) {
                    d.note(id, true);
                }
            }
            Command::ResolveQueries { dst, .. } => d.note(*dst, true),
            Command::ExecuteIndirect { args, count, .. } => {
                d.note(*args, false);
                if let Some((c, _)) = count {
                    d.note(*c, false);
                }
            }
            _ => return Ok(()),
        }
        if d.alias_due() {
            self.settle(d)?;
        }
        d.alias_settle()
    }

    /// Notes what a draw uses besides its bindings: its targets, vertex and index buffers.
    fn note_draw(&self, d: &mut Device<B>, indexed: bool) {
        for c in &self.colors {
            if let Descriptor::RenderTarget(v) = c {
                d.note(v.resource, true);
            }
        }
        if let Some(Descriptor::DepthStencil { view, read_only }) = self.depth {
            d.note(view.resource, read_only != 3);
        }
        for v in self.vertex_buffers.iter().filter(|v| v.size > 0) {
            if let Some((id, _)) = d.addresses.find(v.address) {
                d.note(id, false);
            }
        }
        if indexed && let Some((id, _)) = self.index.and_then(|v| d.addresses.find(v.address)) {
            d.note(id, false);
        }
    }

    fn state(&mut self, compute: bool) -> &mut bind::RootState {
        if compute {
            &mut self.compute
        } else {
            &mut self.graphics
        }
    }

    fn begin_query(
        &mut self,
        d: &mut Device<B>,
        heap: QueryHeapId,
        kind: u32,
        index: u32,
    ) -> Result<(), Error> {
        match kind {
            query::QUERY_PIPELINE_STATISTICS => return self.begin_statistics(d, heap, index),
            k if query::QUERY_SO_STATISTICS.contains(&k) => {
                return self.begin_stream(d, heap, index, k - query::QUERY_SO_STATISTICS.start());
            }
            query::QUERY_BINARY_OCCLUSION | query::QUERY_OCCLUSION => {}
            _ => return Err(Error::Invalid("BeginQuery of this query type")),
        }
        d.occlusion_set(heap)?;
        if self.query.is_some() || self.counted.is_some() {
            return Err(Error::Invalid("an occlusion query begun inside another"));
        }
        if kind == query::QUERY_OCCLUSION {
            // Exact counts come from the occlusion counter, from the targets as they are now.
            self.end_pass(d)?;
            d.counter.begin(&mut d.backend, &mut d.passes)?;
            self.counted = Some((heap, index));
            return Ok(());
        }
        if self.pass_open && self.pass_heap == Some(heap) {
            d.backend.begin_query(index);
            self.query = Some((heap, index, QueryState::Running));
        } else {
            // The next pass counts in this heap's query set.
            self.end_pass(d)?;
            self.query = Some((heap, index, QueryState::Waiting));
        }
        Ok(())
    }

    fn end_query(
        &mut self,
        d: &mut Device<B>,
        heap: QueryHeapId,
        kind: u32,
        index: u32,
    ) -> Result<(), Error> {
        match kind {
            query::QUERY_TIMESTAMP => {
                self.end_pass(d)?;
                return d.timestamp(heap, index);
            }
            query::QUERY_PIPELINE_STATISTICS => return self.end_statistics(d, heap, index),
            k if query::QUERY_SO_STATISTICS.contains(&k) => {
                return self.end_stream(d, heap, index);
            }
            _ => {}
        }
        if kind == query::QUERY_OCCLUSION {
            if self.counted != Some((heap, index)) {
                return Err(Error::Invalid("EndQuery of a query not begun"));
            }
            self.end_pass(d)?;
            self.counted = None;
            let (values, at) = d.value_at(heap, index, 0)?;
            d.counter
                .end_into(&mut d.backend, &mut d.passes, &values, at)?;
            return Ok(());
        }
        match self.query.take() {
            Some((h, i, state)) if (h, i) == (heap, index) => match state {
                QueryState::Running => d.backend.end_query(),
                QueryState::Ended => {}
                QueryState::Waiting => {
                    // No pass drew while it ran: an empty pass writes its zero.
                    self.end_pass(d)?;
                    let set = d.occlusion_set(heap)?;
                    d.backend.begin_pass(&PassDesc {
                        colors: Default::default(),
                        depth: None,
                        empty: Some(wcontainers_gpu::EmptyPass {
                            width: 1,
                            height: 1,
                            samples: 1,
                        }),
                        occlusion: Some(&set),
                    });
                    d.backend.begin_query(index);
                    d.backend.end_query();
                    d.backend.end_pass();
                }
            },
            _ => return Err(Error::Invalid("EndQuery of a query not begun")),
        }
        Ok(())
    }

    fn dispatch(
        &mut self,
        d: &mut Device<B>,
        groups: [u32; 3],
        record: Option<&(B::Buffer, u64)>,
    ) -> Result<(), Error> {
        let Some(pid) = self.pipeline else {
            d.messages.push(NO_PIPELINE_SKIPPED.into());
            return Ok(());
        };
        if d.pipelines[pid.0 as usize].render.is_some() {
            return Err(Error::Invalid("dispatching with a graphics pipeline state"));
        }
        if groups.contains(&0) || !d.arguments_set(&self.compute, pid) {
            return Ok(());
        }
        if let Some(s) = &mut self.statistics {
            // Indirect dispatches are read back while a statistics query runs.
            if record.is_none() {
                let threads = groups.iter().map(|&g| u64::from(g)).product::<u64>()
                    * d.pipelines[pid.0 as usize].group_threads;
                s.values.cs_invocations += threads;
            }
        }
        self.settle(d)?;
        d.pipeline_slow_paths(pid);
        let (mut sets, bound) = d.binding_sets(
            &self.compute,
            pid,
            &[],
            &[],
            StageSet::COMPUTE,
            Some(formats::VertexKind::Own),
        )?;
        let pipeline = &d.pipelines[pid.0 as usize];
        let helpers = bound.as_ref().map_or(&pipeline.helpers, |b| &b.1).clone();
        if helpers.size != 0 {
            let block = d.stage_block(&helpers, 0)?;
            let merged = d.merged_samplers(&helpers, 0, StageSet::COMPUTE)?;
            let compute = wcontainers_gpu::helper_binding(ShaderStage::Compute);
            sets[HELPER_SET as usize] = Some(d.helper_blocks(
                &[(compute, StageSet::COMPUTE, block)],
                &[],
                &merged,
                Vec::new(),
            )?);
        }
        d.alias_settle()?;
        if !d.through.is_empty() {
            self.end_pass(d)?;
            d.through_in()?;
        }
        let call = DispatchCall {
            program: bound
                .as_ref()
                .map_or(&d.pipelines[pid.0 as usize].program, |b| &b.0),
            bindings: sets.each_ref().map(Option::as_ref),
            immediates: &[],
            groups: match record {
                Some((args, offset)) => DispatchGroups::Indirect {
                    args,
                    offset: *offset,
                },
                None => DispatchGroups::Direct(groups),
            },
        };
        d.backend.dispatch(&call)?;
        d.through_out(&mut self.scratch)?;
        Ok(())
    }

    /// A draw; with `record`, its arguments are read from that argument record instead.
    fn draw(
        &mut self,
        d: &mut Device<B>,
        mut range: DrawRange<'_, B>,
        record: Option<&(B::Buffer, u64)>,
    ) -> Result<(), Error> {
        let Some(pid) = self.pipeline else {
            d.messages.push(NO_PIPELINE_SKIPPED.into());
            return Ok(());
        };
        let r = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        let geometry = r.geometry.is_some();
        if !d.arguments_set(&self.graphics, pid) {
            return Ok(());
        }
        // The back ends test depth and stencil after the pixel shader: forced early tests that
        // matter run in the program, in a pass reading the depth-stencil view only.
        let early = match (self.early_tests(d, pid)?, self.early_run) {
            (early::Early::PerPrimitive, None) => {
                return self.draw_per_primitive(d, pid, range, record);
            }
            (early::Early::PerPrimitive, Some(run)) => {
                matches!(run, early::EarlyRun::Tested { .. })
            }
            (e, _) => e == early::Early::InProgram,
        };
        let written = matches!(self.early_run, Some(early::EarlyRun::Written { .. }));
        if self.pass_open && (self.pass_early != early || self.pass_written != written) {
            self.end_pass(d)?;
        }
        self.pass_early = early;
        self.pass_written = written;
        if early {
            d.backend.slow_paths().hit(SlowPath::EarlyTestsInShader);
        }
        d.pipeline_slow_paths(pid);
        if geometry && !self.draw_geometry(d, pid, same_range(&range), record)? {
            return Ok(());
        }
        // Without a geometry stage Direct3D ignores adjacent vertices: the list or strip of the
        // others is drawn from generated indices (`AdjacencyDropped`).
        let mut adjacency = None;
        if crate::state::staged_only(self.api_topology) {
            let DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } = range
            else {
                return Err(Error::NotImplemented(
                    "indexed or indirect adjacency draws without a geometry shader",
                ));
            };
            let (_, kept) = crate::state::adjacency_dropped(self.api_topology, first, count)
                .ok_or(Error::NotImplemented(
                    "patch topologies without a hull shader",
                ))?;
            d.backend.slow_paths().hit(SlowPath::AdjacencyDropped);
            if kept.is_empty() {
                return Ok(());
            }
            let index_size = size_of::<u32>() as u32;
            let bytes: Vec<u8> = kept.iter().flat_map(|i| i.to_le_bytes()).collect();
            let mut at = [0];
            let ring = d.backend.write_buffer_ring(
                wcontainers_gpu::RingKind::Indices,
                &[&bytes],
                index_size,
                &mut at,
            )?;
            range = DrawRange::Indexed {
                first: (at[0] / u64::from(index_size)) as u32,
                count: kept.len() as u32,
                base_vertex: 0,
                first_instance,
                instances,
            };
            adjacency = Some(ring);
        }
        // The bases as `HelperValue::DrawBase` holds them: base vertex, first instance.
        let bases = match &range {
            DrawRange::Vertices {
                first,
                first_instance,
                ..
            } => Some([*first, *first_instance]),
            DrawRange::Indexed {
                base_vertex,
                first_instance,
                ..
            } => Some([*base_vertex as u32, *first_instance]),
            _ => None,
        };
        let render = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        // An `ExecuteIndirect` draw of a program reading the bases reads them from its arguments.
        let by_arguments = render.reads_bases && bases.is_none();
        if let (Some(_), DrawRange::Vertices { first_instance, .. })
        | (Some(_), DrawRange::Indexed { first_instance, .. }) = (record, &range)
            && *first_instance != 0
            && !d
                .backend
                .caps()
                .features
                .contains(Features::INDIRECT_FIRST_INSTANCE)
        {
            return Err(Error::NotImplemented(
                "a predicated draw with a first instance on this back end",
            ));
        }
        let first_instance = match &range {
            DrawRange::Vertices { first_instance, .. }
            | DrawRange::Indexed { first_instance, .. } => Some(*first_instance),
            _ => None,
        };
        // An `ExecuteIndirect` draw without indices, whose vertex count the front does not know.
        let unknown_count = matches!(range, DrawRange::Indirect { .. });
        let range = match (record, range) {
            (Some((args, offset)), DrawRange::Indexed { .. }) => DrawRange::IndexedIndirect {
                args,
                offset: *offset,
            },
            (Some((args, offset)), _) => DrawRange::Indirect {
                args,
                offset: *offset,
            },
            (None, range) => range,
        };
        // Inputs the back end cannot fetch as laid out are fetched in the program.
        let inputs = std::mem::take(&mut d.render_mut(pid).inputs);
        let mut strides = [0; MAX_VERTEX_STREAMS];
        for (s, v) in strides.iter_mut().zip(&self.vertex_buffers) {
            *s = v.stride;
        }
        let pulled = pull::pulled_streams(&inputs, &strides, d.backend.caps());
        let fetch = d.fetch(&inputs, &self.vertex_buffers, &pulled, first_instance);
        let mut used = [false; MAX_VERTEX_STREAMS];
        // Streams that never step, bound at the first instance's element (see `vertex_layout`).
        let mut fixed = [false; MAX_VERTEX_STREAMS];
        let rates = d
            .backend
            .caps()
            .features
            .contains(Features::INSTANCE_STEP_RATE);
        // Each stream's elements this draw reads, and where in an element its inputs end.
        let mut reads = [None; MAX_VERTEX_STREAMS];
        let mut ends = [0u64; MAX_VERTEX_STREAMS];
        for (e, _) in &inputs {
            let slot = e.slot as usize;
            used[slot] |= pulled[slot].is_none();
            fixed[slot] |= e.per_instance && e.step_rate == 0 && !rates && pulled[slot].is_none();
            let size = vertex_format(e.format).map_or(0, |f| f.1);
            ends[slot] = ends[slot].max(u64::from(e.offset) + u64::from(size));
            reads[slot] = match (&range, e.per_instance) {
                (DrawRange::Vertices { first, count, .. }, false) if *count > 0 => {
                    Some(u64::from(*first) + u64::from(*count))
                }
                (
                    DrawRange::Vertices {
                        first_instance,
                        instances,
                        ..
                    }
                    | DrawRange::Indexed {
                        first_instance,
                        instances,
                        ..
                    },
                    true,
                ) if *instances > 0 && e.step_rate > 0 => Some(
                    u64::from(*first_instance)
                        + u64::from(*instances - 1) / u64::from(e.step_rate)
                        + 1,
                ),
                _ => reads[slot],
            };
        }
        let fetches_vertices = inputs
            .iter()
            .any(|(e, _)| !e.per_instance && pulled[e.slot as usize].is_none());
        d.render_mut(pid).inputs = inputs;
        let fetch = fetch?;
        for why in pulled.iter().flatten() {
            d.backend.slow_paths().hit(*why);
        }
        if unknown_count && fetches_vertices {
            d.backend.slow_paths().hit(SlowPath::IndirectFetchPastEnd);
        }
        let variant = if fetch.inputs.is_empty() {
            None
        } else if self.counted.is_some() || self.statistics.is_some() {
            return Err(Error::NotImplemented(
                "a counted draw of vertex inputs fetched in the program",
            ));
        } else {
            let key: PulledKey = (
                fetch.inputs.iter().map(|i| (i.0, i.1)).collect(),
                by_arguments,
            );
            Some(d.pulled_program(pid, key)?)
        };
        let counted = match self.counted {
            Some(_) => Some(self.prepare_counted(d, pid)?),
            None => None,
        };
        let pixel_counting = match self.statistics {
            Some(_) => self.count_draw(d, pid, &range, variant.is_some())?,
            None => None,
        };
        if !self.pass_open {
            self.settle_unbound(d)?;
        }
        let r = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .expect("a graphics pipeline");
        let program: Option<&VertexProgram<B>> = match (variant, by_arguments) {
            (Some(i), _) => r.pulled[i].1.as_ref(),
            (None, true) => Some(
                r.arguments
                    .as_ref()
                    .ok_or(Error::Invalid("an indirect draw reading its bases"))?,
            ),
            (None, false) => None,
        };
        let mut helpers = program.map_or(&r.helpers, |p| &p.helpers).clone();
        let streams = program.map_or(Vec::new(), |p| p.streams.clone());
        let kind = match (variant, by_arguments) {
            (Some(i), _) => formats::VertexKind::Pulled(r.pulled[i].0.clone()),
            (None, true) => formats::VertexKind::Arguments,
            (None, false) => formats::VertexKind::Own,
        };
        let mut extra = Vec::with_capacity(streams.len());
        for &(binding, location) in &streams {
            let slot = r
                .inputs
                .iter()
                .find(|(_, l)| *l == location)
                .ok_or(Error::Invalid("a pulled input the pipeline lacks"))?
                .0
                .slot as usize;
            let (buffer, offset, size) = fetch.ranges[slot]
                .as_ref()
                .ok_or(Error::Invalid("a pulled stream the draw does not bind"))?;
            extra.push((binding, buffer, *offset, *size));
        }
        d.early_draw = early;
        let bound = d.binding_sets(
            &self.graphics,
            pid,
            &[],
            &extra,
            StageSet::VERTEX,
            Some(kind),
        );
        d.early_draw = false;
        let (mut sets, bound) = bound?;
        d.vertex_formats(pid)?;
        self.bind_depth_range(d);
        let early_views = match &self.depth {
            Some(Descriptor::DepthStencil { view, .. }) if early => {
                let words = self.early_words(d, pid)?;
                d.bound_helpers
                    .push((bind::PIXEL, HelperValue::EarlyTests, words));
                Some(d.early_views(view)?)
            }
            _ => None,
        };
        // A vertex program converted for what is bound reads its own helper block.
        if let Some((_, _, Some((vertex, pulled)))) = &bound {
            if self.counted.is_some() || self.statistics.is_some() {
                return Err(Error::NotImplemented(
                    "a counted draw of a vertex program converted for the bound views",
                ));
            }
            if *pulled != streams {
                return Err(Error::Invalid(
                    "a vertex program's pulled streams moved with the bound views",
                ));
            }
            helpers = vertex.clone();
        }
        let pipeline = &d.pipelines[pid.0 as usize];
        let pixel = bound.as_ref().map_or(&pipeline.helpers, |b| &b.1).clone();
        let counter = match &pixel_counting {
            Some(_) => Some(d.pixel_counter()?),
            None => None,
        };
        if helpers.size != 0 || pixel.size != 0 || counter.is_some() || early_views.is_some() {
            let mut entries: Vec<_> = counter.iter().map(statistics::counter_entry).collect();
            entries.extend(early_views.iter().flat_map(early::entries));
            let source = match (bases, &range) {
                (Some(b), _) => pull::Bases::Direct(b),
                (None, DrawRange::Indirect { args, offset }) => pull::Bases::Arguments {
                    args: *args,
                    offset: *offset,
                    indexed: false,
                },
                (None, DrawRange::IndexedIndirect { args, offset }) => pull::Bases::Arguments {
                    args: *args,
                    offset: *offset,
                    indexed: true,
                },
                _ => unreachable!("a draw without bases is indirect"),
            };
            sets[HELPER_SET as usize] = Some(d.helper_set(
                &helpers,
                &source,
                &fetch.inputs,
                StageSet::VERTEX,
                Some(&pixel),
                entries,
            )?);
        }
        if d.aliased != 0 {
            self.note_draw(
                d,
                matches!(
                    range,
                    DrawRange::Indexed { .. } | DrawRange::IndexedIndirect { .. }
                ),
            );
            if d.alias_due() {
                self.end_pass(d)?;
            }
            d.alias_settle()?;
        }
        if !d.through.is_empty() {
            if self.counted.is_some() || self.statistics.is_some() {
                return Err(Error::NotImplemented(
                    "a counted draw reading storage textures through buffers",
                ));
            }
            self.end_pass(d)?;
            d.through_in()?;
        }
        if let Some((heap, _, QueryState::Ended)) = self.query
            && !self.pass_open
            && self.pass_heap != Some(heap)
        {
            return Err(Error::NotImplemented("an occlusion query across passes"));
        }
        let layout = fetch.layout;
        let mut buffers: [Option<(B::Buffer, u64)>; MAX_VERTEX_STREAMS] =
            [const { None }; MAX_VERTEX_STREAMS];
        // Streams a direct draw reads past their view's end: (slot, resource, buffer, offset,
        // bytes the view holds, bytes the draw reads).
        let mut short = Vec::new();
        for (slot, (((b, v), used), fixed)) in buffers
            .iter_mut()
            .zip(&self.vertex_buffers)
            .zip(used)
            .zip(fixed)
            .enumerate()
        {
            if let Some((id, mut offset)) = d.addresses.find(v.address)
                && used
                && v.size > 0
            {
                if fixed {
                    let first = first_instance.ok_or(Error::NotImplemented(
                        "a per-instance stream that never steps in an indirect draw",
                    ))?;
                    offset += u64::from(first) * u64::from(v.stride);
                }
                let buffer = d.buffer(id)?.0.clone();
                if let Some(n) = reads[slot].filter(|_| !fixed) {
                    let needed = (n - 1) * u64::from(v.stride) + ends[slot];
                    if needed > u64::from(v.size) {
                        let held = u64::from(v.size)
                            .min(d.resources[id.0 as usize].desc.width.saturating_sub(offset));
                        short.push((slot, id, buffer.clone(), offset, held, needed));
                    }
                }
                *b = Some((buffer, offset));
            }
        }
        // Direct3D reads zeros past a vertex buffer view's end, where WebGPU refuses the draw:
        // such a stream is drawn from a copy with a zero tail (`VertexFetchZeroed`).
        let mut zeroed = Vec::new();
        if !short.is_empty() {
            self.end_pass(d)?;
            for (slot, id, buffer, offset, held, needed) in short {
                d.backend.slow_paths().hit(SlowPath::VertexFetchZeroed);
                if !offset.is_multiple_of(4) {
                    return Err(Error::NotImplemented(
                        "a vertex buffer view read past its end at an address not a multiple of 4",
                    ));
                }
                let copy = d.scratch(needed, BufferUsage::VERTEX | BufferUsage::COPY_DST)?;
                let bytes = held - held % 4;
                d.upload(id)?;
                if bytes > 0 {
                    d.backend.copy_buffer(&buffer, offset, &copy, 0, bytes);
                }
                // The bytes of an element cut by the view's end that a whole word did not take.
                if bytes < held {
                    let tail = d.read_now(id, offset + bytes, held - bytes)?;
                    let mut word = [0u8; 4];
                    word[..tail.len()].copy_from_slice(&tail);
                    d.backend
                        .write_buffer(&copy, bytes, &word, WriteMode::Ordered);
                }
                buffers[slot] = Some((copy.clone(), 0));
                zeroed.push(copy);
            }
        }
        let vertex_buffers = buffers.each_ref().map(|b| {
            b.as_ref().map(|(buffer, offset)| VertexBinding {
                buffer,
                offset: *offset,
            })
        });
        let index = match (&range, &self.index) {
            _ if adjacency.is_some() => adjacency
                .take()
                .map(|ring| (ring, 0, IndexFormat::U32, None)),
            (DrawRange::Indexed { .. } | DrawRange::IndexedIndirect { .. }, Some(v)) => {
                let (id, offset) = d
                    .addresses
                    .find(v.address)
                    .ok_or(Error::Invalid("an index buffer view of no buffer"))?;
                let format = index_format(v.format).ok_or(Error::Invalid("index format"))?;
                Some((d.buffer(id)?.0.clone(), offset, format, Some(id)))
            }
            (DrawRange::Indexed { .. } | DrawRange::IndexedIndirect { .. }, None) => {
                return Err(Error::Invalid("an indexed draw with no index buffer"));
            }
            _ => None,
        };
        // WebGPU restarts strips at the index format's largest value, Direct3D 12 at the
        // pipeline's cut value: 16-bit indices that must not restart are widened, and 32-bit ones
        // restarting at 0xffff rewritten, into 32-bit indices of the draw's range
        // (`RestartRewrite`). 32-bit indices of 0xffffffff with restart off still restart.
        let strip_cut = d.render_mut(pid).strip_cut;
        let strips = matches!(self.topology, Topology::LineStrip | Topology::TriangleStrip);
        let rewrite = match &index {
            Some((.., format, Some(_))) => strips && restart_rewritten(*format, strip_cut),
            Some(_) | None => false,
        };
        let (index, range) = match (index, range) {
            (
                Some((buffer, offset, format, Some(id))),
                DrawRange::Indexed {
                    first,
                    count,
                    base_vertex,
                    first_instance,
                    instances,
                },
            ) if rewrite && count > 0 => {
                self.end_pass(d)?;
                d.backend.slow_paths().hit(SlowPath::RestartRewrite);
                // 16-bit indices are widened and keep every value; 32-bit ones restart at the
                // largest 16-bit value.
                let wide = format == IndexFormat::U32;
                let cut = if wide {
                    INDEX_FORMATS[IndexFormat::U16 as usize].restart_value
                } else {
                    0
                };
                let size = u64::from(INDEX_FORMATS[format as usize].size);
                let out_index = u64::from(INDEX_FORMATS[IndexFormat::U32 as usize].size);
                if !offset.is_multiple_of(size) {
                    return Err(Error::Invalid(
                        "an index buffer view not aligned to its format",
                    ));
                }
                let held = d.resources[id.0 as usize]
                    .desc
                    .width
                    .next_multiple_of(STORAGE_WORD);
                let out = d.scratch(
                    u64::from(count) * out_index,
                    BufferUsage::STORAGE | BufferUsage::INDEX,
                )?;
                let start = u32::try_from(offset / size)
                    .ok()
                    .and_then(|s| s.checked_add(first))
                    .ok_or(Error::Invalid("an index past 4 GiB"))?;
                d.run_own(
                    query::Own::Restart,
                    &[start, count, u32::from(wide), cut],
                    &[
                        (query::OWN_SOURCE, &buffer, held, false),
                        (query::OWN_RESULT, &out, u64::from(count) * out_index, true),
                    ],
                    count,
                )?;
                zeroed.push(out.clone());
                (
                    Some((out, 0, IndexFormat::U32)),
                    DrawRange::Indexed {
                        first: 0,
                        count,
                        base_vertex,
                        first_instance,
                        instances,
                    },
                )
            }
            (Some(_), DrawRange::IndexedIndirect { .. }) if rewrite => {
                return Err(Error::NotImplemented(
                    "a predicated strip draw whose indices restart at another value",
                ));
            }
            (index, range) => (index.map(|(b, o, f, _)| (b, o, f)), range),
        };
        let bound_depth = self.check_targets(d, pid)?;
        let restart = d.render_mut(pid).strip_cut != 0 || rewrite;
        // The draw's call: built once for the occlusion counter, which keeps a counted draw before
        // its pass opens, and once for the back end.
        macro_rules! call {
            () => {{
                let p = &d.pipelines[pid.0 as usize];
                let r = p.render.as_ref().expect("a graphics pipeline");
                DrawCall {
                    program: match (&bound, variant, &r.arguments, by_arguments) {
                        (Some(b), ..) => &b.0,
                        (None, Some(i), ..) => {
                            &r.pulled[i].1.as_ref().expect("a made program").program
                        }
                        (None, None, Some(a), true) => &a.program,
                        _ => &p.program,
                    },
                    blend: &r.blend,
                    depth_stencil: match (&r.depth_stencil_bound, bound_depth) {
                        (Some(d), Some(_)) => d,
                        _ => &r.depth_stencil,
                    },
                    raster: &r.raster,
                    vertex_layout: &layout,
                    bindings: sets.each_ref().map(Option::as_ref),
                    vertex_buffers: &vertex_buffers,
                    index: index.as_ref().map(|(buffer, offset, format)| IndexBinding {
                        buffer,
                        offset: *offset,
                        format: *format,
                    }),
                    topology: self.topology,
                    restart,
                    // With no stage choosing a viewport (a geometry stage's draw goes by
                    // `draw_runs`, and `SV_ViewportArrayIndex` from the vertex stage is reported
                    // absent), Direct3D 12 draws through the first viewport and scissor.
                    viewports: &self.viewports[..self.viewports.len().min(1)],
                    scissors: &self.scissors[..self.scissors.len().min(1)],
                    blend_constant: self.blend_constant,
                    stencil_reference: [self.stencil_reference; 2],
                    immediates: &[],
                    range: same_range(&range),
                }
            }};
        }
        if let Some((writes, counting)) = &counted {
            let r = d.pipelines[pid.0 as usize]
                .render
                .as_ref()
                .expect("a graphics pipeline");
            let counted = CountedDraw {
                counting,
                alpha_to_coverage: r.alpha_to_coverage,
                writes_depth_stencil: *writes,
                instances_step_by_one: r.instances_step_by_one,
            };
            d.counter
                .draw(&mut d.backend, &mut d.passes, &call!(), &counted)?;
        }
        if !self.pass_open {
            self.open_pass(d, pid)?;
        }
        // The run making a primitive's depth and stencil writes draws the pipeline's own vertex
        // program.
        if matches!(self.early_run, Some(early::EarlyRun::Written { .. }))
            && (variant.is_some() || by_arguments || matches!(&bound, Some((_, _, Some(_)))))
        {
            return Err(Error::NotImplemented(
                "early depth-stencil tests per primitive of a vertex program converted for the \
                 draw",
            ));
        }
        match self.early_run {
            Some(run) => {
                let r = d.pipelines[pid.0 as usize]
                    .render
                    .as_ref()
                    .expect("a graphics pipeline");
                let each = r
                    .per_primitive
                    .as_ref()
                    .expect("checked by draw_per_primitive");
                let base = call!();
                d.backend.draw(&DrawCall {
                    program: match run {
                        early::EarlyRun::Written { .. } => &each.written,
                        early::EarlyRun::Tested { .. } => base.program,
                    },
                    depth_stencil: match run {
                        early::EarlyRun::Tested { .. } => &each.tested,
                        early::EarlyRun::Written { .. } => base.depth_stencil,
                    },
                    raster: match run.odd() {
                        true => &each.flipped,
                        false => base.raster,
                    },
                    ..base
                });
            }
            None => d.backend.draw(&call!()),
        }
        if let Some((counting, off, none)) = &pixel_counting {
            d.backend.draw(&DrawCall {
                program: counting,
                blend: none,
                depth_stencil: off,
                ..call!()
            });
        }
        self.scratch.extend(zeroed);
        if !d.through.is_empty() {
            self.end_pass(d)?;
            d.through_out(&mut self.scratch)?;
        }
        Ok(())
    }

    /// Checks the bound targets' formats against the pipeline's, and gives the bound depth
    /// format.
    fn check_targets(&self, d: &mut Device<B>, pid: PipelineId) -> Result<Option<Format>, Error> {
        let p = d.render_mut(pid);
        for (i, slot) in p.targets.colors.iter().enumerate() {
            let bound = match self.colors.get(i) {
                Some(Descriptor::RenderTarget(v)) => Some(dxgi(v.format)?),
                _ => None,
            };
            if *slot != bound {
                return Err(Error::Invalid(
                    "the pipeline's target formats differ from the bound targets",
                ));
            }
        }
        let bound_depth = match &self.depth {
            Some(Descriptor::DepthStencil { view, .. }) => Some(dxgi(view.format)?),
            _ => None,
        };
        if p.targets.depth_stencil.is_some() && p.targets.depth_stencil != bound_depth {
            return Err(Error::Invalid(
                "the pipeline's depth-stencil format differs from the bound one",
            ));
        }
        if p.clamped_without_target && bound_depth.is_some() {
            return Err(Error::NotImplemented(CLAMP_BOUND_DEPTH));
        }
        Ok(bound_depth)
    }

    /// A pass with no targets covers the viewports, at the pipeline's sample count.
    fn empty_size(&self, d: &Device<B>, pid: PipelineId) -> EmptyPass {
        let end = |at: u32, size: u32| (f32::from_bits(at) + f32::from_bits(size)).ceil();
        let (width, height) = self.viewports.iter().fold((1, 1), |(w, h), v| {
            (
                w.max(end(v.x, v.width) as u32),
                h.max(end(v.y, v.height) as u32),
            )
        });
        let samples = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .map_or(1, |r| r.targets.samples);
        EmptyPass {
            width,
            height,
            samples,
        }
    }

    /// Before a draw while a counted occlusion query runs: the bound targets given to the pass
    /// tracker, where the occlusion counter reads them; before the first draw writing depth or
    /// stencil since the last count, the pass ended and clears done, so the counter copies the
    /// depth-stencil target as the draw begins from it. Gives whether the draw writes depth or
    /// stencil, and its counting program.
    fn prepare_counted(
        &mut self,
        d: &mut Device<B>,
        pid: PipelineId,
    ) -> Result<(bool, B::Program), Error> {
        let counting = d.counting_program(pid)?;
        let writes = matches!(self.depth, Some(Descriptor::DepthStencil { .. }))
            && d.pipelines[pid.0 as usize]
                .render
                .as_ref()
                .is_some_and(|r| r.writes_depth_stencil);
        if writes && !self.counted_copy {
            self.settle(d)?;
            self.counted_copy = true;
        }
        self.track_targets(d, pid)?;
        Ok((writes, counting))
    }

    /// Gives the bound targets to the pass tracker, where the occlusion counter reads them, once
    /// after each pass end.
    fn track_targets(&mut self, d: &mut Device<B>, pid: PipelineId) -> Result<(), Error> {
        if !self.counted_targets {
            let targets = self.pass_targets(d, pid)?;
            d.passes.set_targets(&mut d.backend, targets)?;
            self.counted_targets = true;
        }
        Ok(())
    }

    /// The bound targets as the pass tracker takes them.
    fn pass_targets(&self, d: &mut Device<B>, pid: PipelineId) -> Result<PassTargets<B>, Error> {
        let mut target = |v: &TargetView| -> Result<Target<B>, Error> {
            let view = d.view(v)?;
            let (_, desc) = d.texture(v.resource)?;
            let d3 = desc.dimension == TextureDimension::D3;
            let (texture, layer) =
                d.texture_layer(v.resource, if d3 { 0 } else { v.first_layer })?;
            Ok(Target {
                view,
                texture: texture.clone(),
                format: dxgi(v.format)?,
                mip: v.mip,
                layer,
                depth_slice: 0,
                width: (desc.width >> v.mip).max(1),
                height: (desc.height >> v.mip).max(1),
                samples: desc.samples,
            })
        };
        let mut colors: [Option<ColorTarget<B>>; MAX_COLOR_TARGETS] = Default::default();
        for (slot, c) in colors.iter_mut().zip(&self.colors) {
            if let Descriptor::RenderTarget(v) = c {
                *slot = Some(ColorTarget {
                    target: target(v)?,
                    resolve: None,
                });
            }
        }
        let depth = match &self.depth {
            Some(Descriptor::DepthStencil { view, .. }) => Some(target(view)?),
            _ => None,
        };
        let empty = (colors.iter().all(Option::is_none) && depth.is_none())
            .then(|| self.empty_size(d, pid));
        Ok(PassTargets {
            colors,
            depth,
            empty,
        })
    }

    /// Opens a pass on the bound targets, each taking a whole clear still waiting for it as its
    /// load; with no targets the pass covers the viewports at the pipeline's sample count.
    fn open_pass(&mut self, d: &mut Device<B>, pid: PipelineId) -> Result<(), Error> {
        let occlusion = match self.query {
            Some((heap, _, QueryState::Waiting)) => Some((heap, d.occlusion_set(heap)?)),
            _ => None,
        };
        let mut views = Vec::with_capacity(self.colors.len());
        let mut loads = Vec::with_capacity(self.colors.len());
        let colors = match self.pass_written {
            true => &[][..],
            false => &self.colors[..],
        };
        for c in colors {
            let v = match c {
                Descriptor::RenderTarget(v) => v,
                // A null view: the slot has no target and its writes are dropped.
                Descriptor::Empty => {
                    views.push(None);
                    loads.push(LoadOp::Load);
                    continue;
                }
                _ => {
                    return Err(Error::Invalid(
                        "a render target slot holds no render target view",
                    ));
                }
            };
            views.push(Some(d.view_at(v, self.pass_layer)?));
            let clear = self.clears.iter().position(|(k, _)| k == v);
            loads.push(match clear {
                Some(i) => match self.clears.swap_remove(i).1 {
                    ClearValue::Color(value) => LoadOp::Clear(value),
                    ClearValue::DepthStencil { .. } => LoadOp::Load,
                },
                None => LoadOp::Load,
            });
        }
        let depth = match self.depth {
            None | Some(Descriptor::Empty) => None,
            Some(Descriptor::DepthStencil { view: v, read_only }) => {
                // A draw testing early in the program reads the view as it draws: a clear
                // waiting for it is done first, as the pass cannot both clear and only read.
                let read_only = if self.pass_early {
                    let waiting: Vec<_> = self.clears.extract_if(.., |(k, _)| *k == v).collect();
                    Self::clear_now(d, waiting)?;
                    DSV_READ_ONLY_DEPTH | DSV_READ_ONLY_STENCIL
                } else {
                    read_only
                };
                let view = d.view_at(&v, self.pass_layer)?;
                let (has_depth, has_stencil) = aspects(dxgi(v.format)?);
                let (mut depth, mut stencil) = (None, None);
                if let Some(i) = self.clears.iter().position(|(k, _)| *k == v)
                    && let (
                        _,
                        ClearValue::DepthStencil {
                            depth: d,
                            stencil: s,
                        },
                    ) = self.clears.swap_remove(i)
                {
                    (depth, stencil) = (d, s);
                }
                Some((
                    view,
                    has_depth.then(|| aspect_ops(depth, read_only & DSV_READ_ONLY_DEPTH != 0)),
                    has_stencil
                        .then(|| aspect_ops(stencil, read_only & DSV_READ_ONLY_STENCIL != 0)),
                ))
            }
            Some(_) => {
                return Err(Error::Invalid(
                    "the depth-stencil slot holds no depth-stencil view",
                ));
            }
        };
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: depth
                .as_ref()
                .map(|(view, depth, stencil)| DepthAttachment {
                    view,
                    depth: *depth,
                    stencil: *stencil,
                }),
            empty: None,
            occlusion: occlusion.as_ref().map(|(_, set)| set),
        };
        for (i, (view, load)) in views.iter().zip(loads).enumerate() {
            pass.colors[i] = view.as_ref().map(|view| ColorAttachment {
                view,
                depth_slice: 0,
                resolve: None,
                load,
                store: StoreOp::Store,
            });
        }
        if views.iter().all(Option::is_none) && depth.is_none() {
            pass.empty = Some(self.empty_size(d, pid));
            self.empty_pass = true;
        }
        d.backend.begin_pass(&pass);
        self.pass_open = true;
        if let (Some((heap, _)), Some((_, index, state))) = (&occlusion, &mut self.query) {
            d.backend.begin_query(*index);
            *state = QueryState::Running;
            self.pass_heap = Some(*heap);
        }
        Ok(())
    }
}

/// The same range again, for a second call.
fn same_range<'a, B: Backend>(r: &DrawRange<'a, B>) -> DrawRange<'a, B> {
    match *r {
        DrawRange::Vertices {
            first,
            count,
            first_instance,
            instances,
        } => DrawRange::Vertices {
            first,
            count,
            first_instance,
            instances,
        },
        DrawRange::Indexed {
            first,
            count,
            base_vertex,
            first_instance,
            instances,
        } => DrawRange::Indexed {
            first,
            count,
            base_vertex,
            first_instance,
            instances,
        },
        DrawRange::Indirect { args, offset } => DrawRange::Indirect { args, offset },
        DrawRange::IndexedIndirect { args, offset } => DrawRange::IndexedIndirect { args, offset },
    }
}

/// A viewport; fractional ones only where the back end takes them, until the program variant that
/// places them exists.
fn viewport(v: &ViewportF, fractional: bool) -> Result<Viewport, Error> {
    if !fractional
        && [v.x, v.y, v.width, v.height]
            .iter()
            .any(|f| f.fract() != 0.0)
    {
        return Err(Error::NotImplemented("fractional viewports"));
    }
    Ok(Viewport {
        x: v.x.to_bits(),
        y: v.y.to_bits(),
        width: v.width.to_bits(),
        height: v.height.to_bits(),
        min_depth: v.min_depth.to_bits(),
        max_depth: v.max_depth.to_bits(),
    })
}

fn scissor(r: &Rect) -> ScissorRect {
    let [l, t, r, b] = r.map(|v| v.max(0) as u32);
    ScissorRect {
        x: l,
        y: t,
        width: r.saturating_sub(l),
        height: b.saturating_sub(t),
    }
}

/// The width in bits of each component of a format a storage clear writes, in memory order from
/// the lowest bit.
fn components(format: wcontainers_gpu::Format) -> Option<&'static [u32]> {
    use wcontainers_gpu::Format::*;
    Some(match format {
        R8Unorm | R8Snorm | R8Uint | R8Sint => &[u8::BITS],
        Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint => &[u8::BITS; 2],
        Rgba8Unorm | Rgba8Snorm | Rgba8Uint | Rgba8Sint => &[u8::BITS; 4],
        R16Unorm | R16Snorm | R16Uint | R16Sint | R16Float => &[u16::BITS],
        Rg16Unorm | Rg16Snorm | Rg16Uint | Rg16Sint | Rg16Float => &[u16::BITS; 2],
        Rgba16Unorm | Rgba16Snorm | Rgba16Uint | Rgba16Sint | Rgba16Float => &[u16::BITS; 4],
        R32Uint | R32Sint | R32Float => &[u32::BITS],
        Rg32Uint | Rg32Sint | Rg32Float => &[u32::BITS; 2],
        Rgba32Uint | Rgba32Sint | Rgba32Float => &[u32::BITS; 4],
        Rgb10a2Unorm | Rgb10a2Uint => &[10, 10, 10, 2],
        Rg11b10Float => &[11, 11, 10],
        _ => return None,
    })
}

/// Whether a format of [`components`] is signed normalised, and whether it is normalised at all.
fn normalised(format: wcontainers_gpu::Format) -> (bool, bool) {
    use wcontainers_gpu::Format::*;
    match format {
        R8Snorm | Rg8Snorm | Rgba8Snorm | R16Snorm | Rg16Snorm | Rgba16Snorm => (true, true),
        R8Unorm | Rg8Unorm | Rgba8Unorm | R16Unorm | Rg16Unorm | Rgba16Unorm | Rgb10a2Unorm => {
            (false, true)
        }
        _ => (false, false),
    }
}

/// Bits of a half float.
const HALF_BITS: u32 = (size_of::<half::f16>() as u32) * u8::BITS;
/// Bits of a half float below its sign, whose top bits the small floats of `R11G11B10_FLOAT` keep.
const HALF_MAGNITUDE_BITS: u32 = HALF_BITS - 1;

fn mask(bits: u32) -> u32 {
    if bits == u32::BITS {
        u32::MAX
    } else {
        (1 << bits) - 1
    }
}

/// A float as a float component of `bits` bits: 32 as is, 16 a half, 11 and 10 the unsigned small
/// floats of `R11G11B10_FLOAT` (a half's exponent and top mantissa bits, negatives clamped to zero).
fn small_float(f: f32, bits: u32) -> u32 {
    match bits {
        u32::BITS => f.to_bits(),
        HALF_BITS => u32::from(half::f16::from_f32(f).to_bits()),
        _ if f.is_nan() => mask(bits),
        _ => u32::from(half::f16::from_f32(f.max(0.0)).to_bits()) >> (HALF_MAGNITUDE_BITS - bits),
    }
}

/// The value of a float component of `bits` stored bits.
fn small_float_value(s: u32, bits: u32) -> f32 {
    match bits {
        u32::BITS => f32::from_bits(s),
        HALF_BITS => half::f16::from_bits(s as u16).to_f32(),
        _ => half::f16::from_bits((s << (HALF_MAGNITUDE_BITS - bits)) as u16).to_f32(),
    }
}

/// A storage clear's value as each component's stored bits (masked to its width). Integer values
/// are the bits themselves; float values are converted to the format.
fn storage_bits(
    format: wcontainers_gpu::Format,
    values: [u32; 4],
    float: bool,
) -> Option<Vec<u32>> {
    let widths = components(format)?;
    let (snorm, norm) = normalised(format);
    Some(
        widths
            .iter()
            .zip(values)
            .map(|(&bits, v)| {
                let f = f32::from_bits(v);
                let m = mask(bits);
                let stored = match (float, format.kind()) {
                    (false, _) => v,
                    (true, FormatKind::Uint) => f as u32,
                    (true, FormatKind::Sint) => f as i32 as u32,
                    (true, _) if snorm => {
                        (f.clamp(-1.0, 1.0) * (m >> 1) as f32).round() as i32 as u32
                    }
                    (true, _) if norm => (f.clamp(0.0, 1.0) * m as f32).round() as u32,
                    (true, _) => small_float(f, bits),
                };
                stored & m
            })
            .collect(),
    )
}

/// One texel's bytes for a storage clear of a typed buffer view: the components packed from the
/// lowest bit, little-endian.
fn storage_texel(
    format: wcontainers_gpu::Format,
    values: [u32; 4],
    float: bool,
) -> Option<Vec<u8>> {
    let widths = components(format)?;
    let (mut packed, mut at) = (0u128, 0);
    for (bits, s) in widths.iter().zip(storage_bits(format, values, float)?) {
        packed |= u128::from(s) << at;
        at += bits;
    }
    Some(packed.to_le_bytes()[..at as usize / 8].to_vec())
}

/// A storage clear as the value [`ClearValue::Color`] takes for the format.
fn storage_clear_value(
    format: wcontainers_gpu::Format,
    values: [u32; 4],
    float: bool,
) -> Option<[u32; 4]> {
    let widths = components(format)?;
    let stored = storage_bits(format, values, float)?;
    let (snorm, norm) = normalised(format);
    let mut out = [0; 4];
    for ((o, s), &bits) in out.iter_mut().zip(stored).zip(widths) {
        let m = mask(bits);
        *o = match format.kind() {
            FormatKind::Uint => s,
            // Sign-extended from the component's width.
            FormatKind::Sint => (((s << (u32::BITS - bits)) as i32) >> (u32::BITS - bits)) as u32,
            _ if snorm => {
                let signed = ((s << (u32::BITS - bits)) as i32) >> (u32::BITS - bits);
                (signed as f32 / (m >> 1) as f32).max(-1.0).to_bits()
            }
            _ if norm => (s as f32 / m as f32).to_bits(),
            _ => small_float_value(s, bits).to_bits(),
        };
    }
    Some(out)
}

/// A clear color as the bits the format's kind takes.
fn clear_bits(format: wcontainers_gpu::Format, color: [f32; 4]) -> [u32; 4] {
    match format.kind() {
        FormatKind::Uint => color.map(|c| c as u32),
        FormatKind::Sint => color.map(|c| c as i32 as u32),
        _ => color.map(f32::to_bits),
    }
}

/// `CopyTextureRegion` between a texture and a buffer footprint, or two textures.
fn copy_texture<B: Backend>(
    d: &mut Device<B>,
    dst: &CopyLocation,
    at: [u32; 3],
    src: &CopyLocation,
    area: Option<[u32; 6]>,
) -> Result<(), Error> {
    // The textures holding the subresources, in their stored texels.
    let ((dst, dn), (src, sn)) = (d.texel_location(dst), d.texel_location(src));
    let (dst, src) = (&dst, &src);
    let [x, y, z] = at;
    let at = [x / dn, y, z];
    let area = area.map(|[left, top, front, right, bottom, back]| {
        [left / sn, top, front, right.div_ceil(sn), bottom, back]
    });
    // A subresource's texture, mip, layer and origin, the resource's description, and whether it is
    // in the stencil plane.
    let region = |d: &Device<B>, id: ResourceId, index: u32, origin: Origin| {
        let r = &d.resources[id.0 as usize];
        let (mips, layers) = (r.desc.mips(), r.desc.layers());
        let stencil = index / (mips * layers) == 1;
        let (texture, layer) = d.texture_layer(id, index / mips % layers)?;
        Ok::<_, Error>((
            texture.clone(),
            index % mips,
            layer,
            origin,
            r.desc,
            stencil,
        ))
    };
    let origin = |v: [u32; 3]| Origin {
        x: v[0],
        y: v[1],
        z: v[2],
    };
    let size_of_mip = |desc: &ResourceDesc, mip: u32| Extent {
        width: ((desc.stored_width() >> mip) as u32).max(1),
        height: (desc.height >> mip).max(1),
        depth_or_layers: if desc.dimension == resource::DIMENSION_TEXTURE3D {
            (u32::from(desc.depth_or_array_size) >> mip).max(1)
        } else {
            1
        },
    };
    // An empty box copies nothing.
    if area.is_some_and(|a| a[3] <= a[0] || a[4] <= a[1] || a[5] <= a[2]) {
        return Ok(());
    }
    let src_origin = area.map_or([0; 3], |a| [a[0], a[1], a[2]]);
    let extent_of = |full: Extent| match area {
        Some(a) => Extent {
            width: a[3] - a[0],
            height: a[4] - a[1],
            depth_or_layers: a[5] - a[2],
        },
        None => full,
    };
    match (dst, src) {
        (
            CopyLocation::Footprint {
                resource,
                footprint,
            },
            CopyLocation::Subresource { resource: s, index },
        ) => {
            let (texture, mip, layer, o, _, stencil) = region(d, *s, *index, origin(src_origin))?;
            let (format, _) = crate::resource::ResourceDesc {
                format: footprint.format,
                ..Default::default()
            }
            .texture_format()?;
            let (bw, bh, bytes) = format.block();
            let extent = extent_of(Extent {
                width: footprint.width,
                height: footprint.height,
                depth_or_layers: footprint.depth,
            });
            let offset = footprint.offset
                + u64::from(at[2])
                    * u64::from(footprint.row_pitch)
                    * u64::from(footprint.height / bh)
                + u64::from(at[1] / bh) * u64::from(footprint.row_pitch)
                + u64::from(at[0] / bw) * u64::from(bytes);
            let (buffer, heap) = d.buffer(*resource)?;
            let buffer = buffer.clone();
            d.backend.copy_texture_to_buffer(
                &TextureRegion {
                    texture: &texture,
                    mip,
                    layer,
                    origin: o,
                    stencil,
                },
                &CopyBufferTexture {
                    buffer: &buffer,
                    offset,
                    row_pitch: footprint.row_pitch,
                    rows_per_layer: footprint.height.div_ceil(bh),
                },
                extent,
            );
            if heap.reads_back() {
                let rows = u64::from(extent.height.div_ceil(bh));
                let size = u64::from(footprint.row_pitch)
                    * (rows * u64::from(extent.depth_or_layers) - 1)
                    + u64::from(extent.width.div_ceil(bw)) * u64::from(bytes);
                d.read_into(*resource, offset, size)?;
            }
        }
        (
            CopyLocation::Subresource { resource: t, index },
            CopyLocation::Footprint {
                resource,
                footprint,
            },
        ) => {
            let (texture, mip, layer, o, _, stencil) = region(d, *t, *index, origin(at))?;
            let (format, _) = crate::resource::ResourceDesc {
                format: footprint.format,
                ..Default::default()
            }
            .texture_format()?;
            let (bw, bh, bytes) = format.block();
            let extent = extent_of(Extent {
                width: footprint.width,
                height: footprint.height,
                depth_or_layers: footprint.depth,
            });
            let offset = footprint.offset
                + u64::from(src_origin[2])
                    * u64::from(footprint.row_pitch)
                    * u64::from(footprint.height / bh)
                + u64::from(src_origin[1] / bh) * u64::from(footprint.row_pitch)
                + u64::from(src_origin[0] / bw) * u64::from(bytes);
            let (buffer, _) = d.buffer(*resource)?;
            let buffer = buffer.clone();
            d.backend.copy_buffer_to_texture(
                &CopyBufferTexture {
                    buffer: &buffer,
                    offset,
                    row_pitch: footprint.row_pitch,
                    rows_per_layer: footprint.height.div_ceil(bh),
                },
                &TextureRegion {
                    texture: &texture,
                    mip,
                    layer,
                    origin: o,
                    stencil,
                },
                extent,
            );
        }
        (
            CopyLocation::Subresource {
                resource: t,
                index: ti,
            },
            CopyLocation::Subresource {
                resource: s,
                index: si,
            },
        ) => {
            // A copy between textures moves both planes of a depth-stencil format at once.
            let (st, smip, slayer, so, sdesc, _) = region(d, *s, *si, origin(src_origin))?;
            let (tt, tmip, tlayer, to, _, _) = region(d, *t, *ti, origin(at))?;
            let extent = extent_of(size_of_mip(&sdesc, smip));
            d.backend.copy_texture(
                &TextureRegion {
                    texture: &st,
                    mip: smip,
                    layer: slayer,
                    origin: so,
                    stencil: false,
                },
                &TextureRegion {
                    texture: &tt,
                    mip: tmip,
                    layer: tlayer,
                    origin: to,
                    stencil: false,
                },
                extent,
            );
        }
        _ => return Err(Error::Invalid("a copy between two buffer footprints")),
    }
    Ok(())
}

/// The bytes a buffer of `width` is stored in: whole blocks of Direct3D's constant buffer
/// alignment, zero past `width`, so a constant buffer view may declare more than the buffer's
/// last bytes hold, which Direct3D 12 reads from the buffer's page; at least whole storage words,
/// as a storage binding reaching its end is.
pub(crate) fn stored_size(width: u64, max: u64) -> u64 {
    width
        .next_multiple_of(resource::CONSTANT_BUFFER_ALIGNMENT)
        .min(max.max(width.next_multiple_of(STORAGE_WORD)))
}

/// The slow paths converted `programs` take by themselves, each once.
/// `D3D12_CLIP_OR_CULL_DISTANCE_COUNT`: the most clip and cull distances a stage passes.
const MOST_DISTANCES: u8 = 8;

/// The refusal of clip distances the back end lacks where no pixel shader can apply them.
/// The refusals of depth clamping the back end lacks where the programs cannot apply it.
const CLAMP_BOUND_DEPTH: &str = "depth clamping the back end lacks, in a pipeline made with no \
     depth-stencil format drawing while a depth-stencil view is bound";
const CLAMP_ANTIALIASED_LINES: &str =
    "depth clamping the back end lacks, of alpha-antialiased lines";
const CLAMP_STREAMED: &str = "depth clamping the back end lacks, in the culled vertex or domain \
     program stream output records";
const CLAMP_NAMED_LAYER_WITHOUT_DEPTH: &str = "depth clamping the back end lacks, behind a \
     geometry shader naming a layer, in a pipeline made with no depth-stencil format";

/// The clip distances `outputs` write.
fn clip_distances(outputs: &[StageValue]) -> u8 {
    outputs
        .iter()
        .filter(|v| v.system == Some(SystemValue::ClipDistance))
        .map(|v| v.mask.count_ones() as u8)
        .sum()
}

/// What a pipeline's last vertex stage passes its pixel program beside its outputs: the flags
/// that pass them (`CLIP_DISTANCE`, `DEPTH_CLAMP`, `LAYER_FROM_STAGE`) and where
/// (`VariantKey::linked`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Link {
    flags: VariantFlags,
    values: LinkedValues,
}

impl Link {
    /// Sets the link in `key`, the pixel program's or the last vertex stage's.
    fn apply(self, key: &mut VariantKey) {
        key.flags |= self.flags;
        key.linked = self.values;
    }

    /// What of the link a pipeline without a pixel shader applies in the pixel program for its
    /// variant: clip distances and the clamped depth; default where neither.
    fn unshaded(self) -> Link {
        let flags = self.flags & (VariantFlags::CLIP_DISTANCE | VariantFlags::DEPTH_CLAMP);
        match flags.is_empty() {
            true => Link::default(),
            false => Link {
                flags,
                values: self.values,
            },
        }
    }
}

/// The location past every value `values` lists (a stage's outputs and the next stage's inputs),
/// where the values the programs add for each other pass (`VariantKey::linked`).
fn linked_location<'a>(values: impl IntoIterator<Item = &'a StageValue>) -> u32 {
    values
        .into_iter()
        .map(|v| v.location + 1)
        .max()
        .unwrap_or(0)
        .max(1)
}

fn programs_slow_paths(programs: &[&Converted]) -> Vec<SlowPath> {
    let mut all: Vec<SlowPath> = programs
        .iter()
        .flat_map(|c| c.reflection.slow_paths.iter().copied())
        .collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// Whether a pipeline's pixel shader `ps` forces early depth and stencil tests in a way that
/// changes its results while `ds` tests: its storage writes then happen only for fragments that
/// pass, and a discard does not undo depth and stencil writes (plan 4.12).
fn early_tests_matter(ps: Option<&Converted>, ds: &DepthStencil) -> bool {
    let writes =
        ds.depth_enable && ds.depth_write_all || ds.stencil_enable && ds.stencil_write_mask != 0;
    (ds.depth_enable || ds.stencil_enable)
        && ps.is_some_and(|ps| {
            matches!(
                ps.reflection.extras,
                wcontainers_gpu::StageExtras::Pixel {
                    early_tests: true,
                    side_effects,
                    discards,
                    ..
                } if side_effects || discards && writes
            )
        })
}

/// The pixel shader's colour outputs not of the scalar kind of the target they write, each to be
/// written in the program as its target's format (`VariantKey::targets_in_shader`).
fn retyped_targets(ps: &Converted, colors: &[Option<Format>]) -> Vec<TargetInShader> {
    ps.reflection
        .outputs
        .iter()
        .filter_map(|o| {
            let Some(wcontainers_gpu::SystemValue::Target(target)) = o.system else {
                return None;
            };
            let format = (*colors.get(usize::from(target))?)?;
            let fits = matches!(
                (format.kind(), o.scalar),
                (FormatKind::Float, wcontainers_gpu::ScalarKind::Float)
                    | (FormatKind::Uint, wcontainers_gpu::ScalarKind::Uint)
                    | (FormatKind::Sint, wcontainers_gpu::ScalarKind::Sint)
            );
            (!fits).then_some(TargetInShader {
                target,
                format,
                combine: false,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wcontainers_gpu::Format;

    fn texel(format: Format, values: [f32; 4], float: bool) -> u32 {
        let values = if float {
            values.map(f32::to_bits)
        } else {
            values.map(|v| v as u32)
        };
        let bytes = storage_texel(format, values, float).unwrap();
        u32::from_le_bytes(bytes.try_into().unwrap())
    }

    /// Typed buffer clears as vkd3d's test_clear_unordered_access_view_buffer expects them, which
    /// real Direct3D 12 passes.
    #[test]
    fn storage_clears_pack_each_format() {
        let half = Format::Rg16Float;
        assert_eq!(texel(half, [4660.0, 43981.0, 0.0, 0.0], false), 0xabcd_1234);
        assert_eq!(texel(half, [0.5, 1.0, 0.0, 0.0], true), 0x3c00_3800);
        let norm = Format::Rg16Unorm;
        assert_eq!(texel(norm, [0.25, 1.0, 0.0, 0.0], true), 0xffff_4000);
        assert_eq!(texel(norm, [2.0, 0.0, 0.0, 0.0], true), 0x0000_ffff);
        assert_eq!(texel(norm, [-1.0, 0.0, 1.0, 1.0], true), 0);
        let small = Format::Rg11b10Float;
        assert_eq!(texel(small, [0.0; 4], false), 0);
        assert_eq!(texel(small, [0.5, 1.0, 2.0, 0.0], true), 0x801e_0380);
        assert_eq!(texel(small, [0.5, 0.0, -1.0, 0.5], true), 0x0000_0380);
        assert_eq!(
            texel(Format::Rgba8Uint, [17.0, 34.0, 51.0, 68.0], false),
            0x4433_2211
        );
        assert_eq!(
            texel(Format::Rgb10a2Unorm, [1.0, 0.0, 1.0, 1.0], true),
            0xc000_0000 | 0x3ff << 20 | 0x3ff
        );
    }

    #[test]
    fn storage_clears_of_textures_take_the_stored_bits_as_values() {
        let v = storage_clear_value(Format::Rg16Float, [0x3c00, 0xbc00, 0, 0], false).unwrap();
        assert_eq!(v[..2], [1.0f32.to_bits(), (-1.0f32).to_bits()]);
        let v = storage_clear_value(Format::Rg11b10Float, [0x3c0, 0x380, 0x200, 0], false).unwrap();
        assert_eq!(v[..3], [1.0f32, 0.5, 2.0].map(f32::to_bits));
    }
}
