//! The Direct3D 12 device on one back end: its objects, the one queue order every guest queue shares,
//! the replay of command lists, and fences.

use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use slab::Slab;
use wcontainers_directx_graphics_infrastructure::{SwapChain, SwapChainDesc, SwapChainError};
use wcontainers_gpu::{
    AspectOps, Backend, BufferDesc, BufferUsage, ClearValue, ColorAttachment, CopyBufferTexture,
    DepthAttachment, DispatchCall, DispatchGroups, DrawCall, DrawRange, Extent, Features, Format,
    FormatKind, FormatUse, GpuError, IndexBinding, IndexFormat, LoadOp, MAX_COLOR_TARGETS,
    MAX_VERTEX_STREAMS, MapIntent, Object, Origin, PassDesc, PrepareDesc, Program, ReadSource,
    ReadState, SamplerDesc, ScissorRect, ShaderStage, SlowPath, StageSet, StepMode, StoreOp,
    TargetLayout, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
    Topology, VariantKey, VertexAttribute, VertexBinding, VertexLayoutDesc, VertexStream,
    ViewDimension, Viewport, WriteMode,
};
use wcontainers_shaders::{ConvertError, Converted, Source};

use crate::Error;
use crate::address::AddressMap;
use crate::list::{
    Command, CopyLocation, Descriptor, Rect, TargetView, VertexBufferView, ViewportF,
};
use crate::resource::{self, HeapType, ResourceDesc};
use crate::root::{RootSignatureDesc, deserialize};
use crate::state::{Blend, DepthStencil, InputElement, Raster, resolve_offsets, vertex_format};

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
    /// A root signature.
    RootId,
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
mod indirect;
mod null;
mod query;

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

/// Guest-visible memory of an upload or readback heap, shared by the buffers placed in it.
type HostMemory = Rc<UnsafeCell<Box<[u8]>>>;

/// The bytes `Map` hands out for an upload or readback buffer: its own memory, or its part of its
/// heap's, which overlapping placed buffers share as the memory they alias.
struct Host {
    memory: HostMemory,
    offset: usize,
    len: usize,
}

impl Host {
    fn new(len: usize) -> Self {
        Host {
            memory: Rc::new(UnsafeCell::new(vec![0; len].into_boxed_slice())),
            offset: 0,
            len,
        }
    }

    fn ptr(&self) -> *mut u8 {
        // SAFETY: `offset` lies inside the memory, as `create_placed` checked.
        unsafe { (*self.memory.get()).as_mut_ptr().add(self.offset) }
    }

    /// The bytes. The program writes them through `Map`'s pointer, never while work reading them is
    /// being submitted, and no other reference to them lives across a call of the device.
    fn bytes(&mut self) -> &mut [u8] {
        // SAFETY: as above; `len` bytes from `offset` lie inside the memory.
        unsafe { std::slice::from_raw_parts_mut(self.ptr(), self.len) }
    }
}

enum Storage<B: Backend> {
    Buffer {
        buffer: B::Buffer,
        /// The bytes `Map` hands out, for upload and readback heaps.
        host: Option<Host>,
    },
    Texture {
        texture: B::Texture,
        desc: TextureDesc,
        /// Views made for passes and shaders, by what they view.
        views: Vec<(TextureViewDesc, B::TextureView)>,
        /// False for a swap chain's back buffer, whose texture the swap chain owns.
        owned: bool,
    },
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
}

/// A graphics pipeline's fixed state.
struct Render<B: Backend> {
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
    /// Each input element and the program location it feeds.
    inputs: Vec<(InputElement, u32)>,
    targets: TargetLayout,
    /// `D3D12_INDEX_BUFFER_STRIP_CUT_VALUE`: non-zero restarts strips.
    strip_cut: u32,
    /// The vertex shader reads `SV_VertexID` or `SV_InstanceID`, which leave out the draw's bases.
    reads_bases: bool,
}

/// `D3D12_GRAPHICS_PIPELINE_STATE_DESC`, the parts this front reads.
#[derive(Clone, Debug, Default)]
pub struct GraphicsPipelineDesc<'a> {
    pub root: Option<RootId>,
    pub vs: &'a [u8],
    pub ps: &'a [u8],
    pub blend: Blend,
    pub sample_mask: u32,
    pub raster: Raster,
    pub depth_stencil: DepthStencil,
    pub inputs: Vec<InputElement>,
    pub strip_cut: u32,
    /// `D3D12_PRIMITIVE_TOPOLOGY_TYPE`.
    pub topology_type: u32,
    pub rtv_formats: Vec<u32>,
    pub dsv_format: u32,
    pub samples: u32,
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
    slots: Box<[Descriptor]>,
    gpu: u64,
}

/// Where GPU descriptor handles start, far from GPU virtual addresses.
const HANDLE_BASE: u64 = 1 << 56;

/// An event waiting for fences: the fences and values, whether any one reaching its value is
/// enough, and the shell's token for the event.
type Waiter = (Vec<(FenceId, u64)>, bool, u64);

/// The device: every object and the replay of command lists, on one back end owned by one thread.
pub struct Device<B: Backend> {
    backend: B,
    resources: Slab<Resource<B>>,
    pipelines: Slab<Pipeline<B>>,
    roots: Slab<Arc<RootSignatureDesc>>,
    fences: Slab<Fence<B>>,
    addresses: AddressMap<ResourceId>,
    heaps: Slab<Heap>,
    handles: AddressMap<HeapId>,
    samplers: HashMap<SamplerDesc, B::Sampler>,
    /// Upload buffers already written to the GPU in this `ExecuteCommandLists`.
    uploaded: Vec<ResourceId>,
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
            roots: Slab::new(),
            fences: Slab::new(),
            addresses: AddressMap::new(ADDRESS_BASE),
            heaps: Slab::new(),
            handles: AddressMap::new(HANDLE_BASE),
            samplers: HashMap::new(),
            uploaded: Vec::new(),
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
            nulls: null::Nulls::default(),
            messages: Vec::new(),
            removed: None,
        }
    }

    pub fn backend(&mut self) -> &mut B {
        &mut self.backend
    }

    /// The error that removed the device, if one did (`GetDeviceRemovedReason`).
    pub fn removed(&self) -> Option<Error> {
        self.removed
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
            };
            let buffer = self.backend.create_buffer(&BufferDesc {
                size: desc.width.next_multiple_of(4),
                usage,
                intent: if heap == HeapType::Upload {
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
            if heap != HeapType::Default {
                return Err(Error::Invalid("textures live in default heaps"));
            }
            let (format, typeless) = desc.texture_format()?;
            let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
            if desc.flags & resource::FLAG_DENY_SHADER_RESOURCE == 0 {
                usage |= TextureUsage::SAMPLE;
            }
            // Multisampled textures are made to draw into whatever their flags say (WebGPU's rule).
            if desc.flags & (resource::FLAG_RENDER_TARGET | resource::FLAG_DEPTH_STENCIL) != 0
                || desc.sample_count > 1
            {
                usage |= TextureUsage::RENDER;
            }
            if desc.flags & resource::FLAG_UNORDERED_ACCESS != 0 {
                usage |= TextureUsage::STORAGE;
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
                width: u32::try_from(desc.width).map_err(|_| Error::Invalid("texture too wide"))?,
                height: desc.height.max(1),
                depth_or_layers: u32::from(desc.depth_or_array_size).max(1),
                // Direct3D 12 takes more levels than the chain has; the texture keeps the chain.
                mip_levels: desc.mips().min(desc.full_chain()),
                samples: desc.sample_count.max(1),
                usage,
                cube: false,
                reinterpret: typeless,
            };
            let texture = self.backend.create_texture(&tdesc)?;
            (
                Storage::Texture {
                    texture,
                    desc: tdesc,
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
        });
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
        resource::check_heap(alignment, flags)?;
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
        if heap == HeapType::Default {
            self.inherit(id, &overlaps);
        }
        Ok(id)
    }

    /// The region of subresource `sub` of texture `id` at `origin`, for slice `z` of a box.
    fn subresource_region(
        &self,
        id: ResourceId,
        sub: u32,
        origin: Origin,
        z: u32,
    ) -> Result<TextureRegion<'_, B>, Error> {
        let (texture, desc) = self.texture(id)?;
        let mips = desc.mip_levels;
        let three_d = desc.dimension == TextureDimension::D3;
        Ok(TextureRegion {
            texture,
            mip: sub % mips,
            layer: if three_d { 0 } else { sub / mips },
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
    pub fn map(&mut self, id: ResourceId) -> Result<*mut u8, Error> {
        self.finish_reads(false);
        match self
            .resources
            .get_mut(id.0 as usize)
            .map(|r| &mut r.storage)
        {
            Some(Storage::Buffer {
                host: Some(host), ..
            }) => Ok(host.ptr()),
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
        for t in r.twins {
            self.destroy_resource(t);
        }
        self.reads.retain(|p| p.resource != id);
        match r.storage {
            Storage::Buffer { buffer, .. } => self.backend.destroy(Object::Buffer(buffer)),
            Storage::Texture {
                texture,
                views,
                owned,
                ..
            } => {
                for (_, v) in views {
                    self.backend.destroy(Object::TextureView(v));
                }
                if owned {
                    self.backend.destroy(Object::Texture(texture));
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
        let mut slots = vec![Descriptor::Empty; count as usize].into_boxed_slice();
        let cpu = slots.as_mut_ptr();
        self.heaps.insert(Heap { slots, gpu });
        (id, cpu, gpu)
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
        // SAFETY: a slot of a live heap. The program writes slots through its CPU handles; it may
        // not change a descriptor while work that reads it is being submitted.
        Ok(unsafe { std::ptr::read_volatile(&heap.slots[index]) })
    }

    // ---- Root signatures and pipelines ----

    /// `CreateRootSignature` from a serialized blob.
    pub fn create_root_signature(&mut self, blob: &[u8]) -> Result<RootId, Error> {
        self.live()?;
        let desc = deserialize(blob).map_err(Error::Invalid)?;
        desc.validate().map_err(Error::Invalid)?;
        Ok(RootId(self.roots.insert(Arc::new(desc)) as u32))
    }

    pub fn destroy_root_signature(&mut self, id: RootId) {
        self.roots.try_remove(id.0 as usize);
    }

    /// `CreateGraphicsPipelineState`: the blocks and program are made now and the combination is
    /// prepared with vertex streams packed as the input layout lays them out, the common case, so
    /// the back end builds it before the first draw.
    pub fn create_graphics_pipeline(
        &mut self,
        desc: &GraphicsPipelineDesc<'_>,
    ) -> Result<PipelineId, Error> {
        self.live()?;
        let root = self.pipeline_root(desc.root, &[desc.vs, desc.ps])?;
        let vs = self.convert(desc.vs, ShaderStage::Vertex)?;
        let ps = if desc.ps.is_empty() {
            None
        } else {
            Some(self.convert(desc.ps, ShaderStage::Fragment)?)
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
        let reads_bases = reads.iter().any(|i| {
            matches!(
                i.system,
                Some(
                    wcontainers_gpu::SystemValue::VertexId
                        | wcontainers_gpu::SystemValue::InstanceId
                )
            )
        });
        let mut colors = [None; MAX_COLOR_TARGETS];
        for (slot, f) in colors.iter_mut().zip(&desc.rtv_formats) {
            *slot = Some(dxgi(*f)?);
        }
        let targets = TargetLayout {
            colors,
            depth_stencil: (desc.dsv_format != 0)
                .then(|| dxgi(desc.dsv_format))
                .transpose()?,
            samples: desc.samples.max(1),
        };
        let mut bindings = bind::stage_bindings(&vs, StageSet::VERTEX);
        if let Some(ps) = &ps {
            bindings.extend(bind::stage_bindings(ps, StageSet::FRAGMENT));
        }
        bind::check(&root, &bindings)?;
        self.prepare_nulls(&bind::null_shapes(&bindings))?;
        let caps = self.backend.caps();
        let blend = desc.blend.block(caps, desc.rtv_formats.len())?;
        let raster = desc.raster.block(
            caps,
            desc.sample_mask,
            targets.samples,
            desc.topology_type == 3,
        )?;
        let depth_stencil = desc.depth_stencil.block(targets.depth_stencil.is_some())?;
        let topology = match desc.topology_type {
            1 => Topology::PointList,
            2 => Topology::LineList,
            3 => Topology::TriangleList,
            4 => return Err(Error::NotImplemented("patch topologies")),
            _ => return Err(Error::Invalid("a pipeline needs a topology type")),
        };
        // Alpha-antialiased lines need the stage helper's covered rectangles
        // (`AntialiasedLinesAsQuads`), which the front does not route yet.
        let draws_lines = topology == Topology::LineList || desc.raster.fill == 2;
        if draws_lines && desc.raster.antialiased_line && !desc.raster.multisample {
            return Err(Error::NotImplemented("alpha-antialiased lines"));
        }
        let vs_shader = self.backend.create_shader(&vs.modules[0])?;
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
        let render = Render {
            blend: self.backend.create_blend(&blend)?,
            depth_stencil: self.backend.create_depth_stencil(&depth_stencil)?,
            raster: self.backend.create_raster(&raster)?,
            inputs,
            targets,
            strip_cut: desc.strip_cut,
            reads_bases,
        };
        let packed = packed_strides(&render.inputs);
        let layout = self.vertex_layout(&render.inputs, &packed)?;
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
        Ok(PipelineId(self.pipelines.insert(Pipeline {
            program,
            render: Some(render),
            bindings,
        }) as u32))
    }

    /// `CreateComputePipelineState`.
    pub fn create_compute_pipeline(
        &mut self,
        root: Option<RootId>,
        cs: &[u8],
    ) -> Result<PipelineId, Error> {
        self.live()?;
        let root = self.pipeline_root(root, &[cs])?;
        if !self.backend.caps().has(Features::COMPUTE) {
            return Err(Error::NotImplemented("compute shaders on this back end"));
        }
        let cs = self.convert(cs, ShaderStage::Compute)?;
        let bindings = bind::stage_bindings(&cs, StageSet::COMPUTE);
        bind::check(&root, &bindings)?;
        self.prepare_nulls(&bind::null_shapes(&bindings))?;
        let shader = self.backend.create_shader(&cs.modules[0])?;
        let program = self.backend.create_program(&Program::Compute(&shader));
        self.backend.destroy(Object::Shader(shader));
        Ok(PipelineId(self.pipelines.insert(Pipeline {
            program: program?,
            render: None,
            bindings,
        }) as u32))
    }

    /// The root signature a pipeline is made with: the one given, else one a shader carries.
    fn pipeline_root(
        &self,
        root: Option<RootId>,
        shaders: &[&[u8]],
    ) -> Result<Arc<RootSignatureDesc>, Error> {
        if let Some(r) = root {
            return self
                .roots
                .get(r.0 as usize)
                .cloned()
                .ok_or(Error::Invalid("no such root signature"));
        }
        let code = shaders
            .iter()
            .find(|c| container_has(c, *b"RTS0"))
            .ok_or(Error::Invalid("a pipeline needs its root signature"))?;
        Ok(Arc::new(deserialize(code).map_err(Error::Invalid)?))
    }

    /// A DXIL or DXBC shader in the common form. Shaders reading helper constants wait for the
    /// variants that need them; the converter's own refusals are kept as messages.
    fn convert(&mut self, code: &[u8], stage: ShaderStage) -> Result<Converted, Error> {
        let source = if container_has(code, *b"DXIL") {
            Source::Dxil(code)
        } else {
            Source::Dxbc(code)
        };
        let converted = wcontainers_shaders::convert(source, &VariantKey::default(), None)
            .map_err(|e| {
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
                    .push(format!("a {stage:?} shader did not convert: {e}"));
                error
            })?;
        if converted.modules.len() != 1 || converted.modules[0].stage != stage {
            return Err(Error::Invalid("a shader of another stage"));
        }
        if converted.reflection.helper_size != 0 {
            return Err(Error::NotImplemented("shaders reading helper constants"));
        }
        Ok(converted)
    }

    pub fn destroy_pipeline(&mut self, id: PipelineId) {
        if let Some(p) = self.pipelines.try_remove(id.0 as usize) {
            self.backend.destroy(Object::Program(p.program));
            if let Some(r) = p.render {
                self.backend.destroy(Object::Blend(r.blend));
                self.backend.destroy(Object::DepthStencil(r.depth_stencil));
                self.backend.destroy(Object::Raster(r.raster));
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
        for (i, (e, location)) in inputs.iter().enumerate() {
            let slot = e.slot as usize;
            desc.streams[slot] = Some(VertexStream {
                stride: strides[slot],
                step: if e.per_instance {
                    StepMode::Instance(e.step_rate)
                } else {
                    StepMode::Vertex
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
            let r = self.reads.swap_remove(i);
            if !ready {
                self.backend.wait_read(&r.readback);
            }
            if let Some(Resource {
                storage:
                    Storage::Buffer {
                        host: Some(host), ..
                    },
                ..
            }) = self.resources.get_mut(r.resource.0 as usize)
            {
                self.backend
                    .finish_read(r.readback, &mut host.bytes()[r.offset..r.offset + r.size]);
            }
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
            self.upload(id);
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
                .is_some_and(|r| r.heap == HeapType::Upload)
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
        if heap == HeapType::Readback {
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
            let (texture, desc) = d.texture(id)?;
            let r = &d.resources[id.0 as usize].desc;
            let (mips, layers) = (r.mips(), r.layers());
            if index >= mips * layers {
                return Err(Error::Invalid("a resolve's subresource"));
            }
            let mip = index % mips;
            let size = ((desc.width >> mip).max(1), (desc.height >> mip).max(1));
            Ok((
                texture.clone(),
                desc.format,
                desc.samples,
                mip,
                index / mips,
                size,
            ))
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

    /// The back end's view for a render target or depth view, made once per resource and view.
    fn view(&mut self, v: &TargetView) -> Result<B::TextureView, Error> {
        let three_d = matches!(
            self.resources.get(v.resource.0 as usize).map(|r| &r.storage),
            Some(Storage::Texture { desc, .. }) if desc.dimension == TextureDimension::D3
        );
        // Draws write the view's first slice: no stage chooses another (`SV_RenderTargetArrayIndex`
        // is reported absent). Clears of every slice go by `clear_texture`.
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
                base_layer: if three_d { 0 } else { v.first_layer },
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
                    views,
                    desc: tdesc,
                    ..
                },
            ..
        }) = self.resources.get_mut(resource.0 as usize)
        else {
            return Err(Error::Invalid("a texture view of no texture"));
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
            && !self.backend.caps().has(Features::FORMAT_REINTERPRET)
        {
            return Err(Error::NotImplemented(
                "a view reinterpreting a texture's bits without its twin",
            ));
        }
        let view = self.backend.create_view(texture, &desc)?;
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
        if !rects.is_empty() {
            return Err(Error::NotImplemented(
                "clearing rectangles of an unordered access view",
            ));
        }
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
                let size = u64::from(v.count) * element;
                if !size.is_multiple_of(pattern.len() as u64) {
                    return Err(Error::NotImplemented(
                        "clearing a structured buffer view whose stride is no multiple of 4",
                    ));
                }
                self.backend
                    .clear_buffer(&buffer, v.first_element * element, size, &pattern);
            }
            Some(Storage::Texture { texture, desc, .. }) => {
                let (texture, desc) = (texture.clone(), *desc);
                let format = dxgi(v.format)?;
                let value = storage_clear_value(format, values, float).ok_or(
                    Error::NotImplemented("clearing an unordered access view of this format"),
                )?;
                let layers = match v.dimension {
                    4 => 0..1,
                    5 | 8 => v.first_layer..v.first_layer.saturating_add(v.layers),
                    _ => return Err(Error::Invalid("an unordered access view's dimension")),
                };
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
    let word = |at: usize| {
        code.get(at..at + 4)
            .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    (0..word(28)).any(|i| {
        code.get(word(32 + i * 4)..)
            .is_some_and(|p| p.starts_with(&fourcc))
    })
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
    predicate: Option<query::Predicate<B>>,
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
    fn new() -> Self {
        Replay {
            pipeline: None,
            topology: Topology::TriangleList,
            vertex_buffers: [VertexBufferView::default(); MAX_VERTEX_STREAMS],
            index: None,
            viewports: Vec::new(),
            scissors: vec![ScissorRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }],
            blend_constant: [0; 4],
            stencil_reference: 0,
            colors: Vec::new(),
            depth: None,
            pass_open: false,
            empty_pass: false,
            graphics: bind::RootState::default(),
            compute: bind::RootState::default(),
            clears: Vec::new(),
            query: None,
            pass_heap: None,
            predicate: None,
        }
    }

    fn end_pass(&mut self, d: &mut Device<B>) {
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
    }

    /// Ends any pass and does the clears still waiting, so copies see them.
    fn settle(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        self.end_pass(d);
        for (v, value) in std::mem::take(&mut self.clears) {
            let (texture, desc) = d.texture(v.resource)?;
            let (texture, desc) = (texture.clone(), *desc);
            d.backend.clear_texture(
                &texture,
                &desc,
                dxgi(v.format)?,
                v.mip..v.mip + 1,
                v.first_layer..v.first_layer.saturating_add(v.layers.max(1)),
                value,
            )?;
        }
        Ok(())
    }

    fn finish(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        if let Some(p) = self.predicate.take() {
            d.drop_predicate(p);
        }
        self.settle(d)
    }

    /// Whether a predicate skips a clear, copy or resolve, which have no indirect form.
    fn skipped(&mut self, d: &mut Device<B>) -> Result<bool, Error> {
        if self.predicate.is_none() {
            return Ok(false);
        }
        self.end_pass(d);
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
        if let Some(record) = Device::<B>::next_record(p) {
            return Ok(Some(Some(record)));
        }
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
                let signature = root.and_then(|r| d.roots.get(r.0 as usize).cloned());
                if root.is_some() && signature.is_none() {
                    return Err(Error::Invalid("no such root signature"));
                }
                self.state(*compute).set_root(*root, signature.as_deref());
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
            Command::Topology(t) => self.topology = crate::state::topology(*t)?,
            Command::VertexBuffers { start, views } => {
                let at = *start as usize;
                self.vertex_buffers
                    .get_mut(at..at + views.len())
                    .ok_or(Error::Invalid("vertex buffer slot out of range"))?
                    .copy_from_slice(views);
            }
            Command::IndexBuffer(v) => self.index = *v,
            Command::Viewports(v) => {
                // A pass with no targets is as large as the viewports it began with.
                if self.empty_pass {
                    self.end_pass(d);
                }
                let fractional = d.backend.caps().has(Features::FRACTIONAL_VIEWPORT);
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
                    self.end_pass(d);
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
                    self.end_pass(d);
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
                    self.end_pass(d);
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
                if heap == HeapType::Readback {
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
        let location = |l: &CopyLocation| match l {
            CopyLocation::Subresource { resource, .. }
            | CopyLocation::Footprint { resource, .. } => *resource,
        };
        match c {
            Command::CopyBuffer { dst, src, .. } => {
                d.note(*src, false);
                d.note(*dst, true);
            }
            Command::CopyTexture { dst, src, .. } => {
                d.note(location(src), false);
                d.note(location(dst), true);
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
            query::QUERY_BINARY_OCCLUSION => {}
            query::QUERY_OCCLUSION => {
                return Err(Error::NotImplemented("counted occlusion queries"));
            }
            _ => return Err(Error::Invalid("BeginQuery of this query type")),
        }
        d.occlusion_set(heap)?;
        if self.query.is_some() {
            return Err(Error::Invalid("an occlusion query begun inside another"));
        }
        if self.pass_open && self.pass_heap == Some(heap) {
            d.backend.begin_query(index);
            self.query = Some((heap, index, QueryState::Running));
        } else {
            // The next pass counts in this heap's query set.
            self.end_pass(d);
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
        if kind == query::QUERY_TIMESTAMP {
            self.end_pass(d);
            return d.timestamp(heap, index);
        }
        match self.query.take() {
            Some((h, i, state)) if (h, i) == (heap, index) => match state {
                QueryState::Running => d.backend.end_query(),
                QueryState::Ended => {}
                QueryState::Waiting => {
                    // No pass drew while it ran: an empty pass writes its zero.
                    self.end_pass(d);
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
            return Ok(());
        };
        if d.pipelines[pid.0 as usize].render.is_some() {
            return Err(Error::Invalid("dispatching with a graphics pipeline state"));
        }
        if groups.contains(&0) {
            return Ok(());
        }
        self.settle(d)?;
        let sets = d.binding_sets(&self.compute, pid)?;
        d.alias_settle()?;
        let call = DispatchCall {
            program: &d.pipelines[pid.0 as usize].program,
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
        Ok(())
    }

    /// A draw; with `record`, its arguments are read from that argument record instead.
    fn draw(
        &mut self,
        d: &mut Device<B>,
        range: DrawRange<'_, B>,
        record: Option<&(B::Buffer, u64)>,
    ) -> Result<(), Error> {
        // A draw with no pipeline state draws nothing, as the hardware does.
        let Some(pid) = self.pipeline else {
            return Ok(());
        };
        let bases = match &range {
            DrawRange::Vertices {
                first,
                first_instance,
                ..
            } => *first != 0 || *first_instance != 0,
            DrawRange::Indexed {
                base_vertex,
                first_instance,
                ..
            } => *base_vertex != 0 || *first_instance != 0,
            _ => false,
        };
        let render = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .ok_or(Error::Invalid("drawing with a compute pipeline state"))?;
        if bases && render.reads_bases {
            return Err(Error::NotImplemented(
                "SV_VertexID or SV_InstanceID in a draw with a first vertex or instance",
            ));
        }
        if let (Some(_), DrawRange::Vertices { first_instance, .. })
        | (Some(_), DrawRange::Indexed { first_instance, .. }) = (record, &range)
            && *first_instance != 0
            && !d.backend.caps().has(Features::INDIRECT_FIRST_INSTANCE)
        {
            return Err(Error::NotImplemented(
                "a predicated draw with a first instance on this back end",
            ));
        }
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
        let sets = d.binding_sets(&self.graphics, pid)?;
        if d.aliased != 0 {
            self.note_draw(
                d,
                matches!(
                    range,
                    DrawRange::Indexed { .. } | DrawRange::IndexedIndirect { .. }
                ),
            );
            if d.alias_due() {
                self.end_pass(d);
            }
            d.alias_settle()?;
        }
        if let Some((heap, _, QueryState::Ended)) = self.query
            && !self.pass_open
            && self.pass_heap != Some(heap)
        {
            return Err(Error::NotImplemented("an occlusion query across passes"));
        }
        if !self.pass_open {
            let occlusion = match self.query {
                Some((heap, _, QueryState::Waiting)) => Some((heap, d.occlusion_set(heap)?)),
                _ => None,
            };
            let mut views = Vec::with_capacity(self.colors.len());
            let mut loads = Vec::with_capacity(self.colors.len());
            for c in &self.colors {
                let Descriptor::RenderTarget(v) = c else {
                    return Err(Error::Invalid(
                        "a render target slot holds no render target view",
                    ));
                };
                views.push(d.view(v)?);
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
                    let view = d.view(&v)?;
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
                        has_depth.then(|| aspect_ops(depth, read_only & 1 != 0)),
                        has_stencil.then(|| aspect_ops(stencil, read_only & 2 != 0)),
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
                pass.colors[i] = Some(ColorAttachment {
                    view,
                    depth_slice: 0,
                    resolve: None,
                    load,
                    store: StoreOp::Store,
                });
            }
            if views.is_empty() && depth.is_none() {
                // No targets: the pass covers the viewports, at the pipeline's sample count.
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
                pass.empty = Some(wcontainers_gpu::EmptyPass {
                    width,
                    height,
                    samples,
                });
                self.empty_pass = true;
            }
            d.backend.begin_pass(&pass);
            self.pass_open = true;
            if let (Some((heap, _)), Some((_, index, state))) = (&occlusion, &mut self.query) {
                d.backend.begin_query(*index);
                *state = QueryState::Running;
                self.pass_heap = Some(*heap);
            }
        }
        let mut strides = [0; MAX_VERTEX_STREAMS];
        for (s, v) in strides.iter_mut().zip(&self.vertex_buffers) {
            *s = v.stride;
        }
        let inputs = std::mem::take(&mut d.render_mut(pid).inputs);
        let layout = d.vertex_layout(&inputs, &strides);
        let mut used = [false; MAX_VERTEX_STREAMS];
        inputs
            .iter()
            .for_each(|(e, _)| used[e.slot as usize] = true);
        d.render_mut(pid).inputs = inputs;
        let layout = layout?;
        let mut buffers: [Option<(B::Buffer, u64)>; MAX_VERTEX_STREAMS] =
            [const { None }; MAX_VERTEX_STREAMS];
        for ((b, v), used) in buffers.iter_mut().zip(&self.vertex_buffers).zip(used) {
            if let Some((id, offset)) = d.addresses.find(v.address)
                && used
                && v.size > 0
            {
                *b = Some((d.buffer(id)?.0.clone(), offset));
            }
        }
        let vertex_buffers = buffers.each_ref().map(|b| {
            b.as_ref().map(|(buffer, offset)| VertexBinding {
                buffer,
                offset: *offset,
            })
        });
        let index = match (&range, &self.index) {
            (DrawRange::Indexed { .. } | DrawRange::IndexedIndirect { .. }, Some(v)) => {
                let (id, offset) = d
                    .addresses
                    .find(v.address)
                    .ok_or(Error::Invalid("an index buffer view of no buffer"))?;
                let format = match v.format {
                    57 => IndexFormat::U16,
                    42 => IndexFormat::U32,
                    _ => return Err(Error::Invalid("index format")),
                };
                Some((d.buffer(id)?.0.clone(), offset, format))
            }
            (DrawRange::Indexed { .. } | DrawRange::IndexedIndirect { .. }, None) => {
                return Err(Error::Invalid("an indexed draw with no index buffer"));
            }
            _ => None,
        };
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
        let restart = p.strip_cut != 0;
        let p = &d.pipelines[pid.0 as usize];
        let r = p.render.as_ref().expect("a graphics pipeline");
        let call = DrawCall {
            program: &p.program,
            blend: &r.blend,
            depth_stencil: &r.depth_stencil,
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
            // With no stage choosing a viewport (geometry shaders are refused, and
            // `SV_ViewportArrayIndex` from the vertex stage is reported absent), Direct3D 12 draws
            // through the first viewport and scissor.
            viewports: &self.viewports[..self.viewports.len().min(1)],
            scissors: &self.scissors[..self.scissors.len().min(1)],
            blend_constant: self.blend_constant,
            stencil_reference: [self.stencil_reference; 2],
            immediates: &[],
            range,
        };
        d.backend.draw(&call);
        Ok(())
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

/// The components of a format whose components are each 8, 16 or 32 bits wide: count and bits.
fn components(format: wcontainers_gpu::Format) -> Option<(usize, u32)> {
    use wcontainers_gpu::Format::*;
    Some(match format {
        R8Unorm | R8Snorm | R8Uint | R8Sint => (1, 8),
        Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint => (2, 8),
        Rgba8Unorm | Rgba8Snorm | Rgba8Uint | Rgba8Sint => (4, 8),
        R16Unorm | R16Snorm | R16Uint | R16Sint => (1, 16),
        Rg16Unorm | Rg16Snorm | Rg16Uint | Rg16Sint => (2, 16),
        Rgba16Unorm | Rgba16Snorm | Rgba16Uint | Rgba16Sint => (4, 16),
        R32Uint | R32Sint | R32Float => (1, 32),
        Rg32Uint | Rg32Sint | Rg32Float => (2, 32),
        Rgba32Uint | Rgba32Sint | Rgba32Float => (4, 32),
        _ => return None,
    })
}

/// Whether a format of [`components`] is signed normalised, and whether it is normalised at all.
fn normalised(format: wcontainers_gpu::Format) -> (bool, bool) {
    use wcontainers_gpu::Format::*;
    match format {
        R8Snorm | Rg8Snorm | Rgba8Snorm | R16Snorm | Rg16Snorm | Rgba16Snorm => (true, true),
        R8Unorm | Rg8Unorm | Rgba8Unorm | R16Unorm | Rg16Unorm | Rgba16Unorm => (false, true),
        _ => (false, false),
    }
}

/// A storage clear's value as each component's stored bits (masked to its width).
fn storage_bits(
    format: wcontainers_gpu::Format,
    values: [u32; 4],
    float: bool,
) -> Option<Vec<u32>> {
    let (count, bits) = components(format)?;
    let mask = if bits == 32 {
        u32::MAX
    } else {
        (1 << bits) - 1
    };
    let max = (mask >> 1) as f32;
    let (snorm, norm) = normalised(format);
    Some(
        values[..count]
            .iter()
            .map(|&v| {
                let f = f32::from_bits(v);
                let stored = match (float, format.kind()) {
                    (false, _) => v,
                    (true, FormatKind::Uint) => f as u32,
                    (true, FormatKind::Sint) => f as i32 as u32,
                    (true, _) if snorm => (f.clamp(-1.0, 1.0) * max).round() as i32 as u32,
                    (true, _) if norm => (f.clamp(0.0, 1.0) * mask as f32).round() as u32,
                    (true, _) => v,
                };
                stored & mask
            })
            .collect(),
    )
}

/// One texel's bytes for a storage clear of a typed buffer view.
fn storage_texel(
    format: wcontainers_gpu::Format,
    values: [u32; 4],
    float: bool,
) -> Option<Vec<u8>> {
    let bits = components(format)?.1;
    Some(
        storage_bits(format, values, float)?
            .into_iter()
            .flat_map(|c| c.to_le_bytes().into_iter().take(bits as usize / 8))
            .collect(),
    )
}

/// A storage clear as the value [`ClearValue::Color`] takes for the format.
fn storage_clear_value(
    format: wcontainers_gpu::Format,
    values: [u32; 4],
    float: bool,
) -> Option<[u32; 4]> {
    let (_, bits) = components(format)?;
    let stored = storage_bits(format, values, float)?;
    let (snorm, norm) = normalised(format);
    let mask = if bits == 32 {
        u32::MAX
    } else {
        (1 << bits) - 1
    };
    let mut out = [0; 4];
    for (o, s) in out.iter_mut().zip(stored) {
        *o = match format.kind() {
            FormatKind::Uint => s,
            // Sign-extended from the component's width.
            FormatKind::Sint => (((s << (32 - bits)) as i32) >> (32 - bits)) as u32,
            _ if snorm => {
                let signed = ((s << (32 - bits)) as i32) >> (32 - bits);
                (signed as f32 / (mask >> 1) as f32).max(-1.0).to_bits()
            }
            _ if norm => (s as f32 / mask as f32).to_bits(),
            _ => s,
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
    // A subresource's texture, mip, layer and origin, the resource's description, and whether it is
    // in the stencil plane.
    let region = |d: &Device<B>, id: ResourceId, index: u32, origin: Origin| {
        let r = &d.resources[id.0 as usize];
        let (mips, layers) = (r.desc.mips(), r.desc.layers());
        let (texture, _) = d.texture(id)?;
        let stencil = index / (mips * layers) == 1;
        Ok::<_, Error>((
            texture.clone(),
            index % mips,
            index / mips % layers,
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
        width: ((desc.width >> mip) as u32).max(1),
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
            if heap == HeapType::Readback {
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
