//! The [`Backend`] trait: the base every back end provides, and the fast paths with their defaults.

use crate::caps::{Caps, Features};
use crate::draw::{
    CaptureTargets, ColorAttachment, DepthAttachment, DispatchCall, DrawCall, DrawRange,
    IndexBinding, LoadOp, PassDesc, StoreOp,
};
use crate::error::{DeviceStatus, GpuError};
use crate::format::Format;
use crate::resource::{
    BindingEntry, BufferDesc, BufferUsage, CopyBufferTexture, Extent, MapIntent, QueryKind,
    ReadSource, ReadState, SamplerDesc, TextureDesc, TextureRegion, TextureViewDesc, WriteMode,
};
use crate::shader::{Program, ShaderModule};
use crate::slow::{SlowPath, SlowPaths};
use crate::state::{
    BlendDesc, DepthStencilDesc, IndexFormat, RasterDesc, TargetLayout, Topology, VertexLayoutDesc,
};
use alloc::vec::Vec;
use core::fmt::Debug;

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
pub trait Backend: Sized {
    type Buffer: Debug;
    type Texture: Debug;
    type TextureView: Debug;
    type Sampler: Debug;
    /// A shader in the back end's own language, built from a [`ShaderModule`].
    type Shader: Debug;
    type Program: Debug;
    type Blend: Debug;
    type DepthStencil: Debug;
    type Raster: Debug;
    type VertexLayout: Debug;
    type BindingSet: Debug;
    type QuerySet: Debug;
    /// A read back in flight.
    type Readback: Debug;
    /// A point in the GPU's work, for waiting on.
    type Fence: Debug;
    type ResourceTable: Debug;
    /// A host window's drawable area.
    type Surface: Debug;

    // ---- What the back end is and how it is doing ----

    /// What this back end can do. Fixed for the device's life.
    fn caps(&self) -> &Caps;
    /// The slow-path counters, which fronts and shared helpers also bump.
    fn slow_paths(&mut self) -> &mut SlowPaths;
    /// Whether the device still works.
    fn status(&mut self) -> DeviceStatus;

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

    /// Writes bytes into a buffer. `data` is read in place, straight from guest memory, before the call
    /// returns. `mode` says what the caller promises about the bytes already there.
    fn write_buffer(&mut self, buffer: &Self::Buffer, offset: u64, data: &[u8], mode: WriteMode);
    /// Writes texels into one region of a texture, from guest memory in place, ordered like
    /// [`WriteMode::Ordered`]. `row_pitch` is the bytes between rows of blocks in `data`.
    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
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
    /// Sets bytes of a buffer to zero.
    fn clear_buffer(&mut self, buffer: &Self::Buffer, offset: u64, size: u64);
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
    /// the vertices come from `source`, and instances from `instances`. The default builds a triangle
    /// list or a closed line strip in a fresh index buffer and draws that. WebGL 2 overrides it.
    fn draw_fan_or_loop(
        &mut self,
        call: &DrawCall<'_, Self>,
        kind: FanOrLoop,
        source: IndexSource<'_>,
        first_instance: u32,
        instances: u32,
    ) -> Result<(), GpuError> {
        let (indices, base_vertex) = fan_or_loop_indices(kind, source, call.restart);
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
            FanOrLoop::LineLoop => (Topology::LineStrip, SlowPath::LoopConverted),
        };
        self.slow_paths().hit(path);
        self.draw(&DrawCall {
            program: call.program,
            blend: call.blend,
            depth_stencil: call.depth_stencil,
            raster: call.raster,
            vertex_layout: call.vertex_layout,
            bindings: call.bindings,
            vertex_buffers: call.vertex_buffers,
            index: Some(IndexBinding {
                buffer: &buffer,
                offset: 0,
                format: IndexFormat::U32,
            }),
            topology,
            restart: true,
            viewports: call.viewports,
            scissors: call.scissors,
            blend_constant: call.blend_constant,
            stencil_reference: call.stencil_reference,
            immediates: call.immediates,
            range: DrawRange::Indexed {
                first: 0,
                count: indices.len() as u32,
                base_vertex,
                first_instance,
                instances,
            },
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
    fn clear_texture(&mut self, view: &Self::TextureView, format: Format, value: ClearValue) {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            occlusion: None,
        };
        match value {
            ClearValue::Color(c) => {
                pass.colors[0] = Some(ColorAttachment {
                    view,
                    resolve: None,
                    load: LoadOp::Clear(c),
                    store: StoreOp::Store,
                });
            }
            ClearValue::DepthStencil { depth, stencil } => {
                let has_stencil = !matches!(format.kind(), crate::FormatKind::Depth);
                pass.depth = Some(DepthAttachment {
                    view,
                    depth_load: depth.map_or(LoadOp::Load, LoadOp::Clear),
                    depth_store: StoreOp::Store,
                    stencil_load: if has_stencil {
                        stencil.map_or(LoadOp::Load, LoadOp::Clear)
                    } else {
                        LoadOp::Load
                    },
                    stencil_store: StoreOp::Store,
                    depth_read_only: false,
                    stencil_read_only: false,
                });
            }
        }
        self.begin_pass(&pass);
        self.end_pass();
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

    /// Writes the GPU time into query `index`. Needs [`Features::TIMESTAMPS`].
    fn write_timestamp(&mut self, set: &Self::QuerySet, index: u32) -> Result<(), GpuError> {
        let _ = (set, index);
        Err(GpuError::Absent(Features::TIMESTAMPS))
    }
}

/// The index list that draws a fan as triangles or a loop as a closed strip, honouring restart in guest
/// indices, and the base vertex to draw it with.
fn fan_or_loop_indices(kind: FanOrLoop, source: IndexSource<'_>, restart: bool) -> (Vec<u32>, i32) {
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
    let pieces = values.split(|&i| Some(i) == restart_value);
    for piece in pieces {
        match kind {
            FanOrLoop::TriangleFan => {
                for w in piece.windows(2).skip(1) {
                    out.extend_from_slice(&[piece[0], w[0], w[1]]);
                }
            }
            FanOrLoop::LineLoop => {
                if piece.len() < 2 {
                    continue;
                }
                if !out.is_empty() {
                    out.push(u32::MAX);
                }
                out.extend_from_slice(piece);
                out.push(piece[0]);
            }
        }
    }
    (out, base)
}

#[cfg(test)]
mod tests;
