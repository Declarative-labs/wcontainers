//! The immediate context: its state, passes opened at draws, draws, copies, writes and maps.

use std::cell::Cell;
use std::rc::Rc;

use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BufferRange, CONSTANTS_TEXTURE_WIDTH,
    ClearValue, ColorAttachment, ColorTarget, CountedDraw, DRAW_ARGUMENTS_BINDING, DepthAttachment,
    DispatchCall, DispatchGroups, DrawCall, DrawRange, EARLY_DEPTH_BINDING, EARLY_STENCIL_BINDING,
    EmptyPass, Extent, FORMAT_FACTS, Features, Filter, Format, FormatKind, FormatUse, HELPER_SET,
    HelperValue, IndexBinding, IndexFormat, Interpolation, LoadOp, MAX_BINDING_SETS, Object,
    Origin, PassDesc, PassOps, PassTargets, Program, ReadSource, ReadState, RingKind, SLOT_KINDS,
    SamplerDesc as GpuSampler, ScissorRect, ShaderModule, ShaderStage, SlotKind, SlowPath,
    SourceStage, StageSet, StencilOp, StepMode, StoreOp, SystemValue, TEXEL_BUFFER_TEXTURE_WIDTH,
    Target, TargetInShader, TextureRegion, TextureUsage, TextureViewDesc, Topology, VariantKey,
    VertexAttribute, VertexBinding, VertexLayoutDesc, VertexStream, ViewDimension,
    Viewport as GpuViewport, WriteMode, helper_binding,
};

use crate::desc::{
    self, Box3, DSV_READ_ONLY_DEPTH, DSV_READ_ONLY_STENCIL, DepthStencilDesc, SubresourceData,
    TexDim, TextureDesc, Viewport, level, map, topology, usage,
};
use crate::device::{
    COUNTER_BYTES, Device, HelperBlocks, Linked, Variant, ZERO_BYTES, gpu_buffer, integer_format,
    resolvable,
};
use crate::error::{Error, Result, WARNING_PREFIX, not_implemented};
use crate::format;
use crate::object::*;
use crate::shader::{ConstantsReadAs, Helpers, RelocatedConstants, StageVariant};

mod geometry;
mod split;
mod tessellation;
mod through;
pub(crate) use geometry::StageRun;
pub(crate) use tessellation::TessRun;
use through::Through;

/// The shader stages whose slots this context keeps: vertex (set 0), pixel (set 1), compute (set 0
/// of its own programs), geometry and hull (set 0 of the stage helper's compute programs) and domain
/// (set 0 of the program drawing the tessellated points).
pub const VERTEX: usize = 0;
pub const PIXEL: usize = 1;
pub const COMPUTE: usize = 2;
pub const GEOMETRY: usize = 3;
pub const HULL: usize = 4;
pub const DOMAIN: usize = 5;
pub const STAGES: usize = 6;

/// Unordered access slots of the pixel and compute stages at feature level 11_0.
pub const UAV_SLOTS: usize = 8;
/// Constant buffer slots of a stage (`D3D11_COMMONSHADER_CONSTANT_BUFFER_API_SLOT_COUNT`).
const CONSTANT_BUFFER_SLOTS: usize = 14;
/// Shader resource slots of a stage (`D3D11_COMMONSHADER_INPUT_RESOURCE_SLOT_COUNT`).
pub(crate) const RESOURCE_SLOTS: usize = 128;
/// Sampler slots of a stage (`D3D11_COMMONSHADER_SAMPLER_SLOT_COUNT`).
pub(crate) const SAMPLER_SLOTS: usize = 16;
/// Render targets bound at once (`D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT`).
const TARGET_SLOTS: usize = 8;
/// Stream output targets (`D3D11_SO_BUFFER_SLOT_COUNT`).
const STREAM_OUTPUT_SLOTS: usize = 4;

/// The binding set a stage's program reads its slots from.
/// The bytes a constant buffer window of `(first, count)` 16-byte constants covers: offset and
/// length, cut at the buffer's end.
fn window_of<B: Backend>(c: &Buffer<B>, (first, count): (u32, u32)) -> (u64, u64) {
    let offset = u64::from(first) * 16;
    let length = (u64::from(count) * 16).min(c.size.get().saturating_sub(offset));
    (offset, length)
}

/// A constant buffer window a stage reads through a copy: slot, buffer, offset, length and the
/// block's size.
type ConstantWindow<B> = (u32, Rc<Buffer<B>>, u64, u64, u64, ConstantsReadAs);

/// Bytes of one constant: four 32-bit components.
const CONSTANT_ROW_BYTES: u64 = size_of::<[u32; 4]>() as u64;

/// The shader number standing for the pixel program a draw without a pixel shader takes for its
/// variant (no object is numbered zero).
const BUILT_IN_PIXEL: u64 = 0;

/// Texels copied from a buffer into a texture's rows: their count, bytes each, and texels a row.
#[derive(Clone, Copy)]
struct TexelRows {
    count: u32,
    bytes: u32,
    width: u32,
}

/// The binding set `stage`'s slots are in.
pub(crate) fn set_of(stage: usize) -> u32 {
    if stage == PIXEL { 1 } else { 0 }
}

/// The interpolation the pixel shader `ps` reads its inputs with where it is not the default, by
/// location: the stage before it writes its outputs so.
fn interpolation<B: Backend>(ps: Option<&Shader<B>>) -> Vec<(u32, Interpolation)> {
    let mut interpolation: Vec<(u32, Interpolation)> = ps
        .iter()
        .flat_map(|p| &p.reflection.inputs)
        .filter(|i| i.system.is_none() && i.interpolation != Interpolation::Perspective)
        .map(|i| (i.location, i.interpolation))
        .collect();
    interpolation.sort_by_key(|i| i.0);
    interpolation.dedup_by_key(|i| i.0);
    interpolation
}

/// A vertex buffer slot: buffer, stride, offset.
type VertexSlot<B> = (Rc<Buffer<B>>, u32, u32);

/// Direct3D 11's vertex buffer slots and viewports.
pub(crate) const VERTEX_SLOTS: usize = 32;
const VIEWPORTS: usize = 16;

/// A clear recorded but not yet run: it becomes the next pass's load, or a `clear_texture`.
struct PendingClear<B: Backend> {
    view: Rc<View<B>>,
    value: ClearValue,
}

/// A mapped subresource, released by `unmap`.
pub struct Mapped {
    pub data: Shadow,
    pub row_pitch: u32,
    pub depth_pitch: u32,
}

pub struct ContextState<B: Backend> {
    vs: Option<Rc<Shader<B>>>,
    ps: Option<Rc<Shader<B>>>,
    cs: Option<Rc<Shader<B>>>,
    gs: Option<Rc<Geometry<B>>>,
    /// The geometry shader's stage, whose slots the geometry set binds.
    gs_stage: Option<Rc<Shader<B>>>,
    hs: Option<Rc<Shader<B>>>,
    ds: Option<Rc<Shader<B>>>,
    /// The stream output targets.
    so_targets: [Option<Rc<Buffer<B>>>; STREAM_OUTPUT_SLOTS],
    /// The vertex stage's set is made for the stage helper's compute programs.
    vertex_compute: bool,
    /// The domain stage's set is made for the stage helper's compute programs.
    domain_compute: bool,
    /// Per stage, the storage views its set reads through buffers.
    through: [Vec<Through<B>>; STAGES],
    /// Per stage, whether a view bound to it is made on a twin of its texture or compared
    /// through a depth copy.
    twins: [bool; STAGES],
    /// Per stage, the colour views its program compares through depth copies (`View::compared`).
    compared: [Vec<Rc<ShaderView<B>>>; STAGES],
    pub(crate) layout: Option<Rc<InputLayout>>,
    topology: u32,
    vertex_buffers: Vec<Option<VertexSlot<B>>>,
    index: Option<(Rc<Buffer<B>>, IndexFormat, u32)>,
    constant_buffers: [Vec<Option<Rc<Buffer<B>>>>; STAGES],
    /// Per constant buffer slot, the range bound (first constant, constants), or none for the
    /// whole buffer.
    constant_ranges: [Vec<Option<(u32, u32)>>; STAGES],
    resources: [Vec<Option<Rc<ShaderView<B>>>>; STAGES],
    samplers: [Vec<Option<Rc<Sampler<B>>>>; STAGES],
    /// Per stage, the unordered access views bound (none for the vertex stage).
    uavs: [Vec<Option<Rc<ShaderView<B>>>>; STAGES],
    pub(crate) targets: Vec<Option<Rc<View<B>>>>,
    pub(crate) depth: Option<Rc<View<B>>>,
    pub(crate) blend: Option<Rc<Blend<B>>>,
    blend_factor: Color,
    sample_mask: u32,
    pub(crate) depth_stencil: Option<Rc<DepthStencil<B>>>,
    stencil_ref: u32,
    pub(crate) raster: Option<Rc<Rasterizer<B>>>,
    viewports: Vec<Viewport>,
    scissors: Vec<ScissorRect>,
    pub(crate) sets: [Option<Kept<B, B::BindingSet>>; STAGES],
    sets_dirty: [bool; STAGES],
    /// The constant buffer windows each stage's set reads through copies: slot, buffer, offset,
    /// length and the block's size.
    windows: [Vec<ConstantWindow<B>>; STAGES],
    /// The input layout whose pulled streams the vertex stage's set binds (its id; zero for none).
    pulled_set: u64,
    pass_open: bool,
    /// The draw runs its pixel shader's forced early tests in the program, so the pass it opens
    /// holds the depth-stencil attachment read-only (`EarlyTestsInShader`).
    early: bool,
    /// The open pass was opened so.
    pass_early: bool,
    /// The draw is the testing run of one primitive of a draw whose forced early tests are run per
    /// primitive (`EarlyTestsPerPrimitive`), and the API draw's bases its helpers give.
    early_run: Option<[u32; 2]>,
    /// The layer and viewport of the draw's run after a geometry shader choosing them
    /// (`LayerSplit`, `MultiViewportSplit`), which pixel programs read as theirs; zero otherwise.
    run: [u32; 2],
    /// The layer count of that run's views, which the pass-through reads (`LAYER_FROM_STAGE`).
    run_layers: u32,
    /// Per stage, the constant buffer slots its set binds as storage buffers.
    relocated: [u32; STAGES],
    /// The resources the open pass's draws wrote as storage and read as shader resources, by
    /// address and sorted: a draw using one the other way ends the pass (`UsageScopeSplit`).
    pass_written: Vec<usize>,
    pass_read: Vec<usize>,
    /// The open pass was begun by the pass tracker, which fitted its targets to a larger depth
    /// buffer (`AttachmentsFitted`): the area every attachment covers, the draws' scissors cut
    /// to it.
    fitted: Option<ScissorRect>,
    /// The size of the open pass when it has no attachments.
    empty_pass: Option<EmptyPass>,
    clears: Vec<PendingClear<B>>,
    /// Mapped subresources: the resource's address, subresource, map type.
    mapped: Vec<(usize, u32, u32)>,
    /// The targets last set into the pass tracker for counted occlusion queries, by view address,
    /// and whether the counter has copied the depth its segment's draws began from.
    counted_targets: Vec<usize>,
    counted_copy: bool,
    /// The predicate draws, dispatches, clears, copies and writes are predicated on, and the value
    /// that skips them; none with the hint flag, whose operations are kept, as Direct3D allows.
    predication: Option<(Rc<crate::query::Query<B>>, bool)>,
}

impl<B: Backend> ContextState<B> {
    pub fn new() -> Self {
        ContextState {
            vs: None,
            ps: None,
            cs: None,
            gs: None,
            gs_stage: None,
            hs: None,
            ds: None,
            so_targets: Default::default(),
            vertex_compute: false,
            domain_compute: false,
            through: Default::default(),
            twins: [false; STAGES],
            compared: Default::default(),
            layout: None,
            topology: topology::UNDEFINED,
            vertex_buffers: vec![None; VERTEX_SLOTS],
            index: None,
            constant_buffers: std::array::from_fn(|_| vec![None; CONSTANT_BUFFER_SLOTS]),
            constant_ranges: std::array::from_fn(|_| vec![None; CONSTANT_BUFFER_SLOTS]),
            resources: std::array::from_fn(|_| vec![None; RESOURCE_SLOTS]),
            samplers: std::array::from_fn(|_| vec![None; SAMPLER_SLOTS]),
            uavs: std::array::from_fn(|stage| match stage {
                PIXEL | COMPUTE => vec![None; UAV_SLOTS],
                _ => Vec::new(),
            }),
            targets: vec![None; TARGET_SLOTS],
            depth: None,
            blend: None,
            blend_factor: [1.0; COLOR_COMPONENTS],
            sample_mask: u32::MAX,
            depth_stencil: None,
            stencil_ref: 0,
            raster: None,
            viewports: Vec::new(),
            scissors: Vec::new(),
            sets: Default::default(),
            sets_dirty: [true; STAGES],
            windows: Default::default(),
            pulled_set: 0,
            pass_open: false,
            early: false,
            pass_early: false,
            early_run: None,
            run: [0; 2],
            run_layers: 0,
            relocated: [0; STAGES],
            pass_written: Vec::new(),
            pass_read: Vec::new(),
            fitted: None,
            empty_pass: None,
            clears: Vec::new(),
            mapped: Vec::new(),
            counted_targets: Vec::new(),
            counted_copy: false,
            predication: None,
        }
    }
}

impl<B: Backend> Default for ContextState<B> {
    fn default() -> Self {
        Self::new()
    }
}

fn put<T: Clone>(slots: &mut [Option<T>], start: u32, items: &[Option<T>]) -> Result<()> {
    let start = start as usize;
    if start + items.len() > slots.len() {
        return Err(Error::InvalidArg("slots past the stage's last"));
    }
    slots[start..start + items.len()].clone_from_slice(items);
    Ok(())
}

/// Binds `items` from `start`; true if any slot changed.
fn put_rc<T>(slots: &mut [Option<Rc<T>>], start: u32, items: &[Option<Rc<T>>]) -> Result<bool> {
    let start = start as usize;
    if start + items.len() > slots.len() {
        return Err(Error::InvalidArg("slots past the stage's last"));
    }
    let mut changed = false;
    for (slot, item) in slots[start..start + items.len()].iter_mut().zip(items) {
        if !same(slot, item) {
            *slot = item.clone();
            changed = true;
        }
    }
    Ok(changed)
}

fn same<T>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Indices of the vertices an adjacency topology draws without a geometry shader, as a list of its
/// base topology (OpenGL's definition of primitives with adjacency, which Direct3D shares).
fn adjacency_indices(topology: u32, count: u32) -> Option<(Topology, Vec<u32>)> {
    let mut out = Vec::new();
    let base = match topology {
        topology::LINELIST_ADJ => {
            for k in 0..count / 4 {
                out.extend([4 * k + 1, 4 * k + 2]);
            }
            Topology::LineList
        }
        topology::LINESTRIP_ADJ => {
            for i in 1..count.saturating_sub(2) {
                out.extend([i, i + 1]);
            }
            Topology::LineList
        }
        topology::TRIANGLELIST_ADJ => {
            for k in 0..count / 6 {
                out.extend([6 * k, 6 * k + 2, 6 * k + 4]);
            }
            Topology::TriangleList
        }
        topology::TRIANGLESTRIP_ADJ => {
            let triangles = count.saturating_sub(4) / 2;
            for i in 0..triangles {
                if i % 2 == 0 {
                    out.extend([2 * i, 2 * i + 2, 2 * i + 4]);
                } else {
                    out.extend([2 * i + 2, 2 * i, 2 * i + 4]);
                }
            }
            Topology::TriangleList
        }
        _ => return None,
    };
    Some((base, out))
}

fn plain_topology(t: u32) -> Option<Topology> {
    Some(match t {
        topology::POINTLIST => Topology::PointList,
        topology::LINELIST => Topology::LineList,
        topology::LINESTRIP => Topology::LineStrip,
        topology::TRIANGLELIST => Topology::TriangleList,
        topology::TRIANGLESTRIP => Topology::TriangleStrip,
        _ => return None,
    })
}

/// The pixel counter's programs in the device's counting programs, by the place of a pixel
/// shader's id: one count a pixel, the same on multisampled targets (the first covered sample
/// counts), and one count a sample.
const PER_PIXEL: u64 = u64::MAX;
const PER_PIXEL_MASKED: u64 = u64::MAX - 1;
const PER_SAMPLE: u64 = u64::MAX - 2;

/// Words of `HelperValue::ViewportAdjust`: the scale and offset of clip x and y.
const ADJUST_WORDS: usize = 4;

/// Where a pixel's centre lies within it, in pixels from its top left corner.
const PIXEL_CENTRE: f32 = 0.5;

/// Refused: a vertex and pixel program pair passing more values between stages than the back
/// end's inter-stage locations hold after moving them (`VaryingsBeyondLimit`).
const VARYINGS_BEYOND_LIMIT: &str = "more values between stages than the back end passes";
/// Draws refused by name.
pub(crate) const ALPHA_ANTIALIASED_LINES: &str = "alpha-antialiased lines of a geometry or domain stage, an adjacency topology or beside stream output (AntialiasedLinesAsQuads)";
const LINES_WITHOUT_COMPUTE: &str =
    "alpha-antialiased lines without the back end's compute (AntialiasedLinesAsQuads)";
const LINES_WITHOUT_PIXEL_SHADER: &str =
    "alpha-antialiased lines without a pixel shader to apply their coverage";
/// A colour as Direct3D clears with it (red, green, blue, alpha).
pub type Color = [f32; COLOR_COMPONENTS];
/// The components of a [`Color`].
pub const COLOR_COMPONENTS: usize = 4;
/// A rectangle as Direct3D gives it (left, top, right, bottom).
pub type Rect = [i32; RECT_EDGES];
/// The edges of a [`Rect`].
pub const RECT_EDGES: usize = 4;
const COMPUTE_VARIANT_OF_DRAWS: &str = "a compute program variant only draws take";
const LINES_FACING: &str = "a pixel shader reading its facing on alpha-antialiased lines";
/// The bits of the channels of `R10G10B10A2` and of `R11G11B10_FLOAT`.
const RGB10A2_BITS: [u32; 4] = [10, 10, 10, 2];
const RG11B10_BITS: [u32; 3] = [11, 11, 10];
const CLEAR_OF_PACKED_FORMAT: &str = "a clear of a view of a packed or BGR format";
const CLEAR_VIEW_BUFFER_RECTS: &str = "ClearView of a buffer view's rectangles";
const CLEAR_VIEW_FLOAT_DEPTH_OUTSIDE_RANGE: &str =
    "ClearView of float depth to a value outside 0 to 1, which WebGPU's depth writes cannot store";
const CLEAR_VIEW_STORAGE_RECTS_UNSERVED: &str = "ClearView of rectangles of a storage view stored through a buffer or as an integer twin, of a 3D texture or one no pass draws into";
const CLEAR_STORAGE_UNSERVED: &str = "a clear of a storage view stored through a buffer or as an integer twin that no pass draws into";
/// Blending into a target held as its integer twin is done in the program from a copy of the
/// target (`BlendInShader`), which the shaders crate does not build.
const BLEND_INTO_INTEGER_TWIN: &str =
    "blending into a normalised target held as its integer twin (blending in the shader)";
const VIEWPORT_BEHIND_GEOMETRY: &str =
    "a viewport the back end would round, behind a geometry stage";

/// A viewport the vertex program maps positions onto (`ViewportInShader`): the whole-pixel
/// viewport the back end draws with, the scale and offset of clip x and y
/// (`HelperValue::ViewportAdjust`), and the pixels the API's viewport covers.
struct ShaderViewport {
    viewport: Viewport,
    adjust: [f32; ADJUST_WORDS],
    pixels: ScissorRect,
}

/// How a draw runs its pixel shader's forced early depth and stencil tests.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Early {
    /// The back end's tests serve: no early tests forced, or nothing they hold back.
    No,
    /// In the program, against a read-only attachment (`EarlyTestsInShader`).
    InProgram,
    /// In the program, one primitive at a time, as the draw writes depth or stencil
    /// (`EarlyTestsPerPrimitive`).
    PerPrimitive,
}

/// The vertices of one primitive of a list topology, as Direct3D numbers them; none for strips.
fn list_vertices(t: Topology) -> Option<u32> {
    match t {
        Topology::PointList => Some(1),
        Topology::LineList => Some(2),
        Topology::TriangleList => Some(3),
        _ => None,
    }
}

/// Whether topology `t` is a strip, whose indexed draws restart at a cut index.
fn strip_topology(t: u32) -> bool {
    matches!(t, topology::LINESTRIP | topology::TRIANGLESTRIP)
}

/// The primitives `count` vertices of topology `t` make (Direct3D's input assembler: incomplete
/// primitives are dropped).
fn primitives(t: u32, count: u32) -> u64 {
    let n = u64::from(count);
    match t {
        topology::POINTLIST => n,
        topology::LINELIST => n / 2,
        topology::LINESTRIP => n.saturating_sub(1),
        topology::TRIANGLELIST => n / 3,
        topology::TRIANGLESTRIP => n.saturating_sub(2),
        topology::LINELIST_ADJ => n / 4,
        topology::LINESTRIP_ADJ => n.saturating_sub(3),
        topology::TRIANGLELIST_ADJ => n / 6,
        topology::TRIANGLESTRIP_ADJ => n.saturating_sub(4) / 2,
        // Patch lists of 1 to 32 control points.
        33..=64 => n / u64::from(t - 32),
        _ => 0,
    }
}

/// Direct3D's answer to a copy between textures of DXGI formats `a` and `b`: formats of one typeless
/// group, and from feature level 10_1 its conversions between groups, copy their bits; others are
/// invalid.
fn copy_formats(a: u32, b: u32, level: u32) -> Result<()> {
    if format::typeless_group(a) == format::typeless_group(b)
        || (level >= desc::level::L10_1 && format::copy_cast(a, b))
    {
        Ok(())
    } else {
        Err(Error::InvalidArg(
            "a copy between formats of different groups",
        ))
    }
}

/// Whether a box and a place in an NV12 texture fall on whole chroma pairs.
fn even(b: &Box3, at: [u32; 3]) -> bool {
    [b.left, b.top, b.right, b.bottom, at[0], at[1]]
        .iter()
        .all(|v| v.is_multiple_of(2))
}

/// A box of the program's in pixels as one in texels of a texture whose texels are pixel pairs
/// (YUY2) or not; none for a box at an odd pixel of pairs, which Direct3D drops.
fn pixel_box(pairs: bool, b: Option<&Box3>) -> Option<Option<Box3>> {
    match b {
        Some(b) if pairs => {
            (b.left.is_multiple_of(2) && b.right.is_multiple_of(2)).then_some(Some(Box3 {
                left: b.left / 2,
                right: b.right / 2,
                ..*b
            }))
        }
        b => Some(b.copied()),
    }
}

/// The extent of NV12's chroma plane under an extent of its luma.
fn half(e: Extent) -> Extent {
    Extent {
        width: e.width / 2,
        height: e.height / 2,
        ..e
    }
}

/// The back end's object a draw binds for an empty slot.
enum Zero<B: Backend> {
    /// A buffer of zeros, bound for this many bytes.
    Buffer(B::Buffer, u64),
    View(B::TextureView),
    Sampler(B::Sampler),
    /// The zero buffer read as a structured or raw buffer.
    Storage(B::Buffer),
}

/// How a draw names its range before the front picks the interface's form.
#[derive(Clone, Copy)]
enum Range {
    Vertices {
        count: u32,
        start: u32,
    },
    Indexed {
        count: u32,
        start: u32,
        base: i32,
    },
    /// The arguments are in a buffer, at a byte offset.
    Indirect {
        indexed: bool,
        offset: u32,
    },
}

/// A direct draw's range: its first vertex or index, its count, and an indexed draw's base vertex.
#[derive(Clone, Copy)]
struct Direct {
    start: u32,
    count: u32,
    base: i32,
    indexed: bool,
}

impl Range {
    /// The range of a direct draw; `None` for an indirect one.
    fn direct(self) -> Option<Direct> {
        match self {
            Range::Vertices { count, start } => Some(Direct {
                start,
                count,
                base: 0,
                indexed: false,
            }),
            Range::Indexed { count, start, base } => Some(Direct {
                start,
                count,
                base,
                indexed: true,
            }),
            Range::Indirect { .. } => None,
        }
    }
}

/// The bytes of the whole storage words in `bytes`, the sizes buffer bindings take.
fn whole_words(bytes: u64) -> u64 {
    bytes & !(wcontainers_gpu::STORAGE_WORD - 1)
}

/// `d` writing nothing in the aspects a depth-stencil view's read-only `flags` mark, where `d`
/// writes there.
pub(crate) fn read_only_twin(d: &DepthStencilDesc, flags: u32) -> Option<DepthStencilDesc> {
    let depth = flags & DSV_READ_ONLY_DEPTH != 0 && d.depth_enable && d.depth_write;
    let stencil =
        flags & DSV_READ_ONLY_STENCIL != 0 && d.stencil_enable && d.stencil_write_mask != 0;
    (depth || stencil).then_some(DepthStencilDesc {
        depth_write: d.depth_write && !depth,
        stencil_write_mask: if stencil { 0 } else { d.stencil_write_mask },
        ..*d
    })
}

/// Texels of Direct3D's layout of a format with depth and stencil, `texel` bytes each
/// (D24_UNORM_S8_UINT: depth in the low 24 bits of a word, stencil in its high byte;
/// D32_FLOAT_S8X24_UINT: the float, the stencil byte, three unused), as the back end takes them:
/// depth in four bytes per texel (24-bit depth in the low bits of a word), stencil in one.
pub(crate) fn split_depth_stencil(texel: u32, bytes: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let n = bytes.len() / texel as usize;
    let mut depth = Vec::with_capacity(n * 4);
    let mut stencil = Vec::with_capacity(n);
    for c in bytes.chunks_exact(texel as usize) {
        if texel == 4 {
            depth.extend_from_slice(&[c[0], c[1], c[2], 0]);
            stencil.push(c[3]);
        } else {
            depth.extend_from_slice(&c[..4]);
            stencil.push(c[4]);
        }
    }
    (depth, stencil)
}

/// The bits of the normalised depth formats, whose steps a depth bias counts in.
const DEPTH16_BITS: u32 = 16;
const DEPTH24_BITS: u32 = 24;
/// What a depth bias clamp on a float depth target needs and no back end route gives.
const FLOAT_DEPTH_BIAS_CLAMP: &str = "a depth bias clamp on float depth without the back end's";

/// The bytes of one word of indirect arguments.
const ARGUMENT_WORD_BYTES: usize = size_of::<u32>();
/// Words of `D3D11_DRAW_INSTANCED_INDIRECT_ARGS`.
const DRAW_ARGUMENT_WORDS: usize = 4;
/// Words of `D3D11_DRAW_INDEXED_INSTANCED_INDIRECT_ARGS`.
const INDEXED_DRAW_ARGUMENT_WORDS: usize = 5;
/// A dispatch's axes: its group counts, and the words of its indirect arguments.
pub(crate) const DISPATCH_AXES: usize = 3;

/// Bytes of one indirect draw's arguments.
const fn indirect_bytes(indexed: bool) -> u32 {
    let words = if indexed {
        INDEXED_DRAW_ARGUMENT_WORDS
    } else {
        DRAW_ARGUMENT_WORDS
    };
    (words * ARGUMENT_WORD_BYTES) as u32
}

/// Little-endian words read back from a buffer.
fn argument_words<const N: usize>(bytes: &[u8]) -> [u32; N] {
    let (words, _) = bytes.as_chunks::<ARGUMENT_WORD_BYTES>();
    std::array::from_fn(|i| words.get(i).copied().map_or(0, u32::from_le_bytes))
}

impl<B: Backend> Device<B> {
    pub fn vs_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.vs, &shader) {
            self.ctx.vs = shader;
            self.ctx.sets_dirty[VERTEX] = true;
        }
    }

    pub fn ps_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.ps, &shader) {
            self.ctx.ps = shader;
            self.ctx.sets_dirty[PIXEL] = true;
        }
    }

    pub fn cs_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.cs, &shader) {
            self.ctx.cs = shader;
            self.ctx.sets_dirty[COMPUTE] = true;
        }
    }

    pub fn hs_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.hs, &shader) {
            self.ctx.hs = shader;
            self.ctx.sets_dirty[HULL] = true;
        }
    }

    pub fn ds_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.ds, &shader) {
            self.ctx.ds = shader;
            self.ctx.sets_dirty[DOMAIN] = true;
        }
    }

    pub fn gs_set_shader(&mut self, shader: Option<Rc<Geometry<B>>>) {
        if !same(&self.ctx.gs, &shader) {
            self.ctx.gs_stage = shader
                .as_ref()
                .and_then(|g| g.stage.as_ref().map(|s| s.0.clone()));
            self.ctx.gs = shader;
            self.ctx.sets_dirty[GEOMETRY] = true;
        }
    }

    /// `SOSetTargets`: the buffers stream output writes, each from `offsets` (`u32::MAX` appends
    /// where it was filled to).
    pub fn so_set_targets(
        &mut self,
        buffers: &[Option<Rc<Buffer<B>>>],
        offsets: &[u32],
    ) -> Result<()> {
        if buffers.len() > STREAM_OUTPUT_SLOTS {
            return Err(Error::InvalidArg("more than four stream output targets"));
        }
        let mut targets: [Option<Rc<Buffer<B>>>; STREAM_OUTPUT_SLOTS] = Default::default();
        for (i, b) in buffers.iter().enumerate() {
            let Some(b) = b else { continue };
            if b.desc.bind_flags & desc::bind::STREAM_OUTPUT == 0 {
                return Err(Error::InvalidArg(
                    "a stream output target made without its binding",
                ));
            }
            let offset = offsets.get(i).copied().unwrap_or(0);
            let filled = self.filled_counter(b)?;
            if offset != u32::MAX {
                self.end_pass();
                self.backend
                    .write_buffer(&filled, 0, &offset.to_le_bytes(), WriteMode::Ordered);
            }
            targets[i] = Some(b.clone());
        }
        self.ctx.so_targets = targets;
        Ok(())
    }

    /// Binds unordered access views to `stage` (pixel or compute) from `start`, setting the hidden
    /// counters of those given an initial count (`u32::MAX` keeps a counter as it is).
    pub fn set_uavs(
        &mut self,
        stage: usize,
        start: u32,
        views: &[Option<Rc<ShaderView<B>>>],
        counts: Option<&[u32]>,
    ) -> Result<()> {
        if put_rc(&mut self.ctx.uavs[stage], start, views)? {
            self.ctx.sets_dirty[stage] = true;
            // What a pipeline writes it no longer reads: the pixel stage's views are bound with the
            // targets, for every stage.
            if stage == COMPUTE {
                self.unbind_written(&[COMPUTE]);
            } else {
                self.unbind_written(&[VERTEX, PIXEL, COMPUTE, GEOMETRY, HULL, DOMAIN]);
            }
        }
        for (i, v) in views.iter().enumerate() {
            let count = counts.and_then(|c| c.get(i)).copied().unwrap_or(u32::MAX);
            if let (Some(ShaderView::Buffer(span)), true) = (v.as_deref(), count != u32::MAX)
                && let Some(counter) = &span.counter
            {
                self.end_pass();
                self.backend.write_buffer(
                    &counter.handle,
                    0,
                    &count.to_le_bytes(),
                    WriteMode::Ordered,
                );
            }
        }
        Ok(())
    }

    /// Whether a view is bound to unordered access slot `slot` of `stage`.
    pub fn uav_bound(&self, stage: usize, slot: u32) -> bool {
        self.ctx.uavs[stage]
            .get(slot as usize)
            .is_some_and(Option::is_some)
    }

    pub fn ia_set_input_layout(&mut self, layout: Option<Rc<InputLayout>>) {
        self.ctx.layout = layout;
    }

    pub fn ia_set_primitive_topology(&mut self, topology: u32) {
        self.ctx.topology = topology;
    }

    pub fn ia_set_vertex_buffers(
        &mut self,
        start: u32,
        slots: &[Option<VertexSlot<B>>],
    ) -> Result<()> {
        // The vertex stage's set binds the streams its program fetches.
        if self.ctx.pulled_set != 0 {
            self.ctx.sets_dirty[VERTEX] = true;
        }
        put(&mut self.ctx.vertex_buffers, start, slots)
    }

    pub fn ia_set_index_buffer(
        &mut self,
        buffer: Option<Rc<Buffer<B>>>,
        format: u32,
        offset: u32,
    ) -> Result<()> {
        self.ctx.index = match buffer {
            None => None,
            Some(b) => Some((
                b,
                format::index_format(format).ok_or(Error::InvalidArg("index format"))?,
                offset,
            )),
        };
        Ok(())
    }

    pub fn set_constant_buffers(
        &mut self,
        stage: usize,
        start: u32,
        b: &[Option<Rc<Buffer<B>>>],
    ) -> Result<()> {
        self.set_constant_ranges(stage, start, b, &vec![None; b.len()])
    }

    /// Binds constant buffers from `start`, each with its range of 16-byte constants (first,
    /// count; none for the whole buffer), as Direct3D 11.1 binds them.
    pub fn set_constant_ranges(
        &mut self,
        stage: usize,
        start: u32,
        b: &[Option<Rc<Buffer<B>>>],
        ranges: &[Option<(u32, u32)>],
    ) -> Result<()> {
        for r in ranges.iter().flatten() {
            if r.0 % 16 != 0 || r.1 % 16 != 0 || r.1 > 4096 {
                return Err(Error::InvalidArg(
                    "a constant range not in multiples of 16 constants, or past 4096",
                ));
            }
        }
        let changed = put_rc(&mut self.ctx.constant_buffers[stage], start, b)?;
        let slots = &mut self.ctx.constant_ranges[stage][start as usize..][..ranges.len()];
        if changed || slots != ranges {
            slots.copy_from_slice(ranges);
            self.ctx.sets_dirty[stage] = true;
        }
        Ok(())
    }

    pub fn set_shader_resources(
        &mut self,
        stage: usize,
        start: u32,
        v: &[Option<Rc<ShaderView<B>>>],
    ) -> Result<()> {
        // A view of a subresource the pipeline writes is bound as none, as Direct3D binds it.
        let v: Vec<_> = v
            .iter()
            .map(|v| v.clone().filter(|v| !self.written(stage, v)))
            .collect();
        if put_rc(&mut self.ctx.resources[stage], start, &v)? {
            self.ctx.sets_dirty[stage] = true;
        }
        Ok(())
    }

    /// Whether a shader resource view is bound to `slot` of `stage`.
    pub fn shader_resource_bound(&self, stage: usize, slot: u32) -> bool {
        self.ctx.resources[stage]
            .get(slot as usize)
            .is_some_and(Option::is_some)
    }

    /// Whether `stage`'s pipeline writes a subresource of `view`: through a target, or an unordered
    /// access view of the pixel stage (graphics stages) or of the compute stage.
    fn written(&self, stage: usize, view: &ShaderView<B>) -> bool {
        let writer = if stage == COMPUTE { COMPUTE } else { PIXEL };
        view.texture().is_some_and(|t| self.bound_as_target(t))
            || self.ctx.uavs[writer]
                .iter()
                .flatten()
                .any(|u| u.overlaps(view))
    }

    /// Unbinds the shader resource views of `stages` that name what their pipeline writes.
    fn unbind_written(&mut self, stages: &[usize]) {
        for &stage in stages {
            let mut resources = std::mem::take(&mut self.ctx.resources[stage]);
            for slot in &mut resources {
                if slot.as_ref().is_some_and(|v| self.written(stage, v)) {
                    *slot = None;
                    self.ctx.sets_dirty[stage] = true;
                }
            }
            self.ctx.resources[stage] = resources;
        }
    }

    /// Whether a bound target or depth-stencil view writes what `view` reads: a read-only
    /// depth-stencil view writes neither aspect it marks, so shader views of those may be bound.
    fn bound_as_target(&self, view: &View<B>) -> bool {
        let aspect = if matches!(view.desc.format, 22 | 47) {
            DSV_READ_ONLY_STENCIL
        } else {
            DSV_READ_ONLY_DEPTH
        };
        self.ctx.targets.iter().flatten().any(|t| t.overlaps(view))
            || self
                .ctx
                .depth
                .as_ref()
                .is_some_and(|d| d.overlaps(view) && d.desc.read_only & aspect == 0)
    }

    pub fn set_samplers(
        &mut self,
        stage: usize,
        start: u32,
        s: &[Option<Rc<Sampler<B>>>],
    ) -> Result<()> {
        if put_rc(&mut self.ctx.samplers[stage], start, s)? {
            self.ctx.sets_dirty[stage] = true;
        }
        Ok(())
    }

    pub fn om_set_render_targets(
        &mut self,
        targets: &[Option<Rc<View<B>>>],
        depth: Option<Rc<View<B>>>,
    ) -> Result<()> {
        if targets.len() > TARGET_SLOTS {
            return Err(Error::InvalidArg("more than eight render targets"));
        }
        let mut new = vec![None; TARGET_SLOTS];
        new[..targets.len()].clone_from_slice(targets);
        let changed = !same(&self.ctx.depth, &depth)
            || new.iter().zip(&self.ctx.targets).any(|(a, b)| !same(a, b));
        if changed {
            self.end_pass();
            self.ctx.targets = new;
            self.ctx.depth = depth;
            // Binding a subresource as a target unbinds its shader resource views, as in Direct3D.
            self.unbind_written(&[VERTEX, PIXEL, COMPUTE, GEOMETRY, HULL, DOMAIN]);
        }
        Ok(())
    }

    pub fn om_set_blend_state(
        &mut self,
        blend: Option<Rc<Blend<B>>>,
        factor: Option<Color>,
        mask: u32,
    ) {
        self.ctx.blend = blend;
        self.ctx.blend_factor = factor.unwrap_or([1.0; COLOR_COMPONENTS]);
        self.ctx.sample_mask = mask;
    }

    pub fn om_set_depth_stencil_state(
        &mut self,
        state: Option<Rc<DepthStencil<B>>>,
        reference: u32,
    ) {
        self.ctx.depth_stencil = state;
        self.ctx.stencil_ref = reference;
    }

    pub fn rs_set_state(&mut self, state: Option<Rc<Rasterizer<B>>>) {
        self.ctx.raster = state;
    }

    /// More than Direct3D's 16 viewports is invalid and dropped.
    pub fn rs_set_viewports(&mut self, viewports: &[Viewport]) {
        if viewports.len() <= VIEWPORTS {
            self.ctx.viewports = viewports.to_vec();
        }
    }

    /// Scissor rectangles as (left, top, right, bottom).
    pub fn rs_set_scissor_rects(&mut self, rects: &[Rect]) {
        self.ctx.scissors = rects.iter().map(|&r| pixel_rect(r)).collect();
    }

    pub fn clear_render_target_view(&mut self, view: &Rc<View<B>>, color: Color) {
        if self.predicated_out() {
            return;
        }
        self.add_clear(view, color_value(view, color));
    }

    /// `ClearView` of rectangles (left, top, right, bottom) of a target view, the same rectangles
    /// in each of its layers.
    pub fn clear_view_rects(
        &mut self,
        view: &Rc<View<B>>,
        color: Color,
        rects: &[Rect],
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        let rects: Vec<ScissorRect> = rects.iter().map(|&r| pixel_rect(r)).collect();
        self.settle();
        let value = color_value(view, color);
        if view.texture.desc.dimension == TexDim::D3 {
            return self.clear_slice_rects(view, value, &rects);
        }
        self.clear_rects_drawn(view, value, &rects, desc::bind::RENDER_TARGET, true)
    }

    /// `ClearView` of a depth view, whole or its rectangles: Direct3D clears its depth alone, to
    /// the first value, limited to 0 to 1 in normalised depth (NaN as 0) and stored as given in
    /// float depth (measured on the RTX 3080). WebGPU's depth clears and draws write 0 to 1 alone,
    /// so float depth outside it is refused by name.
    pub fn clear_view_depth(
        &mut self,
        view: &Rc<View<B>>,
        depth: f32,
        rects: &[Rect],
    ) -> Result<()> {
        let float = matches!(
            view.format,
            Format::Depth32Float | Format::Depth32FloatStencil8
        );
        let depth = match depth {
            d if (0.0..=1.0).contains(&d) => d,
            _ if float => return not_implemented(CLEAR_VIEW_FLOAT_DEPTH_OUTSIDE_RANGE),
            d if d.is_nan() => 0.0,
            d => d.clamp(0.0, 1.0),
        };
        if rects.is_empty() {
            self.clear_depth_stencil_view(view, desc::clear::DEPTH, depth, 0);
            return Ok(());
        }
        if self.predicated_out() {
            return Ok(());
        }
        let rects: Vec<ScissorRect> = rects.iter().map(|&r| pixel_rect(r)).collect();
        self.settle();
        let value = ClearValue::DepthStencil {
            depth: Some(depth.to_bits()),
            stencil: None,
        };
        self.clear_rects_drawn(view, value, &rects, desc::bind::DEPTH_STENCIL, true)
    }

    /// `ClearView` of rectangles of a 3D target view: no view draws into one slice of a 3D level
    /// alone, so a texture of one slice is cleared and each rectangle copied from it into each
    /// slice the view sees (`ReinterpretCopy`).
    fn clear_slice_rects(
        &mut self,
        view: &View<B>,
        value: ClearValue,
        rects: &[ScissorRect],
    ) -> Result<()> {
        let t = &view.texture;
        let mip = view.desc.first_mip;
        let extent = t.mip_extent(mip);
        let desc = wcontainers_gpu::TextureDesc {
            dimension: wcontainers_gpu::TextureDimension::D2,
            format: view.format,
            width: extent.width,
            height: extent.height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        };
        let slice = self.backend.create_texture(&desc);
        let slice = self.gpu(slice)?;
        let id = self.id();
        let slice = Kept::new(slice, id, Object::Texture, &self.graveyard);
        self.backend.slow_paths().hit(SlowPath::ReinterpretCopy);
        let cleared =
            self.backend
                .clear_texture(&slice.handle, &desc, view.format, 0..1, 0..1, value);
        self.gpu(cleared)?;
        let layers = view.desc.first_layer..view.desc.first_layer + view.desc.layer_count;
        for r in rects {
            let width = r.x.saturating_add(r.width).min(extent.width);
            let height = r.y.saturating_add(r.height).min(extent.height);
            if r.x >= width || r.y >= height {
                continue;
            }
            let at = Origin {
                x: r.x,
                y: r.y,
                z: 0,
            };
            for z in layers.clone() {
                self.backend.copy_texture(
                    &TextureRegion {
                        texture: &slice.handle,
                        mip: 0,
                        layer: 0,
                        origin: at,
                        stencil: false,
                    },
                    &TextureRegion {
                        texture: &t.gpu.handle,
                        mip,
                        layer: 0,
                        origin: Origin { z, ..at },
                        stencil: false,
                    },
                    Extent {
                        width: width - r.x,
                        height: height - r.y,
                        depth_or_layers: 1,
                    },
                );
            }
        }
        Ok(())
    }

    /// Clears `rects` of each layer of `view` with `value` by drawing into it: through the view
    /// itself where it is a view of one layer of binding `kind` (`itself`), else through a view
    /// of `kind` of each layer.
    fn clear_rects_drawn(
        &mut self,
        view: &Rc<View<B>>,
        value: ClearValue,
        rects: &[ScissorRect],
        kind: u32,
        itself: bool,
    ) -> Result<()> {
        for layer in 0..view.desc.layer_count {
            let one = if itself && view.desc.layer_count == 1 {
                view.clone()
            } else {
                self.layer_view(view, layer, kind)?
            };
            // What lies outside the rectangles is copied in, so that copying out keeps it.
            if one.copied() {
                self.copy_twin(&one, true);
            }
            let cleared = self
                .backend
                .clear_rects(&one.gpu.handle, one.format, value, rects);
            self.gpu(cleared)?;
            if one.copied() {
                self.copy_twin(&one, false);
            }
        }
        Ok(())
    }

    /// `ClearDepthStencilView`: a depth outside 0 to 1 (NaN among them) makes Direct3D drop the
    /// whole call, the stencil too (measured on the RTX 3080).
    pub fn clear_depth_stencil_view(
        &mut self,
        view: &Rc<View<B>>,
        flags: u32,
        depth: f32,
        stencil: u8,
    ) {
        let clears_depth = flags & desc::clear::DEPTH != 0;
        if self.predicated_out() || clears_depth && !(0.0..=1.0).contains(&depth) {
            return;
        }
        let kind = FORMAT_FACTS[view.format as usize].kind;
        let has_stencil = matches!(kind, FormatKind::DepthStencil | FormatKind::Stencil);
        let value = ClearValue::DepthStencil {
            depth: (clears_depth && kind != FormatKind::Stencil).then(|| depth.to_bits()),
            stencil: (flags & desc::clear::STENCIL != 0 && has_stencil).then_some(stencil),
        };
        self.add_clear(view, value);
    }

    fn add_clear(&mut self, view: &Rc<View<B>>, value: ClearValue) {
        self.end_pass();
        if let Some(i) = self
            .ctx
            .clears
            .iter()
            .position(|c| Rc::ptr_eq(&c.view, view))
        {
            let old = self.ctx.clears.remove(i);
            if let (
                ClearValue::DepthStencil {
                    depth: d0,
                    stencil: s0,
                },
                ClearValue::DepthStencil { depth, stencil },
            ) = (old.value, value)
            {
                let merged = ClearValue::DepthStencil {
                    depth: depth.or(d0),
                    stencil: stencil.or(s0),
                };
                self.ctx.clears.push(PendingClear {
                    view: view.clone(),
                    value: merged,
                });
                return;
            }
        }
        self.ctx.clears.push(PendingClear {
            view: view.clone(),
            value,
        });
    }

    /// Ends the open pass. While counted occlusion queries run, the counter then counts the draws
    /// it kept, before anything changes what they read or drew on.
    pub(crate) fn end_pass(&mut self) {
        if self.ctx.pass_open {
            self.close_stretch(false);
            if self.ctx.fitted.take().is_some() {
                let r = self.passes.end(&mut self.backend);
                let _ = self.gpu(r);
            } else {
                self.backend.end_pass();
            }
            self.ctx.pass_open = false;
            self.queries.pass_set = None;
            // Targets drawn on twins of their textures: what the pass drew copied out.
            for t in self.ctx.targets.clone().iter().flatten() {
                if t.copied() {
                    self.copy_twin(t, false);
                }
            }
        }
        if !self.queries.counted.is_empty() {
            let r = self.counter.flush(&mut self.backend, &mut self.passes);
            if let Err(e) = r {
                let _ = self.gpu::<()>(Err(e));
            }
            self.ctx.counted_copy = false;
        }
        if !self.queries.statistics.is_empty() {
            let r = self
                .pixel_counter
                .flush(&mut self.backend, &mut self.passes);
            if let Err(e) = r {
                let _ = self.gpu::<()>(Err(e));
            }
        }
    }

    /// `SetPredication`.
    pub fn set_predication(&mut self, q: Option<Rc<crate::query::Query<B>>>, value: bool) {
        self.ctx.predication = q
            .filter(|q| q.misc & crate::query::PREDICATE_HINT == 0)
            .map(|q| (q, value));
    }

    /// Whether the predicate set skips the operation: its answer equals the value set. A predicate
    /// not ended skips nothing.
    fn predicated_out(&mut self) -> bool {
        let Some((q, value)) = self.ctx.predication.clone() else {
            return false;
        };
        match self.wait_answer(&q) {
            Ok(Some(v)) => (v[0] != 0) == value,
            _ => false,
        }
    }

    /// Before a resource is written while counted occlusion queries run: the draws kept read it as
    /// it was.
    fn before_write(&mut self) {
        if !self.queries.counted.is_empty() || !self.queries.statistics.is_empty() {
            self.end_pass();
        }
    }

    /// Before a draw while counted occlusion queries run: the targets set into the pass tracker,
    /// pending clears run where the counter must copy the depth the draw begins from, and the
    /// draw's counting program, with whether it writes depth or stencil.
    fn prepare_counted(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<(B::Program, bool)> {
        let program = self.counting_program(vs, vk, ps, pk)?;
        self.track_targets()?;
        let ds = &self
            .ctx
            .depth_stencil
            .as_ref()
            .unwrap_or(&self.defaults.depth_stencil)
            .desc;
        let writes = self.ctx.depth.is_some()
            && ((ds.depth_enable && ds.depth_write)
                || (ds.stencil_enable && ds.stencil_write_mask != 0));
        if writes && !self.ctx.counted_copy {
            if !self.ctx.clears.is_empty() {
                self.open_pass();
            }
            self.end_pass();
            self.ctx.counted_copy = true;
        }
        Ok((program, writes))
    }

    /// Sets the bound targets into the pass tracker for the counters, where they changed.
    fn track_targets(&mut self) -> Result<()> {
        let key: Vec<usize> = self
            .ctx
            .targets
            .iter()
            .chain([&self.ctx.depth])
            .map(|v| v.as_ref().map_or(0, |v| Rc::as_ptr(v) as usize))
            .collect();
        if self.ctx.counted_targets != key {
            self.end_pass();
            let targets = self.pass_targets();
            let r = self.passes.set_targets(&mut self.backend, targets);
            self.gpu(r)?;
            self.ctx.counted_targets = key;
        }
        Ok(())
    }

    /// Before a draw while pipeline statistics queries run and a pixel shader is bound: the targets
    /// set into the pass tracker, the depth-stencil block with both tests off, and the program the
    /// pixel counter replays the draw with, counting one per pixel shader invocation.
    fn prepare_pixel_counted(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: &Rc<Shader<B>>,
        samples: u32,
    ) -> Result<(B::Program, Rc<DepthStencil<B>>)> {
        let per_sample = matches!(
            ps.reflection.extras,
            wcontainers_gpu::StageExtras::Pixel {
                per_sample: true,
                ..
            }
        ) || ps.reflection.inputs.iter().any(|i| {
            i.system == Some(wcontainers_gpu::SystemValue::SampleIndex)
                || matches!(
                    i.interpolation,
                    Interpolation::PerspectiveSample | Interpolation::LinearSample
                )
        });
        let (id, code) = match (samples > 1, per_sample) {
            (false, _) => (
                PER_PIXEL,
                "@fragment fn main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }",
            ),
            (true, false) => (
                PER_PIXEL_MASKED,
                "struct Out { @location(0) count: vec4<f32>, @builtin(sample_mask) mask: u32 }
                 @fragment fn main(@builtin(sample_mask) covered: u32) -> Out {
                     return Out(vec4<f32>(1.0), covered & (~covered + 1u));
                 }",
            ),
            (true, true) => (
                PER_SAMPLE,
                "@fragment fn main(@builtin(sample_index) i: u32) -> @location(0) vec4<f32> {
                     return vec4<f32>(1.0);
                 }",
            ),
        };
        self.track_targets()?;
        let off = match &self.tests_off {
            Some(d) => d.clone(),
            None => {
                let d = self.create_depth_stencil_state(&DepthStencilDesc {
                    depth_enable: false,
                    depth_write: false,
                    stencil_enable: false,
                    ..DepthStencilDesc::default_state()
                })?;
                self.tests_off = Some(d.clone());
                d
            }
        };
        let key = (vs.gpu.id, vk, id, StageVariant::default());
        if let Some(p) = self.counting.get(&key) {
            return Ok((p.handle.clone(), off));
        }
        let module = ShaderModule::from_wgsl(code, "main", ShaderStage::Fragment)
            .or_else(|_| not_implemented("the pixel counter's program on this back end"))?;
        let fragment = self.backend.create_shader(&module);
        let fragment = self.gpu(fragment)?;
        let vertex = self
            .interpolated
            .get(&(vs.gpu.id, vk, ps.gpu.id))
            .map_or(&vs.gpu.handle, |s| &s.shader.handle);
        let program = self.backend.create_program(&Program::Render {
            vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(fragment));
        let program = self.gpu(program)?;
        let id = self.id();
        self.counting.insert(
            key,
            Kept::new(program.clone(), id, Object::Program, &self.graveyard),
        );
        Ok((program, off))
    }

    /// Adds a draw to the running pipeline statistics queries: input assembler, vertex shader and
    /// clipper counts from its arguments, as a GPU without vertex reuse or clipping splits counts
    /// them (indirect arguments and the indices of indexed strips read back).
    fn count_draw_statistics(
        &mut self,
        range: Range,
        instances: u32,
        args: Option<&Rc<Buffer<B>>>,
    ) -> Result<()> {
        let (range, instances) = match range {
            Range::Indirect { indexed, offset } => {
                let args = args.expect("indirect arguments");
                let (r, _, n) = self.read_indirect(args, offset, indexed)?;
                (r, n)
            }
            r => (r, instances),
        };
        let t = self.ctx.topology;
        let (count, per_instance) = match range {
            Range::Indexed { count, start, .. } if strip_topology(t) => {
                self.indexed_strip(start, count)?
            }
            Range::Indexed { count, .. } | Range::Vertices { count, .. } => {
                (u64::from(count), primitives(t, count))
            }
            Range::Indirect { .. } => unreachable!("read back above"),
        };
        let vertices = count * u64::from(instances);
        let prims = per_instance * u64::from(instances);
        use crate::query::statistic::*;
        self.add_statistics(&[
            (IA_VERTICES, vertices),
            (IA_PRIMITIVES, prims),
            (VS_INVOCATIONS, vertices),
            (C_INVOCATIONS, prims),
            (C_PRIMITIVES, prims),
        ]);
        Ok(())
    }

    /// The vertices and primitives of an indexed strip of `count` indices from `start`, read
    /// back: each cut index (all ones) ends one strip and starts the next, and is no vertex.
    fn indexed_strip(&mut self, start: u32, count: u32) -> Result<(u64, u64)> {
        let Some((buffer, format, offset)) = self.ctx.index.clone() else {
            return Ok((0, 0));
        };
        let size = match format {
            IndexFormat::U16 => 2u64,
            _ => 4,
        };
        let first = u64::from(offset) + u64::from(start) * size;
        let bytes = (u64::from(count) * size).min(buffer.size.get().saturating_sub(first));
        if bytes == 0 {
            return Ok((0, 0));
        }
        self.backend.slow_paths().hit(SlowPath::ReadBackWait);
        self.settle();
        let handle = buffer.handle();
        let rb = self.backend.read_back(&ReadSource::Buffer {
            buffer: &handle,
            offset: first,
            size: bytes,
        });
        drop(handle);
        let rb = self.gpu(rb)?;
        if self.backend.read_state(&rb) == ReadState::Pending {
            self.backend.wait_read(&rb);
        }
        let mut data = vec![0u8; bytes as usize];
        self.backend.finish_read(rb, &mut data);
        let cut = |i: &[u8]| i.iter().all(|&b| b == 0xff);
        let line = self.ctx.topology == topology::LINESTRIP;
        let (mut vertices, mut total) = (0u64, 0u64);
        for run in data
            .chunks(size as usize)
            .collect::<Vec<_>>()
            .split(|i| cut(i))
        {
            let n = run.len() as u64;
            vertices += n;
            total += if line {
                n.saturating_sub(1)
            } else {
                n.saturating_sub(2)
            };
        }
        Ok((vertices, total))
    }

    /// The bound targets as the pass tracker takes them.
    fn pass_targets(&self) -> PassTargets<B> {
        let target = |v: &Rc<View<B>>| {
            let t = &v.texture;
            let mip = v.desc.first_mip;
            let d3 = t.desc.dimension == TexDim::D3;
            Target {
                view: v.gpu.handle.clone(),
                // A view on a twin draws into the twin.
                texture: v.twin.as_ref().map_or(&t.gpu.handle, |k| &k.handle).clone(),
                format: v.format,
                mip,
                layer: if d3 { 0 } else { v.desc.first_layer },
                depth_slice: if d3 { v.desc.first_layer } else { 0 },
                width: t.mip_extent(mip).width,
                height: t.mip_extent(mip).height,
                samples: t.desc.sample_count,
            }
        };
        let empty = (self.ctx.targets.iter().all(Option::is_none) && self.ctx.depth.is_none())
            .then(|| self.empty_size());
        PassTargets {
            colors: std::array::from_fn(|i| {
                self.ctx.targets[i].as_ref().map(|v| ColorTarget {
                    target: target(v),
                    resolve: None,
                })
            }),
            depth: self.ctx.depth.as_ref().map(target),
            empty,
        }
    }

    /// The counting program of a vertex and pixel shader pair, made at its first counted draw from
    /// the pixel shader's variant `pk`, so it reads the bindings the draw binds.
    fn counting_program(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<B::Program> {
        let pair = (vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id));
        let key = (pair.0, pair.1, pair.2, pk);
        if let Some(p) = self.counting.get(&key) {
            return Ok(p.handle.clone());
        }
        // Made after the pair's own program, so the vertex shader converted for its interpolation
        // and variant is there.
        self.program(vs, vk, ps, pk)?;
        let module = match ps {
            Some(p) => {
                let counted = StageVariant {
                    samples: u8::from(p.reads_samples),
                    ..pk
                };
                let key = self.variant_key(p, counted, SourceStage::Pixel);
                let mut c = crate::shader::convert_variant(&p.bytecode, SourceStage::Pixel, &key)?;
                crate::counting::from_pixel_shader(&c.modules.pop().expect("one module"))?
            }
            None if pk == StageVariant::default() => crate::counting::without_pixel_shader()?,
            None => {
                let key = pk.key(SourceStage::Pixel, &[], 0, &[]);
                let mut c = crate::shader::pixel_program_for_variant(&key)?;
                crate::counting::from_pixel_shader(&c.modules.pop().expect("one module"))?
            }
        };
        let fragment = self.backend.create_shader(&module);
        let fragment = self.gpu(fragment)?;
        let vertex = self
            .interpolated
            .get(&pair)
            .map_or(&vs.gpu.handle, |s| &s.shader.handle);
        let program = self.backend.create_program(&Program::Render {
            vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(fragment));
        let program = self.gpu(program)?;
        let id = self.id();
        self.counting.insert(
            key,
            Kept::new(program.clone(), id, Object::Program, &self.graveyard),
        );
        Ok(program)
    }

    pub(crate) fn ctx_pass_open(&self) -> bool {
        self.ctx.pass_open
    }

    /// Ends the pass and runs every pending clear, before work outside a pass.
    pub(crate) fn settle(&mut self) {
        self.end_pass();
        let clears = std::mem::take(&mut self.ctx.clears);
        self.run_clears(clears);
    }

    fn run_clears(&mut self, clears: Vec<PendingClear<B>>) {
        for c in clears {
            let v = &c.view;
            let mips = v.desc.first_mip..v.desc.first_mip + 1;
            let layers = v.desc.first_layer..v.desc.first_layer + v.desc.layer_count;
            // A view on a twin of its texture clears the twin, copied out after.
            let twin_desc = wcontainers_gpu::TextureDesc {
                format: v.format,
                ..v.texture.gpu_desc
            };
            let (texture, desc) = match &v.twin {
                Some(t) => (&t.handle, &twin_desc),
                None => (&v.texture.gpu.handle, &v.texture.gpu_desc),
            };
            if let Err(e) = self
                .backend
                .clear_texture(texture, desc, v.format, mips, layers, c.value)
            {
                let _ = self.gpu::<()>(Err(e));
            }
            if v.copied() {
                self.copy_twin(v, false);
            }
        }
    }

    /// Before a draw: ends the open pass where the draw reads as a shader resource what an earlier
    /// draw of the pass wrote as storage, or writes as storage what one read, which WebGPU's usage
    /// scope forbids within a pass (`UsageScopeSplit`); then adds the draw's uses to the pass's.
    fn split_scope(&mut self) {
        let address = |v: &ShaderView<B>| match v {
            ShaderView::Texture(v) => Rc::as_ptr(&v.texture) as usize,
            ShaderView::Buffer(b) => Rc::as_ptr(&b.buffer) as usize,
        };
        let written: Vec<usize> = self.ctx.uavs[PIXEL]
            .iter()
            .flatten()
            .map(|v| address(v))
            .collect();
        let read: Vec<usize> = [VERTEX, PIXEL]
            .iter()
            .flat_map(|&s| self.ctx.resources[s].iter().flatten())
            .map(|v| address(v))
            .collect();
        let meets =
            |a: &[usize], sorted: &[usize]| a.iter().any(|x| sorted.binary_search(x).is_ok());
        if self.ctx.pass_open
            && (meets(&written, &self.ctx.pass_read) || meets(&read, &self.ctx.pass_written))
        {
            self.backend.slow_paths().hit(SlowPath::UsageScopeSplit);
            self.end_pass();
        }
        if !self.ctx.pass_open {
            self.ctx.pass_written.clear();
            self.ctx.pass_read.clear();
        }
        for (list, new) in [
            (&mut self.ctx.pass_written, written),
            (&mut self.ctx.pass_read, read),
        ] {
            for x in new {
                if let Err(at) = list.binary_search(&x) {
                    list.insert(at, x);
                }
            }
        }
    }

    /// Whether the bound depth-stencil view is larger than the colour targets, so their pass is
    /// fitted (`AttachmentsFitted`).
    fn fitted_targets(&self) -> bool {
        let size = |v: &Rc<View<B>>| {
            let e = v.texture.mip_extent(v.desc.first_mip);
            (e.width, e.height)
        };
        self.ctx.depth.as_ref().is_some_and(|d| {
            self.ctx
                .targets
                .iter()
                .flatten()
                .any(|t| size(t) != size(d))
        })
    }

    /// Opens a pass on the bound targets, taking pending clears of them as its loads.
    fn open_pass(&mut self) {
        if self.ctx.pass_open {
            return;
        }
        // A clear becomes a load only where the attachment is all the view sees, and is written
        // by the pass.
        let bound = |v: &Rc<View<B>>, ctx: &ContextState<B>| {
            v.desc.layer_count == 1
                && (ctx.targets.iter().flatten().any(|t| Rc::ptr_eq(t, v))
                    || !ctx.early && ctx.depth.as_ref().is_some_and(|d| Rc::ptr_eq(d, v)))
        };
        let flags = self.read_only_flags();
        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.ctx.clears)
            .into_iter()
            .partition(|c| bound(&c.view, &self.ctx));
        self.run_clears(others);
        let clear_of = |v: &Rc<View<B>>| {
            mine.iter()
                .find(|c| Rc::ptr_eq(&c.view, v))
                .map(|c| c.value)
        };
        let targets = self.ctx.targets.clone();
        let depth = self.ctx.depth.clone();
        // Targets on twins of their textures start from the texture's texels.
        for t in targets.iter().flatten() {
            if t.copied() && clear_of(t).is_none() {
                self.backend.slow_paths().hit(SlowPath::ReinterpretCopy);
                self.copy_twin(t, true);
            }
        }
        // A depth buffer larger than the colour targets, which Direct3D allows: the pass tracker
        // begins the pass, fitting the targets to it (`AttachmentsFitted`).
        if self.fitted_targets() {
            let mut ops = PassOps::LOAD_STORE;
            for (i, t) in targets.iter().enumerate() {
                if let Some(ClearValue::Color(bits)) = t.as_ref().and_then(clear_of) {
                    ops.colors[i].0 = LoadOp::Clear(bits);
                }
            }
            if let Some(ClearValue::DepthStencil {
                depth: d,
                stencil: s,
            }) = depth.as_ref().and_then(clear_of)
            {
                ops.depth.load = d.map_or(LoadOp::Load, LoadOp::Clear);
                ops.stencil.load = s.map_or(LoadOp::Load, LoadOp::Clear);
            }
            ops.depth.read_only =
                flags & DSV_READ_ONLY_DEPTH != 0 && ops.depth.load == LoadOp::Load;
            ops.stencil.read_only =
                flags & DSV_READ_ONLY_STENCIL != 0 && ops.stencil.load == LoadOp::Load;
            let occlusion = self.pass_occlusion();
            if !matches!(occlusion, Ok(None)) {
                if let Err(e) = occlusion.and_then(|_| {
                    not_implemented::<()>(
                        "occlusion queries over a depth buffer larger than its targets",
                    )
                }) {
                    self.lose(e);
                }
                return;
            }
            let pass_targets = self.pass_targets();
            let r = self
                .passes
                .begin(&mut self.backend, pass_targets, ops)
                .and_then(|()| self.passes.before_draw(&mut self.backend, &[]));
            if let Ok(area) = self.gpu(r) {
                self.ctx.fitted = area;
                self.ctx.pass_open = true;
                self.ctx.pass_early = self.ctx.early;
                self.ctx.empty_pass = None;
            }
            return;
        }
        let colors = std::array::from_fn(|i| {
            targets[i].as_ref().map(|v| ColorAttachment {
                view: &v.gpu.handle,
                depth_slice: if v.texture.desc.dimension == TexDim::D3 {
                    v.desc.first_layer
                } else {
                    0
                },
                resolve: None,
                load: match clear_of(v) {
                    Some(ClearValue::Color(bits)) => LoadOp::Clear(bits),
                    _ => LoadOp::Load,
                },
                store: StoreOp::Store,
            })
        });
        let depth_attachment = depth.as_ref().map(|v| {
            let (d, s) = match clear_of(v) {
                Some(ClearValue::DepthStencil { depth, stencil }) => (depth, stencil),
                _ => (None, None),
            };
            let kind = FORMAT_FACTS[v.format as usize].kind;
            // An aspect a read-only view marks is only read by the pass's draws (their
            // depth-stencil blocks write nothing there), unless the pass clears it.
            fn ops<V>(clear: Option<V>, read_only: bool) -> AspectOps<V> {
                AspectOps {
                    read_only: read_only && clear.is_none(),
                    load: clear.map_or(LoadOp::Load, LoadOp::Clear),
                    store: StoreOp::Store,
                }
            }
            DepthAttachment {
                view: &v.gpu.handle,
                depth: (kind != FormatKind::Stencil)
                    .then(|| ops(d, flags & DSV_READ_ONLY_DEPTH != 0)),
                stencil: matches!(kind, FormatKind::DepthStencil | FormatKind::Stencil)
                    .then(|| ops(s, flags & DSV_READ_ONLY_STENCIL != 0)),
            }
        });
        let occlusion = match self.pass_occlusion() {
            Ok(set) => set,
            Err(e) => {
                self.lose(e);
                None
            }
        };
        let empty =
            (targets.iter().all(Option::is_none) && depth.is_none()).then(|| self.empty_size());
        self.backend.begin_pass(&PassDesc {
            colors,
            depth: depth_attachment,
            empty,
            occlusion: occlusion.as_ref().map(|s| &s.handle),
        });
        self.ctx.pass_open = true;
        self.ctx.pass_early = self.ctx.early;
        self.ctx.empty_pass = empty;
    }

    /// The size of a pass without attachments (Direct3D's drawing with only unordered access views
    /// bound): what the first viewport reaches, one pixel to the largest 2D texture, one sample.
    fn empty_size(&self) -> EmptyPass {
        let max = self.backend.caps().limits.max_texture_2d;
        let reach = |at: f32, len: f32| ((at + len).ceil().max(1.0) as u32).min(max);
        let v = self.ctx.viewports.first().copied().unwrap_or_default();
        EmptyPass {
            width: reach(v.x, v.width),
            height: reach(v.y, v.height),
            samples: 1,
        }
    }

    pub fn draw(&mut self, count: u32, start: u32) {
        self.draw_any(Range::Vertices { count, start }, 0, 1);
    }

    pub fn draw_instanced(&mut self, count: u32, instances: u32, start: u32, first_instance: u32) {
        self.draw_any(Range::Vertices { count, start }, first_instance, instances);
    }

    pub fn draw_indexed(&mut self, count: u32, start: u32, base: i32) {
        self.draw_any(Range::Indexed { count, start, base }, 0, 1);
    }

    pub fn draw_indexed_instanced(
        &mut self,
        count: u32,
        instances: u32,
        start: u32,
        base: i32,
        first_instance: u32,
    ) {
        self.draw_any(
            Range::Indexed { count, start, base },
            first_instance,
            instances,
        );
    }

    /// `DrawInstancedIndirect` (`indexed` false) and `DrawIndexedInstancedIndirect`. Arguments
    /// Direct3D does not read are invalid; a draw this front cannot serve removes the device.
    pub fn draw_indirect(
        &mut self,
        args: &Rc<Buffer<B>>,
        offset: u32,
        indexed: bool,
    ) -> Result<()> {
        self.check_indirect(args, offset, indexed)?;
        if self.removed.is_some() {
            return Ok(());
        }
        // Fetching in the program steps by the first instance, which only the arguments hold;
        // the stage helper sizes its output by the draw's counts.
        let pulls = self
            .ctx
            .layout
            .as_ref()
            .is_some_and(|l| !l.pulled.is_empty())
            || self.ctx.gs.is_some()
            || self.ctx.vs.as_ref().is_some_and(|vs| vs.culled);
        let r = if !pulls
            && self
                .backend
                .caps()
                .features
                .contains(Features::INDIRECT | Features::INDIRECT_FIRST_INSTANCE)
        {
            self.try_draw(Range::Indirect { indexed, offset }, 0, 1, Some(args))
        } else {
            self.read_indirect(args, offset, indexed)
                .and_then(|(range, first, n)| self.try_draw(range, first, n, None))
        };
        if let Err(e) = r {
            self.lose(e);
        }
        Ok(())
    }

    /// An indirect dispatch's group counts read back (`IndirectReadBack`), for pipeline statistics.
    fn read_dispatch(&mut self, args: &Buffer<B>, offset: u32) -> Result<[u32; DISPATCH_AXES]> {
        self.backend.slow_paths().hit(SlowPath::IndirectReadBack);
        self.settle();
        let handle = args.handle();
        let mut bytes = [0u8; DISPATCH_AXES * ARGUMENT_WORD_BYTES];
        let rb = self.backend.read_back(&ReadSource::Buffer {
            buffer: &handle,
            offset: u64::from(offset),
            size: bytes.len() as u64,
        });
        drop(handle);
        let rb = self.gpu(rb)?;
        if self.backend.read_state(&rb) == ReadState::Pending {
            self.backend.wait_read(&rb);
        }
        self.backend.finish_read(rb, &mut bytes);
        Ok(argument_words(&bytes))
    }

    /// An indirect draw's arguments read back (`IndirectReadBack`), as the range, first instance and
    /// instance count of a direct draw: for back ends without indirect draws or without a first
    /// instance in them.
    fn read_indirect(
        &mut self,
        args: &Buffer<B>,
        offset: u32,
        indexed: bool,
    ) -> Result<(Range, u32, u32)> {
        self.backend.slow_paths().hit(SlowPath::IndirectReadBack);
        self.settle();
        let size = indirect_bytes(indexed) as usize;
        let handle = args.handle();
        let rb = self.backend.read_back(&ReadSource::Buffer {
            buffer: &handle,
            offset: u64::from(offset),
            size: size as u64,
        });
        let rb = self.gpu(rb)?;
        if self.backend.read_state(&rb) == ReadState::Pending {
            self.backend.wait_read(&rb);
        }
        let mut bytes = [0u8; INDEXED_DRAW_ARGUMENT_WORDS * ARGUMENT_WORD_BYTES];
        self.backend.finish_read(rb, &mut bytes[..size]);
        Ok(if indexed {
            let [count, instances, start, base, first_instance] = argument_words(&bytes);
            let range = Range::Indexed {
                count,
                start,
                base: base as i32,
            };
            (range, first_instance, instances)
        } else {
            let [count, instances, start, first_instance] = argument_words(&bytes);
            (Range::Vertices { count, start }, first_instance, instances)
        })
    }

    /// Direct3D reads indirect arguments only from buffers made for them, at whole words inside it.
    fn check_indirect(&self, args: &Buffer<B>, offset: u32, indexed: bool) -> Result<()> {
        if args.desc.misc_flags & desc::MISC_DRAWINDIRECT_ARGS == 0 {
            return Err(Error::InvalidArg(
                "indirect arguments in a buffer not made for them",
            ));
        }
        if !offset.is_multiple_of(ARGUMENT_WORD_BYTES as u32)
            || u64::from(offset) + u64::from(indirect_bytes(indexed))
                > u64::from(args.desc.byte_width)
        {
            return Err(Error::InvalidArg("indirect arguments past the buffer"));
        }
        Ok(())
    }

    /// Draws; a draw this front cannot serve removes the device, the only answer Direct3D has for it.
    fn draw_any(&mut self, range: Range, first_instance: u32, instances: u32) {
        if let Err(e) = self.try_draw(range, first_instance, instances, None) {
            self.lose(e);
        }
    }

    /// The program of a vertex and pixel shader pair as the variants `vk` and `pk`, made at its
    /// first use, and where its two stages read their helper constants.
    pub(crate) fn program(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<(B::Program, [Rc<Helpers>; 2])> {
        let key = (vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id), pk);
        if let Some(p) = self.programs.get(&key) {
            return Ok((p.program.handle.clone(), p.helpers.clone()));
        }
        self.vertex_variant(vs, vk, ps)?;
        let (fragment, ph) = self.pixel_stage(ps, pk)?;
        let (vertex, vh) = self
            .interpolated
            .get(&(key.0, vk, key.2))
            .map_or((&vs.gpu.handle, &vs.helpers), |v| {
                (&v.shader.handle, &v.helpers)
            });
        let helpers = [vh.clone(), ph];
        let keys = self.program_keys(vs, vk, ps, pk);
        let kept = match self.warm.programs.remove(&keys) {
            Some(warm) => warm,
            None => {
                let program = self.backend.create_program(&Program::Render {
                    vertex,
                    fragment: fragment.as_ref(),
                });
                let program = self.gpu(program)?;
                let id = self.id();
                Kept::new(program, id, Object::Program, &self.graveyard)
            }
        };
        let program = kept.handle.clone();
        self.programs.insert(
            key,
            Linked {
                program: kept,
                helpers: helpers.clone(),
            },
        );
        Ok((program, helpers))
    }

    /// The content keys of the modules the program of `vs` and `ps` as `vk` and `pk` is made of.
    pub(crate) fn program_keys(
        &self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> crate::cache::ModuleKeys {
        let vertex = self
            .interpolated
            .get(&(vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id)))
            .map_or(vs.key, |v| v.key);
        let fragment = self
            .variants
            .get(&(ps.map_or(BUILT_IN_PIXEL, |p| p.gpu.id), pk))
            .map(|v| v.key)
            .or(ps.map(|p| p.key));
        (vertex, fragment)
    }

    /// The pixel stage of a program and its helpers: `ps` as the variant `pk`, or without a pixel
    /// shader the shaders crate's program writing nothing with `pk`'s rewrites, where `pk` asks
    /// for any (depth clamped or biased, clip distances); none otherwise.
    pub(crate) fn pixel_stage(
        &mut self,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<(Option<B::Shader>, Rc<Helpers>)> {
        let id = ps.map_or(BUILT_IN_PIXEL, |p| p.gpu.id);
        let slots = self.backend.caps().limits.inter_stage_slots;
        let moves = match ps.map(|p| crate::shader::input_moves(&p.reflection.inputs, slots)) {
            None => Vec::new(),
            Some(Some(moves)) => moves,
            Some(None) => return self.varyings_beyond_limit(),
        };
        if (pk != StageVariant::default() || !moves.is_empty())
            && !self.variants.contains_key(&(id, pk))
        {
            let v = match ps {
                Some(p) => {
                    if !moves.is_empty() && pk.linked != 0 {
                        return self.varyings_beyond_limit();
                    }
                    let key = self.variant_key(p, pk, SourceStage::Pixel);
                    self.convert_kept(&p.bytecode, SourceStage::Pixel, &key, &moves)?
                }
                None => {
                    let key = pk.key(SourceStage::Pixel, &[], 0, &[]);
                    let c = crate::shader::pixel_program_for_variant(&key)?;
                    self.kept_variant(c)?
                }
            };
            self.variants.insert((id, pk), v);
        }
        Ok(match (self.variants.get(&(id, pk)), ps) {
            (Some(v), _) => (Some(v.shader.handle.clone()), v.helpers.clone()),
            (None, Some(p)) => (Some(p.gpu.handle.clone()), p.helpers.clone()),
            (None, None) => (None, Rc::default()),
        })
    }

    /// Converts `vs` again where the pixel shader interpolates its outputs other than by perspective
    /// or the draw asks for a variant, kept in `interpolated`.
    fn vertex_variant(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
    ) -> Result<()> {
        let key = (vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id));
        if self.interpolated.contains_key(&key) {
            return Ok(());
        }
        // WebGPU and GLSL ES require the stages to agree on interpolation, which in Direct3D the
        // pixel shader alone chooses: the vertex shader is converted again to write its outputs so.
        let interpolation = interpolation(ps.map(|p| &**p));
        // Values past the back end's inter-stage locations move to free ones, as the pixel
        // program's move (`Context::pixel_stage`).
        let slots = self.backend.caps().limits.inter_stage_slots;
        let moves = crate::shader::output_moves(
            &vs.reflection.outputs,
            ps.map_or(&[][..], |p| p.reflection.inputs.as_slice()),
            slots,
        );
        let Some(moves) = moves.filter(|m| m.is_empty() || vk.linked == 0) else {
            return self.varyings_beyond_limit();
        };
        if interpolation.is_empty() && vk == StageVariant::default() && moves.is_empty() {
            return Ok(());
        }
        let pulled = match &self.ctx.layout {
            Some(l) if vk.pulled == l.id => {
                l.pulled.iter().map(|e| (e.location, e.format)).collect()
            }
            _ => Vec::new(),
        };
        let variant = VariantKey {
            output_interpolation: interpolation,
            pulled,
            ..self.variant_key(vs, vk, SourceStage::Vertex)
        };
        let v = self.convert_kept(&vs.bytecode, SourceStage::Vertex, &variant, &moves)?;
        self.interpolated.insert(key, v);
        Ok(())
    }

    /// Refused: a vertex and pixel program pair passing more values than the back end's
    /// inter-stage locations hold after moving them (`VaryingsBeyondLimit`).
    fn varyings_beyond_limit<T>(&mut self) -> Result<T> {
        self.backend.slow_paths().hit(SlowPath::VaryingsBeyondLimit);
        not_implemented(VARYINGS_BEYOND_LIMIT)
    }

    /// `bytecode` converted as `key`, its stage values moved by `moves`
    /// ([`crate::shader::relocate`]), and handed to the back end.
    fn convert_kept(
        &mut self,
        bytecode: &[u8],
        stage: SourceStage,
        key: &VariantKey,
        moves: &[(u32, u32)],
    ) -> Result<Variant<B>> {
        let mut c = crate::shader::convert_variant(bytecode, stage, key)?;
        crate::shader::relocate(&mut c, moves)?;
        self.kept_variant(c)
    }

    /// A converted program of one stage handed to the back end.
    fn kept_variant(&mut self, mut c: wcontainers_shaders::Converted) -> Result<Variant<B>> {
        let helpers = Rc::new(Helpers::of(&c));
        let module = c.modules.pop().expect("one module");
        let shader = self.backend.create_shader(&module);
        let shader = self.gpu(shader)?;
        let id = self.id();
        Ok(Variant {
            shader: Kept::new(shader, id, Object::Shader, &self.graveyard),
            key: module.key,
            helpers,
        })
    }

    /// What a draw asks of `stage`'s program `s`: the sampler slots it reads through a biased
    /// sampler, and the targets' `samples` where it reads them.
    fn stage_variant(&mut self, stage: usize, s: &Shader<B>, samples: u8) -> StageVariant {
        let formats = self.format_id(stage, s);
        let caps = self.backend.caps();
        let mut v = StageVariant {
            samples: if s.reads_samples { samples } else { 0 },
            formats,
            ..self.base_variant()
        };
        if s.samplers != 0 {
            for (i, b) in self.ctx.samplers[stage].iter().enumerate().take(16) {
                let Some(b) = b.as_ref().filter(|_| s.samplers & (1 << i) != 0) else {
                    continue;
                };
                if b.desc.mip_lod_bias != 0.0 {
                    v.lod_bias |= 1 << i;
                }
                // The program filters through a border sampler, every address mode included, and
                // mirrors once around plain samples.
                let mirror =
                    b.desc.address.contains(&5) && !caps.features.contains(Features::MIRROR_ONCE);
                if b.desc.address.contains(&4) && !caps.features.contains(Features::BORDER_COLOR)
                    || mirror && s.mirror_filtered & (1 << i) != 0
                {
                    v.border_in_shader |= 1 << i;
                } else if mirror {
                    v.mirror_once_in_shader |= 1 << i;
                }
            }
        }
        if s.textures != 0 {
            for i in 0..self.ctx.resources[stage].len().min(128) {
                if s.textures & (1 << i) == 0 {
                    continue;
                }
                if self.min_lod_of(stage, i as u32).is_some() {
                    v.min_lod_clamped |= 1 << i;
                }
                if let Some(ShaderView::Texture(view)) = self.ctx.resources[stage][i].as_deref()
                    && view.reads_stencil()
                {
                    v.stencil |= 1 << i;
                }
            }
            // An empty slot asked its size answers zero, which its stand-in cannot: the program
            // reads sizes past the helpers' level count, zero for an empty slot, as zero.
            let resources = &self.ctx.resources[stage];
            if (0..u128::BITS as usize)
                .filter(|&i| s.textures & (1 << i) != 0)
                .any(|i| resources.get(i).is_none_or(Option::is_none))
            {
                v.texture_info = true;
            }
        }
        // Textures of formats the back end cannot filter, sampled through a filtering sampler (an
        // empty slot reads Direct3D's default, which filters linearly).
        for &(t, smp) in &s.reflection.sampled_pairs {
            let filters = match self.ctx.samplers[stage]
                .get(smp as usize)
                .and_then(Option::as_ref)
            {
                Some(b) => {
                    b.made.compare.is_none()
                        && [b.made.min, b.made.mag, b.made.mip].contains(&Filter::Linear)
                }
                None => true,
            };
            let unfiltered = match self.ctx.resources[stage]
                .get(t as usize)
                .and_then(Option::as_deref)
            {
                Some(ShaderView::Texture(view)) => {
                    view.decoded.is_none()
                        && !caps.formats.get(view.format).contains(FormatUse::FILTER)
                }
                _ => false,
            };
            if t < 128 && filters && unfiltered {
                v.filter_in_shader |= 1 << t;
            }
        }
        if matches!(stage, PIXEL | COMPUTE) {
            v.stored_through = self.stored_through_slots(stage, s);
        }
        (v.decoded, v.decoded_formats) = self.decoded_slots(stage, s);
        if v.stencil != 0 {
            self.backend.slow_paths().hit(SlowPath::SwizzleInShader);
        }
        if matches!(stage, VERTEX | PIXEL) {
            v.constants_as_storage = self.constants_past_declared(stage, s);
            v.constants_as_texture = self.constants_past_uniform_limit(stage, s, &v);
        }
        self.program_paths(s);
        v
    }

    /// What every variant of a program takes on this back end, whatever is bound.
    pub(crate) fn base_variant(&self) -> StageVariant {
        StageVariant {
            // Variants filtering in the program read level counts too.
            texture_info: !self
                .backend
                .caps()
                .features
                .contains(Features::TEXTURE_LEVEL_QUERY),
            ..StageVariant::default()
        }
    }

    /// The variant of `s` drawn at `stage` with nothing bound that changes it.
    pub(crate) fn unbound_variant(&self, stage: usize, s: &Shader<B>) -> StageVariant {
        let v = self.base_variant();
        StageVariant {
            constants_as_texture: self.constants_past_uniform_limit(stage, s, &v),
            ..v
        }
    }

    /// The constant buffer slots `s` reads as uniform buffers past the back end's uniform buffers
    /// per stage, where the variant `v` reads the others: its program reads them through textures.
    fn constants_past_uniform_limit(&self, stage: usize, s: &Shader<B>, v: &StageVariant) -> u16 {
        let plain = StageVariant {
            formats: 0,
            samples: 0,
            ..*v
        };
        crate::shader::constants_past_limit(
            &s.reflection,
            set_of(stage),
            v.constants_as_storage,
            s.helpers.size > 0 || plain != StageVariant::default(),
            self.backend.caps().limits.uniform_buffers_per_stage,
        )
    }

    /// The constant buffer slots `s` indexes at run time whose buffer bound to `stage` is larger
    /// than `s` declares, where the back end has storage buffers to read them through.
    fn constants_past_declared(&self, stage: usize, s: &Shader<B>) -> u16 {
        if s.indexed_constants == 0 || self.backend.caps().limits.storage_buffers_per_stage == 0 {
            return 0;
        }
        let mut slots = 0;
        for b in &s.reflection.bindings {
            let slot = b.register as usize;
            if b.kind != SlotKind::Uniform
                || b.set != set_of(stage)
                || s.indexed_constants & (1 << slot) == 0
            {
                continue;
            }
            let Some(Some(c)) = self.ctx.constant_buffers[stage].get(slot) else {
                continue;
            };
            let bound = match self.ctx.constant_ranges[stage].get(slot) {
                Some(Some(range)) => window_of(c, *range).1,
                _ => c.size.get(),
            };
            let declared = s.constant_sizes.iter().find(|(n, _)| *n == b.binding);
            if declared.is_some_and(|&(_, d)| bound > d) {
                slots |= 1 << slot;
            }
        }
        slots
    }

    /// Counts the slow paths `s`'s program takes by itself, whatever its variant.
    pub(super) fn program_paths(&mut self, s: &Shader<B>) {
        for &path in &s.reflection.slow_paths {
            self.backend.slow_paths().hit(path);
        }
    }

    /// The device's number for the formats of the typed views `s` declares as bound to `stage`
    /// (`StageVariant::formats`); zero where they are the plain program's placeholders. A slot
    /// holding no view of its kind keeps its placeholder.
    fn format_id(&mut self, stage: usize, s: &Shader<B>) -> u32 {
        if s.typed.is_empty() {
            return 0;
        }
        let formats: Vec<Format> = s
            .typed
            .iter()
            .map(|t| {
                let slots = if t.storage {
                    &self.ctx.uavs[stage]
                } else {
                    &self.ctx.resources[stage]
                };
                match slots.get(t.register as usize).and_then(Option::as_deref) {
                    Some(ShaderView::Texture(v)) if t.storage && !t.buffer => v.values(),
                    Some(ShaderView::Buffer(BufferSpan {
                        texel: Some((f, ..)),
                        ..
                    })) if t.buffer => *f,
                    _ => t.placeholder(),
                }
            })
            .collect();
        if s.typed
            .iter()
            .zip(&formats)
            .all(|(t, f)| t.placeholder() == *f)
        {
            return 0;
        }
        self.format_list_id(formats)
    }

    /// The device's number for the list `formats` (from one).
    fn format_list_id(&mut self, formats: Vec<Format>) -> u32 {
        if let Some(&id) = self.format_ids.get(&formats) {
            return id;
        }
        self.format_lists.push(formats.clone());
        let id = self.format_lists.len() as u32;
        self.format_ids.insert(formats, id);
        id
    }

    /// The shaders crate's key for `v` of `s`, a shader of `stage`.
    fn variant_key(&self, s: &Shader<B>, v: StageVariant, stage: SourceStage) -> VariantKey {
        let formats = match v.formats {
            0 => &[][..],
            n => &self.format_lists[n as usize - 1],
        };
        let mut key = v.key(stage, &s.typed, s.layered, formats);
        if v.retyped != 0 {
            let formats = &self.format_lists[v.retyped_formats as usize - 1];
            key.targets_in_shader = (0..u8::BITS as u8)
                .filter(|t| v.retyped & (1 << t) != 0)
                .zip(formats)
                .map(|(target, &format)| TargetInShader {
                    target,
                    format,
                    combine: false,
                })
                .collect();
        }
        if v.decoded != 0 {
            let formats = &self.format_lists[v.decoded_formats as usize - 1];
            key.decoded = (0..u128::BITS)
                .filter(|t| v.decoded & (1 << t) != 0)
                .zip(formats.iter().copied())
                .collect();
        }
        key
    }

    /// The texture slots of `s` at `stage` bound to views of textures held as their integer
    /// twins, which the program decodes (`NormAsInteger`), and the device's number for their
    /// normalised formats, in slot order; zeros where there are none.
    fn decoded_slots(&mut self, stage: usize, s: &Shader<B>) -> (u128, u32) {
        let mut slots = 0u128;
        let mut formats = Vec::new();
        for (i, r) in self.ctx.resources[stage]
            .iter()
            .enumerate()
            .take(u128::BITS as usize)
        {
            // A view compared through is read through its depth copy, not decoded.
            if s.textures & !s.depth_textures & (1 << i) == 0 {
                continue;
            }
            if let Some(ShaderView::Texture(view)) = r.as_deref()
                && let Some(f) = view.decoded
            {
                slots |= 1 << i;
                formats.push(f);
            }
        }
        if slots == 0 {
            return (0, 0);
        }
        self.backend.slow_paths().hit(SlowPath::NormAsInteger);
        (slots, self.format_list_id(formats))
    }

    /// The targets `ps` writes an output of another kind (float, unsigned, signed) than their
    /// format to, whose bits Direct3D writes (`StageVariant::retyped`), and the device's number
    /// for their formats; zeros where every output fits its target.
    fn retyped_targets(&mut self, ps: &Shader<B>) -> (u8, u32) {
        let mut retyped = 0u8;
        let mut formats = Vec::new();
        for (n, t) in self.ctx.targets.iter().enumerate() {
            let Some(t) = t.as_ref() else {
                continue;
            };
            let output = ps
                .reflection
                .outputs
                .iter()
                .find(|o| o.system == Some(SystemValue::Target(n as u8)));
            // A target held as its integer twin takes its output encoded (`NormAsInteger`).
            let encoded = t.decoded.is_some() && output.is_some();
            if encoded || output.is_some_and(|o| !crate::ahead::fits_target(o.scalar, t.format)) {
                retyped |= 1 << n;
                formats.push(t.values());
            }
        }
        if retyped == 0 {
            return (0, 0);
        }
        (retyped, self.format_list_id(formats))
    }

    /// Lines and points are drawn without the bound depth bias, as Direct3D draws them (measured
    /// on the reference: `own_cases biased_lines`), through a twin of the raster block without it.
    fn bias_dropped(&self) -> bool {
        let r = &self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc;
        (r.depth_bias != 0 || r.slope_scaled_depth_bias != 0.0)
            && self.ctx.depth.is_some()
            && !self.draws_triangles()
    }

    /// The bound raster state draws lines alpha-antialiased: antialiased lines without
    /// multisampling.
    pub(super) fn lines_alpha_antialiased(&self) -> bool {
        let r = &self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc;
        r.antialiased_lines && !r.multisample
    }

    /// The primitives rasterised are triangles: a geometry stage's output, else the tessellator's,
    /// else the bound topology's.
    fn draws_triangles(&self) -> bool {
        let triangles = wcontainers_shaders::stage::TRIANGLE_VERTICES;
        if let Some((gs, _)) = self.ctx.gs.as_ref().and_then(|g| g.stage.as_ref())
            && let wcontainers_gpu::StageExtras::Geometry {
                output_vertices, ..
            } = gs.reflection.extras
        {
            return u32::from(output_vertices) == triangles;
        }
        if let Some(hs) = &self.ctx.hs
            && let wcontainers_gpu::StageExtras::Hull {
                output_primitive, ..
            } = hs.reflection.extras
        {
            return wcontainers_shaders::stage::tess::output_vertices(output_primitive)
                == triangles;
        }
        matches!(
            plain_topology(self.ctx.topology),
            Some(Topology::TriangleList | Topology::TriangleStrip)
        ) || matches!(
            self.ctx.topology,
            topology::TRIANGLELIST_ADJ | topology::TRIANGLESTRIP_ADJ
        )
    }

    /// The smallest step of the bound depth target's format where the pixel program adds the depth
    /// bias to triangles (`DepthBiasInShader`): where the back end lacks the bias clamp, or where
    /// it moves 24-bit depth it stores as float by float steps. Refused for float depth, whose step
    /// varies with the depth.
    fn bias_step(&self) -> Result<Option<f32>> {
        let r = &self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc;
        let Some(depth) = &self.ctx.depth else {
            return Ok(None);
        };
        if r.depth_bias == 0 && r.slope_scaled_depth_bias == 0.0 {
            return Ok(None);
        }
        let caps = self.backend.caps();
        let d24 = matches!(
            depth.format,
            Format::Depth24Plus | Format::Depth24PlusStencil8
        );
        let unclamped =
            r.depth_bias_clamp != 0.0 && !caps.features.contains(Features::DEPTH_BIAS_CLAMP);
        let float_steps =
            r.depth_bias != 0 && d24 && !caps.features.contains(Features::DEPTH24_UNORM);
        if (!unclamped && !float_steps) || !self.draws_triangles() {
            return Ok(None);
        }
        let bits = match depth.format {
            Format::Depth16Unorm => DEPTH16_BITS,
            _ if d24 => DEPTH24_BITS,
            _ => return not_implemented(FLOAT_DEPTH_BIAS_CLAMP),
        };
        Ok(Some(1.0 / ((1u32 << bits) - 1) as f32))
    }

    /// Direct3D's first viewport as the vertex program maps it within a whole-pixel one, where
    /// the back end would not draw it exactly (`ViewportInShader`): a fractional one on a back end
    /// without `FRACTIONAL_VIEWPORT`, and below feature level 11_0 a negative fractional origin,
    /// which Direct3D then keeps exact and the back end, as Direct3D from 11_0, rounds down.
    fn viewport_in_shader(&self) -> Option<ShaderViewport> {
        let v = *self.ctx.viewports.first()?;
        let fractional = |f: f32| f.fract() != 0.0;
        let rounded = |o: f32| o < 0.0 && fractional(o);
        let inexact = !self
            .backend
            .caps()
            .features
            .contains(Features::FRACTIONAL_VIEWPORT)
            && [v.x, v.y, v.width, v.height].into_iter().any(fractional);
        if !inexact && (self.feature_level() >= level::L11_0 || !(rounded(v.x) || rounded(v.y))) {
            return None;
        }
        // Per axis: the enclosing whole-pixel origin and size, the scale and offset of clip
        // coordinates onto the API's viewport within it, and the pixels whose centres it covers.
        let axis = |o: f32, size: f32| {
            let start = o.floor();
            let whole = (o + size).ceil() - start;
            let offset = (2.0 * (o - start) + size) / whole - 1.0;
            let first = (o - PIXEL_CENTRE).ceil().max(0.0);
            let end = (o + size - PIXEL_CENTRE).ceil().max(first);
            (
                start,
                whole,
                size / whole,
                offset,
                first as u32,
                (end - first) as u32,
            )
        };
        let (x, width, scale_x, offset_x, left, columns) = axis(v.x, v.width);
        let (y, height, scale_y, offset_y, top, rows) = axis(v.y, v.height);
        Some(ShaderViewport {
            viewport: Viewport {
                x,
                y,
                width,
                height,
                ..v
            },
            // Clip y grows upwards, window y downwards.
            adjust: [scale_x, scale_y, offset_x, -offset_y],
            pixels: ScissorRect {
                x: left,
                y: top,
                width: columns,
                height: rows,
            },
        })
    }

    /// The sampler bound to `stage`'s `slot`.
    fn bound_sampler(&self, stage: usize, slot: u32) -> Option<&Sampler<B>> {
        self.ctx.samplers[stage]
            .get(slot as usize)
            .and_then(Option::as_deref)
    }

    /// The minimum level-of-detail clamp of the texture view bound to `stage`'s `slot`, in the view's
    /// levels, and the view's level count; none where the clamp leaves every level of the view.
    fn min_lod_of(&self, stage: usize, slot: u32) -> Option<(f32, u32)> {
        let v = self.ctx.resources[stage]
            .get(slot as usize)?
            .as_deref()?
            .texture()?;
        let clamp = v.texture.min_lod.get() - v.desc.first_mip as f32;
        (clamp > 0.0).then_some((clamp, v.desc.mip_count))
    }

    /// The sampler bound to `stage`'s `slot` (Direct3D's default where none is) with its minimum
    /// level raised to `clamp`, made at its first use.
    fn clamped_sampler(
        &mut self,
        stage: usize,
        slot: u32,
        clamp: f32,
    ) -> Result<(B::Sampler, u64)> {
        let s = match self.ctx.samplers[stage]
            .get(slot as usize)
            .cloned()
            .flatten()
        {
            Some(s) => s,
            None => {
                self.default_sampler()?;
                self.zeros.sampler.clone().expect("made")
            }
        };
        let [min, max] = s.made.lod_range.map(f32::from_bits);
        // Direct3D clamps the level to the sampler's range and then to the resource's clamp.
        let lod_range = [min.max(clamp), max.max(clamp)].map(f32::to_bits);
        if let Some((_, k)) = s.clamped.borrow().iter().find(|(m, _)| *m == lod_range[0]) {
            return Ok((k.handle.clone(), k.id));
        }
        let gpu = self.backend.create_sampler(&GpuSampler {
            lod_range,
            ..s.made
        });
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        s.clamped.borrow_mut().push((
            lod_range[0],
            Kept::new(gpu.clone(), id, Object::Sampler, &self.graveyard),
        ));
        Ok((gpu, id))
    }

    /// The strides of the bound vertex buffers in the slots the input layout reads, zero elsewhere.
    pub(crate) fn strides(&self) -> [u32; VERTEX_SLOTS] {
        let mut strides = [0u32; VERTEX_SLOTS];
        if let Some(l) = &self.ctx.layout {
            for e in &l.elements {
                strides[e.slot as usize] = self.ctx.vertex_buffers[e.slot as usize]
                    .as_ref()
                    .map_or(0, |v| v.1);
            }
        }
        strides
    }

    fn vertex_layout(&mut self, vs: &Shader<B>) -> Result<B::VertexLayout> {
        let strides = self.strides();
        self.vertex_layout_for(self.ctx.layout.clone(), strides, vs)
    }

    /// The vertex layout block of an input layout read at `strides`, made at its first use.
    pub(crate) fn vertex_layout_for(
        &mut self,
        layout: Option<Rc<InputLayout>>,
        strides: [u32; 32],
        vs: &Shader<B>,
    ) -> Result<B::VertexLayout> {
        let key = (layout.as_ref().map_or(0, |l| l.id), strides, vs.gpu.id);
        if let Some(v) = self.layouts.get(&key) {
            return Ok(v.handle.clone());
        }
        let desc = layout_desc(layout.as_deref(), strides, &vs.reflection.inputs);
        let gpu = self.backend.create_vertex_layout(&desc);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        self.layouts.insert(
            key,
            Kept::new(gpu.clone(), id, Object::VertexLayout, &self.graveyard),
        );
        Ok(gpu)
    }

    /// Grows each constant buffer a stage whose binding set changed reads that is smaller than its
    /// shader declares (`ConstantBufferGrown`). A grown buffer is a new back-end buffer, so every set
    /// is made again.
    fn grow_constants(&mut self) -> Result<()> {
        let mut small = Vec::new();
        for stage in 0..STAGES {
            let shader = self.stage_shader(stage);
            let Some(shader) = shader.as_ref().filter(|_| self.ctx.sets_dirty[stage]) else {
                continue;
            };
            for b in &shader.reflection.bindings {
                let bound = self.ctx.constant_buffers[stage].get(b.register as usize);
                let whole = self.ctx.constant_ranges[stage]
                    .get(b.register as usize)
                    .is_some_and(Option::is_none);
                if let (SlotKind::Uniform, Some(Some(c))) = (b.kind, bound)
                    && whole
                    && b.set == set_of(stage)
                    && let Some(&(_, declared)) =
                        shader.constant_sizes.iter().find(|(n, _)| *n == b.binding)
                    && declared > c.size.get()
                {
                    small.push((c.clone(), declared));
                }
            }
        }
        for (c, declared) in &small {
            if *declared > c.size.get() {
                self.grow(c, *declared)?;
            }
        }
        if !small.is_empty() {
            self.ctx.sets_dirty = [true; STAGES];
        }
        Ok(())
    }

    /// Moves a buffer into a back-end buffer of `size` bytes whose bytes past its own are zero.
    fn grow(&mut self, b: &Buffer<B>, size: u64) -> Result<()> {
        self.backend.slow_paths().hit(SlowPath::ConstantBufferGrown);
        self.settle();
        let old = b.size.get();
        let new = self.backend.create_buffer(&gpu_buffer(&b.desc, size));
        let new = self.gpu(new)?;
        self.backend.clear_buffer(&new, old, size - old, &[0]);
        self.backend.copy_buffer(&b.handle(), 0, &new, 0, old);
        let id = self.id();
        *b.gpu.borrow_mut() = Kept::new(new, id, Object::Buffer, &self.graveyard);
        b.size.set(size);
        b.grown.set(true);
        let mut shadow = b.shadow.borrow_mut();
        if !shadow.is_empty() {
            let mut grown = vec![0; size as usize];
            grown[..shadow.len()].copy_from_slice(shadow_bytes(&mut shadow));
            *shadow = shadow_of(&mut grown);
        }
        Ok(())
    }

    /// The copy of `c`'s window of `length` bytes at `offset` read by a block of `size` bytes, read
    /// as `read_as`: its bytes past the window are zero; copied again where `c` was written since.
    /// Gives its index in `c`'s windows.
    fn window(
        &mut self,
        c: &Buffer<B>,
        offset: u64,
        length: u64,
        size: u64,
        read_as: ConstantsReadAs,
    ) -> Result<usize> {
        let source = (c.writes.get(), c.gpu.borrow().id);
        let found = c.windows.borrow().iter().position(|w| {
            (w.offset, w.length, w.size) == (offset, length, size)
                && matches!(
                    (&w.copy, read_as),
                    (WindowCopy::Texture { .. }, ConstantsReadAs::Texture)
                        | (WindowCopy::Buffer(_), ConstantsReadAs::Storage)
                )
        });
        let i = match found {
            Some(i) => i,
            None => {
                self.settle();
                let copy = match read_as {
                    ConstantsReadAs::Storage => {
                        let twin = self.backend.create_buffer(&gpu_buffer(&c.desc, size));
                        let twin = self.gpu(twin)?;
                        self.backend.clear_buffer(&twin, 0, size, &[0]);
                        let id = self.id();
                        WindowCopy::Buffer(Kept::new(twin, id, Object::Buffer, &self.graveyard))
                    }
                    ConstantsReadAs::Texture => self.constants_texture(size)?,
                };
                let mut windows = c.windows.borrow_mut();
                windows.push(Window {
                    offset,
                    length,
                    size,
                    copied: Cell::new(None),
                    copy,
                });
                windows.len() - 1
            }
        };
        let windows = c.windows.borrow();
        let w = &windows[i];
        if w.copied.get() != Some(source) {
            self.settle();
            match &w.copy {
                WindowCopy::Buffer(twin) => {
                    self.backend.slow_paths().hit(SlowPath::ConstantBufferGrown);
                    if length > 0 {
                        self.backend
                            .copy_buffer(&c.handle(), offset, &twin.handle, 0, length);
                    }
                }
                WindowCopy::Texture { texture, .. } => {
                    let rows = (length / CONSTANT_ROW_BYTES) as u32;
                    let texels = TexelRows {
                        count: rows,
                        bytes: CONSTANT_ROW_BYTES as u32,
                        width: CONSTANTS_TEXTURE_WIDTH,
                    };
                    self.copy_rows(&c.handle(), offset, texels, &texture.handle);
                }
            }
            w.copied.set(Some(source));
        }
        Ok(i)
    }

    /// A zeroed RGBA32 unsigned texture holding `size` bytes of constants as 16-byte rows,
    /// `CONSTANTS_TEXTURE_WIDTH` rows to a texture row.
    fn constants_texture(&mut self, size: u64) -> Result<WindowCopy<B>> {
        let rows = size.div_ceil(CONSTANT_ROW_BYTES).max(1) as u32;
        let extent = Extent {
            width: CONSTANTS_TEXTURE_WIDTH,
            height: rows.div_ceil(CONSTANTS_TEXTURE_WIDTH),
            depth_or_layers: 1,
        };
        let desc = wcontainers_gpu::TextureDesc {
            dimension: wcontainers_gpu::TextureDimension::D2,
            format: Format::Rgba32Uint,
            width: extent.width,
            height: extent.height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        };
        let texture = self.backend.create_texture(&desc);
        let texture = self.gpu(texture)?;
        let row_pitch = CONSTANTS_TEXTURE_WIDTH * CONSTANT_ROW_BYTES as u32;
        let zeros = vec![0; (row_pitch * extent.height) as usize];
        self.backend.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: 0,
                layer: 0,
                origin: Origin { x: 0, y: 0, z: 0 },
                stencil: false,
            },
            extent,
            &zeros,
            row_pitch,
            extent.height,
            WriteMode::Discard,
        );
        let view = self.backend.create_view(
            &texture,
            &TextureViewDesc {
                format: Format::Rgba32Uint,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        );
        let view = self.gpu(view)?;
        let (t, v) = (self.id(), self.id());
        Ok(WindowCopy::Texture {
            texture: Kept::new(texture, t, Object::Texture, &self.graveyard),
            view: Kept::new(view, v, Object::TextureView, &self.graveyard),
        })
    }

    /// The zero buffer's bytes as a constants texture, made at its first use.
    fn zero_constants(&mut self) -> Result<B::TextureView> {
        if self.zeros.constants.is_none() {
            self.zeros.constants = Some(self.constants_texture(ZERO_BYTES)?);
        }
        match &self.zeros.constants {
            Some(WindowCopy::Texture { view, .. }) => Ok(view.handle.clone()),
            _ => unreachable!("made as a texture"),
        }
    }

    /// Copies `texels` from `offset` in `buffer` into a texture of their rows, from its first.
    fn copy_rows(
        &mut self,
        buffer: &B::Buffer,
        offset: u64,
        texels: TexelRows,
        texture: &B::Texture,
    ) {
        let row_pitch = texels.width * texels.bytes;
        let whole = texels.count / texels.width;
        let rest = texels.count % texels.width;
        for (first, width, height) in [(0, texels.width, whole), (whole, rest, 1)] {
            if width == 0 || height == 0 {
                continue;
            }
            self.backend.copy_buffer_to_texture(
                &wcontainers_gpu::CopyBufferTexture {
                    buffer,
                    offset: offset + u64::from(first * row_pitch),
                    row_pitch,
                    rows_per_layer: height,
                },
                &TextureRegion {
                    texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin {
                        x: 0,
                        y: first,
                        z: 0,
                    },
                    stencil: false,
                },
                Extent {
                    width,
                    height,
                    depth_or_layers: 1,
                },
            );
        }
    }

    /// The texture copy of `b`'s elements as `format` (`TexelBufferAsTexture`), made at its first
    /// use and copied again where `b` was written since.
    fn texel_copy(&mut self, b: &Buffer<B>, format: Format) -> Result<B::TextureView> {
        let source = (b.writes.get(), b.gpu.borrow().id);
        let (.., bytes) = FORMAT_FACTS[format as usize].block;
        let texels = TexelRows {
            count: b.desc.byte_width / bytes,
            bytes,
            width: TEXEL_BUFFER_TEXTURE_WIDTH,
        };
        let found = b.texels.borrow().iter().position(|t| t.format == format);
        let i = match found {
            Some(i) => i,
            None => {
                self.settle();
                let height = texels.count.div_ceil(texels.width).max(1);
                let texture = self.backend.create_texture(&wcontainers_gpu::TextureDesc {
                    dimension: wcontainers_gpu::TextureDimension::D2,
                    format,
                    width: texels.width,
                    height,
                    depth_or_layers: 1,
                    mip_levels: 1,
                    samples: 1,
                    usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
                    cube: false,
                    reinterpret: false,
                });
                let texture = self.gpu(texture)?;
                let view = self.backend.create_view(
                    &texture,
                    &TextureViewDesc {
                        format,
                        dimension: ViewDimension::D2,
                        base_mip: 0,
                        mip_count: 1,
                        base_layer: 0,
                        layer_count: 1,
                        stencil: false,
                    },
                );
                let view = self.gpu(view)?;
                let (t, v) = (self.id(), self.id());
                let mut copies = b.texels.borrow_mut();
                copies.push(TexelCopy {
                    format,
                    texture: Kept::new(texture, t, Object::Texture, &self.graveyard),
                    view: Kept::new(view, v, Object::TextureView, &self.graveyard),
                    copied: Cell::new(None),
                });
                copies.len() - 1
            }
        };
        let copies = b.texels.borrow();
        let copy = &copies[i];
        if copy.copied.get() != Some(source) {
            self.settle();
            self.backend
                .slow_paths()
                .hit(SlowPath::TexelBufferAsTexture);
            self.copy_rows(&b.handle(), 0, texels, &copy.texture.handle);
            copy.copied.set(Some(source));
        }
        Ok(copy.view.handle.clone())
    }

    /// The texture copies of the typed buffers `stage`'s shader reads through them, by slot.
    fn texel_copies(&mut self, stage: usize) -> Result<Vec<(u32, B::TextureView)>> {
        let mut copies = Vec::new();
        if !self.typed_buffers_textured() {
            return Ok(copies);
        }
        let Some(shader) = self.stage_shader(stage).clone() else {
            return Ok(copies);
        };
        for t in shader.typed.iter().filter(|t| t.textured) {
            let bound = self.ctx.resources[stage]
                .get(t.register as usize)
                .and_then(Option::clone);
            if let Some(view) = bound
                && let ShaderView::Buffer(BufferSpan {
                    buffer,
                    texel: Some((format, ..)),
                    ..
                }) = &*view
            {
                copies.push((t.register, self.texel_copy(buffer, *format)?));
            }
        }
        Ok(copies)
    }

    /// Copies again the constant buffer windows and the typed buffers read through copies whose
    /// buffers were written since.
    fn refresh_windows(&mut self) -> Result<()> {
        for stage in 0..STAGES {
            self.texel_copies(stage)?;
            if self.ctx.windows[stage].is_empty() {
                continue;
            }
            let windows = self.ctx.windows[stage].clone();
            for (_, c, offset, length, size, read_as) in &windows {
                self.window(c, *offset, *length, *size, *read_as)?;
            }
        }
        Ok(())
    }

    /// The shader bound to `stage`.
    fn stage_shader(&self, stage: usize) -> &Option<Rc<Shader<B>>> {
        match stage {
            VERTEX => &self.ctx.vs,
            PIXEL => &self.ctx.ps,
            GEOMETRY => &self.ctx.gs_stage,
            HULL => &self.ctx.hs,
            DOMAIN => &self.ctx.ds,
            _ => &self.ctx.cs,
        }
    }

    /// Counts the slow paths of sampling the programs of `variants` do in the shader.
    fn count_sampling_in_shader(&mut self, variants: &[&StageVariant]) {
        let any = |f: fn(&StageVariant) -> bool| variants.iter().any(|v| f(v));
        let paths = [
            (any(|v| v.lod_bias != 0), SlowPath::LodBiasInShader),
            (any(|v| v.min_lod_clamped != 0), SlowPath::MinLodClamp),
            (any(|v| v.border_in_shader != 0), SlowPath::BorderInShader),
            (
                any(|v| v.mirror_once_in_shader != 0),
                SlowPath::MirrorOnceInShader,
            ),
            (any(|v| v.filter_in_shader != 0), SlowPath::FilterInShader),
        ];
        for (_, path) in paths.iter().filter(|(taken, _)| *taken) {
            self.backend.slow_paths().hit(*path);
        }
    }

    /// `Dispatch` and `DispatchIndirect`; one this front cannot serve removes the device.
    pub fn dispatch(
        &mut self,
        groups: [u32; DISPATCH_AXES],
        indirect: Option<(&Rc<Buffer<B>>, u32)>,
    ) {
        if let Err(e) = self.try_dispatch(groups, indirect) {
            self.lose(e);
        }
    }

    fn try_dispatch(
        &mut self,
        groups: [u32; DISPATCH_AXES],
        indirect: Option<(&Rc<Buffer<B>>, u32)>,
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        if self.removed.is_some() {
            return Ok(());
        }
        self.bury();
        // Without a compute shader Direct3D runs nothing.
        let Some(cs) = self.ctx.cs.clone() else {
            return Ok(());
        };
        self.settle();
        self.grow_constants()?;
        self.refresh_windows()?;
        self.update_set(COMPUTE, &[], &[])?;
        let cv = self.stage_variant(COMPUTE, &cs, 1);
        let served = StageVariant {
            formats: cv.formats,
            stored_through: cv.stored_through,
            lod_bias: cv.lod_bias,
            min_lod_clamped: cv.min_lod_clamped,
            border_in_shader: cv.border_in_shader,
            mirror_once_in_shader: cv.mirror_once_in_shader,
            filter_in_shader: cv.filter_in_shader,
            decoded: cv.decoded,
            decoded_formats: cv.decoded_formats,
            stencil: cv.stencil,
            texture_info: cv.texture_info,
            ..StageVariant::default()
        };
        if cv != served {
            return not_implemented(COMPUTE_VARIANT_OF_DRAWS);
        }
        self.count_sampling_in_shader(&[&cv]);
        let varied = cv != StageVariant::default();
        let key = (cs.gpu.id, cv, 0, StageVariant::default());
        let (program, helpers) = match self.programs.get(&key) {
            Some(p) => (p.program.handle.clone(), p.helpers[0].clone()),
            None => {
                if varied && !self.variants.contains_key(&(cs.gpu.id, cv)) {
                    let key = self.variant_key(&cs, cv, SourceStage::Compute);
                    let v = self.convert_kept(&cs.bytecode, SourceStage::Compute, &key, &[])?;
                    self.variants.insert((cs.gpu.id, cv), v);
                }
                let (shader, helpers) = match self.variants.get(&(cs.gpu.id, cv)) {
                    Some(v) if varied => (&v.shader.handle, v.helpers.clone()),
                    _ => (&cs.gpu.handle, cs.helpers.clone()),
                };
                let p = self.backend.create_program(&Program::Compute(shader));
                let p = self.gpu(p)?;
                let id = self.id();
                self.programs.insert(
                    key,
                    Linked {
                        program: Kept::new(p.clone(), id, Object::Program, &self.graveyard),
                        helpers: [helpers.clone(), Rc::default()],
                    },
                );
                (p, helpers)
            }
        };
        if cs.typed.iter().any(|t| t.buffer) {
            self.backend
                .slow_paths()
                .hit(SlowPath::TexelBufferAsStorage);
        }
        let helper_set = self.compute_helpers(&helpers)?;
        if !self.queries.statistics.is_empty() {
            let groups = match indirect {
                None => groups,
                Some((b, offset)) => self.read_dispatch(b, offset)?,
            };
            let size = match cs.reflection.extras {
                wcontainers_gpu::StageExtras::Compute { workgroup_size } => workgroup_size,
                _ => [1; 3],
            };
            let threads = groups
                .iter()
                .chain(&size)
                .fold(1u64, |n, &g| n * u64::from(g));
            self.add_statistics(&[(crate::query::statistic::CS_INVOCATIONS, threads)]);
        }
        let twin_writes = self.twins_in(&[COMPUTE], COMPUTE);
        let through = self.through_in(COMPUTE);
        let args = indirect.map(|(b, offset)| (b.gpu.borrow(), offset));
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        bindings[0] = self.ctx.sets[COMPUTE].as_ref().map(|k| &k.handle);
        bindings[HELPER_SET as usize] = helper_set.as_ref();
        let call = DispatchCall {
            program: &program,
            bindings,
            immediates: &[],
            groups: match &args {
                None => DispatchGroups::Direct(groups),
                Some((b, offset)) => DispatchGroups::Indirect {
                    args: &b.handle,
                    offset: u64::from(*offset),
                },
            },
        };
        let r = self.backend.dispatch(&call);
        drop(args);
        if through {
            self.through_out(COMPUTE);
        }
        if twin_writes {
            self.twins_out(COMPUTE);
        }
        self.gpu(r)
    }

    /// The binding set of a compute program's helper constants (the typed buffer views' places,
    /// minimum level-of-detail clamps) and of its merged samplers, written for this dispatch; none
    /// for a program without them.
    fn compute_helpers(&mut self, h: &Helpers) -> Result<Option<B::BindingSet>> {
        if h.size == 0 && h.merged.is_empty() {
            return Ok(None);
        }
        let block = self.helper_block(h, COMPUTE, [0; 2], None)?;
        let mut merged = Vec::with_capacity(h.merged.len());
        for &(binding, (texture, sampler)) in &h.merged {
            let (clamp, _) = self.min_lod_of(COMPUTE, texture).unwrap_or((0.0, 1));
            merged.push((binding, self.clamped_sampler(COMPUTE, sampler, clamp)?));
        }
        let samplers: Vec<u64> = merged.iter().map(|(_, (_, id))| *id).collect();
        if let Some((written, made, set)) = &self.compute_helpers
            && *written == block
            && *made == samplers
        {
            return Ok(Some(set.handle.clone()));
        }
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offsets = [0u64; 1];
        let ring =
            self.backend
                .write_buffer_ring(RingKind::Data, &[&block[..]], align, &mut offsets);
        let ring = self.gpu(ring)?;
        self.helper_blocks = None;
        let mut entries = Vec::with_capacity(merged.len() + 1);
        if !block.is_empty() {
            entries.push(BindingEntry {
                binding: helper_binding(ShaderStage::Compute),
                stages: StageSet::COMPUTE,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ring,
                    offset: offsets[0],
                    size: block.len() as u64,
                }),
            });
        }
        for (binding, (s, _)) in &merged {
            entries.push(BindingEntry {
                binding: *binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::Sampler(s),
            });
        }
        let set = self.backend.create_binding_set(&entries);
        let set = self.gpu(set)?;
        let id = self.id();
        self.compute_helpers = Some((
            block,
            samplers,
            Kept::new(set.clone(), id, Object::BindingSet, &self.graveyard),
        ));
        Ok(Some(set))
    }

    /// `CopyStructureCount`: the hidden counter of a buffer view, written into `dst` at `offset`.
    pub fn copy_structure_count(
        &mut self,
        dst: &Buffer<B>,
        offset: u32,
        view: &ShaderView<B>,
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        let ShaderView::Buffer(BufferSpan {
            counter: Some(c), ..
        }) = view
        else {
            return Err(Error::InvalidArg("a view without a counter"));
        };
        let count_bytes = COUNTER_BYTES as u64;
        if !u64::from(offset).is_multiple_of(count_bytes)
            || u64::from(offset) + count_bytes > u64::from(dst.desc.byte_width)
        {
            return Err(Error::InvalidArg("a count written past the buffer"));
        }
        self.settle();
        dst.writes.set(dst.writes.get() + 1);
        self.backend
            .copy_buffer(&c.handle, 0, &dst.handle(), u64::from(offset), count_bytes);
        self.buffer_written(dst)
    }

    /// `ClearView` of a storage view, whole or its rectangles (left, top, right, bottom) in each
    /// of its layers or slices: integer formats take each value as an integer clamped to the
    /// channel (`storage_integer`), the others the values converted as
    /// `ClearUnorderedAccessViewFloat` converts them.
    pub fn clear_view_uav(
        &mut self,
        view: &ShaderView<B>,
        color: Color,
        rects: &[Rect],
    ) -> Result<()> {
        let values = match view {
            ShaderView::Texture(v) => Some(v.values()),
            ShaderView::Buffer(b) => b.texel.map(|(f, ..)| f),
        };
        let (values, float) = match values.filter(|&f| integer_format(f)) {
            Some(f) => (integer_clear(f, color, storage_integer), false),
            None => (color.map(f32::to_bits), true),
        };
        if rects.is_empty() {
            return self.clear_uav(view, values, float);
        }
        let ShaderView::Texture(v) = view else {
            return not_implemented(CLEAR_VIEW_BUFFER_RECTS);
        };
        if self.predicated_out() {
            return Ok(());
        }
        self.settle();
        let rects: Vec<ScissorRect> = rects.iter().map(|&r| pixel_rect(r)).collect();
        if self.stored_as_its_format(v) {
            return self.store_clear(v, values, float, &rects);
        }
        if !v.texture.gpu_desc.usage.contains(TextureUsage::RENDER)
            || v.desc.dimension == desc::ViewDim::Texture3D
        {
            return not_implemented(CLEAR_VIEW_STORAGE_RECTS_UNSERVED);
        }
        self.clear_rects_drawn(
            v,
            color_value(v, color),
            &rects,
            desc::bind::RENDER_TARGET,
            false,
        )
    }

    /// Whether storage view `v` is a storage view of its own format on the back end: not stored
    /// through a buffer, not held as an integer twin.
    fn stored_as_its_format(&self, v: &View<B>) -> bool {
        v.decoded.is_none()
            && !self.stored_through(v.format)
            && v.texture.gpu_desc.usage.contains(TextureUsage::STORAGE)
    }

    /// Clears `rects` of storage view `v` (each in every layer or slice it sees; all of it where
    /// there are none) by the storage clear program of its format, `values` taken as
    /// `builtin::storage_clear_value` takes them.
    fn store_clear(
        &mut self,
        v: &View<B>,
        values: [u32; COLOR_COMPONENTS],
        float: bool,
        rects: &[ScissorRect],
    ) -> Result<()> {
        use wcontainers_shaders::builtin;
        if !self.stored_as_its_format(v) {
            return not_implemented(CLEAR_STORAGE_UNSERVED);
        }
        let dimension = match v.desc.dimension {
            desc::ViewDim::Texture1D | desc::ViewDim::Texture2D => ViewDimension::D2,
            desc::ViewDim::Texture1DArray | desc::ViewDim::Texture2DArray => ViewDimension::D2Array,
            desc::ViewDim::Texture3D => ViewDimension::D3,
            _ => return Err(Error::InvalidArg("a storage view of another dimension")),
        };
        let value = builtin::storage_clear_value(v.format, values, float).or_else(refused)?;
        let program = self.storage_clear_program(v.format, dimension)?;
        let extent = v.texture.mip_extent(v.desc.first_mip);
        let slices = match dimension {
            ViewDimension::D3 if !v.window => {
                v.desc.first_layer..v.desc.first_layer + v.desc.layer_count
            }
            ViewDimension::D2 => 0..1,
            _ => 0..v.desc.layer_count,
        };
        let whole = [ScissorRect {
            x: 0,
            y: 0,
            width: extent.width,
            height: extent.height,
        }];
        let boxes: Vec<([u32; builtin::CLEAR_AXES], [u32; builtin::CLEAR_AXES])> =
            if rects.is_empty() { &whole[..] } else { rects }
                .iter()
                .map(|r| {
                    let end = [
                        r.x.saturating_add(r.width).min(extent.width),
                        r.y.saturating_add(r.height).min(extent.height),
                        slices.end,
                    ];
                    ([r.x, r.y, slices.start], end)
                })
                .filter(|(origin, end)| origin.iter().zip(end).all(|(o, e)| o < e))
                .collect();
        if boxes.is_empty() {
            return Ok(());
        }
        let params: Vec<[u8; builtin::CLEAR_PARAMS_BYTES]> = boxes
            .iter()
            .map(|&(origin, end)| builtin::clear_params(value, origin, end))
            .collect();
        let blocks: Vec<&[u8]> = params.iter().map(|p| &p[..]).collect();
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offsets = vec![0u64; blocks.len()];
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &blocks, align, &mut offsets);
        let ring = self.gpu(ring)?;
        self.helper_blocks = None;
        self.compute_helpers = None;
        if v.twin.is_some() {
            self.backend.slow_paths().hit(SlowPath::ReinterpretCopy);
            self.copy_twin(v, true);
        }
        for (&(origin, end), &offset) in boxes.iter().zip(&offsets) {
            let entries = [
                BindingEntry {
                    binding: builtin::CLEAR_PARAMS_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset,
                        size: builtin::CLEAR_PARAMS_BYTES as u64,
                    }),
                },
                BindingEntry {
                    binding: builtin::CLEAR_VIEW_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageTexture {
                        view: &v.gpu.handle,
                        writable: true,
                    },
                },
            ];
            let set = self.backend.create_binding_set(&entries);
            let set = self.gpu(set)?;
            let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
            bindings[0] = Some(&set);
            let r = self.backend.dispatch(&DispatchCall {
                program: &program,
                bindings,
                immediates: &[],
                groups: DispatchGroups::Direct(builtin::clear_groups(origin, end)),
            });
            self.backend.destroy(Object::BindingSet(set));
            self.gpu(r)?;
        }
        if v.twin.is_some() {
            self.copy_twin(v, false);
        }
        Ok(())
    }

    /// The storage clear program of `format` and `dimension`, made at its first use.
    fn storage_clear_program(
        &mut self,
        format: Format,
        dimension: ViewDimension,
    ) -> Result<B::Program> {
        if let Some(p) = self.storage_clears.get(&(format, dimension)) {
            return Ok(p.handle.clone());
        }
        let module =
            wcontainers_shaders::builtin::storage_clear(format, dimension).or_else(refused)?;
        let shader = self.backend.create_shader(&module);
        let shader = self.gpu(shader)?;
        let program = self.backend.create_program(&Program::Compute(&shader));
        self.backend.destroy(Object::Shader(shader));
        let program = self.gpu(program)?;
        let id = self.id();
        self.storage_clears.insert(
            (format, dimension),
            Kept::new(program.clone(), id, Object::Program, &self.graveyard),
        );
        Ok(program)
    }

    /// `ClearUnorderedAccessViewUint` (`float` false) and `Float`: buffer views take the first value
    /// for each 32-bit word; texture views each texel, as raw bits of the view's format.
    pub fn clear_uav(&mut self, view: &ShaderView<B>, values: [u32; 4], float: bool) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        self.settle();
        match view {
            ShaderView::Buffer(BufferSpan {
                buffer,
                offset,
                texel: Some((format, first, count)),
                ..
            }) => {
                if float {
                    return not_implemented("a float clear of a typed buffer view");
                }
                let pattern = uint_element(*format, values)?;
                self.backend.clear_buffer(
                    &buffer.handle(),
                    offset + u64::from(*first),
                    u64::from(*count) * pattern.len() as u64,
                    &pattern,
                );
                Ok(())
            }
            ShaderView::Buffer(span) => {
                // Direct3D clears only float, unorm and snorm views with floats; on raw and
                // structured views the call is dropped.
                if float {
                    return Ok(());
                }
                self.backend.clear_buffer(
                    &span.buffer.handle(),
                    span.offset,
                    span.size,
                    &values[0].to_le_bytes(),
                );
                Ok(())
            }
            ShaderView::Texture(v) => {
                // A float clear of a view held as its integer twin clears with the encoded values.
                let (values, float) = match v.decoded {
                    Some(f) if float => (
                        values.map(|c| format::encode_norm(f, f32::from_bits(c))),
                        false,
                    ),
                    _ => (values, float),
                };
                let integer = v.decoded.is_none()
                    && matches!(
                        FORMAT_FACTS[v.format as usize].kind,
                        FormatKind::Uint | FormatKind::Sint
                    );
                if integer && float {
                    return not_implemented(
                        "a float clear of an unordered access view of an integer format",
                    );
                }
                // A uint clear of a view of floats writes each value's low bits as the channel's:
                // the texels' bytes, written as they are.
                if !integer && !float {
                    return self.fill_texels(v, &uint_element(v.format, values)?);
                }
                if !v.texture.gpu_desc.usage.contains(TextureUsage::RENDER) {
                    return self.store_clear(v, values, float, &[]);
                }
                let r = self.backend.clear_texture(
                    &v.texture.gpu.handle,
                    &v.texture.gpu_desc,
                    v.format,
                    v.desc.first_mip..v.desc.first_mip + 1,
                    v.desc.first_layer..v.desc.first_layer + v.desc.layer_count,
                    ClearValue::Color(values),
                );
                self.gpu(r)
            }
        }
    }

    /// Writes `texel` into every texel view `v` sees (of its one level), outside a pass.
    fn fill_texels(&mut self, v: &View<B>, texel: &[u8]) -> Result<()> {
        let t = &v.texture;
        let mip = v.desc.first_mip;
        let three_d = t.desc.dimension == TexDim::D3;
        let mut extent = t.mip_extent(mip);
        extent.depth_or_layers = v.desc.layer_count;
        let (layer, z) = if three_d {
            (0, v.desc.first_layer)
        } else {
            (v.desc.first_layer, 0)
        };
        let row = extent.width * texel.len() as u32;
        let texels = (extent.width * extent.height * extent.depth_or_layers) as usize;
        let region = TextureRegion {
            texture: &t.gpu.handle,
            mip,
            layer,
            origin: Origin { x: 0, y: 0, z },
            stencil: false,
        };
        self.backend.write_texture(
            &region,
            extent,
            &texel.repeat(texels),
            row,
            extent.height,
            WriteMode::Ordered,
        );
        Ok(())
    }

    /// Makes the binding set of `stage` from its slots and its shader's reflection, when it changed.
    /// Makes `stage`'s binding set again where its bindings changed; `pulled` holds the binding of
    /// each vertex input the program fetches itself, by location.
    /// Makes `stage`'s binding set where it changed: with the streams `pulled` the vertex program
    /// fetches and the constant buffers `relocated` its program reads through other bindings.
    fn update_set(
        &mut self,
        stage: usize,
        pulled: &[(u32, u32)],
        relocated: &[RelocatedConstants],
    ) -> Result<()> {
        let relocated_slots = relocated.iter().fold(0u32, |m, r| {
            let kind = match r.read_as {
                ConstantsReadAs::Storage => 0,
                ConstantsReadAs::Texture => u16::BITS,
            };
            m | 1 << (r.slot + kind)
        });
        if relocated_slots != self.ctx.relocated[stage] {
            self.ctx.sets_dirty[stage] = true;
        }
        if !self.ctx.sets_dirty[stage] {
            return Ok(());
        }
        let set = set_of(stage);
        let shader = self.stage_shader(stage).clone();
        let Some(shader) = shader
            .filter(|s| !pulled.is_empty() || s.reflection.bindings.iter().any(|b| b.set == set))
        else {
            self.ctx.sets[stage] = None;
            self.ctx.through[stage].clear();
            self.ctx.twins[stage] = false;
            self.ctx.compared[stage].clear();
            self.ctx.sets_dirty[stage] = false;
            return Ok(());
        };
        let wanted = || {
            shader
                .reflection
                .bindings
                .iter()
                .filter(move |b| b.set == set)
        };
        let stages = match stage {
            VERTEX if !self.ctx.vertex_compute => StageSet::VERTEX,
            DOMAIN if !self.ctx.domain_compute => StageSet::VERTEX,
            // A geometry shader's replay form is a vertex program (`GeometryAsReplay`).
            GEOMETRY if !self.backend.caps().features.contains(Features::COMPUTE) => {
                StageSet::VERTEX
            }
            PIXEL => StageSet::FRAGMENT,
            _ => StageSet::COMPUTE,
        };
        let stand_in = |n| {
            shader
                .stand_ins
                .iter()
                .find(|(b, _)| *b == n)
                .and_then(|s| s.1)
        };
        // Whether a shader resource binding reads a structured or raw buffer, not a texture.
        let reads_buffer = |n| matches!(stand_in(n), Some(StandIn::ZeroBuffer));
        // Typed buffers the program reads through texture copies, by slot.
        let texel_copies = self.texel_copies(stage)?;
        let textured_buffers = !texel_copies.is_empty();
        // What stands in for each empty slot the shader reads, by binding number.
        let mut empty: Vec<(u32, Zero<B>)> = Vec::new();
        for b in wanted() {
            let slot = b.register as usize;
            let filled = match b.kind {
                SlotKind::Uniform => self.ctx.constant_buffers[stage]
                    .get(slot)
                    .is_some_and(Option::is_some),
                // A view of the other kind (a texture where a buffer is read, integers where
                // floats are) reads as none.
                SlotKind::Texture => {
                    match self.ctx.resources[stage]
                        .get(slot)
                        .and_then(Option::as_deref)
                    {
                        Some(ShaderView::Texture(v)) => {
                            !reads_buffer(b.binding)
                                && match stand_in(b.binding) {
                                    Some(StandIn::Zeros(_, kind, _)) => {
                                        same_kind(v.read_kind(), kind)
                                    }
                                    _ => true,
                                }
                        }
                        Some(ShaderView::Buffer(span)) => {
                            reads_buffer(b.binding) || (textured_buffers && span.texel.is_some())
                        }
                        None => false,
                    }
                }
                SlotKind::Sampler => self.ctx.samplers[stage]
                    .get(slot)
                    .is_some_and(Option::is_some),
                SlotKind::Storage => match self.ctx.uavs[stage].get(slot) {
                    Some(Some(_)) => true,
                    _ => return not_implemented("an empty unordered access slot a shader uses"),
                },
                SlotKind::Counter => match self.ctx.uavs[stage].get(slot) {
                    Some(Some(u))
                        if matches!(
                            &**u,
                            ShaderView::Buffer(BufferSpan {
                                counter: Some(_),
                                ..
                            })
                        ) =>
                    {
                        true
                    }
                    _ => {
                        return Err(Error::InvalidArg(
                            "a shader counts with a view that has no counter",
                        ));
                    }
                },
            };
            if filled {
                continue;
            }
            let zero = match (b.kind, stand_in(b.binding)) {
                (SlotKind::Uniform, _) => {
                    let declared = shader
                        .constant_sizes
                        .iter()
                        .find(|(n, _)| *n == b.binding)
                        .map_or(16, |s| s.1);
                    Zero::Buffer(self.zero_buffer()?, declared)
                }
                (_, Some(StandIn::Zeros(dimension, kind, multi))) => {
                    Zero::View(self.zero_view(dimension, kind, if multi { 4 } else { 1 })?)
                }
                (_, Some(StandIn::DefaultSampler)) => Zero::Sampler(self.default_sampler()?),
                (_, Some(StandIn::ZeroBuffer)) => Zero::Storage(self.zero_storage()?),
                (_, None) => {
                    return not_implemented(
                        "an empty slot read as a depth texture, a multisampled array or a comparison sampler",
                    );
                }
            };
            empty.push((b.binding, zero));
        }
        // Colour views the program compares through are read through depth copies of their
        // texels.
        let mut compared = Vec::new();
        let mut compared_views = Vec::new();
        for b in wanted().filter(|b| {
            b.kind == SlotKind::Texture
                && b.register < u128::BITS
                && shader.depth_textures & (1 << b.register) != 0
        }) {
            let bound = self.ctx.resources[stage]
                .get(b.register as usize)
                .cloned()
                .flatten();
            if let Some(view) = bound
                && let ShaderView::Texture(v) = &*view
                && !matches!(
                    FORMAT_FACTS[v.format as usize].kind,
                    FormatKind::Depth | FormatKind::DepthStencil
                )
            {
                compared_views.push((b.register, self.compared_view(v)?));
                compared.push(view.clone());
            }
        }
        // A sampler read the other way than it compares reads through its twin.
        let mut twins: Vec<(u32, B::Sampler)> = Vec::new();
        for b in wanted().filter(|b| b.kind == SlotKind::Sampler) {
            let compares = !matches!(stand_in(b.binding), Some(StandIn::DefaultSampler));
            let Some(s) = self.ctx.samplers[stage]
                .get(b.register as usize)
                .and_then(Option::clone)
            else {
                continue;
            };
            if s.made.compare.is_some() != compares {
                twins.push((b.binding, self.sampler_twin(&s, compares)?));
            }
        }
        let zero_stream = if pulled.is_empty() {
            None
        } else {
            Some(self.zero_storage()?)
        };
        // Windows shorter than their shader's block read through a copy of the block's size whose
        // bytes past the window are zero, and constant buffers the program reads through textures
        // through a texture copy.
        let textured = |slot: u32| {
            relocated
                .iter()
                .find(|r| r.slot == slot && r.read_as == ConstantsReadAs::Texture)
                .map(|r| r.binding)
        };
        let mut windows = Vec::new();
        let mut empty_textured = Vec::new();
        for b in wanted().filter(|b| b.kind == SlotKind::Uniform) {
            let slot = b.register as usize;
            let declared = shader
                .constant_sizes
                .iter()
                .find(|(n, _)| *n == b.binding)
                .map_or(0, |s| s.1);
            let read_as = if textured(b.register).is_some() {
                ConstantsReadAs::Texture
            } else {
                ConstantsReadAs::Storage
            };
            let Some(Some(c)) = self.ctx.constant_buffers[stage].get(slot) else {
                if read_as == ConstantsReadAs::Texture {
                    empty_textured.push(b.register);
                }
                continue;
            };
            let range = self.ctx.constant_ranges[stage].get(slot).copied().flatten();
            let (offset, length) = match range {
                Some(range) => window_of(c, range),
                None => (0, c.size.get()),
            };
            if read_as == ConstantsReadAs::Texture || (range.is_some() && length < declared) {
                let size = declared.max(length);
                windows.push((b.register, c.clone(), offset, length, size, read_as));
            }
        }
        let mut copies = Vec::new();
        let mut texture_copies = Vec::new();
        for (register, c, offset, length, size, read_as) in &windows {
            let i = self.window(c, *offset, *length, *size, *read_as)?;
            match &c.windows.borrow()[i].copy {
                WindowCopy::Buffer(twin) => copies.push((*register, twin.handle.clone(), *size)),
                WindowCopy::Texture { view, .. } => {
                    texture_copies.push((*register, view.handle.clone()));
                }
            }
        }
        for register in empty_textured {
            texture_copies.push((register, self.zero_constants()?));
        }
        self.ctx.windows[stage] = windows;
        let mut cbs = Vec::new();
        for (slot, (c, range)) in self.ctx.constant_buffers[stage]
            .iter()
            .zip(&self.ctx.constant_ranges[stage])
            .enumerate()
        {
            let Some(c) = c else {
                cbs.push(None);
                continue;
            };
            if let Some((_, twin, size)) = copies.iter().find(|t| t.0 as usize == slot) {
                cbs.push(Some((twin.clone(), 0, *size)));
                continue;
            }
            let (offset, size) = match *range {
                None => (0, c.size.get()),
                Some(range) => window_of(c, range),
            };
            cbs.push(Some((c.handle(), offset, size)));
        }
        // Storage views the program reads through buffers of their texels.
        let mut through = Vec::new();
        for b in wanted().filter(|b| b.kind == SlotKind::Storage) {
            if let (Some(why), Some(Some(view))) = (
                self.reads_through(stage, b.register),
                self.ctx.uavs[stage].get(b.register as usize),
            ) && let ShaderView::Texture(v) = &**view
            {
                let v = v.clone();
                through.push(self.through_view(b.register, &v, why)?);
            }
        }
        self.ctx.through[stage] = through;
        // The back-end buffers of the buffer views bound for unordered access.
        let spans: Vec<_> = self.ctx.uavs[stage]
            .iter()
            .map(|u| match u.as_deref() {
                Some(ShaderView::Buffer(span)) => Some((span.buffer.gpu.borrow(), span)),
                _ => None,
            })
            .collect();
        let srvs: Vec<_> = self.ctx.resources[stage]
            .iter()
            .map(|v| match v.as_deref() {
                Some(ShaderView::Buffer(span)) => Some((span.buffer.gpu.borrow(), span)),
                _ => None,
            })
            .collect();
        // The streams the vertex program fetches, each bound from its offset rounded down to the
        // storage offset alignment to the buffer's end; an empty slot, or an offset past the end,
        // reads the buffer of zeros.
        let limits = &self.backend.caps().limits;
        let align =
            u64::from(limits.min_storage_offset_alignment).max(wcontainers_gpu::STORAGE_WORD);
        let largest = whole_words(limits.max_storage_buffer_size);
        let streams: Vec<_> = pulled
            .iter()
            .map(|&(binding, location)| {
                let bound = self
                    .ctx
                    .layout
                    .as_ref()
                    .and_then(|l| l.all.iter().find(|e| e.location == location))
                    .and_then(|e| self.ctx.vertex_buffers[e.slot as usize].as_ref())
                    .map(|(b, _, offset)| (b, u64::from(*offset) / align * align))
                    .filter(|(b, start)| start + 4 <= b.size.get());
                (
                    binding,
                    bound.map(|(b, start)| {
                        let size = whole_words(b.size.get() - start).min(largest);
                        (b.gpu.borrow(), start, size)
                    }),
                )
            })
            .collect();
        let uavs = &self.ctx.uavs[stage];
        let views = &self.ctx.resources[stage];
        let samplers = &self.ctx.samplers[stage];
        let mut entries = Vec::new();
        for (binding, bound) in &streams {
            let range = match (bound, &zero_stream) {
                (Some((b, offset, size)), _) => BufferRange {
                    buffer: &b.handle,
                    offset: *offset,
                    size: *size,
                },
                (None, Some(zero)) => BufferRange {
                    buffer: zero,
                    offset: 0,
                    size: ZERO_BYTES,
                },
                (None, None) => unreachable!("made with the streams"),
            };
            entries.push(BindingEntry {
                binding: *binding,
                stages,
                resource: BindingResource::StorageBuffer {
                    range,
                    writable: false,
                },
            });
        }
        for b in wanted() {
            let slot = b.register as usize;
            let resource = match b.kind {
                SlotKind::Uniform => {
                    cbs.get(slot)
                        .and_then(Option::as_ref)
                        .map(|(c, offset, size)| {
                            BindingResource::UniformBuffer(BufferRange {
                                buffer: c,
                                offset: *offset,
                                size: *size,
                            })
                        })
                }
                SlotKind::Texture => match (
                    views.get(slot).and_then(Option::as_deref),
                    srvs.get(slot).and_then(Option::as_ref),
                ) {
                    (Some(ShaderView::Texture(_)), _)
                        if let Some((_, depth)) =
                            compared_views.iter().find(|c| c.0 == b.register) =>
                    {
                        Some(BindingResource::Texture(depth))
                    }
                    (Some(ShaderView::Texture(v)), _)
                        if !empty.iter().any(|(n, _)| *n == b.binding) =>
                    {
                        Some(BindingResource::Texture(&v.gpu.handle))
                    }
                    (Some(ShaderView::Buffer(_)), _)
                        if let Some((_, copy)) =
                            texel_copies.iter().find(|c| c.0 == b.register) =>
                    {
                        Some(BindingResource::Texture(copy))
                    }
                    (_, Some((buffer, span))) if reads_buffer(b.binding) => {
                        Some(BindingResource::StorageBuffer {
                            range: BufferRange {
                                buffer: &buffer.handle,
                                offset: span.offset,
                                size: span.size,
                            },
                            writable: false,
                        })
                    }
                    _ => None,
                },
                SlotKind::Sampler => samplers.get(slot).and_then(Option::as_ref).map(|s| {
                    BindingResource::Sampler(
                        twins
                            .iter()
                            .find(|t| t.0 == b.binding)
                            .map_or(&s.gpu.handle, |t| &t.1),
                    )
                }),
                SlotKind::Storage => match (uavs[slot].as_deref(), &spans[slot]) {
                    (Some(ShaderView::Texture(_)), _)
                        if let Some(t) = self.ctx.through[stage]
                            .iter()
                            .find(|t| t.register() == b.register) =>
                    {
                        Some(BindingResource::StorageBuffer {
                            range: t.range(),
                            writable: true,
                        })
                    }
                    (Some(ShaderView::Texture(v)), _) => Some(BindingResource::StorageTexture {
                        view: &v.gpu.handle,
                        writable: true,
                    }),
                    (_, Some((buffer, span))) => Some(BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer: &buffer.handle,
                            offset: span.offset,
                            size: span.size,
                        },
                        writable: true,
                    }),
                    _ => None,
                },
                SlotKind::Counter => match &spans[slot] {
                    Some((
                        _,
                        BufferSpan {
                            counter: Some(c), ..
                        },
                    )) => Some(BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer: &c.handle,
                            offset: 0,
                            size: 4,
                        },
                        writable: true,
                    }),
                    _ => None,
                },
            };
            let resource = match resource {
                Some(r) => r,
                None => match &empty
                    .iter()
                    .find(|(n, _)| *n == b.binding)
                    .expect("a stand-in")
                    .1
                {
                    Zero::Buffer(buffer, size) => BindingResource::UniformBuffer(BufferRange {
                        buffer,
                        offset: 0,
                        size: *size,
                    }),
                    Zero::View(view) => BindingResource::Texture(view),
                    Zero::Storage(buffer) => BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer,
                            offset: 0,
                            size: ZERO_BYTES,
                        },
                        writable: false,
                    },
                    Zero::Sampler(sampler) => BindingResource::Sampler(sampler),
                },
            };
            // A constant buffer the program reads as storage or through a texture, at the variant's
            // binding.
            let stored = relocated
                .iter()
                .find(|r| r.slot == b.register && r.read_as == ConstantsReadAs::Storage);
            let (binding, resource) = match (b.kind, resource) {
                (SlotKind::Uniform, _)
                    if let (Some(binding), Some((_, view))) = (
                        textured(b.register),
                        texture_copies.iter().find(|t| t.0 == b.register),
                    ) =>
                {
                    (binding, BindingResource::Texture(view))
                }
                (SlotKind::Uniform, BindingResource::UniformBuffer(range))
                    if let Some(&RelocatedConstants { binding, .. }) = stored =>
                {
                    let resource = BindingResource::StorageBuffer {
                        range,
                        writable: false,
                    };
                    (binding, resource)
                }
                (_, resource) => (b.binding, resource),
            };
            entries.push(BindingEntry {
                binding,
                stages,
                resource,
            });
        }
        let set = self.backend.create_binding_set(&entries);
        drop(entries);
        drop(streams);
        drop(cbs);
        drop(spans);
        drop(srvs);
        let set = self.gpu(set)?;
        let id = self.id();
        self.ctx.sets[stage] = Some(Kept::new(set, id, Object::BindingSet, &self.graveyard));
        self.ctx.sets_dirty[stage] = false;
        self.ctx.relocated[stage] = relocated_slots;
        self.ctx.twins[stage] = !compared.is_empty()
            || self.ctx.resources[stage]
                .iter()
                .chain(&self.ctx.uavs[stage])
                .flatten()
                .any(|v| matches!(&**v, ShaderView::Texture(t) if t.twin.is_some()));
        self.ctx.compared[stage] = compared;
        Ok(())
    }

    /// The bound raster block made as `key` says; kept on the state from its first draw.
    fn raster_twin(&mut self, key: RasterKey) -> Result<B::Raster> {
        let r = self
            .ctx
            .raster
            .clone()
            .unwrap_or_else(|| self.defaults.raster.clone());
        if let Some((_, k)) = r.masked.borrow().iter().find(|(m, _)| *m == key) {
            return Ok(k.handle.clone());
        }
        let block = twin_block(&r.desc, self.backend.caps(), key);
        let gpu = self.backend.create_raster(&block);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        r.masked.borrow_mut().push((
            key,
            Kept::new(gpu.clone(), id, Object::Raster, &self.graveyard),
        ));
        Ok(gpu)
    }

    /// Makes, at the first draw needing it, the bound depth-stencil state's twin writing nothing
    /// the bound read-only depth-stencil view marks (Direct3D writes neither aspect through it).
    fn fit_depth_stencil(&mut self) -> Result<()> {
        let flags = self.read_only_flags();
        let bound = self.bound_depth_stencil();
        let Some(desc) = read_only_twin(&bound.desc, flags) else {
            return Ok(());
        };
        if bound.read_only.borrow().iter().any(|(f, _)| *f == flags) {
            return Ok(());
        }
        let twin = self.create_depth_stencil_state(&desc)?;
        bound.read_only.borrow_mut().push((flags, twin));
        Ok(())
    }

    /// The aspects of the bound depth-stencil view the pass only reads: those the view marks, or
    /// both where the draw runs its early tests in the program.
    fn read_only_flags(&self) -> u32 {
        let marked = self.ctx.depth.as_ref().map_or(0, |v| v.desc.read_only);
        if self.ctx.early {
            marked | DSV_READ_ONLY_DEPTH | DSV_READ_ONLY_STENCIL
        } else {
            marked
        }
    }

    /// Whether the draw runs its pixel shader `ps`'s forced early depth and stencil tests in the
    /// program (`EarlyTestsInShader`): a shader with `[earlydepthstencil]` and storage writes,
    /// under a depth or stencil test. Those writes happen only for fragments passing the tests,
    /// which the back ends run after the shader.
    fn early_tests(&self, ps: Option<&Rc<Shader<B>>>) -> Result<Early> {
        let forced = ps.is_some_and(|ps| {
            matches!(
                ps.reflection.extras,
                wcontainers_gpu::StageExtras::Pixel {
                    early_tests: true,
                    side_effects: true,
                    ..
                }
            )
        });
        let Some(view) = self.ctx.depth.as_ref().filter(|_| forced) else {
            return Ok(Early::No);
        };
        let d = &self.bound_depth_stencil().desc;
        if !d.depth_enable && !d.stencil_enable {
            return Ok(Early::No);
        }
        let marked = view.desc.read_only;
        let mut ops = [d.front, d.back]
            .into_iter()
            .flat_map(|f| [f.fail, f.depth_fail, f.pass]);
        let keeps = ops.try_fold(true, |all, op| {
            Ok::<_, Error>(all && crate::device::stencil_op(op)? == StencilOp::Keep)
        })?;
        let writes_depth = d.depth_enable && d.depth_write && marked & DSV_READ_ONLY_DEPTH == 0;
        let writes_stencil = d.stencil_enable
            && d.stencil_write_mask != 0
            && !keeps
            && marked & DSV_READ_ONLY_STENCIL == 0;
        if self.ctx.gs.is_some() || self.ctx.hs.is_some() || self.ctx.ds.is_some() {
            return not_implemented(
                "early depth-stencil tests before storage writes behind a geometry or \
                 tessellation stage",
            );
        }
        Ok(
            if (writes_depth || writes_stencil) && self.ctx.early_run.is_none() {
                Early::PerPrimitive
            } else {
                Early::InProgram
            },
        )
    }

    /// A draw whose forced early tests run in the program while it writes depth or stencil
    /// (`EarlyTestsPerPrimitive`): each primitive is drawn twice, first with the tests in the
    /// program against a read-only attachment (its colour and storage writes), then without a
    /// pixel shader or targets, making the API's depth and stencil writes.
    fn draw_early_per_primitive(
        &mut self,
        range: Range,
        first_instance: u32,
        instances: u32,
    ) -> Result<()> {
        let per = plain_topology(self.ctx.topology)
            .and_then(list_vertices)
            .ok_or(Error::NotImplemented(
                "early depth-stencil tests per primitive of a strip or patch topology",
            ))?;
        if instances != 1 {
            return not_implemented("early depth-stencil tests per primitive of instanced draws");
        }
        // What the second run of each primitive would do again, and must not.
        if self.queries.running_any() || self.ctx.so_targets.iter().any(Option::is_some) {
            return not_implemented(
                "early depth-stencil tests per primitive while queries run or with stream output",
            );
        }
        let (count, bases) = match range {
            Range::Vertices { count, start } => (count, [start, first_instance]),
            Range::Indexed { count, base, .. } => (count, [base as u32, first_instance]),
            Range::Indirect { .. } => {
                return not_implemented(
                    "early depth-stencil tests per primitive of indirect draws",
                );
            }
        };
        self.backend
            .slow_paths()
            .hit(SlowPath::EarlyTestsPerPrimitive);
        let targets = self.ctx.targets.clone();
        let ps = self.ctx.ps.clone();
        let mut r = Ok(());
        for p in 0..count / per {
            let one = match range {
                Range::Vertices { start, .. } => Range::Vertices {
                    count: per,
                    start: start + p * per,
                },
                Range::Indexed { start, base, .. } => Range::Indexed {
                    count: per,
                    start: start + p * per,
                    base,
                },
                Range::Indirect { .. } => unreachable!("refused above"),
            };
            self.end_pass();
            self.ctx.early_run = Some(bases);
            r = self.draw_once(one, first_instance, instances, None);
            self.ctx.early_run = None;
            if r.is_err() {
                break;
            }
            self.end_pass();
            self.ctx.targets = vec![None; targets.len()];
            self.ctx.ps = None;
            r = self.draw_once(one, first_instance, instances, None);
            self.ctx.targets = targets.clone();
            self.ctx.ps = ps.clone();
            if r.is_err() {
                break;
            }
        }
        self.end_pass();
        r
    }

    fn bound_depth_stencil(&self) -> Rc<DepthStencil<B>> {
        self.ctx
            .depth_stencil
            .clone()
            .unwrap_or_else(|| self.defaults.depth_stencil.clone())
    }

    /// The depth-stencil state a draw uses: the bound one, or its twin `fit_depth_stencil` made.
    pub(crate) fn drawn_depth_stencil(&self) -> Rc<DepthStencil<B>> {
        let bound = self.bound_depth_stencil();
        let flags = self.read_only_flags();
        let twin = bound
            .read_only
            .borrow()
            .iter()
            .find(|(f, _)| *f == flags)
            .map(|(_, t)| t.clone());
        twin.unwrap_or(bound)
    }

    /// The raster block a draw uses: the bound state's, or its twin of `masked`'s sample mask and bias.
    pub(crate) fn raster_desc(&self, masked: Option<RasterKey>) -> wcontainers_gpu::RasterDesc {
        let r = self.ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
        match masked {
            Some(key) => twin_block(&r.desc, self.backend.caps(), key),
            None => crate::device::raster_block(&r.desc, self.backend.caps()),
        }
    }

    /// `s` made comparing or not as `compares` says, kept on `s` from its first use.
    fn sampler_twin(&mut self, s: &Sampler<B>, compares: bool) -> Result<B::Sampler> {
        if let Some(t) = &*s.twin.borrow() {
            return Ok(t.handle.clone());
        }
        // Direct3D leaves comparing through a sampler that does not compare undefined; the twin
        // compares with the function its description names.
        let compare = if compares {
            Some(crate::device::compare_fn(s.desc.comparison)?)
        } else {
            None
        };
        let gpu = self
            .backend
            .create_sampler(&GpuSampler { compare, ..s.made });
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        *s.twin.borrow_mut() = Some(Kept::new(gpu.clone(), id, Object::Sampler, &self.graveyard));
        Ok(gpu)
    }

    /// Writes the helper constants of the draw's two program stages where they differ from the
    /// last written, and keeps the set binding them (with an indirect draw's arguments, for a vertex
    /// program that reads its bases from them).
    /// How the vertex program fetches its input at `location` for a draw from `first_instance`:
    /// the first element's byte offset in the stream's binding, the stride, a divisor and a bias.
    fn pulled_stream(&self, location: u32, first_instance: u32) -> [u32; 4] {
        let Some(e) = self
            .ctx
            .layout
            .as_ref()
            .and_then(|l| l.all.iter().find(|e| e.location == location))
        else {
            return [0; 4];
        };
        let align = self
            .backend
            .caps()
            .limits
            .min_storage_offset_alignment
            .max(4);
        let (stride, offset) = self.ctx.vertex_buffers[e.slot as usize]
            .as_ref()
            .map_or((0, 0), |(_, stride, offset)| (*stride, *offset));
        let offset = offset % align + e.offset;
        match self
            .ctx
            .layout
            .as_ref()
            .and_then(|l| l.instance_steps[e.slot as usize])
        {
            // Every instance reads the first instance's element.
            Some(0) => [
                offset.wrapping_add(first_instance.wrapping_mul(stride)),
                0,
                1,
                0,
            ],
            Some(rate) => [
                offset,
                stride,
                rate,
                (rate - 1).wrapping_mul(first_instance),
            ],
            None => [offset, stride, 0, 0],
        }
    }

    /// The first element's byte in the bound range and the element count of the typed buffer view
    /// bound to `stage` at `binding`; none for an empty slot or a view of another kind.
    fn texel_view(&self, stage: usize, binding: u32) -> [u32; 2] {
        let texture = SLOT_KINDS[SlotKind::Texture as usize].first_binding;
        let storage = SLOT_KINDS[SlotKind::Storage as usize].first_binding;
        let view = match binding {
            b if (texture..storage).contains(&b) => {
                self.ctx.resources[stage].get((b - texture) as usize)
            }
            b => self.ctx.uavs[stage].get(b.wrapping_sub(storage) as usize),
        };
        match view.and_then(Option::as_deref) {
            Some(ShaderView::Buffer(BufferSpan {
                texel: Some((_, first, count)),
                ..
            })) => [*first, *count],
            _ => [0, 0],
        }
    }

    /// The helper block `h` of `stage`'s program for a draw from `bases` (vertex, instance), with
    /// an indirect draw's arguments.
    fn helper_block(
        &mut self,
        h: &Helpers,
        stage: usize,
        bases: [u32; 2],
        args: Option<(&Rc<Buffer<B>>, u32, bool)>,
    ) -> Result<Vec<u8>> {
        let mut block = vec![0u8; h.size as usize];
        let mut put = |at: u32, words: &[u32]| {
            let bytes = &mut block[at as usize..][..size_of_val(words)];
            for (b, w) in bytes
                .as_chunks_mut::<{ size_of::<u32>() }>()
                .0
                .iter_mut()
                .zip(words)
            {
                *b = w.to_le_bytes();
            }
        };
        for slot in &h.values {
            let at = slot.offset;
            match slot.value {
                HelperValue::DrawBase => put(at, &[bases[0], bases[1], 0]),
                HelperValue::DrawArguments => {
                    let (_, offset, indexed) = args.expect("an indirect draw");
                    put(
                        at,
                        &[offset / size_of::<u32>() as u32, u32::from(indexed), 0],
                    );
                }
                HelperValue::PulledStream { input } => {
                    put(at, &self.pulled_stream(input, bases[1]));
                }
                HelperValue::LodBias { sampler: s } => {
                    let bias = self
                        .bound_sampler(stage, s)
                        .map_or(0.0, |s| s.desc.mip_lod_bias);
                    put(at, &[bias.to_bits()]);
                }
                HelperValue::SamplerState { sampler: s } => {
                    let state = match self.bound_sampler(stage, s) {
                        Some(b) => crate::device::sampler_state(b),
                        None => {
                            self.default_sampler()?;
                            crate::device::sampler_state(self.zeros.sampler.as_ref().expect("made"))
                        }
                    };
                    put(at, &state);
                }
                HelperValue::BorderColor { sampler: s } => {
                    let border = self
                        .bound_sampler(stage, s)
                        .map_or(Color::default(), |s| s.desc.border);
                    put(at, &border.map(f32::to_bits));
                }
                HelperValue::TextureComponents { texture } => {
                    let components = match self.ctx.resources[stage]
                        .get(texture as usize)
                        .and_then(Option::as_deref)
                    {
                        Some(ShaderView::Texture(v)) => crate::format::components(v.format),
                        _ => 0b1111,
                    };
                    put(at, &[components]);
                }
                HelperValue::DepthBias => {
                    let r = &self
                        .ctx
                        .raster
                        .as_ref()
                        .unwrap_or(&self.defaults.raster)
                        .desc;
                    let step = self.bias_step()?.unwrap_or(0.0);
                    let values = [
                        r.depth_bias as f32,
                        r.slope_scaled_depth_bias,
                        r.depth_bias_clamp,
                        step,
                    ];
                    put(at, &values.map(f32::to_bits));
                }
                HelperValue::MinLod { texture } => {
                    let (clamp, levels) = self.min_lod_of(stage, texture).unwrap_or((0.0, 1));
                    put(at, &[clamp.to_bits(), levels]);
                }
                HelperValue::TexelBufferView { binding, .. } => {
                    put(at, &self.texel_view(stage, binding));
                }
                HelperValue::ViewportAdjust => {
                    let adjust = self
                        .viewport_in_shader()
                        .map_or([1.0, 1.0, 0.0, 0.0], |a| a.adjust);
                    put(at, &adjust.map(f32::to_bits));
                }
                HelperValue::DepthRange => {
                    let v = self.ctx.viewports.first().copied().unwrap_or_default();
                    put(at, &[v.min_depth.to_bits(), v.max_depth.to_bits()]);
                }
                HelperValue::TexelLayout { slot } => {
                    put(at, &self.through_layout(stage, slot));
                }
                // An empty slot, or one holding a buffer, answers zero as Direct3D's does.
                HelperValue::TextureLevels { texture }
                | HelperValue::TextureSamples { texture } => {
                    self.backend
                        .slow_paths()
                        .hit(SlowPath::TextureInfoInHelpers);
                    let view = match self.ctx.resources[stage]
                        .get(texture as usize)
                        .and_then(Option::as_deref)
                    {
                        Some(ShaderView::Texture(v)) => Some(v),
                        _ => None,
                    };
                    let info = view.map_or(0, |v| match slot.value {
                        HelperValue::TextureLevels { .. } => v.desc.mip_count,
                        _ => v.texture.desc.sample_count.max(1),
                    });
                    put(at, &[info]);
                }
                HelperValue::EarlyTests => {
                    let d = &self.bound_depth_stencil().desc;
                    let code = |on: bool, f: u32| -> Result<u32> {
                        Ok(if on {
                            crate::device::compare_fn(f)? as u32
                        } else {
                            0
                        })
                    };
                    let values = [
                        code(d.depth_enable, d.depth_func)?,
                        code(d.stencil_enable, d.front.func)?,
                        code(d.stencil_enable, d.back.func)?,
                        u32::from(self.ctx.stencil_ref as u8),
                        u32::from(d.stencil_read_mask),
                    ];
                    put(at, &values);
                }
                HelperValue::Layer => put(at, &[self.ctx.run[0]]),
                HelperValue::Viewport => put(at, &[self.ctx.run[1]]),
                HelperValue::LayerCount => put(at, &[self.ctx.run_layers]),
                _ => unreachable!("refused at conversion"),
            }
        }
        Ok(block)
    }

    fn write_helpers(
        &mut self,
        helpers: &[Rc<Helpers>; 2],
        bases: [u32; 2],
        args: Option<(&Rc<Buffer<B>>, u32, bool)>,
    ) -> Result<()> {
        let blocks = [
            self.helper_block(&helpers[0], VERTEX, bases, args)?,
            self.helper_block(&helpers[1], PIXEL, bases, args)?,
        ];
        // The merged samplers of textures under a minimum level-of-detail clamp; both stages
        // number theirs from the same binding, so only one of them may have any.
        if !helpers[0].merged.is_empty() && !helpers[1].merged.is_empty() {
            return not_implemented(
                "minimum level-of-detail clamps on textures both draw stages sample",
            );
        }
        let mut merged = Vec::new();
        for (h, stage) in helpers.iter().zip([VERTEX, PIXEL]) {
            for &(binding, (texture, sampler)) in &h.merged {
                let (clamp, _) = self.min_lod_of(stage, texture).unwrap_or((0.0, 1));
                let (s, id) = self.clamped_sampler(stage, sampler, clamp)?;
                merged.push((binding, stage, s, id));
            }
        }
        let samplers: Vec<u64> = merged.iter().map(|m| m.3).collect();
        let early = if helpers[1]
            .values
            .iter()
            .any(|h| h.value == HelperValue::EarlyTests)
        {
            Some(self.early_views()?)
        } else {
            None
        };
        let early_id = early.as_ref().map_or(0, |e| e.0);
        let blocks = [&blocks[0][..], &blocks[1][..]];
        let arguments = args.map_or(0, |(b, ..)| b.gpu.borrow().id);
        if self.helper_blocks.as_ref().is_some_and(|h| {
            h.arguments == arguments
                && h.blocks[0] == blocks[0]
                && h.blocks[1] == blocks[1]
                && h.samplers == samplers
                && h.early == early_id
        }) {
            return Ok(());
        }
        if helpers[0]
            .values
            .iter()
            .any(|h| h.value == HelperValue::DrawBase)
        {
            self.backend.slow_paths().hit(SlowPath::DrawBaseInHelpers);
        }
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offsets = [0u64; 2];
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &blocks, align, &mut offsets);
        let ring = self.gpu(ring)?;
        self.compute_helpers = None;
        let args = args.map(|(b, ..)| (b.gpu.borrow(), whole_words(b.size.get())));
        let mut entries = Vec::with_capacity(3);
        for (i, stage) in [ShaderStage::Vertex, ShaderStage::Fragment]
            .into_iter()
            .enumerate()
        {
            if !blocks[i].is_empty() {
                entries.push(BindingEntry {
                    binding: helper_binding(stage),
                    stages: if i == 0 {
                        StageSet::VERTEX
                    } else {
                        StageSet::FRAGMENT
                    },
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[i],
                        size: blocks[i].len() as u64,
                    }),
                });
            }
        }
        if let Some((b, size)) = &args {
            entries.push(BindingEntry {
                binding: DRAW_ARGUMENTS_BINDING,
                stages: StageSet::VERTEX,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &b.handle,
                        offset: 0,
                        size: *size,
                    },
                    writable: false,
                },
            });
        }
        for (binding, stage, s, _) in &merged {
            entries.push(BindingEntry {
                binding: *binding,
                stages: if *stage == VERTEX {
                    StageSet::VERTEX
                } else {
                    StageSet::FRAGMENT
                },
                resource: BindingResource::Sampler(s),
            });
        }
        if let Some((_, depth, stencil)) = &early {
            for (binding, view) in [
                (EARLY_DEPTH_BINDING, depth),
                (EARLY_STENCIL_BINDING, stencil),
            ] {
                entries.push(BindingEntry {
                    binding,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(view),
                });
            }
        }
        let set = self.backend.create_binding_set(&entries);
        drop(entries);
        drop(args);
        let set = self.gpu(set)?;
        let id = self.id();
        self.helper_blocks = Some(HelperBlocks {
            blocks: blocks.map(<[u8]>::to_vec),
            arguments,
            samplers,
            early: early_id,
            set: Kept::new(set, id, Object::BindingSet, &self.graveyard),
        });
        Ok(())
    }

    /// The bound depth-stencil view's id and its depth and stencil aspects as early tests in the
    /// program read them; for a format without stencil, a texture of unsigned zeros stands in.
    fn early_views(&mut self) -> Result<(u64, B::TextureView, B::TextureView)> {
        let view = self
            .ctx
            .depth
            .clone()
            .expect("early tests only with a depth-stencil view bound");
        let stencil = if FORMAT_FACTS[view.format as usize].kind == FormatKind::DepthStencil {
            if view.stencil_aspect.borrow().is_none() {
                let gpu = self.backend.create_view(
                    &view.texture.gpu.handle,
                    &TextureViewDesc {
                        format: view.format,
                        dimension: ViewDimension::D2,
                        base_mip: view.desc.first_mip,
                        mip_count: 1,
                        base_layer: view.desc.first_layer,
                        layer_count: 1,
                        stencil: true,
                    },
                );
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                *view.stencil_aspect.borrow_mut() =
                    Some(Kept::new(gpu, id, Object::TextureView, &self.graveyard));
            }
            let kept = view.stencil_aspect.borrow();
            kept.as_ref().expect("made above").handle.clone()
        } else {
            self.zero_view(ViewDimension::D2, FormatKind::Uint, 1)?
        };
        Ok((view.gpu.id, view.gpu.handle.clone(), stencil))
    }

    /// A draw in one run; `args` holds an indirect draw's arguments.
    fn draw_once(
        &mut self,
        range: Range,
        first_instance: u32,
        instances: u32,
        args: Option<&Rc<Buffer<B>>>,
    ) -> Result<()> {
        if self.removed.is_some() || self.predicated_out() {
            return Ok(());
        }
        self.bury();
        // Without a vertex shader Direct3D draws nothing.
        let Some(vs) = self.ctx.vs.clone() else {
            return Ok(());
        };
        // Nor when the vertex shader reads an input the bound input layout, or none, does not
        // give.
        let given = |at: u32| {
            self.ctx
                .layout
                .as_ref()
                .is_some_and(|l| l.all.iter().any(|e| e.location == at))
        };
        if vs
            .reflection
            .inputs
            .iter()
            .any(|i| i.system.is_none() && !given(i.location))
        {
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| {
                eprintln!(
                    "{WARNING_PREFIX}: invalid: draws whose vertex shader reads an input the \
                     input layout does not give are dropped"
                );
            });
            return Ok(());
        }
        // Direct3D uses the mask's bits up to the targets' sample count, bit 0 alone without
        // multisampling; a draw it leaves no sample of writes nothing.
        let samples = self
            .ctx
            .targets
            .iter()
            .chain([&self.ctx.depth])
            .flatten()
            .map(|v| v.texture.desc.sample_count)
            .max()
            .unwrap_or(1);
        let all = u32::MAX >> (32 - samples.min(32));
        let mask = self.ctx.sample_mask & all;
        if mask == 0 || instances == 0 {
            return Ok(());
        }
        let early = match self.early_tests(self.ctx.ps.as_ref())? {
            Early::PerPrimitive => {
                return self.draw_early_per_primitive(range, first_instance, instances);
            }
            e => e == Early::InProgram,
        };
        if self.ctx.pass_open && self.ctx.pass_early != early {
            self.end_pass();
        }
        self.ctx.early = early;
        self.fit_depth_stencil()?;
        // A mask leaving out samples of multisampled targets draws through a twin of the raster
        // block made with it.
        let masks = samples > 1 && mask != all;
        if masks && !self.backend.caps().features.contains(Features::SAMPLE_MASK) {
            return not_implemented("a sample mask without the back end's (SampleMaskInShader)");
        }
        let depth_bias = self.bias_step()?.is_some();
        let unbiased = depth_bias || self.bias_dropped();
        // A fitted pass draws with the scissor test on, cut to the area every attachment covers.
        let scissored = self.fitted_targets()
            && !self
                .ctx
                .raster
                .as_ref()
                .unwrap_or(&self.defaults.raster)
                .desc
                .scissor;
        // Alpha-antialiased lines of the vertex stage: the stage helper draws a rectangle around
        // each, unculled and filled, and the pixel program multiplies its alpha by the coverage
        // passed after the linked values (`AntialiasedLinesAsQuads`).
        let aa_lines = self.lines_alpha_antialiased()
            && matches!(self.ctx.topology, topology::LINELIST | topology::LINESTRIP)
            && self.ctx.gs.is_none()
            && self.ctx.hs.is_none()
            && self.ctx.ds.is_none();
        let masked_key = (masks || unbiased || scissored || aa_lines).then_some((
            if masks { mask } else { u32::MAX },
            unbiased,
            scissored,
            aa_lines,
        ));
        let masked = match masked_key {
            Some(key) => Some(self.raster_twin(key)?),
            None => None,
        };
        let ps = self.ctx.ps.clone();
        // Direct3D writes the bits of an output of another kind (float, unsigned, signed) than
        // its target, which the back ends' pipelines refuse: the program writes them so.
        let (retyped, retyped_formats) = match &ps {
            Some(p) => self.retyped_targets(p),
            None => (0, 0),
        };
        if let Some(b) = &self.ctx.blend {
            let blends = |n: usize| b.desc.targets[if b.desc.independent { n } else { 0 }].enable;
            let decoded = |t: &Option<Rc<View<B>>>| t.as_ref().is_some_and(|v| v.decoded.is_some());
            if self
                .ctx
                .targets
                .iter()
                .enumerate()
                .any(|(n, t)| decoded(t) && blends(n))
            {
                return not_implemented(BLEND_INTO_INTEGER_TWIN);
            }
        }
        let dual_source = ps.is_some()
            && self
                .ctx
                .blend
                .as_ref()
                .is_some_and(|b| b.desc.dual_source());
        if dual_source
            && !self
                .backend
                .caps()
                .features
                .contains(Features::DUAL_SOURCE_BLEND)
        {
            return not_implemented("dual-source blending without the back end's");
        }
        let target_samples = samples.min(u32::from(u8::MAX)) as u8;
        let indirect = matches!(range, Range::Indirect { .. });
        let pulled = self
            .ctx
            .layout
            .as_ref()
            .filter(|l| !l.pulled.is_empty())
            .map_or(0, |l| l.id);
        // Depth clamped in the programs where the back end clips it alone (`DepthClampInShader`):
        // their added value takes the location past both stages' own.
        let clamps = !self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc
            .depth_clip
            && !self.backend.caps().features.contains(Features::DEPTH_CLAMP);
        // Clip distances the back end lacks, applied by the pixel program
        // (`ClipDistanceInShader`), passed at the location the last stage before the rasteriser
        // (the vertex program, the domain program behind a hull shader, or a geometry stage's
        // pass-through) was made with.
        let tessellated = self.ctx.hs.is_some() || self.ctx.ds.is_some();
        let geometry_stage = self
            .ctx
            .gs
            .as_ref()
            .and_then(|g| g.stage.as_ref())
            .map(|(s, _)| s.clone());
        let feeding = match (&geometry_stage, &self.ctx.ds) {
            (Some(g), _) => g.clone(),
            (None, Some(ds)) if tessellated => ds.clone(),
            _ => vs.clone(),
        };
        let clip = feeding.clip;
        // A geometry stage naming layers passes the layer named to the pixel program after the
        // linked values (`LAYER_FROM_STAGE`).
        let layer_named = geometry_stage
            .as_ref()
            .is_some_and(|g| crate::shader::names_layer(&g.reflection.outputs));
        if aa_lines {
            if !self.backend.caps().features.contains(Features::COMPUTE) {
                return not_implemented(LINES_WITHOUT_COMPUTE);
            }
            match &ps {
                None => {
                    return not_implemented(LINES_WITHOUT_PIXEL_SHADER);
                }
                Some(p)
                    if p.reflection
                        .inputs
                        .iter()
                        .any(|i| i.system == Some(SystemValue::FrontFacing)) =>
                {
                    return not_implemented(LINES_FACING);
                }
                Some(_) => {}
            }
        }
        let linked = if clamps || clip.is_some() || layer_named || aa_lines {
            let at = crate::shader::linked_location(
                feeding
                    .reflection
                    .outputs
                    .iter()
                    .chain(ps.iter().flat_map(|p| &p.reflection.inputs)),
            );
            if clip.is_some_and(|(made, _)| made != at) {
                return not_implemented(
                    "a pixel shader reading past its vertex shader's outputs, under clip distances in the program",
                );
            }
            at
        } else {
            0
        };
        let clip_distances = clip.map_or(0, |c| c.1);
        // Behind a hull shader or a geometry stage a later program passes the clip distances.
        let vertex_clip = if tessellated || geometry_stage.is_some() {
            0
        } else {
            clip_distances
        };
        let shader_viewport = self.viewport_in_shader();
        if shader_viewport.is_some() {
            self.backend.slow_paths().hit(SlowPath::ViewportInShader);
        }
        let vk = StageVariant {
            viewport_adjust: shader_viewport.is_some(),
            from_arguments: indirect && vs.helpers.size > 0,
            pulled,
            linked: if vertex_clip == 0 && !clamps {
                0
            } else {
                linked
            },
            depth_clamp: clamps,
            clip_distances: vertex_clip,
            ..self.stage_variant(VERTEX, &vs, target_samples)
        };
        // Without a pixel shader, the pixel program writing nothing applies these
        // (`Context::pixel_stage`).
        let unshaded = StageVariant {
            depth_bias,
            linked: if clamps || clip.is_some() { linked } else { 0 },
            depth_clamp: clamps,
            clip_distances,
            ..StageVariant::default()
        };
        let pk = ps.as_ref().map_or(unshaded, |p| StageVariant {
            dual_source,
            depth_bias,
            linked,
            depth_clamp: clamps,
            clip_distances,
            layer_from_stage: layer_named,
            line_coverage: aa_lines,
            early_tests: early,
            retyped,
            retyped_formats,
            ..self.stage_variant(PIXEL, p, target_samples)
        });
        if clip.is_some() {
            self.backend
                .slow_paths()
                .hit(SlowPath::ClipDistanceInShader);
        }
        let pk = if early || clip.is_some() {
            if early {
                self.backend.slow_paths().hit(SlowPath::EarlyTestsInShader);
            }
            StageVariant {
                samples: target_samples,
                ..pk
            }
        } else {
            pk
        };
        if clamps {
            self.backend.slow_paths().hit(SlowPath::DepthClampInShader);
        }
        if depth_bias {
            self.backend.slow_paths().hit(SlowPath::DepthBiasInShader);
        }
        self.count_sampling_in_shader(&[&vk, &pk]);
        if vk.from_arguments {
            self.backend
                .slow_paths()
                .hit(SlowPath::DrawBaseFromArguments);
        }
        if vk.pulled != 0 {
            for path in self.ctx.layout.iter().flat_map(|l| &l.pulled_paths) {
                self.backend.slow_paths().hit(*path);
            }
        }
        if vs
            .typed
            .iter()
            .chain(ps.iter().flat_map(|p| &p.typed))
            .any(|t| t.buffer)
        {
            self.backend
                .slow_paths()
                .hit(SlowPath::TexelBufferAsStorage);
        }
        if self.ctx.hs.is_some() || self.ctx.ds.is_some() {
            return self.draw_tessellated(
                &vs,
                vk,
                ps.as_ref(),
                pk,
                masked.as_ref(),
                range,
                first_instance,
                instances,
            );
        }
        let gs = match self.ctx.gs.clone() {
            None if vs.culled || aa_lines => Some(self.no_geometry()),
            gs => gs,
        };
        if let Some(gs) = gs {
            let topology = match geometry::input_topology(self.ctx.topology) {
                Some(t) => t,
                None if self.ctx.topology == topology::UNDEFINED => return Ok(()),
                None => return not_implemented("patches without hull and domain shaders"),
            };
            let drawn = self.draw_staged(
                &gs,
                &vs,
                vk,
                ps.as_ref(),
                pk,
                masked.as_ref(),
                range,
                first_instance,
                instances,
                topology,
            )?;
            if !drawn {
                return Ok(());
            }
        }
        let (program, helpers) = self.program(&vs, vk, ps.as_ref(), pk)?;
        if !helpers[0].pulled.is_empty() {
            let storage = vs
                .reflection
                .bindings
                .iter()
                .filter(|b| b.kind == SlotKind::Storage)
                .count()
                + helpers[0].pulled.len();
            if storage > self.backend.caps().limits.storage_buffers_per_stage as usize {
                return not_implemented(
                    "more vertex inputs fetched in the program than the back end's storage bindings",
                );
            }
        }
        let vertex_layout = self.vertex_layout(&vs)?;
        // Direct3D reads zeros from a slot the layout reads that has no buffer: the zero buffer, at
        // stride zero.
        let empty_streams = self.ctx.layout.as_ref().is_some_and(|l| {
            l.elements
                .iter()
                .any(|e| self.ctx.vertex_buffers[e.slot as usize].is_none())
        });
        let zero_stream = if empty_streams {
            Some(self.zero_buffer()?)
        } else {
            None
        };
        self.grow_constants()?;
        self.refresh_windows()?;
        if vk.pulled != self.ctx.pulled_set {
            self.ctx.sets_dirty[VERTEX] = true;
        }
        self.update_set(VERTEX, &helpers[0].pulled, &helpers[0].relocated)?;
        self.ctx.pulled_set = vk.pulled;
        self.update_set(PIXEL, &[], &helpers[1].relocated)?;
        for r in helpers.iter().flat_map(|h| &h.relocated) {
            self.backend.slow_paths().hit(match r.read_as {
                ConstantsReadAs::Storage => SlowPath::ConstantsAsStorage,
                ConstantsReadAs::Texture => SlowPath::ConstantsAsTexture,
            });
        }
        // SV_VertexID and SV_InstanceID leave out the draw's bases, which the common form's indices
        // include: a vertex program reading them takes the bases from its helper constants or from
        // the indirect arguments; level-of-detail biases are helper constants too.
        let reads_helpers = helpers.iter().any(|h| h.size > 0 || !h.merged.is_empty());
        if reads_helpers {
            let (base, arguments) = match range {
                Range::Vertices { start, .. } => (start, None),
                Range::Indexed { base, .. } => (base as u32, None),
                Range::Indirect { indexed, offset } => (0, args.map(|b| (b, offset, indexed))),
            };
            // One primitive's run gives the API draw's bases.
            let bases = self.ctx.early_run.unwrap_or([base, first_instance]);
            self.write_helpers(&helpers, bases, arguments)?;
        }
        if !self.queries.statistics.is_empty() {
            self.count_draw_statistics(range, instances, args)?;
        }

        let indexed = matches!(
            range,
            Range::Indexed { .. } | Range::Indirect { indexed: true, .. }
        );
        let args = args.map(|b| b.gpu.borrow());
        let (topology, gpu_range, generated) = match plain_topology(self.ctx.topology) {
            Some(t) => {
                let r = match range {
                    Range::Indirect { indexed, offset } => {
                        let args = &args.as_ref().expect("indirect arguments").handle;
                        let offset = u64::from(offset);
                        if indexed {
                            DrawRange::IndexedIndirect { args, offset }
                        } else {
                            DrawRange::Indirect { args, offset }
                        }
                    }
                    Range::Vertices { count, start } => DrawRange::Vertices {
                        first: start,
                        count,
                        first_instance,
                        instances,
                    },
                    Range::Indexed { count, start, base } => DrawRange::Indexed {
                        first: start,
                        count,
                        base_vertex: base,
                        first_instance,
                        instances,
                    },
                };
                (t, r, None)
            }
            None => {
                let Range::Vertices { count, start } = range else {
                    return not_implemented(
                        "indexed or indirect adjacency draws without a geometry shader",
                    );
                };
                let Some((t, list)) = adjacency_indices(self.ctx.topology, count) else {
                    return not_implemented("a topology without a geometry or hull shader");
                };
                self.backend.slow_paths().hit(SlowPath::AdjacencyDropped);
                let bytes: Vec<u8> = list.iter().flat_map(|i| i.to_le_bytes()).collect();
                let mut offsets = [0u64];
                let ring = self.backend.write_buffer_ring(
                    wcontainers_gpu::RingKind::Indices,
                    &[&bytes],
                    4,
                    &mut offsets,
                );
                let ring = self.gpu(ring)?;
                let r = DrawRange::Indexed {
                    first: (offsets[0] / 4) as u32,
                    count: list.len() as u32,
                    base_vertex: start as i32,
                    first_instance,
                    instances,
                };
                (t, r, Some(ring))
            }
        };
        // Alpha-antialiased lines reach here only beside stream output without a geometry stage or
        // from an adjacency topology, which the line route does not take; with multisampling
        // Direct3D draws lines by the quadrilateral rule, here by the back end's (a listed loss,
        // plan 4.12).
        if matches!(topology, Topology::LineList | Topology::LineStrip)
            && self.lines_alpha_antialiased()
        {
            return not_implemented(ALPHA_ANTIALIASED_LINES);
        }
        let nothing = matches!(
            gpu_range,
            DrawRange::Indexed { count: 0, .. } | DrawRange::Vertices { count: 0, .. }
        );
        if nothing || self.ctx.viewports.is_empty() || (indexed && self.ctx.index.is_none()) {
            return Ok(());
        }
        // A pass without attachments is as large as the viewport of its first draw.
        if self.ctx.pass_open
            && let Some(open) = self.ctx.empty_pass
        {
            let wanted = self.empty_size();
            if wanted.width > open.width || wanted.height > open.height {
                self.end_pass();
            }
        }
        let counting = if self.queries.counted.is_empty() {
            None
        } else {
            Some(self.prepare_counted(&vs, vk, ps.as_ref(), pk)?)
        };
        // A draw without a pixel shader invokes none.
        let pixel_counting = match &ps {
            Some(p) if !self.queries.statistics.is_empty() => {
                Some(self.prepare_pixel_counted(&vs, vk, p, samples)?)
            }
            _ => None,
        };
        let keys = self.program_keys(&vs, vk, ps.as_ref(), pk);
        self.count_combination(&vs, ps.as_ref(), keys, masked_key, topology, samples);
        let alpha_to_coverage = self
            .ctx
            .blend
            .as_ref()
            .is_some_and(|b| b.desc.alpha_to_coverage);
        let instances_step_by_one = self
            .ctx
            .layout
            .as_ref()
            .is_none_or(|l| l.instance_steps.iter().flatten().all(|&s| s == 1));

        // Builds the draw's call and gives it to `$then`: once to the occlusion counter, which
        // keeps a counted draw before its pass opens, and once to the back end.
        macro_rules! with_call {
            ($call:ident => $then:expr) => {{
                let ctx = &self.ctx;
                // Fixed arrays: no allocation per draw (Direct3D has 32 vertex slots).
                let vertex_handles: [_; VERTEX_SLOTS] = std::array::from_fn(|slot| {
                    ctx.vertex_buffers[slot]
                        .as_ref()
                        .map(|(b, _, offset)| (b.gpu.borrow(), *offset))
                });
                let read = |slot: usize| {
                    ctx.layout
                        .as_ref()
                        .is_some_and(|l| l.elements.iter().any(|e| e.slot as usize == slot))
                };
                // Only the slots the back end fetches: those the vertex program fetches past its
                // streams are bound as storage.
                let vertex_buffers: [Option<VertexBinding<'_, B>>; VERTEX_SLOTS] =
                    std::array::from_fn(|slot| match (&vertex_handles[slot], &zero_stream) {
                        (Some((b, offset)), _) if read(slot) => Some(VertexBinding {
                            buffer: &b.handle,
                            offset: u64::from(*offset),
                        }),
                        (None, Some(zero)) if read(slot) => Some(VertexBinding {
                            buffer: zero,
                            offset: 0,
                        }),
                        _ => None,
                    });
                let index_handle = ctx.index.as_ref().map(|(b, _, _)| b.gpu.borrow());
                let used = vertex_buffers
                    .iter()
                    .rposition(Option::is_some)
                    .map_or(0, |i| i + 1);
                let index = match (&generated, &ctx.index) {
                    (Some(ring), _) => Some(IndexBinding {
                        buffer: ring,
                        offset: 0,
                        format: IndexFormat::U32,
                    }),
                    (None, Some((_, f, offset))) if indexed => Some(IndexBinding {
                        buffer: &index_handle.as_ref().expect("an index buffer").handle,
                        offset: u64::from(*offset),
                        format: *f,
                    }),
                    _ => None,
                };
                // Without a shader choosing one (none is served), Direct3D draws through the first viewport
                // and scissor rectangle; with no rectangle set, the scissor test passes nothing.
                let v = shader_viewport.as_ref().map_or(ctx.viewports[0], |a| a.viewport);
                let viewport = GpuViewport {
                    x: v.x.to_bits(),
                    y: v.y.to_bits(),
                    width: v.width.to_bits(),
                    height: v.height.to_bits(),
                    min_depth: v.min_depth.to_bits(),
                    max_depth: v.max_depth.to_bits(),
                };
                let blend = ctx.blend.as_ref().unwrap_or(&self.defaults.blend);
                let depth_stencil = &self.drawn_depth_stencil();
                let raster = ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
                let no_rect = [ScissorRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }];
                let scissors: &[ScissorRect] = match ctx.scissors.first() {
                    _ if !raster.desc.scissor => &[],
                    Some(rect) => std::slice::from_ref(rect),
                    None => &no_rect,
                };
                // A viewport mapped in the program draws only the pixels the API's covers.
                let mut covered = Vec::new();
                let scissors: &[ScissorRect] = match &shader_viewport {
                    Some(a) => {
                        wcontainers_shaders::front::cut_scissors(scissors, a.pixels, &mut covered);
                        &covered
                    }
                    None => scissors,
                };
                // A fitted pass draws only where every attachment is.
                let mut cut = Vec::new();
                let scissors: &[ScissorRect] = match ctx.fitted {
                    Some(area) => {
                        wcontainers_shaders::front::cut_scissors(scissors, area, &mut cut);
                        &cut
                    }
                    None => scissors,
                };
                let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
                bindings[0] = ctx.sets[VERTEX].as_ref().map(|k| &k.handle);
                bindings[1] = ctx.sets[PIXEL].as_ref().map(|k| &k.handle);
                if reads_helpers {
                    bindings[HELPER_SET as usize] =
                        self.helper_blocks.as_ref().map(|h| &h.set.handle);
                }
                let strip = matches!(topology, Topology::LineStrip | Topology::TriangleStrip);
                let call = DrawCall {
                    program: &program,
                    blend: &blend.gpu.handle,
                    depth_stencil: &depth_stencil.gpu.handle,
                    raster: masked.as_ref().unwrap_or(&raster.gpu.handle),
                    vertex_layout: &vertex_layout,
                    bindings,
                    vertex_buffers: &vertex_buffers[..used],
                    index,
                    topology,
                    restart: strip && indexed,
                    viewports: std::slice::from_ref(&viewport),
                    scissors,
                    blend_constant: ctx.blend_factor.map(f32::to_bits),
                    stencil_reference: [ctx.stencil_ref as u8; 2],
                    immediates: &[],
                    range: same_range(&gpu_range),
                };
                let $call = call;
                $then
            }};
        }
        if let Some((counting, writes)) = &counting {
            let counted = CountedDraw {
                counting,
                alpha_to_coverage,
                writes_depth_stencil: *writes,
                instances_step_by_one,
            };
            let kept = with_call!(call => self.counter.draw(
                &mut self.backend,
                &mut self.passes,
                &call,
                &counted,
            ));
            self.gpu(kept)?;
        }
        if let Some((counting, off)) = &pixel_counting {
            let counted = CountedDraw {
                counting,
                alpha_to_coverage: false,
                writes_depth_stencil: false,
                instances_step_by_one,
            };
            let kept = with_call!(call => self.pixel_counter.draw(
                &mut self.backend,
                &mut self.passes,
                &DrawCall { depth_stencil: &off.gpu.handle, ..call },
                &counted,
            ));
            self.gpu(kept)?;
        }
        let twin_writes = self.twins_in(&[VERTEX, PIXEL], PIXEL);
        let through = self.through_in(PIXEL);
        self.split_scope();
        self.open_pass();
        if !self.open_stretch() {
            // The pass's occlusion set has no slot left: the draw opens the next pass on another.
            self.end_pass();
            self.open_pass();
            self.open_stretch();
        }
        with_call!(call => self.backend.draw(&call));
        if through {
            self.through_out(PIXEL);
        }
        if twin_writes {
            self.twins_out(PIXEL);
        }
        Ok(())
    }

    /// `CopyResource`: two resources of one shape.
    pub fn copy_resource(&mut self, dst: &Resource<B>, src: &Resource<B>) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        match (dst, src) {
            (Resource::Buffer(d), Resource::Buffer(s)) => {
                if d.desc.byte_width != s.desc.byte_width || Rc::ptr_eq(d, s) {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two resources of one size",
                    ));
                }
                self.settle();
                d.writes.set(d.writes.get() + 1);
                self.backend.copy_buffer(
                    &s.handle(),
                    0,
                    &d.handle(),
                    0,
                    u64::from(s.desc.byte_width),
                );
                self.buffer_written(d)
            }
            (Resource::Texture(d), Resource::Texture(s)) => {
                let (a, b) = (&d.desc, &s.desc);
                // Measured in blocks: a block-compressed texture copies with an uncompressed one
                // whose texels are its blocks.
                let shape = |t: &TextureDesc, f: Format| {
                    let (bw, bh, _) = FORMAT_FACTS[f as usize].block;
                    (
                        t.dimension,
                        t.width.div_ceil(bw),
                        t.height.div_ceil(bh),
                        t.depth,
                        t.mip_levels,
                        t.array_size,
                        t.sample_count,
                    )
                };
                if shape(a, d.format) != shape(b, s.format) || Rc::ptr_eq(d, s) {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two textures of one shape",
                    ));
                }
                copy_formats(a.format, b.format, self.feature_level())?;
                for sub in 0..a.mip_levels * a.array_size {
                    self.copy_subresource_region(dst, sub, [0; 3], src, sub, None)?;
                }
                Ok(())
            }
            _ => Err(Error::InvalidArg(
                "CopyResource between a buffer and a texture",
            )),
        }
    }

    pub fn copy_subresource_region(
        &mut self,
        dst: &Resource<B>,
        dst_sub: u32,
        at: [u32; 3],
        src: &Resource<B>,
        src_sub: u32,
        src_box: Option<&Box3>,
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        match (dst, src) {
            (Resource::Buffer(d), Resource::Buffer(s)) => {
                let (from, to) = src_box.map_or((0, s.desc.byte_width), |b| (b.left, b.right));
                if to <= from
                    || to > s.desc.byte_width
                    || at[0] + (to - from) > d.desc.byte_width
                    || src_box.is_some_and(Box3::empty)
                {
                    return Ok(());
                }
                self.settle();
                d.writes.set(d.writes.get() + 1);
                self.backend.copy_buffer(
                    &s.handle(),
                    u64::from(from),
                    &d.handle(),
                    u64::from(at[0]),
                    u64::from(to - from),
                );
                self.buffer_written(d)
            }
            (Resource::Texture(d), Resource::Texture(s)) => {
                copy_formats(d.desc.format, s.desc.format, self.feature_level())?;
                // Multisampled textures are copied from 10_1; below it Direct3D drops the copy
                // (measured on the RTX 3080).
                if self.feature_level() < desc::level::L10_1
                    && (d.desc.sample_count > 1 || s.desc.sample_count > 1)
                {
                    return Ok(());
                }
                // YUY2's pixel pairs are its texels; Direct3D drops an odd box or place.
                let pairs = d.desc.format == format::YUY2;
                let Some(src_box) = pixel_box(s.desc.format == format::YUY2, src_box) else {
                    return Ok(());
                };
                if pairs && !at[0].is_multiple_of(2) {
                    return Ok(());
                }
                let at = [if pairs { at[0] / 2 } else { at[0] }, at[1], at[2]];
                let src_box = src_box.as_ref();
                let (Some((src_mip, src_layer)), Some((dst_mip, dst_layer))) =
                    (s.subresource(src_sub), d.subresource(dst_sub))
                else {
                    return Err(Error::InvalidArg("subresource past the resource"));
                };
                let se = s.mip_extent(src_mip);
                let b = src_box.copied().unwrap_or(Box3 {
                    left: 0,
                    top: 0,
                    front: 0,
                    right: se.width,
                    bottom: se.height,
                    back: se.depth_or_layers,
                });
                let de = d.mip_extent(dst_mip);
                if b.right <= b.left
                    || b.bottom <= b.top
                    || b.back <= b.front
                    || b.right > se.width
                    || b.bottom > se.height
                    || b.back > se.depth_or_layers
                {
                    return Ok(());
                }
                let extent = Extent {
                    width: b.right - b.left,
                    height: b.bottom - b.top,
                    depth_or_layers: b.back - b.front,
                };
                // The destination's extent covers the same blocks; a block-compressed level is
                // as large as its whole blocks.
                let ((sbw, sbh, _), (dbw, dbh, _)) = (
                    FORMAT_FACTS[s.format as usize].block,
                    FORMAT_FACTS[d.format as usize].block,
                );
                if at[0] + extent.width.div_ceil(sbw) * dbw > de.width.div_ceil(dbw) * dbw
                    || at[1] + extent.height.div_ceil(sbh) * dbh > de.height.div_ceil(dbh) * dbh
                    || at[2] + extent.depth_or_layers > de.depth_or_layers
                {
                    return Ok(());
                }
                // Subresources of depth-stencil textures are copied whole; Direct3D drops a part.
                if (s.desc.bind_flags | d.desc.bind_flags) & desc::bind::DEPTH_STENCIL != 0
                    && (at != [0; 3]
                        || (extent.width, extent.height) != (se.width, se.height)
                        || (de.width, de.height) != (se.width, se.height))
                {
                    return Ok(());
                }
                // NV12 copies whole chroma pairs; Direct3D drops a box or place at an odd texel.
                let planes = d.chroma.as_ref().zip(s.chroma.as_ref());
                if planes.is_some() && !even(&b, at) {
                    return Ok(());
                }
                self.settle();
                let (x, y, z) = (b.left, b.top, b.front);
                self.backend.copy_texture(
                    &s.region(src_mip, src_layer, Origin { x, y, z }),
                    &d.region(
                        dst_mip,
                        dst_layer,
                        Origin {
                            x: at[0],
                            y: at[1],
                            z: at[2],
                        },
                    ),
                    extent,
                );
                if let Some((dc, sc)) = planes {
                    self.backend.copy_texture(
                        &sc.region(
                            src_mip,
                            src_layer,
                            Origin {
                                x: x / 2,
                                y: y / 2,
                                z,
                            },
                        ),
                        &dc.region(
                            dst_mip,
                            dst_layer,
                            Origin {
                                x: at[0] / 2,
                                y: at[1] / 2,
                                z: at[2],
                            },
                        ),
                        half(extent),
                    );
                }
                self.texture_written(d, dst_sub)
            }
            _ => Err(Error::InvalidArg("a copy between a buffer and a texture")),
        }
    }

    /// `ResolveSubresource`: the samples of a multisampled subresource averaged into a
    /// single-sampled subresource of its size, both read as `format`.
    pub fn resolve_subresource(
        &mut self,
        dst: &Resource<B>,
        dst_sub: u32,
        src: &Resource<B>,
        src_sub: u32,
        format: u32,
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        let (Resource::Texture(d), Resource::Texture(s)) = (dst, src) else {
            return Err(Error::InvalidArg("a resolve between buffers"));
        };
        let (dm, sm) = (d.desc.mip_levels, s.desc.mip_levels);
        if s.desc.sample_count < 2
            || d.desc.sample_count != 1
            || dst_sub >= dm * d.desc.array_size
            || src_sub >= sm * s.desc.array_size
        {
            return Err(Error::InvalidArg(
                "a resolve of a multisampled subresource into a single-sampled one",
            ));
        }
        let (de, se) = (d.mip_extent(dst_sub % dm), s.mip_extent(src_sub % sm));
        if (de.width, de.height) != (se.width, se.height) {
            return Err(Error::InvalidArg(
                "a resolve between subresources of two sizes",
            ));
        }
        // A swap chain's buffer is also viewed as its sRGB twin, as DXGI allows its targets.
        let named = |t: &Texture<B>| {
            if t.chain != ChainBuffer::None && format::srgb_twin(t.desc.format) == Some(format) {
                format
            } else {
                t.desc.format
            }
        };
        let (into, _) = format::view_format(format, named(d), d.format)?;
        format::view_format(format, named(s), s.format)?;
        if !resolvable(into) {
            return Err(Error::InvalidArg(
                "a resolve in a format that does not average",
            ));
        }
        self.settle();
        self.backend.resolve(
            &s.region(src_sub % sm, src_sub / sm, Origin { x: 0, y: 0, z: 0 }),
            &d.region(dst_sub % dm, dst_sub / dm, Origin { x: 0, y: 0, z: 0 }),
            into,
        );
        self.texture_written(d, dst_sub)
    }

    /// `UpdateSubresource`: `bytes` is the stand-in's copy of the program's data.
    pub fn update_subresource(
        &mut self,
        dst: &Resource<B>,
        sub: u32,
        dst_box: Option<&Box3>,
        bytes: &[u8],
        row_pitch: u32,
        slice_pitch: u32,
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        self.before_write();
        match dst {
            Resource::Buffer(b) => {
                if b.desc.usage != usage::DEFAULT {
                    return Err(Error::InvalidArg("UpdateSubresource needs default usage"));
                }
                let (from, to) = dst_box.map_or((0, b.desc.byte_width), |x| (x.left, x.right));
                if to <= from || to > b.desc.byte_width || dst_box.is_some_and(Box3::empty) {
                    return Ok(());
                }
                let bytes = &bytes[..(to - from) as usize];
                b.writes.set(b.writes.get() + 1);
                if from == 0 && to == b.desc.byte_width {
                    // A whole write discards; a grown buffer's bytes past the program's stay zero.
                    if b.grown.get() {
                        let mut padded = bytes.to_vec();
                        padded.resize(b.size.get() as usize, 0);
                        self.backend
                            .write_buffer(&b.handle(), 0, &padded, WriteMode::Discard);
                    } else {
                        self.backend
                            .write_buffer(&b.handle(), 0, bytes, WriteMode::Discard);
                    }
                } else {
                    self.backend.write_buffer(
                        &b.handle(),
                        u64::from(from),
                        bytes,
                        WriteMode::Ordered,
                    );
                }
                Ok(())
            }
            Resource::Texture(t) => {
                if t.desc.usage != usage::DEFAULT {
                    return Err(Error::InvalidArg("UpdateSubresource needs default usage"));
                }
                let Some((mip, layer)) = t.subresource(sub) else {
                    return Err(Error::InvalidArg("subresource past the resource"));
                };
                let Some(dst_box) = pixel_box(t.desc.format == format::YUY2, dst_box) else {
                    return Ok(());
                };
                let e = t.mip_extent(mip);
                let b = dst_box.unwrap_or(Box3 {
                    right: e.width,
                    bottom: e.height,
                    back: e.depth_or_layers,
                    ..Default::default()
                });
                if b.right <= b.left
                    || b.bottom <= b.top
                    || b.back <= b.front
                    || b.right > e.width
                    || b.bottom > e.height
                    || b.back > e.depth_or_layers
                {
                    return Ok(());
                }
                // NV12 is written by whole chroma pairs, as copies are.
                if t.chroma.is_some() && !even(&b, [b.left, b.top, b.front]) {
                    return Ok(());
                }
                self.settle();
                let origin = Origin {
                    x: b.left,
                    y: b.top,
                    z: b.front,
                };
                let extent = Extent {
                    width: b.right - b.left,
                    height: b.bottom - b.top,
                    depth_or_layers: b.back - b.front,
                };
                let data = SubresourceData {
                    bytes,
                    row_pitch,
                    slice_pitch,
                };
                self.write_texels(t.format, &t.region(mip, layer, origin), extent, &data);
                if let Some(chroma) = &t.chroma {
                    // The chroma rows follow the luma's at the same pitch.
                    let at = (row_pitch * extent.height) as usize;
                    let origin = Origin {
                        x: origin.x / 2,
                        y: origin.y / 2,
                        z: origin.z,
                    };
                    self.write_texels(
                        chroma.format,
                        &chroma.region(mip, layer, origin),
                        half(extent),
                        &SubresourceData {
                            bytes: &bytes[at..],
                            ..data
                        },
                    );
                }
                Ok(())
            }
        }
    }

    pub fn map(
        &mut self,
        resource: &Resource<B>,
        sub: u32,
        kind: u32,
        flags: u32,
    ) -> Result<Mapped> {
        self.live()?;
        let reads = matches!(kind, map::READ | map::READ_WRITE);
        let writes = kind != map::READ;
        let wait = flags & map::DO_NOT_WAIT == 0;
        let mapped = self.map_any(resource, sub, kind, reads, writes, wait);
        if let Err(e @ Error::DeviceRemoved) = mapped {
            self.lose(e);
        }
        // A read back the back end failed is a loss it reports through its status.
        self.live()?;
        mapped
    }

    fn map_any(
        &mut self,
        resource: &Resource<B>,
        sub: u32,
        kind: u32,
        reads: bool,
        writes: bool,
        wait: bool,
    ) -> Result<Mapped> {
        match resource {
            Resource::Buffer(b) => {
                let d = &b.desc;
                let allowed = match kind {
                    map::WRITE_DISCARD | map::WRITE_NO_OVERWRITE => d.usage == usage::DYNAMIC,
                    _ => {
                        d.usage == usage::STAGING
                            && (!reads || d.cpu_access_flags & desc::cpu::READ != 0)
                            && (!writes || d.cpu_access_flags & desc::cpu::WRITE != 0)
                    }
                };
                if sub != 0 || !allowed {
                    return Err(Error::InvalidArg("the resource cannot be mapped this way"));
                }
                if d.usage == usage::STAGING {
                    let handle = b.handle();
                    let src = ReadSource::Buffer {
                        buffer: &handle,
                        offset: 0,
                        size: u64::from(d.byte_width),
                    };
                    let mut staged = b.staged.borrow_mut();
                    let mut shadow = b.shadow.borrow_mut();
                    let shadow = shadow_bytes(&mut shadow);
                    self.refresh(&mut staged, &src, None, shadow, reads, wait)?;
                }
                self.ctx.mapped.push((Rc::as_ptr(b) as usize, 0, kind));
                let data = b.shadow.borrow().clone();
                Ok(Mapped {
                    data,
                    row_pitch: d.byte_width,
                    depth_pitch: d.byte_width,
                })
            }
            Resource::Texture(t) => {
                let d = &t.desc;
                let ok = match d.usage {
                    usage::STAGING => {
                        (!reads || d.cpu_access_flags & desc::cpu::READ != 0)
                            && (!writes || d.cpu_access_flags & desc::cpu::WRITE != 0)
                            && matches!(kind, map::READ | map::WRITE | map::READ_WRITE)
                    }
                    usage::DYNAMIC => kind == map::WRITE_DISCARD,
                    _ => false,
                };
                let whole = t.whole(sub).filter(|_| ok);
                let Some((region, extent)) = whole else {
                    return Err(Error::InvalidArg("the resource cannot be mapped this way"));
                };
                let (row, rows, _) = t.packed(region.mip);
                let src = ReadSource::Texture { region, extent };
                let second = t.second_region(sub);
                let mut shadow = t.shadow.borrow_mut();
                let mut staged = t.staged.borrow_mut();
                let s = sub as usize;
                // A dynamic texture's shadow is the program's to fill; nothing is read back.
                if d.usage == usage::STAGING {
                    let second = second
                        .map(|(region, extent, how)| (ReadSource::Texture { region, extent }, how));
                    let second = second.as_ref().map(|(s, how)| (s, *how));
                    let bytes = shadow_bytes(&mut shadow[s]);
                    self.refresh(&mut staged[s], &src, second, bytes, reads, wait)?;
                }
                self.ctx.mapped.push((Rc::as_ptr(t) as usize, sub, kind));
                Ok(Mapped {
                    data: shadow[s].clone(),
                    row_pitch: row,
                    depth_pitch: row * rows,
                })
            }
        }
    }

    /// After a copy into `b`: a staging buffer the program can read starts its read back now (plan
    /// 4.6), so a map frames later finds it ready.
    fn buffer_written(&mut self, b: &Buffer<B>) -> Result<()> {
        if b.desc.usage != usage::STAGING {
            return Ok(());
        }
        let state = if b.desc.cpu_access_flags & desc::cpu::READ != 0 {
            let handle = b.handle();
            let r = self.backend.read_back(&ReadSource::Buffer {
                buffer: &handle,
                offset: 0,
                size: u64::from(b.desc.byte_width),
            });
            Staged::Reading(self.gpu(r)?, None)
        } else {
            Staged::Written
        };
        *b.staged.borrow_mut() = state;
        Ok(())
    }

    /// After a copy into subresource `sub` of `t`, as [`Self::buffer_written`].
    fn texture_written(&mut self, t: &Texture<B>, sub: u32) -> Result<()> {
        if t.desc.usage != usage::STAGING {
            return Ok(());
        }
        let state = if t.desc.cpu_access_flags & desc::cpu::READ != 0 {
            let (region, extent) = t.whole(sub).expect("a subresource the copy checked");
            let r = self
                .backend
                .read_back(&ReadSource::Texture { region, extent });
            let r = self.gpu(r)?;
            let second = match t.second_region(sub) {
                Some((region, extent, _)) => {
                    let s = self
                        .backend
                        .read_back(&ReadSource::Texture { region, extent });
                    Some(self.gpu(s)?)
                }
                None => None,
            };
            Staged::Reading(r, second)
        } else {
            Staged::Written
        };
        t.staged.borrow_mut()[sub as usize] = state;
        Ok(())
    }

    /// Brings a staging subresource's shadow up to the GPU's bytes before a map hands it out: the
    /// read back started at the copy, or one started now from `src` (and from `second`, the
    /// stencil aspect or NV12's chroma plane, see [`Second`]); a write-only map that needs it is
    /// `StagingRefreshed`. Without `wait`, a read back not yet done answers `StillDrawing` (after
    /// submitting, so it gets done).
    fn refresh(
        &mut self,
        staged: &mut Staged<B>,
        src: &ReadSource<'_, B>,
        second: Option<(&ReadSource<'_, B>, Second)>,
        shadow: &mut [u8],
        reads: bool,
        wait: bool,
    ) -> Result<()> {
        let (rb, stencil_rb) = match std::mem::replace(staged, Staged::Current) {
            Staged::Current => return Ok(()),
            Staged::Written => {
                self.settle();
                let r = self.backend.read_back(src);
                let r = self.gpu(r)?;
                let s = match second {
                    Some((src, _)) => {
                        let s = self.backend.read_back(src);
                        Some(self.gpu(s)?)
                    }
                    None => None,
                };
                (r, s)
            }
            Staged::Reading(rb, s) => (rb, s),
        };
        let pending = self.backend.read_state(&rb) == ReadState::Pending
            || stencil_rb
                .as_ref()
                .is_some_and(|s| self.backend.read_state(s) == ReadState::Pending);
        if pending {
            if !wait {
                *staged = Staged::Reading(rb, stencil_rb);
                self.flush();
                return Err(Error::StillDrawing);
            }
            self.backend.slow_paths().hit(SlowPath::ReadBackWait);
            self.backend.wait_read(&rb);
            if let Some(s) = &stencil_rb {
                self.backend.wait_read(s);
            }
        }
        if !reads {
            self.backend.slow_paths().hit(SlowPath::StagingRefreshed);
        }
        match (stencil_rb, second.map(|s| s.1)) {
            (None, _) => self.backend.finish_read(rb, shadow),
            (Some(s), Some(Second::Chroma)) => {
                // The luma's rows, then the chroma's (as many bytes a row, half the rows).
                let (luma, chroma) = shadow.split_at_mut(shadow.len() / 3 * 2);
                self.backend.finish_read(rb, luma);
                self.backend.finish_read(s, chroma);
            }
            (Some(s), how) => {
                // Direct3D's layouts: D24_UNORM_S8_UINT is depth in the low 24 bits of a word and
                // stencil in its high byte; D32_FLOAT_S8X24_UINT is the float, the stencil byte and
                // three unused bytes. The back end gives depth in four bytes and stencil in one.
                let texel = match how {
                    Some(Second::Stencil(texel)) => texel,
                    _ => 4,
                };
                let n = shadow.len() / texel;
                let mut depth = vec![0u8; n * 4];
                let mut stencil = vec![0u8; n];
                self.backend.finish_read(rb, &mut depth);
                self.backend.finish_read(s, &mut stencil);
                for i in 0..n {
                    let t = &mut shadow[i * texel..(i + 1) * texel];
                    if texel == 4 {
                        t[..3].copy_from_slice(&depth[i * 4..i * 4 + 3]);
                        t[3] = stencil[i];
                    } else {
                        t[..4].copy_from_slice(&depth[i * 4..i * 4 + 4]);
                        t[4] = stencil[i];
                        t[5..].fill(0);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn unmap(&mut self, resource: &Resource<B>, sub: u32) {
        let address = match resource {
            Resource::Buffer(b) => Rc::as_ptr(b) as usize,
            Resource::Texture(t) => Rc::as_ptr(t) as usize,
        };
        let Some(i) = self
            .ctx
            .mapped
            .iter()
            .position(|&(a, s, _)| a == address && s == sub)
        else {
            return;
        };
        let (_, _, kind) = self.ctx.mapped.remove(i);
        if kind == map::READ {
            return;
        }
        self.before_write();
        self.backend.slow_paths().hit(SlowPath::MappedUploadWhole);
        match resource {
            Resource::Buffer(b) => {
                b.writes.set(b.writes.get() + 1);
                let mode = match kind {
                    map::WRITE_DISCARD => WriteMode::Discard,
                    map::WRITE_NO_OVERWRITE => WriteMode::NoOverwrite,
                    _ => WriteMode::Ordered,
                };
                let mut shadow = b.shadow.borrow_mut();
                self.backend
                    .write_buffer(&b.handle(), 0, shadow_bytes(&mut shadow), mode);
            }
            Resource::Texture(t) => {
                let (region, extent) = t.whole(sub).expect("a subresource the map checked");
                let (row, rows, _) = t.packed(region.mip);
                self.end_pass();
                let mut shadows = t.shadow.borrow_mut();
                let shadow: &[u8] = shadow_bytes(&mut shadows[sub as usize]);
                let (stencil, texel) = match t.second_region(sub) {
                    None => {
                        self.backend.write_texture(
                            &region,
                            extent,
                            shadow,
                            row,
                            rows,
                            WriteMode::Ordered,
                        );
                        return;
                    }
                    Some((chroma, half, Second::Chroma)) => {
                        // The luma's rows, then the chroma's at as many bytes a row.
                        let luma = (row * extent.height) as usize;
                        let (a, b) = shadow.split_at(luma);
                        self.backend.write_texture(
                            &region,
                            extent,
                            a,
                            row,
                            extent.height,
                            WriteMode::Ordered,
                        );
                        self.backend.write_texture(
                            &chroma,
                            half,
                            b,
                            row,
                            half.height,
                            WriteMode::Ordered,
                        );
                        return;
                    }
                    Some((stencil, _, Second::Stencil(texel))) => (stencil, texel as u32),
                };
                // Each aspect of a format with depth and stencil apart, from Direct3D's layout
                // (see `refresh`): depth in four bytes per texel, stencil in one.
                let (depth, stencils) = split_depth_stencil(texel, shadow);
                let texels = row / texel;
                self.backend.write_texture(
                    &region,
                    extent,
                    &depth,
                    texels * 4,
                    rows,
                    WriteMode::Ordered,
                );
                self.backend.write_texture(
                    &stencil,
                    extent,
                    &stencils,
                    texels,
                    rows,
                    WriteMode::Ordered,
                );
            }
        }
    }

    /// `GenerateMips`: fills the view's levels below its first from the first, in its format. A
    /// texture made without mip generation is left alone, as Direct3D leaves it.
    pub fn generate_mips(&mut self, view: &View<B>) {
        if self.predicated_out() {
            return;
        }
        let t = &view.texture;
        if t.desc.misc_flags & desc::MISC_GENERATE_MIPS == 0 {
            return;
        }
        // Direct3D claims mip generation of no integer format and defines no result for one;
        // the lower levels are left as they are.
        if format::is_integer(view.values()) {
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| {
                eprintln!(
                    "{WARNING_PREFIX}: GenerateMips of an integer view, which Direct3D does not \
                     claim for its format: its levels are left as they are"
                );
            });
            return;
        }
        let v = &view.desc;
        let layers = v.first_layer..v.first_layer + v.layer_count;
        self.settle();
        let r = self.backend.generate_mips(
            &t.gpu.handle,
            &t.gpu_desc,
            view.values(),
            v.first_mip + 1..v.first_mip + v.mip_count,
            layers,
        );
        if let Err(e) = r {
            let _ = self.gpu::<()>(Err(e));
        }
    }

    /// `ClearState`: every slot and state back to its default. Recorded work is kept.
    pub fn clear_state(&mut self) {
        self.end_pass();
        let clears = std::mem::take(&mut self.ctx.clears);
        let mapped = std::mem::take(&mut self.ctx.mapped);
        self.ctx = ContextState::new();
        self.ctx.clears = clears;
        self.ctx.mapped = mapped;
        self.bury();
    }

    /// `Flush`: runs what is recorded.
    pub fn flush(&mut self) {
        self.settle();
        self.bury();
        let fence = self.backend.submit();
        self.submitted(fence);
        self.poll_removed();
    }

    /// `Flush` and a wait until the GPU has done it (`Flush1` signalling an event at the end).
    pub fn finish(&mut self) {
        self.flush();
        if let Some(fence) = self.queries.fence.take() {
            self.backend.wait_fence(&fence);
            self.queries.fence = Some(fence);
        }
        self.poll_removed();
    }
}

/// The same range again, for a draw's call built twice.
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

/// One element of `format` holding the low bits of each of `values` in its channel, as
/// `ClearUnorderedAccessViewUint` writes it: formats whose channels are stored in RGBA order.
fn uint_element(format: Format, values: [u32; 4]) -> Result<Vec<u8>> {
    let (_, _, bytes) = FORMAT_FACTS[format as usize].block;
    let Some(bits) = channel_bits(format) else {
        return not_implemented(CLEAR_OF_PACKED_FORMAT);
    };
    let mut element = 0u128;
    let mut at = 0;
    for (&b, &v) in bits.iter().zip(&values) {
        let mask = if b == u32::BITS {
            u32::MAX
        } else {
            (1 << b) - 1
        };
        element |= u128::from(v & mask) << at;
        at += b;
    }
    Ok(element.to_le_bytes()[..bytes as usize].to_vec())
}

/// Whether a view of format kind `view` reads as a declaration of kind `declared`: integers as
/// their own signedness only, the rest (depth included) as floats.
fn same_kind(view: FormatKind, declared: FormatKind) -> bool {
    let integer = |k| matches!(k, FormatKind::Uint | FormatKind::Sint);
    !(integer(view) || integer(declared)) || view == declared
}

/// The vertex layout block of input layout `layout` read at `strides`.
/// The vertex layout block of input layout `layout` read at `strides` by a vertex program of
/// `inputs` (32-bit elements typed as the program reads them).
pub(crate) fn layout_desc(
    layout: Option<&InputLayout>,
    strides: [u32; 32],
    inputs: &[wcontainers_gpu::StageValue],
) -> VertexLayoutDesc {
    let mut desc = VertexLayoutDesc {
        streams: [None; 32],
        attributes: [None; 32],
    };
    if let Some(l) = &layout {
        for (i, e) in l.elements.iter().enumerate() {
            let slot = e.slot as usize;
            desc.streams[slot] = Some(VertexStream {
                stride: strides[slot],
                step: match l.instance_steps[slot] {
                    None => StepMode::Vertex,
                    Some(rate) => StepMode::Instance(rate),
                },
            });
            let read = inputs
                .iter()
                .find(|i| i.system.is_none() && i.location == e.location);
            desc.attributes[i] = Some(VertexAttribute {
                location: e.location,
                stream: e.slot as u8,
                offset: e.offset,
                format: read.map_or(e.format, |r| {
                    format::vertex_format_read_as(e.format, r.scalar)
                }),
            });
        }
    }
    desc
}

/// A raster block's twin: its sample mask, without the depth bias (the pixel program adds it, or
/// the draw is of lines or points), with the scissor test on (a fitted pass), and unculled and
/// filled (the rectangles of alpha-antialiased lines, which Direct3D neither culls nor fills as
/// wireframe).
pub(crate) type RasterKey = (u32, bool, bool, bool);

/// The raster block of `desc` as twin `key`.
fn twin_block(
    desc: &crate::desc::RasterizerDesc,
    caps: &wcontainers_gpu::Caps,
    key: RasterKey,
) -> wcontainers_gpu::RasterDesc {
    let mut block = wcontainers_gpu::RasterDesc {
        sample_mask: key.0,
        ..crate::device::raster_block(desc, caps)
    };
    block.scissor |= key.2;
    if key.1 {
        block.depth_bias = 0;
        block.depth_bias_slope = 0;
        block.depth_bias_clamp = 0;
    }
    if key.3 {
        block.cull = wcontainers_gpu::CullMode::None;
        block.fill = wcontainers_gpu::FillMode::Solid;
    }
    block
}

/// A rectangle (left, top, right, bottom) as the pixels it holds.
fn pixel_rect([l, t, r, b]: Rect) -> ScissorRect {
    ScissorRect {
        x: l.max(0) as u32,
        y: t.max(0) as u32,
        width: r.saturating_sub(l.max(0)).max(0) as u32,
        height: b.saturating_sub(t.max(0)).max(0) as u32,
    }
}

/// A target view's clear value of `color`: integer formats take each value as a target clear
/// converts it.
fn color_value<B: Backend>(view: &View<B>, color: Color) -> ClearValue {
    ClearValue::Color(if let Some(f) = view.decoded {
        color.map(|c| format::encode_norm(f, c))
    } else if integer_format(view.format) {
        integer_clear(view.format, color, target_integer)
    } else {
        color.map(f32::to_bits)
    })
}

/// How a clear converts a value to an integer channel of range `[min, max]` (signed where `min`
/// is below zero).
type IntegerConversion = fn(value: f32, min: i64, max: i64) -> i64;

/// A target clear's conversion, as the RTX 3080 makes it: fractions dropped, NaN as zero and the
/// infinities as the channel's limits; a signed value outside 32 bits as the least 32-bit
/// integer, an unsigned one below zero or past 64 bits as zero and any other as its low 32 bits,
/// each then limited to the channel.
fn target_integer(value: f32, min: i64, max: i64) -> i64 {
    // The 32- and 64-bit ranges as floats: [-2^31, 2^31) and [0, 2^63).
    let narrow = i32::MIN as f32..-(i32::MIN as f32);
    let wide = 0.0..-(i64::MIN as f32);
    let c = value.trunc();
    let integer = if c.is_nan() {
        0
    } else if c.is_infinite() {
        if c > 0.0 { max } else { min }
    } else if min < 0 {
        if narrow.contains(&c) {
            c as i64
        } else {
            i64::from(i32::MIN)
        }
    } else if wide.contains(&c) {
        i64::from(c as i64 as u32)
    } else {
        0
    };
    integer.clamp(min, max)
}

/// A storage view's conversion: fractions dropped, NaN as zero, the rest clamped to the channel.
fn storage_integer(value: f32, min: i64, max: i64) -> i64 {
    (value as i64).clamp(min, max)
}

/// The integers a clear of an integer format writes for `color`, each converted by `convert`.
fn integer_clear(
    format: Format,
    color: Color,
    convert: IntegerConversion,
) -> [u32; COLOR_COMPONENTS] {
    // Every integer format stores its channels in RGBA order.
    let bits = channel_bits(format).unwrap_or_else(|| vec![u32::BITS; COLOR_COMPONENTS]);
    let signed = FORMAT_FACTS[format as usize].kind == FormatKind::Sint;
    std::array::from_fn(|i| {
        let Some(&b) = bits.get(i) else { return 0 };
        let (min, max) = if signed {
            (-(1i64 << (b - 1)), (1i64 << (b - 1)) - 1)
        } else {
            (0, (1i64 << b) - 1)
        };
        convert(color[i], min, max) as u32
    })
}

/// The bits of each channel of `format` in the order they are stored, for formats whose channels
/// are stored in RGBA order; none for the packed and BGR ones.
fn channel_bits(format: Format) -> Option<Vec<u32>> {
    let (_, _, bytes) = FORMAT_FACTS[format as usize].block;
    let mask = FORMAT_FACTS[format as usize].components;
    let n = mask.count_ones();
    match format {
        Format::Rgb10a2Unorm | Format::Rgb10a2Uint => Some(RGB10A2_BITS.to_vec()),
        Format::Rg11b10Float => Some(RG11B10_BITS.to_vec()),
        Format::Bgra8Unorm
        | Format::Bgra8UnormSrgb
        | Format::Bgrx8Unorm
        | Format::Bgrx8UnormSrgb
        | Format::B5g6r5Unorm
        | Format::Rgb9e5Float => None,
        _ if mask == (1 << n) - 1
            && bytes % n == 0
            && bytes.is_power_of_two()
            && bytes as usize <= size_of::<u128>() =>
        {
            Some(vec![u8::BITS * bytes / n; n as usize])
        }
        _ => None,
    }
}

/// Fails as unserved with the reason the shaders crate gave.
fn refused<T>(e: wcontainers_shaders::ConvertError) -> Result<T> {
    match e {
        wcontainers_shaders::ConvertError::Unsupported(why) => not_implemented(why),
        _ => not_implemented(CLEAR_STORAGE_UNSERVED),
    }
}
