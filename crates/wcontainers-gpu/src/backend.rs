//! The [`Backend`] trait: the base every back end provides, and the fast paths with their defaults.

use crate::caps::{Caps, Features};
use crate::draw::{
    AspectOps, CaptureTargets, ColorAttachment, DepthAttachment, DispatchCall, DrawCall, DrawRange,
    IndexBinding, LoadOp, PassDesc, ScissorRect, StoreOp, Viewport,
};
use crate::error::{DeviceStatus, GpuError};
use crate::format::{Format, FormatKind};
use crate::helpers::{self, Builtin, Helpers, PARAMS_SIZE, Ring};
use crate::resource::{
    BindingEntry, BindingResource, BufferDesc, BufferRange, BufferUsage, CopyBufferTexture, Extent,
    MapIntent, QueryKind, ReadSource, ReadState, SamplerDesc, TextureDesc, TextureDimension,
    TextureRegion, TextureViewDesc, ViewDimension, WriteMode,
};
use crate::shader::{Program, ShaderModule, StageSet};
use crate::slow::{SlowPath, SlowPaths};
use crate::state::{
    BlendDesc, DepthStencilDesc, IndexFormat, ProvokingVertex, RasterDesc, TargetLayout, Topology,
    VertexLayoutDesc,
};
use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

/// Bytes of the ring [`Backend::write_buffer_ring`] starts with; it grows to fit larger writes.
const RING_SIZE: u64 = 1 << 22;

/// Where the vertices of a fan or loop come from.
#[derive(Clone, Copy, Debug)]
pub enum IndexSource<'a> {
    /// Vertices `first` to `first + count - 1`, in order.
    Sequential { first: u32, count: u32 },
    /// Index values read from guest memory, in place.
    Guest {
        indices: &'a [u8],
        format: IndexFormat,
        base_vertex: i32,
    },
}

/// The primitive kinds [`Backend::draw_fan_or_loop`] draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanOrLoop {
    TriangleFan,
    LineLoop,
}

/// A clear value for [`Backend::clear_texture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearValue {
    /// Raw bits: f32 for float formats, the integer for integer formats.
    Color([u32; 4]),
    /// Depth as f32 bits and stencil; None leaves that aspect as it is.
    DepthStencil {
        depth: Option<u32>,
        stencil: Option<u8>,
    },
}

/// One combination of state blocks to build ahead of drawing with it.
#[derive(Debug)]
pub struct PrepareDesc<'a, B: Backend> {
    pub program: &'a B::Program,
    pub blend: &'a B::Blend,
    pub depth_stencil: &'a B::DepthStencil,
    pub raster: &'a B::Raster,
    pub vertex_layout: &'a B::VertexLayout,
    pub targets: TargetLayout,
    pub topology: Topology,
}

/// Any object a back end made, for [`Backend::destroy`].
#[derive(Debug)]
pub enum Object<B: Backend> {
    Buffer(B::Buffer),
    Texture(B::Texture),
    TextureView(B::TextureView),
    Sampler(B::Sampler),
    Shader(B::Shader),
    Program(B::Program),
    Blend(B::Blend),
    DepthStencil(B::DepthStencil),
    Raster(B::Raster),
    VertexLayout(B::VertexLayout),
    BindingSet(B::BindingSet),
    QuerySet(B::QuerySet),
    ResourceTable(B::ResourceTable),
    Surface(B::Surface),
}

/// A GPU back end. One thread owns it; every method is one ordinary call.
///
/// The base methods have no default. The fast paths have a default made from the base. The absent-or-
/// present methods default to [`GpuError::Absent`] and are overridden by back ends that declare the
/// matching [`Features`] bit.
///
/// Objects are named by handles. A handle is cheap to clone and a clone names the same object, which is
/// destroyed once, through any of its handles.
pub trait Backend: Sized {
    type Buffer: Clone + Debug;
    type Texture: Clone + Debug;
    type TextureView: Clone + Debug;
    type Sampler: Clone + Debug;
    /// A shader in the back end's own language, built from a [`ShaderModule`].
    type Shader: Clone + Debug;
    type Program: Clone + Debug;
    type Blend: Clone + Debug;
    type DepthStencil: Clone + Debug;
    type Raster: Clone + Debug;
    type VertexLayout: Clone + Debug;
    type BindingSet: Clone + Debug;
    type QuerySet: Clone + Debug;
    /// A read back in flight.
    type Readback: Clone + Debug;
    /// A point in the GPU's work, for waiting on.
    type Fence: Clone + Debug;
    type ResourceTable: Clone + Debug;
    /// A host window's drawable area.
    type Surface: Clone + Debug;

    // ---- What the back end is and how it is doing ----

    /// What this back end can do. Fixed for the device's life.
    fn caps(&self) -> &Caps;
    /// The slow-path counters, which fronts and shared helpers also bump.
    fn slow_paths(&mut self) -> &mut SlowPaths;
    /// Whether the device still works.
    fn status(&mut self) -> DeviceStatus;
    /// What the fast-path defaults keep between calls. Back ends hold a `Helpers::default()`.
    fn helpers(&mut self) -> &mut Helpers<Self>;

    // ---- Objects ----

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Self::Buffer, GpuError>;
    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Self::Texture, GpuError>;
    fn create_view(
        &mut self,
        texture: &Self::Texture,
        desc: &TextureViewDesc,
    ) -> Result<Self::TextureView, GpuError>;
    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Self::Sampler, GpuError>;
    /// Builds a shader from the common form. Back ends may keep it to build later, when its program
    /// and state are known.
    fn create_shader(&mut self, module: &ShaderModule) -> Result<Self::Shader, GpuError>;
    /// Makes a program. It keeps what it needs of its shaders, which may be destroyed after.
    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<Self::Program, GpuError>;
    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Self::Blend, GpuError>;
    fn create_depth_stencil(
        &mut self,
        desc: &DepthStencilDesc,
    ) -> Result<Self::DepthStencil, GpuError>;
    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Self::Raster, GpuError>;
    fn create_vertex_layout(
        &mut self,
        desc: &VertexLayoutDesc,
    ) -> Result<Self::VertexLayout, GpuError>;
    /// A set of bound resources, as one block. Back ends may return an equal set they already hold.
    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<Self::BindingSet, GpuError>;
    fn create_query_set(&mut self, kind: QueryKind, count: u32)
    -> Result<Self::QuerySet, GpuError>;
    /// Releases an object. Work already recorded that uses it still sees it; the back end frees it when
    /// that work is done.
    fn destroy(&mut self, object: Object<Self>);

    // ---- Bytes in ----

    /// Writes bytes into a buffer, at any byte offset and length. `data` is read during the call and not
    /// after; fronts pass slices of blocks they own in guest memory, so nothing is copied before this
    /// call (plan section 4.6 says which blocks). `mode` says what the caller promises about the bytes
    /// already there.
    fn write_buffer(&mut self, buffer: &Self::Buffer, offset: u64, data: &[u8], mode: WriteMode);
    /// Writes texels into one region of a texture, read during the call as for
    /// [`Backend::write_buffer`]. `row_pitch` is the bytes between rows of blocks in `data`.
    /// [`WriteMode::Discard`] drops the whole texture's old contents (Direct3D's dynamic textures), so a
    /// back end may rename it instead of ordering the write.
    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    );

    // ---- Recording ----

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>);
    fn end_pass(&mut self);
    /// Draws lists, strips and points. Inside a pass.
    fn draw(&mut self, call: &DrawCall<'_, Self>);
    /// Starts counting into query `index` of the pass's occlusion set. Inside a pass.
    fn begin_query(&mut self, index: u32);
    fn end_query(&mut self);
    fn copy_buffer(
        &mut self,
        src: &Self::Buffer,
        src_offset: u64,
        dst: &Self::Buffer,
        dst_offset: u64,
        size: u64,
    );
    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    );
    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    );
    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    );
    /// Fills a range of a buffer with `pattern` repeated (1, 2, 4, 8 or 16 bytes; the range is a whole
    /// number of patterns): zero for most uses, an element's bits for Direct3D's storage view clears.
    fn clear_buffer(&mut self, buffer: &Self::Buffer, offset: u64, size: u64, pattern: &[u8]);
    /// Resolves a whole multisampled subresource into a single-sampled one.
    fn resolve(&mut self, src: &TextureRegion<'_, Self>, dst: &TextureRegion<'_, Self>);
    /// Sends everything recorded so far to the GPU, and returns a fence for it.
    fn submit(&mut self) -> Self::Fence;
    /// Whether the GPU has finished the work before `fence`.
    fn fence_done(&mut self, fence: &Self::Fence) -> bool;
    /// Pauses the device thread until `fence` is done.
    fn wait_fence(&mut self, fence: &Self::Fence);

    // ---- Bytes out ----

    /// Starts reading data back. The copy is recorded now and finishes after a later [`Backend::submit`].
    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Self::Readback, GpuError>;
    /// Starts reading query results back, one u64 per query.
    fn read_queries(
        &mut self,
        set: &Self::QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Self::Readback, GpuError>;
    fn read_state(&mut self, readback: &Self::Readback) -> ReadState;
    /// Pauses the device thread until the read back is ready, submitting first if needed.
    fn wait_read(&mut self, readback: &Self::Readback);
    /// Copies a ready read back into guest memory. Rows of textures arrive tightly packed.
    fn finish_read(&mut self, readback: Self::Readback, dst: &mut [u8]);

    // ---- Frames ----

    /// Attaches to a host window, given by the number the host's graphics hook uses for it.
    fn create_surface(&mut self, window: u32) -> Result<Self::Surface, GpuError>;
    /// Sets the size, format and whether presenting waits for the display.
    fn configure_surface(
        &mut self,
        surface: &Self::Surface,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError>;
    /// The texture to draw this frame's picture into.
    fn current_texture(&mut self, surface: &Self::Surface) -> Result<Self::TextureView, GpuError>;
    /// Shows the current texture and submits.
    fn present(&mut self, surface: &Self::Surface) -> Result<(), GpuError>;

    // ---- Fast paths, each with a default made from the base ----

    /// Reads back and waits for the answer. The default starts a read back and waits for it, which pauses
    /// the device thread. WebGL 2 overrides it with its own blocking reads.
    fn read_back_now(
        &mut self,
        src: &ReadSource<'_, Self>,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
        let rb = self.read_back(src)?;
        if self.read_state(&rb) == ReadState::Pending {
            self.slow_paths().hit(SlowPath::ReadBackWait);
            self.wait_read(&rb);
        }
        self.finish_read(rb, dst);
        Ok(())
    }

    /// Draws a triangle fan or a line loop. `call.topology`, `call.index` and `call.range` are ignored:
    /// the vertices come from `source`, and instances from `instances`. `provoking` is the convention
    /// of the front's API, so flat-shaded values come from the vertex it names. The default builds a
    /// triangle list or a line list in a fresh index buffer and draws that. WebGL 2 overrides it.
    fn draw_fan_or_loop(
        &mut self,
        call: &DrawCall<'_, Self>,
        kind: FanOrLoop,
        source: IndexSource<'_>,
        provoking: ProvokingVertex,
        first_instance: u32,
        instances: u32,
    ) -> Result<(), GpuError> {
        let (indices, base_vertex) = fan_or_loop_indices(kind, source, call.restart, provoking);
        if indices.is_empty() {
            return Ok(());
        }
        let bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let buffer = self.create_buffer(&BufferDesc {
            size: bytes.len() as u64,
            usage: BufferUsage::INDEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })?;
        self.write_buffer(&buffer, 0, &bytes, WriteMode::Discard);
        let (topology, path) = match kind {
            FanOrLoop::TriangleFan => (Topology::TriangleList, SlowPath::FanConverted),
            FanOrLoop::LineLoop => (Topology::LineList, SlowPath::LoopConverted),
        };
        self.slow_paths().hit(path);
        self.draw(&DrawCall {
            index: Some(IndexBinding {
                buffer: &buffer,
                offset: 0,
                format: IndexFormat::U32,
            }),
            topology,
            restart: false,
            range: DrawRange::Indexed {
                first: 0,
                count: indices.len() as u32,
                base_vertex,
                first_instance,
                instances,
            },
            ..*call
        });
        self.destroy(Object::Buffer(buffer));
        Ok(())
    }

    /// Builds a state combination ahead of its first draw. The default does nothing, so the first draw
    /// builds it. WebGPU overrides it to compile the pipeline in the background.
    fn prepare(&mut self, desc: &PrepareDesc<'_, Self>) {
        let _ = desc;
    }

    /// Clears a whole view outside a pass. The default opens a pass that clears it and closes it again.
    /// WebGL 2 overrides it with its own clears.
    fn clear_texture(&mut self, view: &Self::TextureView, format: Format, value: ClearValue) {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        match value {
            ClearValue::Color(c) => {
                pass.colors[0] = Some(ColorAttachment {
                    view,
                    depth_slice: 0,
                    resolve: None,
                    load: LoadOp::Clear(c),
                    store: StoreOp::Store,
                });
            }
            ClearValue::DepthStencil { depth, stencil } => {
                let (has_depth, has_stencil) = aspects(format);
                pass.depth = Some(DepthAttachment {
                    view,
                    depth: has_depth.then_some(AspectOps {
                        load: depth.map_or(LoadOp::Load, LoadOp::Clear),
                        store: StoreOp::Store,
                        read_only: false,
                    }),
                    stencil: has_stencil.then_some(AspectOps {
                        load: stencil.map_or(LoadOp::Load, LoadOp::Clear),
                        store: StoreOp::Store,
                        read_only: false,
                    }),
                });
            }
        }
        self.begin_pass(&pass);
        self.end_pass();
    }

    /// Clears rectangles of one view outside a pass (Direct3D 11.1's ClearView, Direct3D 12's clears
    /// with rectangles). `view` is one mip level and one layer; the rectangles lie inside it. The
    /// default draws a full-screen triangle per rectangle, scissored to it, with a built-in program
    /// writing the value. WebGL 2 overrides it with scissored clears.
    fn clear_rects(
        &mut self,
        view: &Self::TextureView,
        format: Format,
        value: ClearValue,
        rects: &[ScissorRect],
    ) -> Result<(), GpuError> {
        if rects.is_empty() {
            return Ok(());
        }
        let b = helpers::builtins(self)?;
        let mut params = [0u8; PARAMS_SIZE];
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        fn keep<V>(read_only: bool) -> AspectOps<V> {
            AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only,
            }
        }
        let (program, depth_stencil, stencil_reference) = match value {
            ClearValue::Color(c) => {
                for (i, v) in c.iter().enumerate() {
                    params[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
                pass.colors[0] = Some(ColorAttachment {
                    view,
                    depth_slice: 0,
                    resolve: None,
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                });
                let program = match format.kind() {
                    FormatKind::Uint => Builtin::ClearUint,
                    FormatKind::Sint => Builtin::ClearSint,
                    _ => Builtin::ClearFloat,
                };
                (program, &b.no_depth, 0)
            }
            ClearValue::DepthStencil { depth, stencil } => {
                let (has_depth, has_stencil) = aspects(format);
                let depth = depth.filter(|_| has_depth);
                let stencil = stencil.filter(|_| has_stencil);
                let which = match (depth, stencil) {
                    (None, None) => return Ok(()),
                    (Some(_), None) => 0,
                    (None, Some(_)) => 1,
                    (Some(_), Some(_)) => 2,
                };
                params[20..24].copy_from_slice(&depth.unwrap_or(0).to_le_bytes());
                pass.depth = Some(DepthAttachment {
                    view,
                    depth: has_depth.then_some(keep(depth.is_none())),
                    stencil: has_stencil.then_some(keep(stencil.is_none())),
                });
                let program = if depth.is_some() {
                    Builtin::ClearDepth
                } else {
                    Builtin::ClearStencil
                };
                (program, &b.write_depth_stencil[which], stencil.unwrap_or(0))
            }
        };
        let set = if program == Builtin::ClearStencil {
            None
        } else {
            let align = self.caps().limits.min_uniform_offset_alignment;
            let mut offset = [0];
            let ring = self.write_buffer_ring(&[&params], align, &mut offset)?;
            Some(self.create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ring,
                    offset: offset[0],
                    size: PARAMS_SIZE as u64,
                }),
            }])?)
        };
        let width = rects.iter().map(|r| r.x + r.width).max().unwrap_or(0);
        let height = rects.iter().map(|r| r.y + r.height).max().unwrap_or(0);
        let viewport = [full_viewport(width, height)];
        self.begin_pass(&pass);
        for rect in rects {
            self.draw(&DrawCall {
                bindings: [set.as_ref(), None, None, None],
                depth_stencil,
                stencil_reference: [stencil_reference; 2],
                scissors: core::slice::from_ref(rect),
                ..builtin_draw(&b, program, &viewport)
            });
        }
        self.end_pass();
        if let Some(set) = set {
            self.destroy(Object::BindingSet(set));
        }
        Ok(())
    }

    /// Fills mip levels `mips` (each from the level above) of `layers` of a texture made from `desc`,
    /// outside a pass (Direct3D's GenerateMips, OpenGL's glGenerateMipmap). The default draws each level
    /// and layer, or each slice of a 3D level, with a built-in program sampling the level above through a
    /// linear sampler. WebGL 2 overrides it with generateMipmap where the format allows.
    fn generate_mips(
        &mut self,
        texture: &Self::Texture,
        desc: &TextureDesc,
        mips: Range<u32>,
        layers: Range<u32>,
    ) -> Result<(), GpuError> {
        let b = helpers::builtins(self)?;
        let three_d = desc.dimension == TextureDimension::D3;
        let (dimension, program, binding) = if three_d {
            (ViewDimension::D3, Builtin::Mip3d, 33)
        } else {
            (ViewDimension::D2, Builtin::Mip2d, 32)
        };
        let layers = if three_d { 0..1 } else { layers };
        for mip in mips.start.max(1)..mips.end.min(desc.mip_levels) {
            let size = |n: u32| (n >> mip).max(1);
            let (width, height) = (size(desc.width), size(desc.height));
            let slices = if three_d {
                size(desc.depth_or_layers)
            } else {
                1
            };
            let viewport = [full_viewport(width, height)];
            let scissor = [ScissorRect {
                x: 0,
                y: 0,
                width,
                height,
            }];
            for layer in layers.clone() {
                let view = |base_mip| TextureViewDesc {
                    format: desc.format,
                    dimension,
                    base_mip,
                    mip_count: 1,
                    base_layer: layer,
                    layer_count: 1,
                    stencil: false,
                };
                let src = self.create_view(texture, &view(mip - 1))?;
                let dst = self.create_view(texture, &view(mip))?;
                for slice in 0..slices {
                    let ring;
                    let mut entries = alloc::vec![
                        BindingEntry {
                            binding: 16,
                            stages: StageSet::FRAGMENT,
                            resource: BindingResource::Sampler(&b.linear),
                        },
                        BindingEntry {
                            binding,
                            stages: StageSet::FRAGMENT,
                            resource: BindingResource::Texture(&src),
                        },
                    ];
                    if three_d {
                        let mut params = [0u8; PARAMS_SIZE];
                        let w = (slice as f32 + 0.5) / slices as f32;
                        params[16..20].copy_from_slice(&w.to_bits().to_le_bytes());
                        let align = self.caps().limits.min_uniform_offset_alignment;
                        let mut offset = [0];
                        ring = self.write_buffer_ring(&[&params], align, &mut offset)?;
                        entries.push(BindingEntry {
                            binding: 0,
                            stages: StageSet::FRAGMENT,
                            resource: BindingResource::UniformBuffer(BufferRange {
                                buffer: &ring,
                                offset: offset[0],
                                size: PARAMS_SIZE as u64,
                            }),
                        });
                    }
                    let set = self.create_binding_set(&entries)?;
                    let mut pass = PassDesc {
                        colors: Default::default(),
                        depth: None,
                        empty: None,
                        occlusion: None,
                    };
                    pass.colors[0] = Some(ColorAttachment {
                        view: &dst,
                        depth_slice: slice,
                        resolve: None,
                        load: LoadOp::DontCare,
                        store: StoreOp::Store,
                    });
                    self.begin_pass(&pass);
                    self.draw(&DrawCall {
                        bindings: [Some(&set), None, None, None],
                        scissors: &scissor,
                        ..builtin_draw(&b, program, &viewport)
                    });
                    self.end_pass();
                    self.destroy(Object::BindingSet(set));
                }
                self.destroy(Object::TextureView(src));
                self.destroy(Object::TextureView(dst));
            }
        }
        Ok(())
    }

    /// Writes the data one draw needs (Direct3D 9's vertices from user memory, OpenGL's client arrays,
    /// spilled immediates) into a ring buffer usable as vertices, indices and uniforms. Each part starts
    /// at a multiple of `align`; its offset goes into `offsets`. The returned buffer and offsets stay
    /// valid for commands recorded before the next call. The default writes with
    /// [`WriteMode::NoOverwrite`], and with [`WriteMode::Discard`] when it wraps, so nothing waits. No
    /// back end overrides it today; a native back end would map the ring.
    fn write_buffer_ring(
        &mut self,
        parts: &[&[u8]],
        align: u32,
        offsets: &mut [u64],
    ) -> Result<Self::Buffer, GpuError> {
        let align = u64::from(align.max(4));
        let total: u64 = parts
            .iter()
            .map(|p| (p.len() as u64).next_multiple_of(align))
            .sum();
        let fits = matches!(&self.helpers().ring, Some(r) if r.size >= total);
        if !fits {
            let size = total.max(RING_SIZE).next_power_of_two();
            let buffer = self.create_buffer(&BufferDesc {
                size,
                usage: BufferUsage::VERTEX
                    | BufferUsage::INDEX
                    | BufferUsage::UNIFORM
                    | BufferUsage::COPY_DST,
                intent: MapIntent::Often,
            })?;
            let old = self.helpers().ring.replace(Ring {
                buffer,
                size,
                head: 0,
            });
            if let Some(old) = old {
                self.destroy(Object::Buffer(old.buffer));
            }
        }
        let Some(ring) = &self.helpers().ring else {
            unreachable!("the ring was made above")
        };
        let (buffer, size, mut head) = (ring.buffer.clone(), ring.size, ring.head);
        if total == 0 {
            offsets.iter_mut().for_each(|o| *o = head);
            return Ok(buffer);
        }
        let mut mode = WriteMode::NoOverwrite;
        if head.next_multiple_of(align) + total > size {
            head = 0;
            mode = WriteMode::Discard;
        }
        for (part, offset) in parts.iter().zip(offsets.iter_mut()) {
            head = head.next_multiple_of(align);
            *offset = head;
            let whole = part.len() & !3;
            if whole > 0 {
                self.write_buffer(&buffer, head, &part[..whole], mode);
                mode = WriteMode::NoOverwrite;
            }
            let tail = &part[whole..];
            if !tail.is_empty() {
                let mut word = [0u8; 4];
                word[..tail.len()].copy_from_slice(tail);
                self.write_buffer(&buffer, head + whole as u64, &word, mode);
                mode = WriteMode::NoOverwrite;
            }
            head += part.len() as u64;
        }
        if let Some(ring) = &mut self.helpers().ring {
            ring.head = head;
        }
        Ok(buffer)
    }

    // ---- Present or absent: no cheap default ----

    /// Runs a compute program. Outside a pass. Needs [`Features::COMPUTE`].
    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        let _ = call;
        Err(GpuError::Absent(Features::COMPUTE))
    }

    /// Draws with nothing rasterised, writing the last vertex stage's outputs to `targets` in order.
    /// Outside a pass. Needs [`Features::TRANSFORM_FEEDBACK`].
    fn capture_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: CaptureTargets<'_, Self>,
    ) -> Result<(), GpuError> {
        let _ = (call, targets);
        Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK))
    }

    /// Makes a big table of resources a program indexes. Needs [`Features::RESOURCE_TABLES`].
    fn create_resource_table(&mut self, size: u32) -> Result<Self::ResourceTable, GpuError> {
        let _ = size;
        Err(GpuError::Absent(Features::RESOURCE_TABLES))
    }

    /// Fills slots of a resource table, from `first` on. Needs [`Features::RESOURCE_TABLES`].
    fn write_resource_table(
        &mut self,
        table: &Self::ResourceTable,
        first: u32,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<(), GpuError> {
        let _ = (table, first, entries);
        Err(GpuError::Absent(Features::RESOURCE_TABLES))
    }

    /// Writes the GPU time into query `index`, outside a pass: back ends whose API writes timestamps only
    /// where passes begin and end attach it to the next pass, or to an empty one. A front asked for one
    /// between draws ends the pass first (slow path [`SlowPath::TimestampSplitPass`]). Needs
    /// [`Features::TIMESTAMPS`].
    fn write_timestamp(&mut self, set: &Self::QuerySet, index: u32) -> Result<(), GpuError> {
        let _ = (set, index);
        Err(GpuError::Absent(Features::TIMESTAMPS))
    }
}

/// Which aspects a depth-stencil format has: depth, stencil.
fn aspects(format: Format) -> (bool, bool) {
    match format.kind() {
        FormatKind::Depth => (true, false),
        FormatKind::Stencil => (false, true),
        FormatKind::DepthStencil => (true, true),
        _ => (false, false),
    }
}

/// A viewport over `width` by `height` pixels at depth zero to one.
fn full_viewport(width: u32, height: u32) -> Viewport {
    Viewport {
        x: 0,
        y: 0,
        width: (width as f32).to_bits(),
        height: (height as f32).to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }
}

/// A draw of the built-in full-screen triangle with `program`, no bindings and no depth test.
fn builtin_draw<'a, B: Backend>(
    b: &'a helpers::Builtins<B>,
    program: Builtin,
    viewports: &'a [Viewport],
) -> DrawCall<'a, B> {
    DrawCall {
        program: &b.programs[program as usize],
        blend: &b.blend,
        depth_stencil: &b.no_depth,
        raster: &b.raster,
        vertex_layout: &b.vertex_layout,
        bindings: [None; crate::MAX_BINDING_SETS],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
    }
}

/// The index list that draws a fan as a triangle list or a loop as a line list, honouring restart in
/// guest indices, and the base vertex to draw it with. Each fan triangle keeps the fan's winding and puts
/// first the vertex `provoking` names, since lists take the first vertex as the provoking one where the
/// back end has no choice.
fn fan_or_loop_indices(
    kind: FanOrLoop,
    source: IndexSource<'_>,
    restart: bool,
    provoking: ProvokingVertex,
) -> (Vec<u32>, i32) {
    let (values, restart_value, base): (Vec<u32>, Option<u32>, i32) = match source {
        IndexSource::Sequential { first, count } => {
            ((first..first.saturating_add(count)).collect(), None, 0)
        }
        IndexSource::Guest {
            indices,
            format,
            base_vertex,
        } => {
            let v: Vec<u32> = match format {
                IndexFormat::U16 => indices
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes(*b) as u32)
                    .collect(),
                IndexFormat::U32 => indices
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| u32::from_le_bytes(*b))
                    .collect(),
            };
            let r = restart.then_some(match format {
                IndexFormat::U16 => 0xffff,
                IndexFormat::U32 => 0xffff_ffff,
            });
            (v, r, base_vertex)
        }
    };
    let mut out = Vec::new();
    for piece in values.split(|&i| Some(i) == restart_value) {
        match kind {
            FanOrLoop::TriangleFan => {
                for w in piece.windows(2).skip(1) {
                    match provoking {
                        ProvokingVertex::First => out.extend_from_slice(&[w[0], w[1], piece[0]]),
                        ProvokingVertex::Last => out.extend_from_slice(&[piece[0], w[0], w[1]]),
                    }
                }
            }
            FanOrLoop::LineLoop => {
                if piece.len() < 2 {
                    continue;
                }
                for (k, &v) in piece.iter().enumerate() {
                    out.extend_from_slice(&[v, piece[(k + 1) % piece.len()]]);
                }
            }
        }
    }
    (out, base)
}

#[cfg(test)]
mod tests;
