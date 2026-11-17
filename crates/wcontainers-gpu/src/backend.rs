//! The [`Backend`] trait: the base every back end provides, and the fast paths with their defaults.

use crate::caps::{Caps, Features, FormatUse};
use crate::draw::{
    AspectOps, CaptureTargets, ColorAttachment, DepthAttachment, DispatchCall, DrawCall, DrawRange,
    IndexBinding, LoadOp, PassDesc, ScissorRect, StoreOp, Viewport,
};
use crate::error::{DeviceStatus, GpuError};
use crate::format::{Format, FormatKind};
use crate::helpers::{self, Builtin, Helpers, MipRead, PARAMS_SIZE, Ring};
use crate::resource::{
    BindingEntry, BindingLayoutKey, BindingResource, BufferDesc, BufferRange, BufferUsage,
    CopyBufferTexture, Extent, MapIntent, Origin, QueryKind, ReadSource, ReadState, SamplerDesc,
    TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, ViewDimension,
    WriteMode,
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

/// Which ring [`Backend::write_buffer_ring`] writes into. Indices have a ring of their own because WebGL 2
/// never lets one buffer hold both indices and other data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingKind {
    /// Index values, bound as [`DrawCall::index`].
    Indices,
    /// Vertex data and uniform blocks.
    Data,
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
    /// The layout keys of the binding sets the draws will bind, as [`Backend::binding_layout_key`] gave
    /// them or a cache recorded them; the zero key means every texture can be filtered.
    pub layouts: [BindingLayoutKey; crate::MAX_BINDING_SETS],
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
    /// The layout key of a binding set, for [`PrepareDesc::layouts`] and the cache.
    fn binding_layout_key(&self, set: &Self::BindingSet) -> BindingLayoutKey;
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
    /// Starts counting into query `index` of the pass's occlusion set. Inside a pass, one query at a
    /// time, and each index at most once in a pass (as WebGPU requires): a front whose API lets queries overlap ends the running query at every begin and end of
    /// its own, starts the next, and adds each stretch's result to every guest query open during it
    /// (slow path [`crate::SlowPath::QueriesSegmented`]).
    fn begin_query(&mut self, index: u32);
    fn end_query(&mut self);
    /// Copies bytes between buffers. Source and destination may be the same buffer when the ranges do
    /// not overlap (OpenGL, Vulkan and Direct3D 12's `CopyBufferRegion` allow it): a back end whose API
    /// forbids that
    /// copies through a temporary buffer ([`crate::SlowPath::CopyWithinBuffer`]).
    fn copy_buffer(
        &mut self,
        src: &Self::Buffer,
        src_offset: u64,
        dst: &Self::Buffer,
        dst_offset: u64,
        size: u64,
    );
    /// Copies texels between two regions whose formats are equal, sRGB twins, two formats whose texels
    /// are the same bits seen differently (Direct3D's typeless groups, such as `R8G8B8A8_UNORM` and
    /// `R8G8B8A8_UINT`, or `R32_FLOAT` and `D32_FLOAT`), or (as Direct3D and Vulkan allow) a
    /// block-compressed format and an uncompressed one of the same block size. A back end whose API
    /// copies only equal formats copies the others through a buffer
    /// ([`crate::SlowPath::CopyThroughBuffer`]); into a depth format its API cannot write by a copy, by a
    /// draw writing depth ([`crate::SlowPath::DepthWrittenByDraw`]); and into a texture whose bytes it
    /// keeps on the host, through that copy ([`crate::SlowPath::HostCopyReadBack`]). A combined
    /// depth-stencil format is copied whole, both aspects in one call (Direct3D's copies; WebGPU
    /// copies such textures only so).
    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    );
    /// Copies texels from a buffer into a texture region. Into a depth format the API cannot write from
    /// a buffer, the back end copies into a colour texture first and draws the depth
    /// ([`crate::SlowPath::DepthWrittenByDraw`]): `r32float` for 32-bit float depth, and `r32uint`
    /// for 24-bit depth, whose buffer bytes are 24-bit unorm in the low bits of a 32-bit word, drawn as
    /// `(v & 0xffffff) / 16777215`; see [`CopyBufferTexture::row_pitch`] for pitches. Float depth so
    /// drawn holds only what a draw can write: values outside 0 to 1, denormals and NaN are lost
    /// (plan 4.12).
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
    /// Resolves a whole multisampled subresource into a single-sampled one, averaging the samples as
    /// values of `format`: the source's stored format or its sRGB twin, so a texture made with
    /// [`TextureDesc::reinterpret`] resolves in linear light when `format` is sRGB (Direct3D's
    /// ResolveSubresource format). A back end that cannot read samples in the twin averages the stored
    /// values ([`SlowPath::ResolveInStoredFormat`], a listed loss).
    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    );
    /// Sends everything recorded so far to the GPU, and returns a fence for it.
    fn submit(&mut self) -> Self::Fence;
    /// Whether the GPU has finished the work before `fence`.
    fn fence_done(&mut self, fence: &Self::Fence) -> bool;
    /// Pauses the device thread until `fence` is done.
    fn wait_fence(&mut self, fence: &Self::Fence);

    // ---- Bytes out ----

    /// Starts reading data back. The copy is recorded now and finishes after a later [`Backend::submit`].
    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Self::Readback, GpuError>;
    /// Starts reading query results back to the host, one u64 per query.
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
    /// Sets the size, format and whether presenting waits for the display. `format` is one with
    /// [`crate::FormatUse::PRESENT`]; colors are shown as sRGB, values outside zero to one clipped.
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
    /// triangle list or a line list in the ring ([`Backend::write_buffer_ring`]) and draws that. WebGL 2
    /// overrides it.
    fn draw_fan_or_loop(
        &mut self,
        call: &DrawCall<'_, Self>,
        kind: FanOrLoop,
        source: IndexSource<'_>,
        provoking: ProvokingVertex,
        first_instance: u32,
        instances: u32,
    ) -> Result<(), GpuError> {
        let mut bytes = core::mem::take(&mut self.helpers().scratch);
        bytes.clear();
        let base_vertex = fan_or_loop_indices(kind, source, call.restart, provoking, &mut bytes);
        let count = (bytes.len() / 4) as u32;
        let mut offset = [0];
        let ring = (count > 0)
            .then(|| self.write_buffer_ring(RingKind::Indices, &[&bytes], 4, &mut offset));
        self.helpers().scratch = bytes;
        let Some(buffer) = ring.transpose()? else {
            return Ok(());
        };
        let (topology, path) = match kind {
            FanOrLoop::TriangleFan => (Topology::TriangleList, SlowPath::FanConverted),
            FanOrLoop::LineLoop => (Topology::LineList, SlowPath::LoopConverted),
        };
        self.slow_paths().hit(path);
        self.draw(&DrawCall {
            index: Some(IndexBinding {
                buffer: &buffer,
                offset: offset[0],
                format: IndexFormat::U32,
            }),
            topology,
            restart: false,
            range: DrawRange::Indexed {
                first: 0,
                count,
                base_vertex,
                first_instance,
                instances,
            },
            ..*call
        });
        Ok(())
    }

    /// Builds a state combination ahead of its first draw. The default does nothing, so the first draw
    /// builds it. WebGPU overrides it to compile the pipeline in the background.
    fn prepare(&mut self, desc: &PrepareDesc<'_, Self>) {
        let _ = desc;
    }

    /// Clears mip levels `mips` and layers `layers` of a texture made from `desc`, seen in `format` (its
    /// own format or the sRGB twin), outside a pass. For a 3D texture `layers` are depth slices, cut to
    /// each level's depth. The default opens one clearing pass per level and layer or slice, on a view to
    /// draw into of one level and one layer (a whole level of a 3D texture), which every back end makes.
    /// WebGL 2 overrides it with its own clears.
    fn clear_texture(
        &mut self,
        texture: &Self::Texture,
        desc: &TextureDesc,
        format: Format,
        mips: Range<u32>,
        layers: Range<u32>,
        value: ClearValue,
    ) -> Result<(), GpuError> {
        let three_d = desc.dimension == TextureDimension::D3;
        for mip in mips.start..mips.end.min(desc.mip_levels) {
            let (views, slices) = if three_d {
                let depth = (desc.depth_or_layers >> mip).max(1);
                (0..1, layers.start..layers.end.min(depth))
            } else {
                (layers.start..layers.end.min(desc.depth_or_layers), 0..1)
            };
            for layer in views {
                let view = self.create_view(
                    texture,
                    &TextureViewDesc {
                        format,
                        dimension: if three_d {
                            ViewDimension::D3
                        } else {
                            ViewDimension::D2
                        },
                        base_mip: mip,
                        mip_count: 1,
                        base_layer: layer,
                        layer_count: 1,
                        stencil: false,
                    },
                )?;
                for slice in slices.clone() {
                    let mut pass = PassDesc {
                        colors: Default::default(),
                        depth: None,
                        empty: None,
                        occlusion: None,
                    };
                    match value {
                        ClearValue::Color(c) => {
                            pass.colors[0] = Some(ColorAttachment {
                                view: &view,
                                depth_slice: slice,
                                resolve: None,
                                load: LoadOp::Clear(c),
                                store: StoreOp::Store,
                            });
                        }
                        ClearValue::DepthStencil { depth, stencil } => {
                            let (has_depth, has_stencil) = aspects(format);
                            pass.depth = Some(DepthAttachment {
                                view: &view,
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
                self.destroy(Object::TextureView(view));
            }
        }
        Ok(())
    }

    /// Clears rectangles of one view outside a pass (Direct3D 11.1's ClearView, Direct3D 12's clears
    /// with rectangles). `view` is one mip level and one layer; rectangles are clipped to it. The
    /// default draws a full-screen triangle per rectangle, scissored to it (the scissor clipped as
    /// [`DrawCall::scissors`] says), with a built-in program
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
        let (which, depth_stencil, stencil_reference) = match value {
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
        let program = helpers::program(self, which)?;
        let set = if which == Builtin::ClearStencil {
            None
        } else {
            let align = self.caps().limits.min_uniform_offset_alignment;
            let mut offset = [0];
            let ring = self.write_buffer_ring(RingKind::Data, &[&params], align, &mut offset)?;
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
        let width = rects
            .iter()
            .map(|r| r.x.saturating_add(r.width))
            .max()
            .unwrap_or(0);
        let height = rects
            .iter()
            .map(|r| r.y.saturating_add(r.height))
            .max()
            .unwrap_or(0);
        let viewport = [full_viewport(width, height)];
        self.begin_pass(&pass);
        for rect in rects {
            self.draw(&DrawCall {
                bindings: [set.as_ref(), None, None, None],
                depth_stencil,
                stencil_reference: [stencil_reference; 2],
                scissors: core::slice::from_ref(rect),
                ..builtin_draw(&b, &program, &viewport)
            });
        }
        self.end_pass();
        if let Some(set) = set {
            self.destroy(Object::BindingSet(set));
        }
        Ok(())
    }

    /// Fills mip levels `mips` (each from the level above) of `layers` of a texture made from `desc`,
    /// outside a pass (Direct3D's GenerateMips, OpenGL's glGenerateMipmap). `values` is the format whose
    /// values the texels hold: `desc.format`, its sRGB twin (Direct3D's GenerateMips through an sRGB view
    /// of a typeless texture, which filters in linear light), or the normalised format a front stores
    /// as its integer twin ([`crate::SlowPath::NormAsInteger`]).
    ///
    /// The default draws each level and layer, or each slice of a 3D level, with a built-in program
    /// reading the level above: through the linear sampler where the format has
    /// [`crate::FormatUse::FILTER`]; otherwise filtered in the program with the same weights, from
    /// samples at texel centres through the nearest sampler (float formats) or from integer loads
    /// decoded and encoded again (integer twins), each such draw counted [`SlowPath::FilterInShader`].
    /// When the ranges hold no level to draw, nothing is built or counted. It reads the
    /// level above through a view of all layers in the texture's own dimension, the layer chosen in the
    /// program, and draws into a view of one level and one layer, so it needs no
    /// [`Features::TEXTURE_VIEWS`]; the one exception is a cube read by integer loads, which no
    /// language has for cubes, read through a 2D array view, or without [`Features::TEXTURE_VIEWS`] from
    /// a copy of the level above ([`SlowPath::ReinterpretCopy`]). WebGL 2 overrides it with
    /// generateMipmap where the format allows and uses the default otherwise, through
    /// [`generate_mips_by_draws`], which holds the default's body.
    fn generate_mips(
        &mut self,
        texture: &Self::Texture,
        desc: &TextureDesc,
        values: Format,
        mips: Range<u32>,
        layers: Range<u32>,
    ) -> Result<(), GpuError> {
        generate_mips_by_draws(self, texture, desc, values, mips, layers)
    }

    /// Writes the data one draw needs (Direct3D 9's vertices and indices from user memory, OpenGL's
    /// client arrays, converted fans, spilled immediates and helper constants) into the ring of `kind`:
    /// one for indices, one for vertex data and uniform blocks, so no buffer ever holds both (WebGL 2
    /// forbids it). Each part starts at a multiple of `align`; its offset goes into `offsets`. The
    /// returned buffer and offsets stay valid for commands recorded before the next call into the same
    /// ring. The default writes with [`WriteMode::NoOverwrite`], and with [`WriteMode::Discard`] when it
    /// wraps, so nothing waits. No back end overrides it today; a native back end would map the ring.
    fn write_buffer_ring(
        &mut self,
        kind: RingKind,
        parts: &[&[u8]],
        align: u32,
        offsets: &mut [u64],
    ) -> Result<Self::Buffer, GpuError> {
        let align = u64::from(align.max(4));
        let total: u64 = parts
            .iter()
            .map(|p| (p.len() as u64).next_multiple_of(align))
            .sum();
        let slot = kind as usize;
        let fits = matches!(&self.helpers().rings[slot], Some(r) if r.size >= total);
        if !fits {
            let size = total.max(RING_SIZE).next_power_of_two();
            let usage = match kind {
                RingKind::Indices => BufferUsage::INDEX,
                RingKind::Data => BufferUsage::VERTEX | BufferUsage::UNIFORM,
            };
            let buffer = self.create_buffer(&BufferDesc {
                size,
                usage: usage | BufferUsage::COPY_DST,
                intent: MapIntent::Often,
            })?;
            let old = self.helpers().rings[slot].replace(Ring {
                buffer,
                size,
                head: 0,
            });
            if let Some(old) = old {
                self.destroy(Object::Buffer(old.buffer));
            }
        }
        let Some(ring) = &self.helpers().rings[slot] else {
            unreachable!("the ring was made above")
        };
        let (buffer, size, mut head) = (ring.buffer.clone(), ring.size, ring.head);
        if total == 0 {
            let at = head.next_multiple_of(align);
            let at = if at > size { 0 } else { at };
            offsets.iter_mut().for_each(|o| *o = at);
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
        if let Some(ring) = &mut self.helpers().rings[slot] {
            ring.head = head;
        }
        Ok(buffer)
    }

    /// Writes the results of queries `first` to `first + count - 1` into `dst` on the GPU, one u64 per
    /// query at `dst_offset` (a multiple of 8; `dst` has [`BufferUsage::COPY_DST`]), outside a pass and
    /// ordered with the work around it: Direct3D 12's ResolveQueryData, Vulkan's
    /// vkCmdCopyQueryPoolResults, OpenGL's query buffer objects. The default reads the results back to
    /// the host, pausing the device thread until the GPU has them ([`SlowPath::QueryResolveWait`]), and
    /// writes them with [`WriteMode::Ordered`]. WebGPU overrides it with resolveQuerySet.
    fn resolve_queries(
        &mut self,
        set: &Self::QuerySet,
        first: u32,
        count: u32,
        dst: &Self::Buffer,
        dst_offset: u64,
    ) -> Result<(), GpuError> {
        if !dst_offset.is_multiple_of(8) {
            return Err(GpuError::Invalid(
                "query results go to multiples of 8 bytes",
            ));
        }
        let rb = self.read_queries(set, first, count)?;
        if self.read_state(&rb) == ReadState::Pending {
            self.slow_paths().hit(SlowPath::QueryResolveWait);
            self.wait_read(&rb);
        }
        let mut bytes = core::mem::take(&mut self.helpers().scratch);
        bytes.clear();
        bytes.resize(count as usize * 8, 0);
        self.finish_read(rb, &mut bytes);
        self.write_buffer(dst, dst_offset, &bytes, WriteMode::Ordered);
        self.helpers().scratch = bytes;
        Ok(())
    }

    // ---- Present or absent: no cheap default ----

    /// Runs a compute program. Outside a pass. Needs [`Features::COMPUTE`].
    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        let _ = call;
        Err(GpuError::Absent(Features::COMPUTE))
    }

    /// Draws `call` writing the last vertex stage's outputs to `targets` in order. With `rasterize`
    /// false nothing is drawn and the call comes outside a pass; with it true the primitives are also
    /// drawn, inside a pass, in the same run of the vertex work (Direct3D's rasterized stream, OpenGL's
    /// transform feedback without rasterizer discard). Needs [`Features::TRANSFORM_FEEDBACK`].
    ///
    /// What is written: the vertex stage's location outputs in location order, each as its 32-bit
    /// components with no padding; built-ins such as the position are not captured (a program whose
    /// position is captured writes it to a location too). With one target every output of a vertex is
    /// written one after another, vertex after vertex; with more, output n goes to target n alone and
    /// there are as many targets as outputs, at most [`crate::Limits::capture_buffers`]. Vertices are
    /// written once per vertex of each whole primitive, in draw order, instance after instance, and the
    /// targets hold every vertex the draw makes. Strips are written as the lists they draw: triangle
    /// `i` of a strip is `(i, i + 1, i + 2)` for even `i`, and for odd `i` `(i, i + 2, i + 1)`, or
    /// `(i + 1, i, i + 2)` with [`crate::ProvokingVertex::Last`] (each keeps the winding and the
    /// provoking vertex every API gives the triangle). Fans and loops need
    /// [`Features::FAN_AND_LOOP`].
    fn capture_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: CaptureTargets<'_, Self>,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        let _ = (call, targets, rasterize);
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

    /// Writes the GPU time into query `index` of a [`QueryKind::Timestamp`] set, outside a pass: back ends whose API writes timestamps only
    /// where passes begin and end attach it to the next pass, or to an empty one. A front asked for one
    /// between draws ends the pass first (slow path [`SlowPath::TimestampSplitPass`]). Needs
    /// [`Features::TIMESTAMPS`].
    fn write_timestamp(&mut self, set: &Self::QuerySet, index: u32) -> Result<(), GpuError> {
        let _ = (set, index);
        Err(GpuError::Absent(Features::TIMESTAMPS))
    }

    /// Whether something since the last call (a clock change, a context switch) made the timestamps
    /// written in that time meaningless, clearing the condition: WebGL 2's `GPU_DISJOINT_EXT`, from which
    /// fronts answer Direct3D's disjoint query and OpenGL's `GL_GPU_DISJOINT`. A back end whose API never
    /// reports one (WebGPU) answers false. Needs [`Features::TIMESTAMPS`].
    fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
        Err(GpuError::Absent(Features::TIMESTAMPS))
    }
}

/// The default of [`Backend::generate_mips`], for a back end whose override takes only some cases.
pub fn generate_mips_by_draws<B: Backend>(
    b: &mut B,
    texture: &B::Texture,
    desc: &TextureDesc,
    values: Format,
    mips: Range<u32>,
    layers: Range<u32>,
) -> Result<(), GpuError> {
    let own = desc.own_view();
    let three_d = own == ViewDimension::D3;
    let layers = if three_d {
        0..1
    } else {
        layers.start..layers.end.min(desc.depth_or_layers)
    };
    let mips = mips.start.max(1)..mips.end.min(desc.mip_levels);
    if mips.is_empty() || layers.is_empty() {
        return Ok(());
    }
    let built = helpers::builtins(b)?;
    let decoded = norm_twin(values, desc.format)?;
    // The views read and write the texels as their values: the sRGB twin is a view every back end
    // makes, so filtering happens in linear light.
    let seen = if decoded.is_some() {
        desc.format
    } else {
        values
    };
    let read = match decoded {
        Some((_, false)) => MipRead::Unorm,
        Some((_, true)) => MipRead::Snorm,
        None if b.caps().formats.get(seen).contains(FormatUse::FILTER) => MipRead::Filtered,
        None => MipRead::Unfiltered,
    };
    let cube = matches!(own, ViewDimension::Cube | ViewDimension::CubeArray);
    let read_dim = if decoded.is_some() && cube {
        ViewDimension::D2Array
    } else {
        own
    };
    let copied = read_dim != own && !b.caps().has(Features::TEXTURE_VIEWS);
    let which = Builtin::Mip(read_dim, read);
    let program = helpers::program(b, which)?;
    for mip in mips {
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
        let all = if three_d { 1 } else { desc.depth_or_layers };
        let mut source_view = TextureViewDesc {
            format: seen,
            dimension: read_dim,
            base_mip: mip - 1,
            mip_count: 1,
            base_layer: 0,
            layer_count: all,
            stencil: false,
        };
        // A cube read by loads without views: the level above copied into a 2D array first.
        let copy = if copied {
            b.slow_paths().hit(SlowPath::ReinterpretCopy);
            let above = |n: u32| (n >> (mip - 1)).max(1);
            let extent = Extent {
                width: above(desc.width),
                height: above(desc.height),
                depth_or_layers: all,
            };
            let scratch = b.create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format: desc.format,
                width: extent.width,
                height: extent.height,
                depth_or_layers: all,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
                reinterpret: false,
                cube: false,
            })?;
            let region = |texture, mip| TextureRegion {
                texture,
                mip,
                layer: 0,
                origin: Origin { x: 0, y: 0, z: 0 },
                stencil: false,
            };
            b.copy_texture(&region(texture, mip - 1), &region(&scratch, 0), extent);
            source_view.base_mip = 0;
            Some(scratch)
        } else {
            None
        };
        let src = b.create_view(copy.as_ref().unwrap_or(texture), &source_view)?;
        for layer in layers.clone() {
            let dst = b.create_view(
                texture,
                &TextureViewDesc {
                    format: seen,
                    dimension: if three_d {
                        ViewDimension::D3
                    } else {
                        ViewDimension::D2
                    },
                    base_mip: mip,
                    mip_count: 1,
                    base_layer: layer,
                    layer_count: 1,
                    stencil: false,
                },
            )?;
            for slice in 0..slices {
                if read != MipRead::Filtered {
                    b.slow_paths().hit(SlowPath::FilterInShader);
                }
                let ring;
                let mut entries = alloc::vec![BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&src),
                }];
                if which.uses_sampler() {
                    let sampler = match read {
                        MipRead::Filtered => &built.linear,
                        _ => &built.nearest,
                    };
                    entries.push(BindingEntry {
                        binding: 16,
                        stages: StageSet::FRAGMENT,
                        resource: BindingResource::Sampler(sampler),
                    });
                }
                if which.uses_params() {
                    let mut params = [0u8; PARAMS_SIZE];
                    let scale = decoded.map_or(0, |(scale, _)| scale);
                    params[0..4].copy_from_slice(&scale.to_le_bytes());
                    let w = (slice as f32 + 0.5) / slices as f32;
                    params[16..20].copy_from_slice(&w.to_bits().to_le_bytes());
                    params[24..28].copy_from_slice(&layer.to_le_bytes());
                    let align = b.caps().limits.min_uniform_offset_alignment;
                    let mut offset = [0];
                    ring = b.write_buffer_ring(RingKind::Data, &[&params], align, &mut offset)?;
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
                let set = b.create_binding_set(&entries)?;
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
                b.begin_pass(&pass);
                b.draw(&DrawCall {
                    bindings: [Some(&set), None, None, None],
                    scissors: &scissor,
                    ..builtin_draw(&built, &program, &viewport)
                });
                b.end_pass();
                b.destroy(Object::BindingSet(set));
            }
            b.destroy(Object::TextureView(dst));
        }
        b.destroy(Object::TextureView(src));
        if let Some(copy) = copy {
            b.destroy(Object::Texture(copy));
        }
    }
    Ok(())
}

/// Which aspects a depth-stencil format has: depth, stencil.
pub(crate) fn aspects(format: Format) -> (bool, bool) {
    match format.kind() {
        FormatKind::Depth => (true, false),
        FormatKind::Stencil => (false, true),
        FormatKind::DepthStencil => (true, true),
        _ => (false, false),
    }
}

/// For a texture a front stores as the integer twin of a normalised format, the largest encoded value
/// and whether it is signed; None when the texture holds its own format's values.
fn norm_twin(values: Format, stored: Format) -> Result<Option<(u32, bool)>, GpuError> {
    use Format::*;
    Ok(match (values, stored) {
        _ if values.linear() == stored.linear() => None,
        (R16Unorm, R16Uint) | (Rg16Unorm, Rg16Uint) | (Rgba16Unorm, Rgba16Uint) => {
            Some((65535, false))
        }
        (R16Snorm, R16Sint) | (Rg16Snorm, Rg16Sint) | (Rgba16Snorm, Rgba16Sint) => {
            Some((32767, true))
        }
        (R8Snorm, R8Sint) | (Rg8Snorm, Rg8Sint) | (Rgba8Snorm, Rgba8Sint) => Some((127, true)),
        _ => {
            return Err(GpuError::Invalid(
                "generate_mips: values are neither the stored format, its srgb twin nor its integer twin",
            ));
        }
    })
}

/// A viewport over `width` by `height` pixels at depth zero to one.
pub(crate) fn full_viewport(width: u32, height: u32) -> Viewport {
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
pub(crate) fn builtin_draw<'a, B: Backend>(
    b: &'a helpers::Builtins<B>,
    program: &'a B::Program,
    viewports: &'a [Viewport],
) -> DrawCall<'a, B> {
    DrawCall {
        program,
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

/// Appends to `out`, as little-endian u32, the index list that draws a fan as a triangle list or a loop
/// as a line list, honouring restart in guest indices, and returns the base vertex to draw it with. Each
/// fan triangle keeps the fan's winding and puts the vertex `provoking` names where the list's own
/// provoking vertex is: first for [`ProvokingVertex::First`] (vertex i + 1 of the fan), last for
/// [`ProvokingVertex::Last`] (vertex i + 2).
fn fan_or_loop_indices(
    kind: FanOrLoop,
    source: IndexSource<'_>,
    restart: bool,
    provoking: ProvokingVertex,
    out: &mut Vec<u8>,
) -> i32 {
    // Generic over the index width, so each width gets its own loop with no call per index.
    fn walk(
        kind: FanOrLoop,
        provoking: ProvokingVertex,
        values: impl Iterator<Item = u32>,
        restart_value: Option<u32>,
        out: &mut Vec<u8>,
    ) {
        let mut push = |v: [u32; 3], n: usize| {
            for i in &v[..n] {
                out.extend_from_slice(&i.to_le_bytes());
            }
        };
        // The piece's first vertex, the previous one, and how many it has had. A loop closes when its
        // piece ends.
        let (mut first, mut prev, mut n) = (0, 0, 0usize);
        let closes = |n: usize| kind == FanOrLoop::LineLoop && n >= 2;
        for v in values {
            if Some(v) == restart_value {
                if closes(n) {
                    push([prev, first, 0], 2);
                }
                n = 0;
                continue;
            }
            match (kind, n) {
                (_, 0) => first = v,
                (FanOrLoop::TriangleFan, 1) => {}
                (FanOrLoop::TriangleFan, _) => match provoking {
                    ProvokingVertex::First => push([prev, v, first], 3),
                    ProvokingVertex::Last => push([first, prev, v], 3),
                },
                (FanOrLoop::LineLoop, _) => push([prev, v, 0], 2),
            }
            prev = v;
            n += 1;
        }
        if closes(n) {
            push([prev, first, 0], 2);
        }
    }
    match source {
        IndexSource::Sequential { first, count } => {
            walk(
                kind,
                provoking,
                first..first.saturating_add(count),
                None,
                out,
            );
            0
        }
        IndexSource::Guest {
            indices,
            format,
            base_vertex,
        } => {
            let restart_value = restart.then(|| format.restart_value());
            match format {
                IndexFormat::U8 => walk(
                    kind,
                    provoking,
                    indices.iter().map(|&b| u32::from(b)),
                    restart_value,
                    out,
                ),
                IndexFormat::U16 => walk(
                    kind,
                    provoking,
                    indices
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| u32::from(u16::from_le_bytes(*b))),
                    restart_value,
                    out,
                ),
                IndexFormat::U32 => walk(
                    kind,
                    provoking,
                    indices
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| u32::from_le_bytes(*b)),
                    restart_value,
                    out,
                ),
            }
            base_vertex
        }
    }
}

#[cfg(test)]
mod tests;
