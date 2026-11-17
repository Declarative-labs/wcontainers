//! The Direct3D 12 device on one back end: its objects, the one queue order every guest queue shares,
//! the replay of command lists, and fences.

use std::collections::HashMap;
use std::sync::Arc;

use slab::Slab;
use wcontainers_directx_graphics_infrastructure::{SwapChain, SwapChainDesc, SwapChainError};
use wcontainers_gpu::{
    AspectOps, Backend, BufferDesc, BufferUsage, ClearValue, ColorAttachment, CopyBufferTexture,
    DepthAttachment, DrawCall, DrawRange, Extent, Features, FormatKind, GpuError, IndexBinding,
    IndexFormat, LoadOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, MapIntent, Object, Origin,
    PassDesc, PrepareDesc, Program, ReadSource, ReadState, SamplerDesc, ScissorRect, ShaderStage,
    SlowPath, StageSet, StepMode, StoreOp, TargetLayout, TextureDesc, TextureDimension,
    TextureRegion, TextureUsage, TextureViewDesc, Topology, VariantKey, VertexAttribute,
    VertexBinding, VertexLayoutDesc, VertexStream, ViewDimension, Viewport, WriteMode,
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
    HeapId
);

mod bind;

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
    .union(BufferUsage::INDIRECT)
    .union(BufferUsage::COPY_SRC)
    .union(BufferUsage::COPY_DST);
/// Readback buffers are only written by copies and read back.
const READBACK_BUFFER: BufferUsage = BufferUsage::COPY_DST.union(BufferUsage::READ_BACK);

enum Storage<B: Backend> {
    Buffer {
        buffer: B::Buffer,
        /// The bytes `Map` hands out, for upload and readback heaps.
        host: Option<Box<[u8]>>,
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
}

/// What a pipeline state object holds.
struct Pipeline<B: Backend> {
    program: B::Program,
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
    /// What the program's stages bind, and the size each stage's uniform blocks declare.
    bindings: Vec<bind::StageBinding>,
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

/// A descriptor heap: the slots CPU handles point into, and the first GPU handle.
struct Heap {
    slots: Box<[Descriptor]>,
    gpu: u64,
}

/// Where GPU descriptor handles start, far from GPU virtual addresses.
const HANDLE_BASE: u64 = 1 << 56;

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
    /// Events to set when a fence reaches a value: fence, value, and the shell's token for the event.
    waiters: Vec<(FenceId, u64, u64)>,
    chains: Slab<Chain<B>>,
    /// Messages not yet shown, such as why a shader did not convert.
    messages: Vec<String>,
    removed: Option<&'static str>,
}

/// A swap chain and the resources standing for its back buffers.
struct Chain<B: Backend> {
    chain: SwapChain<B>,
    buffers: Vec<ResourceId>,
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
            messages: Vec::new(),
            removed: None,
        }
    }

    pub fn backend(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Why the device was removed, if it was (`GetDeviceRemovedReason`).
    pub fn removed(&self) -> Option<&'static str> {
        self.removed
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
        self.live()?;
        let id = ResourceId(self.resources.vacant_key() as u32);
        let (storage, address) = if desc.dimension == resource::DIMENSION_BUFFER {
            if desc.width == 0 {
                return Err(Error::Invalid("a buffer of zero bytes"));
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
                .then(|| vec![0u8; desc.width as usize].into_boxed_slice());
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
            if desc.flags & (resource::FLAG_RENDER_TARGET | resource::FLAG_DEPTH_STENCIL) != 0 {
                usage |= TextureUsage::RENDER;
            }
            if desc.flags & resource::FLAG_UNORDERED_ACCESS != 0 {
                usage |= TextureUsage::STORAGE;
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
                mip_levels: desc.mips(),
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
        });
        Ok(id)
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
            }) => Ok(host.as_mut_ptr()),
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
        let root = desc
            .root
            .filter(|r| self.roots.contains(r.0 as usize))
            .ok_or(Error::Invalid("a pipeline needs its root signature"))?;
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
        bind::check(&self.roots[root.0 as usize], &bindings)?;
        let caps = self.backend.caps();
        let blend = desc.blend.block(caps, desc.rtv_formats.len())?;
        let raster = desc.raster.block(caps, desc.sample_mask, targets.samples)?;
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
        let pipeline = Pipeline {
            program: program?,
            blend: self.backend.create_blend(&blend)?,
            depth_stencil: self.backend.create_depth_stencil(&depth_stencil)?,
            raster: self.backend.create_raster(&raster)?,
            inputs,
            targets,
            strip_cut: desc.strip_cut,
            reads_bases,
            bindings,
        };
        let packed = packed_strides(&pipeline.inputs);
        let layout = self.vertex_layout(&pipeline.inputs, &packed)?;
        self.backend.prepare(&PrepareDesc {
            program: &pipeline.program,
            blend: &pipeline.blend,
            depth_stencil: &pipeline.depth_stencil,
            raster: &pipeline.raster,
            vertex_layout: &layout,
            targets,
            topology,
            layouts: Default::default(),
        });
        Ok(PipelineId(self.pipelines.insert(pipeline) as u32))
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
            self.backend.destroy(Object::Blend(p.blend));
            self.backend.destroy(Object::DepthStencil(p.depth_stencil));
            self.backend.destroy(Object::Raster(p.raster));
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

    // ---- Fences ----

    pub fn create_fence(&mut self, initial: u64) -> FenceId {
        FenceId(self.fences.insert(Fence {
            value: initial,
            pending: Vec::new(),
        }) as u32)
    }

    pub fn destroy_fence(&mut self, id: FenceId) {
        self.fences.try_remove(id.0 as usize);
        self.waiters.retain(|w| w.0 != id);
    }

    /// `SetEventOnCompletion` with an event: `token` comes back from [`Device::reached_events`]
    /// once the fence reaches `value`. Nothing waits here.
    pub fn add_waiter(&mut self, id: FenceId, value: u64, token: u64) {
        self.waiters.push((id, value, token));
    }

    pub fn has_waiters(&self) -> bool {
        !self.waiters.is_empty()
    }

    /// The tokens of events whose fence has reached its value, asking the back end without waiting.
    pub fn reached_events(&mut self) -> Vec<u64> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.waiters.len() {
            let (id, value, token) = self.waiters[i];
            if self.completed(id) >= value {
                out.push(token);
                self.waiters.swap_remove(i);
            } else {
                i += 1;
            }
        }
        out
    }

    /// `ID3D12CommandQueue::Signal`: the fence reaches `value` when the work submitted so far is done.
    pub fn queue_signal(&mut self, id: FenceId, value: u64) -> Result<(), Error> {
        self.live()?;
        let fence = self.backend.submit();
        self.fences
            .get_mut(id.0 as usize)
            .ok_or(Error::Invalid("no such fence"))?
            .pending
            .push((value, fence));
        Ok(())
    }

    /// `ID3D12Fence::Signal` from the CPU.
    pub fn cpu_signal(&mut self, id: FenceId, value: u64) {
        if let Some(f) = self.fences.get_mut(id.0 as usize) {
            f.value = value;
        }
    }

    /// `GetCompletedValue`. Read backs whose work is done are in readback memory first (waiting only
    /// for their bytes to arrive), so the program sees the bytes when it sees the value.
    pub fn completed(&mut self, id: FenceId) -> u64 {
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
                    .finish_read(r.readback, &mut host[r.offset..r.offset + r.size]);
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
        Ok(ChainId(self.chains.insert(Chain { chain, buffers }) as u32))
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
    pub fn execute(&mut self, lists: &[Arc<[Command]>]) -> Result<(), Error> {
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
            l.iter().try_for_each(|c| replay.run(self, c))?;
            replay.finish(self)
        });
        let submission = self.backend.submit();
        for r in &mut self.reads[first_read..] {
            r.submission = Some(submission.clone());
        }
        if let Err(e) = result {
            let reason = match e {
                Error::Invalid(s) | Error::NotImplemented(s) => s,
                Error::OutOfMemory => "out of memory",
                Error::DeviceRemoved => "device removed",
            };
            self.removed = Some(reason);
            return Err(Error::DeviceRemoved);
        }
        Ok(())
    }

    /// Adds the upload resources command `c` reads.
    fn uploads_read(&self, c: &Command, out: &mut Vec<ResourceId>) {
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
        self.texture_view(
            v.resource,
            TextureViewDesc {
                format: dxgi(v.format)?,
                dimension: if three_d {
                    ViewDimension::D3
                } else if v.layers > 1 {
                    ViewDimension::D2Array
                } else {
                    ViewDimension::D2
                },
                base_mip: v.mip,
                mip_count: 1,
                base_layer: if three_d { 0 } else { v.first_layer },
                layer_count: if three_d { 1 } else { v.layers.max(1) },
                stencil: false,
            },
        )
    }

    /// The back end's view `desc` of texture `resource`, made once.
    fn texture_view(
        &mut self,
        resource: ResourceId,
        desc: TextureViewDesc,
    ) -> Result<B::TextureView, Error> {
        let Some(Resource {
            storage: Storage::Texture { texture, views, .. },
            ..
        }) = self.resources.get_mut(resource.0 as usize)
        else {
            return Err(Error::Invalid("a texture view of no texture"));
        };
        if let Some((_, view)) = views.iter().find(|(k, _)| *k == desc) {
            return Ok(view.clone());
        }
        let view = self.backend.create_view(texture, &desc)?;
        views.push((desc, view.clone()));
        Ok(view)
    }

    /// Records a read back of `size` bytes at `offset` of a readback buffer, finished into its
    /// memory when the submission is done.
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
struct Replay {
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
    graphics: bind::RootState,
    /// Whole-view clears not yet done: each becomes the next pass's `Clear` load, or a clear of its
    /// own before anything else reads the texture.
    clears: Vec<(TargetView, ClearValue)>,
}

impl Replay {
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
            graphics: bind::RootState::default(),
            clears: Vec::new(),
        }
    }

    fn end_pass<B: Backend>(&mut self, d: &mut Device<B>) {
        if self.pass_open {
            d.backend.end_pass();
            self.pass_open = false;
        }
    }

    /// Ends any pass and does the clears still waiting, so copies see them.
    fn settle<B: Backend>(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        self.end_pass(d);
        for (v, value) in std::mem::take(&mut self.clears) {
            let (texture, desc) = d.texture(v.resource)?;
            let (texture, desc) = (texture.clone(), *desc);
            d.backend.clear_texture(
                &texture,
                &desc,
                dxgi(v.format)?,
                v.mip..v.mip + 1,
                v.first_layer..v.first_layer + v.layers.max(1),
                value,
            )?;
        }
        Ok(())
    }

    fn finish<B: Backend>(&mut self, d: &mut Device<B>) -> Result<(), Error> {
        self.settle(d)
    }

    fn run<B: Backend>(&mut self, d: &mut Device<B>, c: &Command) -> Result<(), Error> {
        match c {
            Command::Pipeline(p) => self.pipeline = Some(*p),
            Command::RootSignature { compute, root } => {
                if *compute {
                    return Err(Error::NotImplemented("compute root signatures"));
                }
                let signature = root.and_then(|r| d.roots.get(r.0 as usize).cloned());
                if root.is_some() && signature.is_none() {
                    return Err(Error::Invalid("no such root signature"));
                }
                self.graphics.set_root(*root, signature.as_deref());
            }
            Command::RootArgument {
                compute,
                index,
                argument,
            } => {
                if *compute {
                    return Err(Error::NotImplemented("compute root arguments"));
                }
                *self
                    .graphics
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
                if *compute {
                    return Err(Error::NotImplemented("compute root constants"));
                }
                let at = *offset as usize;
                self.graphics
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
                    let view = d.view(v)?;
                    let rects: Vec<_> = rects.iter().map(scissor).collect();
                    d.backend.clear_rects(
                        &view,
                        dxgi(v.format)?,
                        ClearValue::Color(value),
                        &rects,
                    )?;
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
                    let view = d.view(v)?;
                    let rects: Vec<_> = rects.iter().map(scissor).collect();
                    d.backend.clear_rects(
                        &view,
                        format,
                        ClearValue::DepthStencil { depth, stencil },
                        &rects,
                    )?;
                }
            }
            Command::Draw {
                vertices,
                instances,
                first_vertex,
                first_instance,
            } => self.draw(
                d,
                DrawRange::Vertices {
                    first: *first_vertex,
                    count: *vertices,
                    first_instance: *first_instance,
                    instances: *instances,
                },
            )?,
            Command::DrawIndexed {
                indices,
                instances,
                first_index,
                base_vertex,
                first_instance,
            } => self.draw(
                d,
                DrawRange::Indexed {
                    first: *first_index,
                    count: *indices,
                    base_vertex: *base_vertex,
                    first_instance: *first_instance,
                    instances: *instances,
                },
            )?,
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
        }
        Ok(())
    }

    fn draw<B: Backend>(
        &mut self,
        d: &mut Device<B>,
        range: DrawRange<'_, B>,
    ) -> Result<(), Error> {
        let pid = self
            .pipeline
            .ok_or(Error::Invalid("drawing with no pipeline state"))?;
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
        if bases && d.pipelines[pid.0 as usize].reads_bases {
            return Err(Error::NotImplemented(
                "SV_VertexID or SV_InstanceID in a draw with a first vertex or instance",
            ));
        }
        if !self.pass_open {
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
                occlusion: None,
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
                return Err(Error::NotImplemented("drawing with no render target"));
            }
            d.backend.begin_pass(&pass);
            self.pass_open = true;
        }
        let mut strides = [0; MAX_VERTEX_STREAMS];
        for (s, v) in strides.iter_mut().zip(&self.vertex_buffers) {
            *s = v.stride;
        }
        let inputs = std::mem::take(&mut d.pipelines[pid.0 as usize].inputs);
        let layout = d.vertex_layout(&inputs, &strides);
        let mut used = [false; MAX_VERTEX_STREAMS];
        inputs
            .iter()
            .for_each(|(e, _)| used[e.slot as usize] = true);
        d.pipelines[pid.0 as usize].inputs = inputs;
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
            (DrawRange::Indexed { .. }, Some(v)) => {
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
            (DrawRange::Indexed { .. }, None) => {
                return Err(Error::Invalid("an indexed draw with no index buffer"));
            }
            _ => None,
        };
        let p = &d.pipelines[pid.0 as usize];
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
        let sets = d.binding_sets(&self.graphics, pid)?;
        let p = &d.pipelines[pid.0 as usize];
        let call = DrawCall {
            program: &p.program,
            blend: &p.blend,
            depth_stencil: &p.depth_stencil,
            raster: &p.raster,
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
            viewports: &self.viewports,
            scissors: &self.scissors,
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
    let region = |d: &Device<B>, id: ResourceId, index: u32, origin: Origin| {
        let r = &d.resources[id.0 as usize];
        let mips = r.desc.mips();
        let (texture, _) = d.texture(id)?;
        Ok::<_, Error>((texture.clone(), index % mips, index / mips, origin, r.desc))
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
            let (texture, mip, layer, o, _) = region(d, *s, *index, origin(src_origin))?;
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
                    stencil: false,
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
            let (texture, mip, layer, o, _) = region(d, *t, *index, origin(at))?;
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
                    stencil: false,
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
            let (st, smip, slayer, so, sdesc) = region(d, *s, *si, origin(src_origin))?;
            let (tt, tmip, tlayer, to, _) = region(d, *t, *ti, origin(at))?;
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
