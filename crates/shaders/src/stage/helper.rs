//! The stage helper's run time: it makes the compute programs once per pair of vertex and geometry
//! shaders (or a vertex shader whose output is streamed), and for each API draw records the
//! dispatches and says what the front draws after them.

use super::tessellate::{Tessellation, Tessellator};
use super::{
    GEOMETRY_BINDING, INDICES_BINDING, LINE_ADJACENCY_VERTICES, PATCHES_BINDING, POINTS_BINDING,
    TRIANGLE_ADJACENCY_VERTICES, TRIANGLE_VERTICES, VALUES_BINDING, VERTICES_BINDING, WORKGROUP,
    tess, word,
};
use crate::build::WORD;
use crate::{ConvertError, Converted};
use alloc::string::String;
use alloc::vec::Vec;
use core::mem::{offset_of, size_of};
use wcontainers_gpu::{
    BindingEntry, BindingResource, BufferDesc, BufferRange, BufferUsage, DispatchCall,
    DispatchGroups, DrawRange, Gpu, GpuError, IndexFormat, MAX_BINDING_SETS, MapIntent, Program,
    ProvokingVertex, RingKind, ShaderModule, SlowPath, StageExtras, StageSet, StageValue, StepMode,
    SystemValue, Topology, VertexAttribute, VertexFormat, VertexLayoutDesc, VertexStream, Viewport,
};

/// The API's primitive topology, adjacency included (it reaches only the geometry stage).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTopology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
    LineListAdjacency,
    LineStripAdjacency,
    TriangleListAdjacency,
    TriangleStripAdjacency,
    /// Patches of this many control points (1 to 32), for a hull stage.
    PatchList(u8),
}

/// Why the stage helper cannot run a pair of shaders or a draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageError {
    Convert(ConvertError),
    Gpu(GpuError),
    /// Something the helper does not provide yet, named.
    Unsupported(&'static str),
    /// The draw's topology does not give the geometry shader's input primitive (or a patch the hull
    /// shader's input control points), which Direct3D does not draw either.
    TopologyMismatch,
    /// A stream output declaration names an output the shader does not write, or does not fit its
    /// stride, which Direct3D refuses when the shader is made.
    Declaration,
}

impl From<GpuError> for StageError {
    fn from(e: GpuError) -> Self {
        StageError::Gpu(e)
    }
}

impl From<ConvertError> for StageError {
    fn from(e: ConvertError) -> Self {
        StageError::Convert(e)
    }
}

/// Direct3D's stream output buffers (`D3D11_SO_BUFFER_SLOT_COUNT`).
pub const STREAM_BUFFERS: usize = 4;
/// Direct3D's geometry streams (`D3D11_SO_STREAM_COUNT`).
pub const STREAMS: usize = 4;
/// The words of [`StreamTargets::filled`]: each buffer's filled size, then each stream's primitives
/// written and made.
pub const STREAM_FILLED_WORDS: usize = STREAM_BUFFERS + 2 * STREAMS;

/// One element of a stream output declaration (Direct3D's `D3D11_SO_DECLARATION_ENTRY`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOutputEntry {
    pub stream: u8,
    /// The output's semantic; `None` for a gap of `component_count` components.
    pub semantic: Option<String>,
    pub semantic_index: u32,
    pub start_component: u8,
    pub component_count: u8,
    /// The buffer it is written to, below [`STREAM_BUFFERS`].
    pub slot: u8,
}

/// A stream output declaration: its elements, and each buffer's bytes per vertex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOutput {
    pub entries: Vec<StreamOutputEntry>,
    pub strides: [u32; STREAM_BUFFERS],
}

/// Where one draw's stream output goes. Its buffers are made with [`BufferUsage::STORAGE`] for
/// [`Stages`], and with [`BufferUsage::COPY_SRC`] and [`BufferUsage::COPY_DST`] for
/// [`super::GeometryReplay`].
#[derive(Debug)]
pub struct StreamTargets<'a, B: Gpu> {
    /// The bound buffers, each written from its range's offset (aligned as storage bindings are)
    /// plus its filled size.
    pub buffers: [Option<BufferRange<&'a B::Buffer>>; STREAM_BUFFERS],
    /// [`STREAM_FILLED_WORDS`] u32: the
    /// bytes each buffer is filled to, where the draw appends and which it advances (the front writes
    /// them for Direct3D's explicit offsets and keeps them for `DrawAuto`), then for each stream the
    /// primitives the draw wrote and those it made (Direct3D's stream output statistics); the words
    /// up to the shader's last stream's are enough. A stream whose buffers are all unbound writes
    /// nothing and still counts what it made.
    pub filled: BufferRange<&'a B::Buffer>,
}

/// The draw's index buffer, which the vertex stage's compute form reads as storage: made with
/// [`BufferUsage::STORAGE`] as well as [`BufferUsage::INDEX`].
#[derive(Debug)]
pub struct StageIndex<'a, B: Gpu> {
    pub buffer: &'a B::Buffer,
    /// The buffer's size in bytes, a multiple of 4.
    pub size: u64,
    pub offset: u64,
    pub format: IndexFormat,
}

/// One API draw through the stage helper.
#[derive(Debug)]
pub struct StageDraw<'a, B: Gpu> {
    /// The vertex stage's sets, as for a draw (its slots and pulled streams in set 0, its helper
    /// constants in set 3), made visible to compute. Set 1 is the helper's own.
    pub vertex_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
    /// The geometry or hull stage's sets: its slots in set 0, its helper constants in set 3.
    pub geometry_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
    pub index: Option<StageIndex<'a, B>>,
    pub topology: InputTopology,
    /// A direct draw's range.
    pub range: DrawRange<'a, B>,
    /// Where stream output goes, for a helper made with a declaration; `None` when no buffer is bound.
    pub stream_output: Option<StreamTargets<'a, B>>,
}

/// The sets of the stages after a domain stage, for [`Stages::run_after_domain`].
#[derive(Debug)]
pub struct DomainSets<'a, B: Gpu> {
    /// The domain stage's sets, as for its draw (its slots in set 0, its helper constants in set 3),
    /// made visible to compute, without the patches and points: the helper binds those itself.
    pub domain_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
    /// The geometry stage's sets, when there is one.
    pub geometry_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
}

/// What the front draws after [`Stages::run`], in its pass: the program it made of
/// [`Stages::passthrough`] and its pixel program, with [`Stages::vertex_layout`], `buffer` as vertex
/// stream 0 at offset 0, and with indices from `buffer` at `index_offset` (u32, strips restarting)
/// when given; `count` vertices or indices, one instance. After a tessellation stage, `tessellated`
/// says how it differs. A geometry shader writing several streams (of points) has each stream's
/// points after the stream's before it: stream `k`'s are vertices `k * count` to `(k + 1) * count`.
#[derive(Debug)]
pub struct FinalDraw<B: Gpu> {
    pub buffer: B::Buffer,
    pub index_offset: Option<u64>,
    pub topology: Topology,
    pub count: u32,
    pub tessellated: Option<TessellatedDraw<B>>,
}

/// The draw after a tessellation stage: the domain program (converted) with the pixel program, no
/// vertex streams, indices (u32) from [`FinalDraw::buffer`] bound at offset 0 (the arguments start
/// past the tessellator's index header), as an indexed indirect draw
/// whose arguments are at offset 0 of `arguments` ([`FinalDraw::count`] is the most indices it can
/// draw). The domain program reads `patches` and `points`, bound read-only at
/// [`wcontainers_gpu::DOMAIN_PATCHES_BINDING`] and [`wcontainers_gpu::DOMAIN_POINTS_BINDING`] in its
/// helper set.
#[derive(Debug)]
pub struct TessellatedDraw<B: Gpu> {
    pub arguments: B::Buffer,
    pub patches: BufferRange<B::Buffer>,
    pub points: BufferRange<B::Buffer>,
}

/// Buffers the helper owns, grown as draws need.
pub(super) struct Owned<B: Gpu> {
    pub(super) buffer: B::Buffer,
    pub(super) size: u64,
}

/// The geometry stage's compute program and what its output is.
struct Geometry<B: Gpu> {
    program: B::Program,
    passthrough: ShaderModule,
    input_vertices: u32,
    max_vertices: u32,
    instances: u32,
    /// Vertices per output primitive: 1, 2 or 3.
    output: u32,
    /// Streams written; each has its own vertex and index slots, stream after stream.
    streams: u32,
    out_words: u32,
    /// Vertex attributes after the position: the output locations, then the layer and viewport slot
    /// when the shader writes either, then the clip distances' slots; the cull distances' slots
    /// follow them in the record.
    out_locations: u32,
    selects: bool,
    cull: Option<GeometryCull<B>>,
    /// Whether the shader reads its primitive's number, which after a domain stage it takes from
    /// the tessellator's points bound at [`POINTS_BINDING`].
    numbered: bool,
}

/// The geometry cull program, and where the geometry output's records hold the cull distances.
struct GeometryCull<B: Gpu> {
    program: B::Program,
    cull_word: u32,
    distances: u32,
}

/// The geometry cull program's values (`geometry_cull.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct GeometryCullValues {
    invocations: u32,
    index_slots: u32,
    primitives: u32,
    vertices: u32,
    record_words: u32,
    cull_word: u32,
    cull_count: u32,
    index_base: u32,
    list_base: u32,
    empty: u32,
    rotate: u32,
    pad: u32,
}

/// The hull stage's compute program, the tessellator, and the patch records between them.
struct Hull<B: Gpu> {
    program: B::Program,
    tessellator: Tessellator<B>,
    tessellation: Tessellation,
    input_points: u32,
    /// Words per patch record.
    patch_words: u32,
    patches: Option<Owned<B>>,
}

/// The domain stage's compute form, which runs when a geometry stage or stream output follows it.
struct Domain<B: Gpu> {
    program: B::Program,
    /// Words per record it writes.
    words: u32,
    records: Option<Owned<B>>,
}

/// The cull program and where the records hold what it reads.
struct Cull<B: Gpu> {
    program: B::Program,
    passthrough: ShaderModule,
    /// Slots the pass-through reads after the position: the locations and the clip distances.
    drawn_slots: u32,
    /// Words per record.
    words: u32,
    /// The word of a record where the cull distances start, and their count.
    cull_word: u32,
    distances: u32,
}

impl<B: Gpu> Cull<B> {
    /// The cull program for records `programs` writes and its pass-through draws.
    fn new(backend: &mut B, programs: super::CullPrograms) -> Result<Self, StageError> {
        let shader = backend.create_shader(&super::cull_program()?)?;
        let program = backend.create_program(&Program::Compute(&shader))?;
        let drawn_slots =
            programs.locations.len() as u32 + super::distance_slots(programs.clip_distances);
        Ok(Cull {
            program,
            passthrough: programs.passthrough,
            drawn_slots,
            words: super::record_words(
                (drawn_slots + super::distance_slots(programs.cull_distances)) as usize,
            ),
            cull_word: (1 + drawn_slots) * crate::build::VEC4,
            distances: programs.cull_distances,
        })
    }
}

/// The cull program's values (`cull.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct CullValues {
    units: u32,
    primitives: u32,
    count: u32,
    vertices: u32,
    strip: u32,
    record_words: u32,
    cull_word: u32,
    cull_count: u32,
    index_base: u32,
    empty: u32,
    listed: u32,
    pad: u32,
}

/// The line program and what its corners hold.
struct Lines<B: Gpu> {
    program: B::Program,
    passthrough: ShaderModule,
    /// Slots the pass-through reads after the position: the locations, the clip distances and the
    /// coverage.
    drawn_slots: u32,
    interpolated: u32,
    linear: u32,
    /// The slot holds the line's ends, for Direct3D's aliased rule.
    diamond: bool,
}

/// The line program's values (`lines.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct LineValues {
    units: u32,
    primitives: u32,
    count: u32,
    strip: u32,
    in_words: u32,
    out_words: u32,
    index_base: u32,
    interpolated: u32,
    linear: u32,
    ends: u32,
    half_width: f32,
    half_height: f32,
    reach_across: f32,
    reach_along: f32,
    origin_x: f32,
    origin_y: f32,
}

/// The line program's bindings: its values, the vertex records, and the corners followed by the index
/// list.
const LINE_VALUES_BINDING: u32 = 0;
const LINE_RECORDS_BINDING: u32 = 1;
const LINE_CORNERS_BINDING: u32 = 2;
/// Corners of a line's rectangle, and the indices of its two triangles.
const LINE_CORNERS: u32 = 4;
pub(super) const LINE_INDICES: u32 = 2 * TRIANGLE_VERTICES;
/// The vertices of a line.
pub(super) const LINE_VERTICES: u32 = 2;
/// A draw on the line route of another topology than lines.
pub(super) const LINE_TOPOLOGY: &str = "AntialiasedLinesTopology";
/// The line route run without the viewport's size, or another route with it.
pub(super) const LINE_VIEWPORT: &str = "the viewport's size, given to the line route alone";
/// A viewport at the target's corner, `width` and `height` pixels.
pub(super) fn sized(width: f32, height: f32) -> Viewport {
    Viewport {
        x: 0,
        y: 0,
        width: width.to_bits(),
        height: height.to_bits(),
        min_depth: 0,
        max_depth: 0,
    }
}

/// The aliased line route run without the viewport's place.
pub(super) const LINE_ORIGIN: &str = "the aliased line route run without the viewport's place";

/// The primitive program and what its copies hold.
struct Numbered<B: Gpu> {
    program: B::Program,
    passthrough: ShaderModule,
    /// Slots the pass-through reads after the position: the locations, the clip distances and the
    /// primitive's number.
    drawn_slots: u32,
}

/// The primitive program's values (`primitives.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct PrimitiveValues {
    units: u32,
    primitives: u32,
    count: u32,
    vertices: u32,
    strip: u32,
    in_words: u32,
    out_words: u32,
    pad: u32,
}

/// The primitive program's bindings: its values, the vertex records, and the copies.
const PRIMITIVE_VALUES_BINDING: u32 = 0;
const PRIMITIVE_RECORDS_BINDING: u32 = 1;
const PRIMITIVE_COPIES_BINDING: u32 = 2;
/// A draw on the primitive route with adjacency, whose adjacent vertices Direct3D draws without.
pub(super) const PRIMITIVE_ID_ADJACENCY: &str = "PrimitiveIdAdjacency";

/// The strip program, and the list it writes ([`tess::INDEX_HEADER`] words, then the indices).
struct Strips<B: Gpu> {
    program: B::Program,
    list: Option<Owned<B>>,
}

/// The strip program's values (`strips.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct StripValues {
    count: u32,
    first: u32,
    index_bytes: u32,
    index_offset: u32,
    vertices: u32,
    restart: u32,
    header: u32,
    pad: u32,
}

/// The strip program's bindings: its values, the draw's indices, and the list.
const STRIP_VALUES_BINDING: u32 = 0;
const STRIP_INDICES_BINDING: u32 = 1;
const STRIP_LIST_BINDING: u32 = 2;

/// The cull program's bindings: its values, the records followed by the index list, and the
/// tessellator's index list.
const CULL_VALUES_BINDING: u32 = 0;
const CULL_DATA_BINDING: u32 = 1;
const CULL_LIST_BINDING: u32 = 2;
/// Refusals by name, and what is malformed in a call.
const WITHOUT_WORK: &str = "the stage helper without work";
const STAGE_INDEX_U8: &str = "StageIndexU8";
pub(super) const STAGE_INDIRECT: &str = "StageIndirect";
const DOMAIN_SETS: &str = "the sets of the stages after a domain stage";
const NOT_A_DOMAIN_SHADER: &str = "not a domain shader";
const NOT_A_HULL_SHADER: &str = "not a hull shader";
pub(super) const NOT_A_GEOMETRY_SHADER: &str = "not a geometry shader";
const STREAM_OUTPUT_ENTRIES: &str = "StreamOutputEntries";
/// A filled sizes block without the words of every stream the shader writes.
pub(super) const STREAM_FILLED_SIZE: &str =
    "the filled sizes block shorter than the shader's streams";
const STREAM_OUTPUT_SYSTEM_VALUE: &str = "StreamOutputSystemValue";
/// Stream output of a vertex stage drawn with adjacency.
pub(super) const STREAM_OUTPUT_ADJACENCY: &str = "StreamOutputAdjacency";
/// A vertex stage writing cull distances, drawn with adjacency, which Direct3D draws without the
/// adjacent vertices.
pub(super) const CULL_DISTANCE_ADJACENCY: &str = "CullDistanceAdjacency";
/// Stream output from a domain stage writing cull distances, without a geometry stage.
const DOMAIN_CULL_DISTANCE_STREAM_OUTPUT: &str = "DomainCullDistanceStreamOutput";
/// A vertex stage writing storage, drawn alone with adjacency.
const VERTEX_STORAGE_ADJACENCY: &str = "VertexStorageAdjacency";
/// Stream output from a vertex stage writing cull distances whose records do not hold its reflected
/// locations first.
const CULL_DISTANCE_STREAM_LOCATIONS: &str = "CullDistanceStreamLocations";
/// Flat values from another vertex than the front draws them from, for a draw through a vertex stage
/// alone, whose records the front lists itself.
const PROVOKING_WITHOUT_STAGES: &str = "ProvokingWithoutStages";

/// How the final draw's primitives are turned (`geometry_cull.wgsl`'s and `tessellator.wgsl`'s
/// `rotate`): kept, the last vertex leading, or the first trailing.
const ROTATE_NONE: u32 = 0;
const ROTATE_LAST_FIRST: u32 = 1;
const ROTATE_FIRST_LAST: u32 = 2;

/// The stream output program and its declaration's elements.
struct Stream<B: Gpu> {
    program: B::Program,
    entries: Vec<(u8, [u32; ENTRY_WORDS])>,
    strides: [u32; STREAM_BUFFERS],
    buffer_streams: [u8; STREAM_BUFFERS],
}

/// A stream output declaration's elements located in the records of the stage whose outputs are
/// streamed.
pub(super) struct StreamDeclaration {
    /// Each element's stream, and its first words in the program's table ([`StreamEntry`]).
    pub(super) entries: Vec<(u8, [u32; ENTRY_WORDS])>,
    /// Each buffer's bytes per vertex.
    pub(super) strides: [u32; STREAM_BUFFERS],
    /// The stream each buffer takes (stream 0 for a buffer no element names).
    pub(super) buffer_streams: [u8; STREAM_BUFFERS],
}

/// An element of a [`StreamDeclaration`]: its buffer, first record word, word count and byte
/// offset in the buffer's vertex.
pub(super) struct StreamElement {
    pub(super) buffer: usize,
    pub(super) word: u32,
    pub(super) count: u32,
    pub(super) offset: u32,
}

impl StreamDeclaration {
    /// The elements of stream `stream`.
    pub(super) fn elements(&self, stream: u8) -> impl Iterator<Item = StreamElement> + '_ {
        let field = |packed: u32, shift: u32| (packed >> shift) & u32::from(u8::MAX);
        self.entries.iter().filter(move |(s, _)| *s == stream).map(
            move |(_, [packed, offset, slot])| StreamElement {
                buffer: field(*packed, 0) as usize,
                word: slot * crate::build::VEC4 + field(*packed, PACKED_FIRST_SHIFT),
                count: field(*packed, PACKED_COUNT_SHIFT),
                offset: *offset,
            },
        )
    }
}

/// The stream output program's values (`stream_out.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct StreamValues {
    units: u32,
    geometry: u32,
    vertices: u32,
    strip: u32,
    slots: u32,
    index_base: u32,
    record_words: u32,
    entries: u32,
    primitives: u32,
    count: u32,
    listed: u32,
    stream: u32,
    listed_total: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
    strides: [u32; STREAM_BUFFERS],
    sizes: [u32; STREAM_BUFFERS],
    table: [StreamEntry; STREAM_ENTRIES],
}

/// An element of its table: buffer, first component and count packed ([`PACKED_FIRST_SHIFT`],
/// [`PACKED_COUNT_SHIFT`]), offset in the buffer's vertex, record slot.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct StreamEntry {
    packed: u32,
    offset: u32,
    slot: u32,
    pad: u32,
}

/// The elements its table holds.
const STREAM_ENTRIES: usize = 64;
/// The words of an element the helper writes.
const ENTRY_WORDS: usize = word_at(offset_of!(StreamEntry, pad));
const PACKED_FIRST_SHIFT: u32 = u8::BITS;
const PACKED_COUNT_SHIFT: u32 = 2 * u8::BITS;
/// The stream output program's bindings: its values, the records it reads, the filled sizes, the
/// buffers from [`STREAM_OUT_BINDING`] on, and the listed primitives.
const STREAM_VALUES_BINDING: u32 = 0;
const STREAM_DATA_BINDING: u32 = 1;
const STREAM_FILLED_BINDING: u32 = 2;
const STREAM_OUT_BINDING: u32 = 3;
const STREAM_LIST_BINDING: u32 = STREAM_OUT_BINDING + STREAM_BUFFERS as u32;

/// The word `offset` bytes into a block of words.
pub(super) const fn word_at(offset: usize) -> usize {
    offset / size_of::<u32>()
}

/// The stand-in buffer's ranges: the index words, then the stream output buffers, each at a multiple
/// of the adapter's storage offset alignment.
const UNUSED_RANGES: u64 = 1 + STREAM_BUFFERS as u64;
/// The bytes of each.
const UNUSED_RANGE_BYTES: u64 = super::RECORD_SLOT_BYTES as u64;

/// A vertex shader, and a geometry shader or stream output or both, or a hull shader and the
/// tessellator (then the domain shader too, when a geometry shader or stream output follows it), run
/// as compute work before the draw ([`SlowPath::GeometryAsCompute`],
/// [`SlowPath::StreamOutAsCompute`], [`SlowPath::TessellationAsCompute`]).
pub struct Stages<B: Gpu> {
    vertex: B::Program,
    /// The vertex program writes storage ([`SlowPath::VertexStorageAsCompute`]).
    stores: bool,
    geometry: Option<Geometry<B>>,
    hull: Option<Hull<B>>,
    domain: Option<Domain<B>>,
    stream: Option<Stream<B>>,
    cull: Option<Cull<B>>,
    lines: Option<Lines<B>>,
    numbered: Option<Numbered<B>>,
    /// The strip program and the list it writes, made at the first indexed strip.
    strips: Option<Strips<B>>,
    /// Words per vertex record.
    in_words: u32,
    records: Option<Owned<B>>,
    out: Option<Owned<B>>,
    /// Stands in for bindings a draw does not use ([`UNUSED_RANGES`] storage ranges).
    unused: B::Buffer,
    /// Bytes between its ranges.
    unused_stride: u64,
    /// How the final draw's primitives are turned ([`Self::provoking`]).
    rotate: u32,
}

impl<B: Gpu> Stages<B> {
    /// Makes the compute programs of `vertex` (converted with every input pulled), of `geometry` if
    /// given, and of stream output from the last of them if declared. Without a geometry shader, a
    /// vertex program writing storage is made as [`Self::stored`] makes it.
    pub fn new(
        backend: &mut B,
        vertex: &Converted,
        geometry: Option<&Converted>,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        if geometry.is_none() && super::writes_storage(vertex) {
            return Self::stored(backend, vertex, stream_output);
        }
        if geometry.is_none() && stream_output.is_none() {
            return Err(StageError::Convert(ConvertError::Malformed(
                WITHOUT_WORK.into(),
            )));
        }
        // The vertex records hold what the next stage reads: the geometry shader's inputs, or every
        // output of a vertex shader whose output is streamed.
        let (read, distances, last) = match geometry {
            Some(g) => (
                super::locations(&g.reflection.inputs),
                super::read_distances(&g.reflection),
                &g.reflection.outputs,
            ),
            None => (
                super::locations(&vertex.reflection.outputs),
                (0, 0),
                &vertex.reflection.outputs,
            ),
        };
        let shader =
            backend.create_shader(&super::vertex_program_for(vertex, &read, distances)?)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let geometry = match geometry {
            Some(g) => Some(geometry_parts(backend, g)?),
            None => None,
        };
        let streams = geometry.as_ref().map_or(1, |g| g.streams);
        let stream = match stream_output {
            Some(so) => Some(stream_parts(backend, so, last, streams)?),
            None => None,
        };
        let unused_stride =
            u64::from(backend.caps().limits.min_storage_offset_alignment).max(UNUSED_RANGE_BYTES);
        let unused = backend.create_buffer(&BufferDesc {
            size: UNUSED_RANGES * unused_stride,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            stores: super::writes_storage(vertex),
            geometry,
            hull: None,
            domain: None,
            stream,
            cull: None,
            lines: None,
            numbered: None,
            strips: None,
            in_words: super::record_words(super::read_slots(&read, distances)),
            records: None,
            out: None,
            unused,
            unused_stride,
            rotate: ROTATE_NONE,
        })
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled), which writes cull
    /// distances, and of stream output from it if declared: each draw runs it, drops the primitives
    /// its cull distances drop, and the final draw draws the rest through [`Self::passthrough`]
    /// ([`SlowPath::CullDistanceAsGeometry`]).
    pub fn culled(
        backend: &mut B,
        vertex: &Converted,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        Self::drawn_records(
            backend,
            vertex,
            super::cull_programs(vertex)?,
            stream_output,
        )
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled), which writes
    /// storage ([`super::writes_storage`]), and of stream output from it if declared: each draw runs it,
    /// and the final draw draws its records through [`Self::passthrough`], less the primitives its
    /// cull distances drop if it writes any ([`SlowPath::VertexStorageAsCompute`]).
    pub fn stored(
        backend: &mut B,
        vertex: &Converted,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        Self::drawn_records(
            backend,
            vertex,
            super::stored_programs(vertex)?,
            stream_output,
        )
    }

    /// The cull route's helper of `vertex` made into `programs`.
    fn drawn_records(
        backend: &mut B,
        vertex: &Converted,
        programs: super::CullPrograms,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        let shader = backend.create_shader(&programs.compute)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let stream = match stream_output {
            Some(so) => {
                let reflected = super::locations(&vertex.reflection.outputs);
                if !programs.locations.starts_with(&reflected) {
                    return Err(StageError::Unsupported(CULL_DISTANCE_STREAM_LOCATIONS));
                }
                Some(stream_parts(backend, so, &vertex.reflection.outputs, 1)?)
            }
            None => None,
        };
        let cull = Cull::new(backend, programs)?;
        let unused_stride =
            u64::from(backend.caps().limits.min_storage_offset_alignment).max(UNUSED_RANGE_BYTES);
        let unused = backend.create_buffer(&BufferDesc {
            size: UNUSED_RANGES * unused_stride,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            stores: super::writes_storage(vertex),
            geometry: None,
            hull: None,
            domain: None,
            stream,
            in_words: cull.words,
            cull: Some(cull),
            lines: None,
            numbered: None,
            strips: None,
            records: None,
            out: None,
            unused,
            unused_stride,
            rotate: ROTATE_NONE,
        })
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled) drawing its lines
    /// as rectangles for a pixel program converted with `pixel` (its key, with
    /// `VariantFlags::LINE_COVERAGE` for antialiased lines, or `LINE_DIAMOND` for aliased lines into
    /// a multisampled target): each draw through [`Self::run_lines`] runs it and writes a rectangle
    /// around each line, which the final draw draws through [`Self::passthrough`] with culling off
    /// ([`SlowPath::AntialiasedLinesAsQuads`], [`SlowPath::AliasedLinesAsQuads`]).
    pub fn antialiased_lines(
        backend: &mut B,
        vertex: &Converted,
        pixel: &wcontainers_gpu::VariantKey,
    ) -> Result<Self, StageError> {
        let programs = super::line_programs(vertex, pixel)?;
        let shader = backend.create_shader(&programs.records)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let shader = backend.create_shader(&super::line_program()?)?;
        let program = backend.create_program(&Program::Compute(&shader))?;
        let record_slots =
            programs.locations.len() as u32 + super::distance_slots(programs.clip_distances);
        let unused_stride =
            u64::from(backend.caps().limits.min_storage_offset_alignment).max(UNUSED_RANGE_BYTES);
        let unused = backend.create_buffer(&BufferDesc {
            size: UNUSED_RANGES * unused_stride,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            stores: super::writes_storage(vertex),
            geometry: None,
            hull: None,
            domain: None,
            stream: None,
            cull: None,
            lines: Some(Lines {
                program,
                passthrough: programs.passthrough,
                drawn_slots: record_slots + 1,
                interpolated: programs.interpolated,
                linear: programs.linear,
                diamond: programs.diamond,
            }),
            numbered: None,
            strips: None,
            in_words: super::record_words(record_slots as usize),
            records: None,
            out: None,
            unused,
            unused_stride,
            rotate: ROTATE_NONE,
        })
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled) for a pixel program
    /// converted with `pixel` (its key, with `VariantFlags::PRIMITIVE_ID_FROM_STAGE`) that reads its
    /// primitive's number: each draw through [`Self::run`] runs it and copies each primitive's vertex
    /// records with its number, which the final draw draws through [`Self::passthrough`]
    /// ([`SlowPath::PrimitiveIdAsGeometry`]).
    pub fn primitive_numbered(
        backend: &mut B,
        vertex: &Converted,
        pixel: &wcontainers_gpu::VariantKey,
    ) -> Result<Self, StageError> {
        let programs = super::primitive_programs(vertex, pixel)?;
        let shader = backend.create_shader(&programs.compute)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let shader = backend.create_shader(&super::primitive_program()?)?;
        let program = backend.create_program(&Program::Compute(&shader))?;
        let record_slots =
            programs.locations.len() as u32 + super::distance_slots(programs.clip_distances);
        let unused_stride =
            u64::from(backend.caps().limits.min_storage_offset_alignment).max(UNUSED_RANGE_BYTES);
        let unused = backend.create_buffer(&BufferDesc {
            size: UNUSED_RANGES * unused_stride,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            stores: super::writes_storage(vertex),
            geometry: None,
            hull: None,
            domain: None,
            stream: None,
            cull: None,
            lines: None,
            numbered: Some(Numbered {
                program,
                passthrough: programs.passthrough,
                drawn_slots: record_slots + 1,
            }),
            strips: None,
            in_words: super::record_words(record_slots as usize),
            records: None,
            out: None,
            unused,
            unused_stride,
            rotate: ROTATE_NONE,
        })
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled) and `hull`, and the
    /// tessellator, for draws whose final draw runs `domain` ([`TessellatedDraw`]); one writing cull
    /// distances as [`Self::tessellated_with`] says.
    pub fn tessellated(
        backend: &mut B,
        vertex: &Converted,
        hull: &Converted,
        domain: &Converted,
    ) -> Result<Self, StageError> {
        Self::tessellated_with(backend, vertex, hull, domain, None, None)
    }

    /// As [`Self::tessellated`], with a geometry shader or stream output (or both) after the domain
    /// stage, which then runs as compute too; its draws go through [`Self::run_after_domain`]. With a
    /// geometry shader the final draw is the geometry stage's, without one the tessellated draw. A
    /// domain program writing cull distances ([`super::writes_cull_distances`]) runs as compute
    /// without a geometry shader too, its draws through [`Self::run_after_domain`], and the final draw
    /// is the primitives they keep, drawn through [`Self::passthrough`] as the cull route's
    /// ([`SlowPath::CullDistanceAsGeometry`]); stream output from it is refused by name.
    pub fn tessellated_with(
        backend: &mut B,
        vertex: &Converted,
        hull: &Converted,
        domain: &Converted,
        geometry: Option<&Converted>,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        let StageExtras::Hull {
            input_control_points,
            output_control_points,
            domain: hull_domain,
            partitioning,
            output_primitive,
            max_tess_factor,
            point_locations,
            constant_locations,
        } = hull.reflection.extras
        else {
            return Err(StageError::Convert(ConvertError::Malformed(
                NOT_A_HULL_SHADER.into(),
            )));
        };
        match domain.reflection.extras {
            StageExtras::Domain { domain: d, .. } if d == hull_domain => {}
            StageExtras::Domain { .. } => return Err(StageError::Declaration),
            _ => {
                return Err(StageError::Convert(ConvertError::Malformed(
                    NOT_A_DOMAIN_SHADER.into(),
                )));
            }
        }
        let read = super::locations(&hull.reflection.inputs);
        let shader = backend.create_shader(&super::vertex_program_for(vertex, &read, (0, 0))?)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let shader = backend.create_shader(&hull.modules[0])?;
        let hull_program = backend.create_program(&Program::Compute(&shader))?;
        let slot = crate::build::VEC4;
        let patch_words = u32::from(output_control_points) * slot * u32::from(point_locations)
            + slot * u32::from(constant_locations)
            + tess::FACTOR_WORDS;
        // After the domain stage: its records hold what the next stage reads.
        let tessellated_vertices = tess::output_vertices(output_primitive);
        // A domain program writing cull distances is drawn as the cull route draws a vertex program's,
        // over the tessellator's primitives; before a geometry shader they reach no rasteriser.
        let culled = geometry.is_none() && super::writes_cull_distances(domain);
        let mut cull = None;
        let (geometry, domain_stage, stream) = match (geometry, stream_output) {
            (None, Some(_)) if culled => {
                return Err(StageError::Unsupported(DOMAIN_CULL_DISTANCE_STREAM_OUTPUT));
            }
            (None, None) if culled => {
                let programs = super::domain_cull_programs(domain)?;
                let shader = backend.create_shader(&programs.compute)?;
                let program = backend.create_program(&Program::Compute(&shader))?;
                let c = Cull::new(backend, programs)?;
                let domain = Domain {
                    program,
                    words: c.words,
                    records: None,
                };
                cull = Some(c);
                (None, Some(domain), None)
            }
            (None, None) => (None, None, None),
            _ => {
                let (read, distances, last) = match geometry {
                    Some(g) => (
                        super::locations(&g.reflection.inputs),
                        super::read_distances(&g.reflection),
                        &g.reflection.outputs,
                    ),
                    None => (
                        super::locations(&domain.reflection.outputs),
                        (0, 0),
                        &domain.reflection.outputs,
                    ),
                };
                let shader =
                    backend.create_shader(&super::domain_program_for(domain, &read, distances)?)?;
                let program = backend.create_program(&Program::Compute(&shader))?;
                let geometry = match geometry {
                    Some(g) => {
                        let parts = geometry_parts(backend, g)?;
                        if parts.input_vertices != tessellated_vertices {
                            return Err(StageError::TopologyMismatch);
                        }
                        Some(parts)
                    }
                    None => None,
                };
                let streams = geometry.as_ref().map_or(1, |g| g.streams);
                let stream = match stream_output {
                    Some(so) => Some(stream_parts(backend, so, last, streams)?),
                    None => None,
                };
                let domain = Domain {
                    program,
                    words: super::record_words(super::read_slots(&read, distances)),
                    records: None,
                };
                (geometry, Some(domain), stream)
            }
        };
        let unused_stride =
            u64::from(backend.caps().limits.min_storage_offset_alignment).max(UNUSED_RANGE_BYTES);
        let unused = backend.create_buffer(&BufferDesc {
            size: UNUSED_RANGES * unused_stride,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            stores: super::writes_storage(vertex),
            geometry,
            hull: Some(Hull {
                program: hull_program,
                tessellator: Tessellator::new(backend)?,
                tessellation: Tessellation {
                    domain: hull_domain,
                    partitioning,
                    output: output_primitive,
                    max_factor: max_tess_factor as f32,
                },
                input_points: input_control_points.into(),
                patch_words,
                patches: None,
            }),
            domain: domain_stage,
            stream,
            cull,
            lines: None,
            numbered: None,
            strips: None,
            in_words: super::record_words(read.len()),
            records: None,
            out: None,
            unused,
            unused_stride,
            rotate: ROTATE_NONE,
        })
    }

    /// Releases everything the helper made.
    pub fn destroy(self, backend: &mut B) {
        use wcontainers_gpu::Object;
        backend.destroy(Object::Program(self.vertex));
        if let Some(g) = self.geometry {
            backend.destroy(Object::Program(g.program));
            if let Some(c) = g.cull {
                backend.destroy(Object::Program(c.program));
            }
        }
        if let Some(h) = self.hull {
            backend.destroy(Object::Program(h.program));
            h.tessellator.destroy(backend);
            if let Some(p) = h.patches {
                backend.destroy(Object::Buffer(p.buffer));
            }
        }
        if let Some(d) = self.domain {
            backend.destroy(Object::Program(d.program));
            if let Some(r) = d.records {
                backend.destroy(Object::Buffer(r.buffer));
            }
        }
        if let Some(s) = self.stream {
            backend.destroy(Object::Program(s.program));
        }
        if let Some(c) = self.cull {
            backend.destroy(Object::Program(c.program));
        }
        if let Some(l) = self.lines {
            backend.destroy(Object::Program(l.program));
        }
        if let Some(n) = self.numbered {
            backend.destroy(Object::Program(n.program));
        }
        if let Some(s) = self.strips {
            backend.destroy(Object::Program(s.program));
            if let Some(l) = s.list {
                backend.destroy(Object::Buffer(l.buffer));
            }
        }
        for owned in [self.records, self.out].into_iter().flatten() {
            backend.destroy(Object::Buffer(owned.buffer));
        }
        backend.destroy(Object::Buffer(self.unused));
    }

    /// Gives the final draw's flat values from the vertex `api` names of each primitive the geometry
    /// or tessellation stage makes (a strip's triangle `i` has vertices `i` to `i + 2` in the strip's
    /// order, the tessellator's are as it lists them), for a final draw that takes them from `drawn`
    /// (its [`wcontainers_gpu::RasterDesc::provoking`]). Where the two differ each primitive is
    /// listed turned, its winding kept and a line reversed, and a geometry stage's strips are drawn as
    /// lists ([`SlowPath::ProvokingRewrite`]). Stream output keeps a geometry stage's order and
    /// takes the tessellator's turned one, the list it reads. Both start as
    /// [`ProvokingVertex::First`], Direct3D's. Without either stage the front lists the records
    /// itself, and a difference is refused by name.
    pub fn provoking(
        &mut self,
        backend: &mut B,
        api: ProvokingVertex,
        drawn: ProvokingVertex,
    ) -> Result<(), StageError> {
        let rotate = match (api, drawn) {
            (ProvokingVertex::Last, ProvokingVertex::First) => ROTATE_LAST_FIRST,
            (ProvokingVertex::First, ProvokingVertex::Last) => ROTATE_FIRST_LAST,
            _ => ROTATE_NONE,
        };
        if rotate != ROTATE_NONE && self.geometry.is_none() && self.hull.is_none() {
            return Err(StageError::Unsupported(PROVOKING_WITHOUT_STAGES));
        }
        if let Some(g) = &mut self.geometry
            && rotate != ROTATE_NONE
            && g.output > 1
            && g.cull.is_none()
        {
            let shader = backend.create_shader(&super::geometry_cull_program()?)?;
            g.cull = Some(GeometryCull {
                program: backend.create_program(&Program::Compute(&shader))?,
                cull_word: 0,
                distances: 0,
            });
        }
        self.rotate = rotate;
        Ok(())
    }

    /// The vertex program of the draw after a geometry stage or the cull program, for the front's
    /// program with its pixel program.
    pub fn passthrough(&self) -> Option<&ShaderModule> {
        match (&self.geometry, &self.cull, &self.lines, &self.numbered) {
            (Some(g), ..) => Some(&g.passthrough),
            (None, Some(c), ..) => Some(&c.passthrough),
            (None, None, Some(l), _) => Some(&l.passthrough),
            (None, None, None, Some(n)) => Some(&n.passthrough),
            (None, None, None, None) => None,
        }
    }

    /// Whether the geometry stage writes a layer or viewport ([`SlowPath::LayerSplit`],
    /// [`SlowPath::MultiViewportSplit`]): the front then draws the final draw once per layer of its
    /// target views and per viewport, each run into that layer's views with that viewport alone, the
    /// pass-through's helper constants ([`super::PASSTHROUGH_HELPERS`]) and the pixel program's
    /// `HelperValue::Layer` and `HelperValue::Viewport` set to the run's; each run draws only the
    /// primitives whose vertices name it.
    pub fn selects(&self) -> bool {
        self.geometry.as_ref().is_some_and(|g| g.selects)
    }

    /// The vertex layout of that draw.
    pub fn vertex_layout(&self) -> Option<VertexLayoutDesc> {
        let extended = self.in_words + crate::build::VEC4;
        let (words, slots) = match (&self.geometry, &self.cull, &self.lines, &self.numbered) {
            (Some(g), ..) => (g.out_words, g.out_locations),
            (None, Some(c), ..) => (c.words, c.drawn_slots),
            (None, None, Some(l), _) => (extended, l.drawn_slots),
            (None, None, None, Some(n)) => (extended, n.drawn_slots),
            (None, None, None, None) => return None,
        };
        Some(record_layout(words, slots))
    }

    /// Records the stages' dispatches for one API draw (outside a pass), and returns what the front
    /// draws next after a geometry or tessellation stage; `None` when the draw makes no primitive, or
    /// when there is neither (the front draws as it would, its output having been streamed). A helper
    /// with a stage after its domain stage runs through [`Self::run_after_domain`] instead.
    pub fn run(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        self.run_with(backend, draw, None, None)
    }

    /// [`Self::run`] for a helper made by [`Self::antialiased_lines`], whose draw's viewport is
    /// `viewport` (its depth range is not read).
    pub fn run_lines(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
        viewport: &Viewport,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        self.run_with(backend, draw, None, Some(*viewport))
    }

    /// [`Self::run_lines`] for antialiased lines, whose draw's viewport is `viewport` pixels wide
    /// and high (its place is not needed).
    pub fn run_antialiased_lines(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
        [width, height]: [f32; 2],
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        if self.lines.as_ref().is_some_and(|l| l.diamond) {
            return Err(StageError::Convert(ConvertError::Malformed(
                LINE_ORIGIN.into(),
            )));
        }
        self.run_with(backend, draw, None, Some(sized(width, height)))
    }

    /// [`Self::run`] for a helper made by [`Self::tessellated_with`] with a geometry shader, stream
    /// output or a domain program writing cull distances: `draw.geometry_sets` are the hull stage's,
    /// `later` the stages' after it.
    pub fn run_after_domain(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
        later: &DomainSets<'_, B>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        self.run_with(backend, draw, Some(later), None)
    }

    fn run_with(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
        later: Option<&DomainSets<'_, B>>,
        viewport: Option<Viewport>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        if self.domain.is_some() != later.is_some() {
            return Err(StageError::Convert(ConvertError::Malformed(
                DOMAIN_SETS.into(),
            )));
        }
        if self.lines.is_some() != viewport.is_some() {
            return Err(StageError::Convert(ConvertError::Malformed(
                LINE_VIEWPORT.into(),
            )));
        }
        let (first, count, base_vertex, first_instance, instances) = match draw.range {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => (first, count, 0, first_instance, instances),
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => (first, count, base_vertex, first_instance, instances),
            _ => return Err(StageError::Unsupported(STAGE_INDIRECT)),
        };
        use InputTopology as T;
        let (n, strip) = match draw.topology {
            T::PointList => (1, false),
            T::LineList => (LINE_VERTICES, false),
            T::LineStrip => (LINE_VERTICES, true),
            T::TriangleList => (TRIANGLE_VERTICES, false),
            T::TriangleStrip => (TRIANGLE_VERTICES, true),
            T::LineListAdjacency => (LINE_ADJACENCY_VERTICES, false),
            T::LineStripAdjacency => (LINE_ADJACENCY_VERTICES, true),
            T::TriangleListAdjacency => (TRIANGLE_ADJACENCY_VERTICES, false),
            T::TriangleStripAdjacency => (TRIANGLE_ADJACENCY_VERTICES, true),
            T::PatchList(n) => (u32::from(n), false),
        };
        let patches = matches!(draw.topology, T::PatchList(_));
        match (&self.geometry, &self.hull) {
            (_, Some(h)) if !patches || n < h.input_points => {
                return Err(StageError::TopologyMismatch);
            }
            (None, None) if patches => return Err(StageError::TopologyMismatch),
            // Patches without a hull stage reach a geometry shader taking them, as lists.
            (Some(g), None) if n != g.input_vertices => return Err(StageError::TopologyMismatch),
            (None, None)
                if self.lines.is_some() && !matches!(draw.topology, T::LineList | T::LineStrip) =>
            {
                return Err(StageError::Unsupported(LINE_TOPOLOGY));
            }
            (None, None) if n > TRIANGLE_VERTICES && self.numbered.is_some() => {
                return Err(StageError::Unsupported(PRIMITIVE_ID_ADJACENCY));
            }
            (None, None) if n > TRIANGLE_VERTICES && self.cull.is_some() => {
                return Err(StageError::Unsupported(
                    match self.cull.as_ref().is_some_and(|c| c.distances != 0) {
                        true => CULL_DISTANCE_ADJACENCY,
                        false => VERTEX_STORAGE_ADJACENCY,
                    },
                ));
            }
            (None, None) if n > TRIANGLE_VERTICES => {
                return Err(StageError::Unsupported(STREAM_OUTPUT_ADJACENCY));
            }
            _ => {}
        }
        // A triangle strip with adjacency steps by two places, its first triangle taking six.
        let adjacent_strip = draw.topology == T::TriangleStripAdjacency;
        let primitives = match (strip, adjacent_strip) {
            (false, _) => count / n,
            (true, false) => (count + 1).saturating_sub(n),
            (true, true) => (count / 2).saturating_sub(TRIANGLE_VERTICES - 1),
        };
        let streamed = self.stream.is_some() && draw.stream_output.is_some();
        let drawn = self.geometry.is_some()
            || self.hull.is_some()
            || self.cull.is_some()
            || self.lines.is_some()
            || self.numbered.is_some();
        if primitives == 0 || instances == 0 || (!drawn && !streamed) {
            return Ok(None);
        }
        let limits = backend.caps().limits;
        // An indexed strip, and a triangle strip with adjacency, is first listed as an indexed list
        // of its primitives (a restart index ending a strip, odd triangles taking their last two
        // vertices swapped), which the stages then take as any list, with a place for every
        // primitive the strip could have: those past the listed ones hold none.
        let listed_total = strip && (draw.index.is_some() || adjacent_strip);
        let (count, first, strip) = match listed_total {
            true => {
                let listed = primitives.checked_mul(n).ok_or(GpuError::OutOfMemory)?;
                let index = draw.index.as_ref();
                self.list_strip(backend, index, (first, count), n, listed)?;
                (listed, 0, false)
            }
            false => (count, first, strip),
        };
        let record_count = u64::from(count) * u64::from(instances);
        let record_bytes = record_count * u64::from(self.in_words) * u64::from(WORD);
        // On the cull route the records end with the empty record, then the index list the final draw
        // draws.
        // The cull route of a vertex stage; after a tessellation stage it is the domain stage's.
        let vertex_cull = self.cull.as_ref().filter(|_| self.hull.is_none());
        let (empty, list_base, records_size, records_usage) = match vertex_cull {
            Some(_) => {
                let empty = u32::try_from(record_count).map_err(|_| GpuError::OutOfMemory)?;
                let base = (record_count + 1) * u64::from(self.in_words);
                let base = u32::try_from(base).map_err(|_| GpuError::OutOfMemory)?;
                let listed = u64::from(primitives) * u64::from(instances) * u64::from(n);
                let bytes = (u64::from(base) + listed) * u64::from(WORD);
                let usage = BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX;
                (empty, base, bytes, usage)
            }
            None => (0, 0, record_bytes, BufferUsage::STORAGE),
        };
        grow(backend, &mut self.records, records_size, records_usage)?;
        // After a domain stage: the most points and primitives the tessellator can make for the
        // draw's patches, and the primitives' vertices. The domain stage's compute form runs for every
        // point there can be; the geometry stage for every primitive there can be, those past the
        // listed ones closing their slots.
        let patch_total = primitives
            .checked_mul(instances)
            .ok_or(GpuError::OutOfMemory)?;
        let listed = match (&self.hull, &self.domain) {
            (Some(h), Some(_)) => {
                let (per_points, per_indices) = tess::patch_bounds(h.tessellation.max_factor);
                let vertices = tess::output_vertices(h.tessellation.output);
                let points = u64::from(patch_total) * u64::from(per_points);
                let points = u32::try_from(points.next_multiple_of(u64::from(WORKGROUP)))
                    .map_err(|_| GpuError::OutOfMemory)?;
                let listed = u64::from(patch_total) * u64::from(per_indices / vertices);
                let listed = u32::try_from(listed).map_err(|_| GpuError::OutOfMemory)?;
                Some((points, listed, vertices))
            }
            _ => None,
        };
        // The geometry stage's input primitives per API instance and API instances.
        let (geometry_primitives, geometry_instances) = match listed {
            Some((_, listed, _)) => (listed, 1),
            None => (primitives, instances),
        };
        // The geometry output's size and where its index slots start.
        let (invocations, out_bytes, index_base) = match &self.geometry {
            Some(g) => {
                let invocations = u64::from(geometry_primitives)
                    * u64::from(g.instances)
                    * u64::from(geometry_instances);
                // Each stream's vertex slots, then each stream's index slots. With cull distances, the
                // empty record follows the vertex slots, and the cull program's list the index slots.
                let slots = invocations * u64::from(g.streams) * u64::from(g.max_vertices);
                let culled = g.cull.is_some();
                let vertex_words = (slots + u64::from(culled)) * u64::from(g.out_words);
                let index_words = 2 * slots;
                let list_words = match culled {
                    true => {
                        invocations
                            * u64::from(super::replay_slots(g.max_vertices, g.output))
                            * u64::from(g.output)
                    }
                    false => 0,
                };
                let index_base = u32::try_from(vertex_words).map_err(|_| GpuError::OutOfMemory)?;
                let bytes = (vertex_words + index_words + list_words) * u64::from(WORD);
                u32::try_from(bytes / u64::from(WORD)).map_err(|_| GpuError::OutOfMemory)?;
                let usage = BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX;
                grow(backend, &mut self.out, bytes, usage)?;
                (invocations, bytes, index_base)
            }
            None => match &self.lines {
                Some(_) => {
                    let units = u64::from(primitives) * u64::from(instances);
                    let corner_words = units
                        * u64::from(LINE_CORNERS)
                        * u64::from(self.in_words + crate::build::VEC4);
                    let index_base =
                        u32::try_from(corner_words).map_err(|_| GpuError::OutOfMemory)?;
                    let bytes = (corner_words + units * u64::from(LINE_INDICES)) * u64::from(WORD);
                    let usage = BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX;
                    grow(backend, &mut self.out, bytes, usage)?;
                    (0, bytes, index_base)
                }
                None => match &self.numbered {
                    Some(_) => {
                        let copies = u64::from(primitives) * u64::from(instances) * u64::from(n);
                        let words = copies * u64::from(self.in_words + crate::build::VEC4);
                        let bytes = words * u64::from(WORD);
                        let usage = BufferUsage::STORAGE | BufferUsage::VERTEX;
                        grow(backend, &mut self.out, bytes, usage)?;
                        (0, bytes, 0)
                    }
                    None => (0, 0, 0),
                },
            },
        };

        let strip_list = match &self.strips {
            Some(Strips { list: Some(l), .. }) if listed_total => Some(BufferRange {
                buffer: &l.buffer,
                offset: 0,
                size: l.size,
            }),
            _ => None,
        };
        let (index_bytes, index_offset, indices) = match (&draw.index, strip_list) {
            (_, Some(list)) => (WORD, tess::INDEX_HEADER * WORD, list),
            (Some(index), None) => index_words(index, limits.min_storage_offset_alignment)?,
            (None, _) => (0, 0, self.unused_range(0)),
        };
        // The API draw's values, then those of the stages after a domain stage.
        let mut values = [0u32; word::COUNT_OF as usize];
        values[word::COUNT as usize] = count;
        values[word::FIRST as usize] = first;
        values[word::BASE_VERTEX as usize] = base_vertex as u32;
        values[word::INDEX_BYTES as usize] = index_bytes;
        values[word::INDEX_OFFSET as usize] = index_offset;
        values[word::FIRST_INSTANCE as usize] = first_instance;
        values[word::INSTANCES as usize] = instances;
        values[word::STRIP as usize] = strip.into();
        values[word::PRIMITIVES as usize] = primitives;
        values[word::INDEX_BASE as usize] = index_base;
        values[word::PATCH_POINTS as usize] = n;
        values[word::LISTED_TOTAL as usize] = listed_total.into();
        let mut after = [0u32; word::COUNT_OF as usize];
        if let Some((points, listed, _)) = listed {
            after[word::COUNT as usize] = points;
            after[word::INSTANCES as usize] = 1;
            after[word::PRIMITIVES as usize] = listed;
            after[word::INDEX_BASE as usize] = index_base;
            after[word::LISTED as usize] = 1;
        }
        let bytes = words_as_bytes(&values);
        let after_bytes = words_as_bytes(&after);
        // Each stage's values are written into the data ring just before its set is made: a range of
        // the ring holds only until the ring's next write, which may give it fresh storage.
        let values_part = |part: usize| if part == 0 { &bytes } else { &after_bytes };
        let records = &self.records.as_ref().unwrap().buffer;
        let at = ring_values(backend, &bytes)?;
        let vertex_set = backend.create_binding_set(&[
            values_entry(&at, &bytes),
            BindingEntry {
                binding: INDICES_BINDING,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: indices,
                    writable: false,
                },
            },
            storage(VERTICES_BINDING, records, record_bytes, true),
        ])?;
        let groups = |x: u32, y: u32| DispatchGroups::Direct([x.div_ceil(WORKGROUP), y, 1]);
        let mut sets = draw.vertex_sets;
        sets[super::STAGE_SET as usize] = Some(&vertex_set);
        backend.dispatch(&DispatchCall {
            program: &self.vertex,
            bindings: sets,
            immediates: &[],
            groups: groups(count, instances),
        })?;

        let mut last = None;
        if self.stores {
            backend.record_slow_path(SlowPath::VertexStorageAsCompute);
        }
        if let Some(c) = vertex_cull {
            if c.distances != 0 {
                backend.record_slow_path(SlowPath::CullDistanceAsGeometry);
            }
            let mut cv = [0u32; word_at(size_of::<CullValues>())];
            let at = |offset| word_at(offset);
            cv[at(offset_of!(CullValues, units))] = primitives * instances;
            cv[at(offset_of!(CullValues, primitives))] = primitives;
            cv[at(offset_of!(CullValues, count))] = count;
            cv[at(offset_of!(CullValues, vertices))] = n;
            cv[at(offset_of!(CullValues, strip))] = strip.into();
            cv[at(offset_of!(CullValues, record_words))] = c.words;
            cv[at(offset_of!(CullValues, cull_word))] = c.cull_word;
            cv[at(offset_of!(CullValues, cull_count))] = c.distances;
            cv[at(offset_of!(CullValues, index_base))] = list_base;
            cv[at(offset_of!(CullValues, empty))] = empty;
            let bytes = words_as_bytes(&cv);
            let mut offsets = [0u64];
            let ring = backend.write_buffer_ring(
                RingKind::Data,
                &[&bytes],
                limits.min_uniform_offset_alignment,
                &mut offsets,
            )?;
            let set = backend.create_binding_set(&[
                BindingEntry {
                    binding: CULL_VALUES_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[0],
                        size: bytes.len() as u64,
                    }),
                },
                storage(CULL_DATA_BINDING, records, records_size, true),
                storage(CULL_LIST_BINDING, &self.unused, UNUSED_RANGE_BYTES, false),
            ])?;
            let mut sets = [None; MAX_BINDING_SETS];
            sets[super::STAGE_SET as usize] = Some(&set);
            backend.dispatch(&DispatchCall {
                program: &c.program,
                bindings: sets,
                immediates: &[],
                groups: groups(primitives * instances, 1),
            })?;
            last = Some(FinalDraw {
                buffer: records.clone(),
                index_offset: Some(u64::from(list_base) * u64::from(WORD)),
                topology: list_topology(n),
                count: primitives * instances * n,
                tessellated: None,
            });
        }
        if let (Some(l), Some(viewport)) = (&self.lines, viewport) {
            let [x, y, width, height] =
                [viewport.x, viewport.y, viewport.width, viewport.height].map(f32::from_bits);
            let (path, across, along) = match l.diamond {
                true => (
                    SlowPath::AliasedLinesAsQuads,
                    crate::variant::DIAMOND_REACH,
                    crate::variant::DIAMOND_REACH,
                ),
                false => (
                    SlowPath::AntialiasedLinesAsQuads,
                    crate::variant::LINE_REACH_ACROSS,
                    crate::variant::LINE_REACH_ALONG,
                ),
            };
            backend.record_slow_path(path);
            let units = primitives * instances;
            let mut lv = [0u32; word_at(size_of::<LineValues>())];
            let at = |offset| word_at(offset);
            lv[at(offset_of!(LineValues, units))] = units;
            lv[at(offset_of!(LineValues, primitives))] = primitives;
            lv[at(offset_of!(LineValues, count))] = count;
            lv[at(offset_of!(LineValues, strip))] = strip.into();
            lv[at(offset_of!(LineValues, in_words))] = self.in_words;
            lv[at(offset_of!(LineValues, out_words))] = self.in_words + crate::build::VEC4;
            lv[at(offset_of!(LineValues, index_base))] = index_base;
            lv[at(offset_of!(LineValues, interpolated))] = l.interpolated;
            lv[at(offset_of!(LineValues, linear))] = l.linear;
            lv[at(offset_of!(LineValues, ends))] = l.diamond.into();
            lv[at(offset_of!(LineValues, half_width))] = (width / 2.0).to_bits();
            lv[at(offset_of!(LineValues, half_height))] = (height / 2.0).to_bits();
            lv[at(offset_of!(LineValues, reach_across))] = across.to_bits();
            lv[at(offset_of!(LineValues, reach_along))] = along.to_bits();
            lv[at(offset_of!(LineValues, origin_x))] = x.to_bits();
            lv[at(offset_of!(LineValues, origin_y))] = y.to_bits();
            let bytes = words_as_bytes(&lv);
            let mut offsets = [0u64];
            let ring = backend.write_buffer_ring(
                RingKind::Data,
                &[&bytes],
                limits.min_uniform_offset_alignment,
                &mut offsets,
            )?;
            let out = &self.out.as_ref().unwrap().buffer;
            let set = backend.create_binding_set(&[
                BindingEntry {
                    binding: LINE_VALUES_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[0],
                        size: bytes.len() as u64,
                    }),
                },
                storage(LINE_RECORDS_BINDING, records, record_bytes, false),
                storage(LINE_CORNERS_BINDING, out, out_bytes, true),
            ])?;
            let mut sets = [None; MAX_BINDING_SETS];
            sets[super::STAGE_SET as usize] = Some(&set);
            backend.dispatch(&DispatchCall {
                program: &l.program,
                bindings: sets,
                immediates: &[],
                groups: groups(units, 1),
            })?;
            last = Some(FinalDraw {
                buffer: out.clone(),
                index_offset: Some(u64::from(index_base) * u64::from(WORD)),
                topology: Topology::TriangleList,
                count: units * LINE_INDICES,
                tessellated: None,
            });
        }
        if let Some(p) = &self.numbered {
            backend.record_slow_path(SlowPath::PrimitiveIdAsGeometry);
            let units = primitives * instances;
            let mut pv = [0u32; word_at(size_of::<PrimitiveValues>())];
            let at = |offset| word_at(offset);
            pv[at(offset_of!(PrimitiveValues, units))] = units;
            pv[at(offset_of!(PrimitiveValues, primitives))] = primitives;
            pv[at(offset_of!(PrimitiveValues, count))] = count;
            pv[at(offset_of!(PrimitiveValues, vertices))] = n;
            pv[at(offset_of!(PrimitiveValues, strip))] = strip.into();
            pv[at(offset_of!(PrimitiveValues, in_words))] = self.in_words;
            pv[at(offset_of!(PrimitiveValues, out_words))] = self.in_words + crate::build::VEC4;
            let bytes = words_as_bytes(&pv);
            let mut offsets = [0u64];
            let ring = backend.write_buffer_ring(
                RingKind::Data,
                &[&bytes],
                limits.min_uniform_offset_alignment,
                &mut offsets,
            )?;
            let out = &self.out.as_ref().unwrap().buffer;
            let set = backend.create_binding_set(&[
                BindingEntry {
                    binding: PRIMITIVE_VALUES_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[0],
                        size: bytes.len() as u64,
                    }),
                },
                storage(PRIMITIVE_RECORDS_BINDING, records, record_bytes, false),
                storage(PRIMITIVE_COPIES_BINDING, out, out_bytes, true),
            ])?;
            let mut sets = [None; MAX_BINDING_SETS];
            sets[super::STAGE_SET as usize] = Some(&set);
            backend.dispatch(&DispatchCall {
                program: &p.program,
                bindings: sets,
                immediates: &[],
                groups: groups(units, 1),
            })?;
            last = Some(FinalDraw {
                buffer: out.clone(),
                index_offset: None,
                topology: list_topology(n),
                count: units * n,
                tessellated: None,
            });
        }
        // The tessellator's primitives are turned when they are the ones drawn.
        let rotate = match self.geometry {
            None => self.rotate,
            Some(_) => ROTATE_NONE,
        };
        if let Some(h) = &mut self.hull {
            backend.record_slow_path(SlowPath::TessellationAsCompute);
            if rotate != ROTATE_NONE {
                backend.record_slow_path(SlowPath::ProvokingRewrite);
            }
            let patch_bytes = (u64::from(tess::HEADER_WORDS)
                + u64::from(patch_total) * u64::from(h.patch_words))
                * u64::from(WORD);
            grow(backend, &mut h.patches, patch_bytes, BufferUsage::STORAGE)?;
            let patches = &h.patches.as_ref().unwrap().buffer;
            let at = ring_values(backend, &bytes)?;
            let hull_set = backend.create_binding_set(&[
                values_entry(&at, &bytes),
                storage(VERTICES_BINDING, records, record_bytes, false),
                storage(PATCHES_BINDING, patches, patch_bytes, true),
            ])?;
            let mut sets = draw.geometry_sets;
            sets[super::STAGE_SET as usize] = Some(&hull_set);
            backend.dispatch(&DispatchCall {
                program: &h.program,
                bindings: sets,
                immediates: &[],
                groups: groups(primitives, instances),
            })?;
            let hull_range = BufferRange {
                buffer: patches,
                offset: 0,
                size: patch_bytes,
            };
            h.tessellator.run(
                backend,
                &h.tessellation,
                hull_range,
                patch_total,
                primitives,
                rotate,
            )?;
            let (_, per_indices) = tess::patch_bounds(h.tessellation.max_factor);
            let (arguments, points, indices) = h.tessellator.owned();
            last = Some(FinalDraw {
                buffer: indices.buffer.clone(),
                index_offset: Some(0),
                topology: match h.tessellation.output {
                    0 => Topology::PointList,
                    1 => Topology::LineList,
                    _ => Topology::TriangleList,
                },
                count: patch_total.saturating_mul(per_indices),
                tessellated: Some(TessellatedDraw {
                    arguments: arguments.buffer.clone(),
                    patches: BufferRange {
                        buffer: patches.clone(),
                        offset: 0,
                        size: patch_bytes,
                    },
                    points: BufferRange {
                        buffer: points.buffer.clone(),
                        offset: 0,
                        size: points.size,
                    },
                }),
            });
        }

        // The domain stage's records, for the stage after it.
        // With cull distances they end with the empty record, then the index list the final draw
        // draws.
        let mut domain_records = None;
        let domain_cull = self.cull.as_ref().filter(|_| self.hull.is_some());
        if let (Some(d), Some((points, listed, vertices))) = (&mut self.domain, listed) {
            let words = u64::from(points) * u64::from(d.words);
            let (words, usage) = match domain_cull {
                Some(_) => (
                    words + u64::from(d.words) + u64::from(listed) * u64::from(vertices),
                    BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX,
                ),
                None => (words, BufferUsage::STORAGE),
            };
            u32::try_from(words).map_err(|_| GpuError::OutOfMemory)?;
            grow(backend, &mut d.records, words * u64::from(WORD), usage)?;
        }
        if let (Some(d), Some(later), Some(h), Some((points, _, _))) =
            (&self.domain, later, &self.hull, listed)
        {
            let bytes = u64::from(points) * u64::from(d.words) * u64::from(WORD);
            let out = &d.records.as_ref().unwrap().buffer;
            let (_, tess_points, _) = h.tessellator.owned();
            let patches = &h.patches.as_ref().unwrap();
            let at = ring_values(backend, &after_bytes)?;
            let set = backend.create_binding_set(&[
                values_entry(&at, &after_bytes),
                BindingEntry {
                    binding: INDICES_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: self.unused_range(0),
                        writable: false,
                    },
                },
                storage(VERTICES_BINDING, out, bytes, true),
                storage(PATCHES_BINDING, &patches.buffer, patches.size, false),
                storage(POINTS_BINDING, &tess_points.buffer, tess_points.size, false),
            ])?;
            let mut sets = later.domain_sets;
            sets[super::STAGE_SET as usize] = Some(&set);
            backend.dispatch(&DispatchCall {
                program: &d.program,
                bindings: sets,
                immediates: &[],
                groups: groups(points, 1),
            })?;
            domain_records = Some((out, bytes));
        }
        // The tessellator's index list, which lists the domain stage's primitives.
        let list = match (&self.hull, listed) {
            (Some(h), Some(_)) => {
                let (_, _, indices) = h.tessellator.owned();
                Some(BufferRange {
                    buffer: &indices.buffer,
                    offset: 0,
                    size: indices.size,
                })
            }
            _ => strip_list,
        };
        if let (Some(c), Some(d), Some(indices), Some((points, listed, vertices))) =
            (domain_cull, &self.domain, list, listed)
        {
            // The tessellator's primitives the domain stage's cull distances keep, listed.
            backend.record_slow_path(SlowPath::CullDistanceAsGeometry);
            let records = d.records.as_ref().unwrap();
            let list_base = (points + 1) * d.words;
            let mut cv = [0u32; word_at(size_of::<CullValues>())];
            let at = |offset| word_at(offset);
            cv[at(offset_of!(CullValues, units))] = listed;
            cv[at(offset_of!(CullValues, primitives))] = listed;
            cv[at(offset_of!(CullValues, vertices))] = vertices;
            cv[at(offset_of!(CullValues, record_words))] = c.words;
            cv[at(offset_of!(CullValues, cull_word))] = c.cull_word;
            cv[at(offset_of!(CullValues, cull_count))] = c.distances;
            cv[at(offset_of!(CullValues, index_base))] = list_base;
            cv[at(offset_of!(CullValues, empty))] = points;
            cv[at(offset_of!(CullValues, listed))] = 1;
            let bytes = words_as_bytes(&cv);
            let mut offsets = [0u64];
            let ring = backend.write_buffer_ring(
                RingKind::Data,
                &[&bytes],
                limits.min_uniform_offset_alignment,
                &mut offsets,
            )?;
            let set = backend.create_binding_set(&[
                BindingEntry {
                    binding: CULL_VALUES_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[0],
                        size: bytes.len() as u64,
                    }),
                },
                storage(CULL_DATA_BINDING, &records.buffer, records.size, true),
                BindingEntry {
                    binding: CULL_LIST_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: indices,
                        writable: false,
                    },
                },
            ])?;
            let mut sets = [None; MAX_BINDING_SETS];
            sets[super::STAGE_SET as usize] = Some(&set);
            backend.dispatch(&DispatchCall {
                program: &c.program,
                bindings: sets,
                immediates: &[],
                groups: groups(listed, 1),
            })?;
            last = Some(FinalDraw {
                buffer: records.buffer.clone(),
                index_offset: Some(u64::from(list_base) * u64::from(WORD)),
                topology: list_topology(vertices),
                count: listed * vertices,
                tessellated: None,
            });
        }

        if let Some(g) = &self.geometry {
            backend.record_slow_path(SlowPath::GeometryAsCompute);
            let out = &self.out.as_ref().unwrap().buffer;
            let (input, input_bytes, part, sets) = match (domain_records, later) {
                (Some((r, bytes)), Some(later)) => (r, bytes, 1, later.geometry_sets),
                _ => (records, record_bytes, 0, draw.geometry_sets),
            };
            let part = values_part(part);
            let at = ring_values(backend, part)?;
            let mut entries = vec![
                values_entry(&at, part),
                BindingEntry {
                    binding: INDICES_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: list.unwrap_or(self.unused_range(0)),
                        writable: false,
                    },
                },
                storage(VERTICES_BINDING, input, input_bytes, false),
                storage(GEOMETRY_BINDING, out, out_bytes, true),
            ];
            if g.numbered {
                // The points hold listed primitives' numbers; unlisted ones are numbered in order.
                let range = match (&self.hull, listed) {
                    (Some(h), Some(_)) => {
                        let (_, points, _) = h.tessellator.owned();
                        BufferRange {
                            buffer: &points.buffer,
                            offset: 0,
                            size: points.size,
                        }
                    }
                    _ => self.unused_range(0),
                };
                entries.push(BindingEntry {
                    binding: POINTS_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range,
                        writable: false,
                    },
                });
            }
            let geometry_set = backend.create_binding_set(&entries)?;
            let mut sets = sets;
            sets[super::STAGE_SET as usize] = Some(&geometry_set);
            backend.dispatch(&DispatchCall {
                program: &g.program,
                bindings: sets,
                immediates: &[],
                groups: groups(geometry_primitives * g.instances, geometry_instances),
            })?;
            let total = u32::try_from(invocations * u64::from(g.max_vertices))
                .map_err(|_| GpuError::OutOfMemory)?;
            let strips = g.output != 1;
            last = Some(FinalDraw {
                buffer: out.clone(),
                index_offset: strips.then_some(u64::from(index_base) * u64::from(WORD)),
                topology: match g.output {
                    1 => Topology::PointList,
                    2 => Topology::LineStrip,
                    _ => Topology::TriangleStrip,
                },
                count: if strips { 2 * total } else { total },
                tessellated: None,
            });
            let listing = g
                .cull
                .as_ref()
                .filter(|c| c.distances > 0 || self.rotate != ROTATE_NONE);
            if let Some(c) = listing {
                // The drawn stream's primitives its cull distances keep, listed and turned as the
                // provoking vertex needs; stream output still takes every primitive from the index
                // slots.
                if c.distances > 0 {
                    backend.record_slow_path(SlowPath::CullDistanceAsGeometry);
                }
                if self.rotate != ROTATE_NONE {
                    backend.record_slow_path(SlowPath::ProvokingRewrite);
                }
                let units = u32::try_from(invocations).map_err(|_| GpuError::OutOfMemory)?;
                let index_slots = 2 * g.max_vertices;
                let primitives = super::replay_slots(g.max_vertices, g.output);
                let slots = u32::try_from(invocations * u64::from(g.streams))
                    .ok()
                    .and_then(|i| i.checked_mul(g.max_vertices))
                    .ok_or(GpuError::OutOfMemory)?;
                let list_base = index_base + slots * 2;
                let mut cv = [0u32; word_at(size_of::<GeometryCullValues>())];
                let at = |offset| word_at(offset);
                cv[at(offset_of!(GeometryCullValues, invocations))] = units;
                cv[at(offset_of!(GeometryCullValues, index_slots))] = index_slots;
                cv[at(offset_of!(GeometryCullValues, primitives))] = primitives;
                cv[at(offset_of!(GeometryCullValues, vertices))] = g.output;
                cv[at(offset_of!(GeometryCullValues, record_words))] = g.out_words;
                cv[at(offset_of!(GeometryCullValues, cull_word))] = c.cull_word;
                cv[at(offset_of!(GeometryCullValues, cull_count))] = c.distances;
                cv[at(offset_of!(GeometryCullValues, index_base))] = index_base;
                cv[at(offset_of!(GeometryCullValues, list_base))] = list_base;
                cv[at(offset_of!(GeometryCullValues, empty))] = slots;
                cv[at(offset_of!(GeometryCullValues, rotate))] = self.rotate;
                let bytes = words_as_bytes(&cv);
                let mut offsets = [0u64];
                let ring = backend.write_buffer_ring(
                    RingKind::Data,
                    &[&bytes],
                    limits.min_uniform_offset_alignment,
                    &mut offsets,
                )?;
                let set = backend.create_binding_set(&[
                    BindingEntry {
                        binding: CULL_VALUES_BINDING,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &ring,
                            offset: offsets[0],
                            size: bytes.len() as u64,
                        }),
                    },
                    storage(CULL_DATA_BINDING, out, out_bytes, true),
                ])?;
                let mut sets = [None; MAX_BINDING_SETS];
                sets[super::STAGE_SET as usize] = Some(&set);
                backend.dispatch(&DispatchCall {
                    program: &c.program,
                    bindings: sets,
                    immediates: &[],
                    groups: groups(units, 1),
                })?;
                last = Some(FinalDraw {
                    buffer: out.clone(),
                    index_offset: Some(u64::from(list_base) * u64::from(WORD)),
                    topology: list_topology(g.output),
                    count: units * primitives * g.output,
                    tessellated: None,
                });
            }
        }

        if let (Some(declared), Some(targets)) = (&self.stream, &draw.stream_output) {
            backend.record_slow_path(SlowPath::StreamOutAsCompute);
            let streams = self.geometry.as_ref().map_or(1, |g| g.streams);
            let filled_words = STREAM_BUFFERS as u64 + 2 * u64::from(streams);
            if targets.filled.size < filled_words * u64::from(WORD) {
                return Err(StageError::Convert(ConvertError::Malformed(
                    STREAM_FILLED_SIZE.into(),
                )));
            }
            for stream in 0..streams {
                let mut so = [0u32; word_at(size_of::<StreamValues>())];
                let at = |offset| word_at(offset);
                let (data, data_bytes) = match (&self.geometry, domain_records, listed) {
                    (Some(g), _, _) => {
                        so[at(offset_of!(StreamValues, units))] = invocations as u32;
                        so[at(offset_of!(StreamValues, geometry))] = 1;
                        so[at(offset_of!(StreamValues, vertices))] = g.output;
                        so[at(offset_of!(StreamValues, slots))] = 2 * g.max_vertices;
                        let before =
                            u64::from(stream) * invocations * 2 * u64::from(g.max_vertices);
                        so[at(offset_of!(StreamValues, index_base))] =
                            u32::try_from(u64::from(index_base) + before)
                                .map_err(|_| GpuError::OutOfMemory)?;
                        so[at(offset_of!(StreamValues, record_words))] = g.out_words;
                        (&self.out.as_ref().unwrap().buffer, out_bytes)
                    }
                    (None, Some((r, bytes)), Some((_, _, vertices))) => {
                        so[at(offset_of!(StreamValues, vertices))] = vertices;
                        so[at(offset_of!(StreamValues, record_words))] =
                            self.domain.as_ref().unwrap().words;
                        so[at(offset_of!(StreamValues, listed))] = 1;
                        (r, bytes)
                    }
                    _ => {
                        so[at(offset_of!(StreamValues, units))] = primitives * instances;
                        so[at(offset_of!(StreamValues, vertices))] = n;
                        so[at(offset_of!(StreamValues, strip))] = strip.into();
                        so[at(offset_of!(StreamValues, listed_total))] = listed_total.into();
                        so[at(offset_of!(StreamValues, record_words))] = self.in_words;
                        (records, record_bytes)
                    }
                };
                so[at(offset_of!(StreamValues, primitives))] = primitives;
                so[at(offset_of!(StreamValues, count))] = count;
                so[at(offset_of!(StreamValues, stream))] = stream;
                // Elements of buffers not bound are left out; such a buffer neither holds nor limits.
                let mut entries = 0;
                for (s, e) in &declared.entries {
                    let buffer = (e[0] & u32::from(u8::MAX)) as usize;
                    if u32::from(*s) != stream || targets.buffers[buffer].is_none() {
                        continue;
                    }
                    let first =
                        at(offset_of!(StreamValues, table) + size_of::<StreamEntry>() * entries);
                    so[first..first + ENTRY_WORDS].copy_from_slice(e);
                    entries += 1;
                }
                so[at(offset_of!(StreamValues, entries))] = entries as u32;
                for (b, target) in targets.buffers.iter().enumerate() {
                    if let Some(t) = target
                        && u32::from(declared.buffer_streams[b]) == stream
                    {
                        so[at(offset_of!(StreamValues, strides)) + b] = declared.strides[b];
                        so[at(offset_of!(StreamValues, sizes)) + b] =
                            u32::try_from(t.size).unwrap_or(u32::MAX);
                    }
                }
                let bytes = words_as_bytes(&so);
                let mut offsets = [0u64];
                let ring = backend.write_buffer_ring(
                    RingKind::Data,
                    &[&bytes],
                    limits.min_uniform_offset_alignment,
                    &mut offsets,
                )?;
                let mut entries = alloc::vec![
                    BindingEntry {
                        binding: STREAM_VALUES_BINDING,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &ring,
                            offset: offsets[0],
                            size: bytes.len() as u64,
                        }),
                    },
                    storage(STREAM_DATA_BINDING, data, data_bytes, false),
                    BindingEntry {
                        binding: STREAM_FILLED_BINDING,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::StorageBuffer {
                            range: BufferRange {
                                buffer: targets.filled.buffer,
                                offset: targets.filled.offset,
                                size: targets.filled.size,
                            },
                            writable: true,
                        },
                    },
                ];
                for (b, target) in targets.buffers.iter().enumerate() {
                    let range = match target {
                        Some(t) => BufferRange {
                            buffer: t.buffer,
                            offset: t.offset,
                            size: t.size,
                        },
                        None => self.unused_range(1 + b as u64),
                    };
                    entries.push(BindingEntry {
                        binding: STREAM_OUT_BINDING + b as u32,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::StorageBuffer {
                            range,
                            writable: true,
                        },
                    });
                }
                // Without listed primitives the list is the data itself, read the same way.
                let list = match (&self.geometry, list) {
                    (None, Some(list)) => list,
                    _ => BufferRange {
                        buffer: data,
                        offset: 0,
                        size: data_bytes,
                    },
                };
                entries.push(BindingEntry {
                    binding: STREAM_LIST_BINDING,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: list,
                        writable: false,
                    },
                });
                let set = backend.create_binding_set(&entries)?;
                let mut sets = [None; MAX_BINDING_SETS];
                sets[super::STAGE_SET as usize] = Some(&set);
                backend.dispatch(&DispatchCall {
                    program: &declared.program,
                    bindings: sets,
                    immediates: &[],
                    groups: DispatchGroups::Direct([1, 1, 1]),
                })?;
            }
        }
        Ok(last)
    }

    /// Records the strip program's dispatch listing the primitives of `vertices` each that the
    /// draw's places (`first` and `count` of its indices, or of its vertices without `index`) make,
    /// into a list of `listed` places.
    fn list_strip(
        &mut self,
        backend: &mut B,
        index: Option<&StageIndex<'_, B>>,
        (first, count): (u32, u32),
        vertices: u32,
        listed: u32,
    ) -> Result<(), StageError> {
        let limits = backend.caps().limits;
        let (index_bytes, index_offset, indices) = match index {
            Some(index) => index_words(index, limits.min_storage_offset_alignment)?,
            // The stand-in's first range, borrowed apart from the strip program.
            None => (
                0,
                0,
                BufferRange {
                    buffer: &self.unused,
                    offset: 0,
                    size: UNUSED_RANGE_BYTES,
                },
            ),
        };
        let strips = match &mut self.strips {
            Some(s) => s,
            None => {
                let shader = backend.create_shader(&super::strip_program()?)?;
                let program = backend.create_program(&Program::Compute(&shader))?;
                self.strips.insert(Strips {
                    program,
                    list: None,
                })
            }
        };
        let bytes = (u64::from(tess::INDEX_HEADER) + u64::from(listed)) * u64::from(WORD);
        grow(backend, &mut strips.list, bytes, BufferUsage::STORAGE)?;
        let mut sv = [0u32; word_at(size_of::<StripValues>())];
        let at = |offset| word_at(offset);
        sv[at(offset_of!(StripValues, count))] = count;
        sv[at(offset_of!(StripValues, first))] = first;
        sv[at(offset_of!(StripValues, index_bytes))] = index_bytes;
        sv[at(offset_of!(StripValues, index_offset))] = index_offset;
        sv[at(offset_of!(StripValues, vertices))] = vertices;
        sv[at(offset_of!(StripValues, restart))] = match index.map(|i| i.format) {
            Some(IndexFormat::U16) => u32::from(u16::MAX),
            _ => u32::MAX,
        };
        sv[at(offset_of!(StripValues, header))] = tess::INDEX_HEADER;
        let values = words_as_bytes(&sv);
        let mut offsets = [0u64];
        let ring = backend.write_buffer_ring(
            RingKind::Data,
            &[&values],
            limits.min_uniform_offset_alignment,
            &mut offsets,
        )?;
        let list = &strips.list.as_ref().unwrap().buffer;
        let set = backend.create_binding_set(&[
            BindingEntry {
                binding: STRIP_VALUES_BINDING,
                stages: StageSet::COMPUTE,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ring,
                    offset: offsets[0],
                    size: values.len() as u64,
                }),
            },
            BindingEntry {
                binding: STRIP_INDICES_BINDING,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: indices,
                    writable: false,
                },
            },
            storage(STRIP_LIST_BINDING, list, bytes, true),
        ])?;
        let mut sets = [None; MAX_BINDING_SETS];
        sets[super::STAGE_SET as usize] = Some(&set);
        backend.dispatch(&DispatchCall {
            program: &strips.program,
            bindings: sets,
            immediates: &[],
            groups: DispatchGroups::Direct([1, 1, 1]),
        })?;
        Ok(())
    }

    /// Range `i` of the stand-in buffer.
    fn unused_range(&self, i: u64) -> BufferRange<&B::Buffer> {
        BufferRange {
            buffer: &self.unused,
            offset: self.unused_stride * i,
            size: UNUSED_RANGE_BYTES,
        }
    }
}

/// The geometry stage's compute program and its output's shape.
fn geometry_parts<B: Gpu>(
    backend: &mut B,
    geometry: &Converted,
) -> Result<Geometry<B>, StageError> {
    let StageExtras::Geometry {
        input_vertices,
        max_output_vertices,
        output_vertices,
        instances,
        streams,
    } = geometry.reflection.extras
    else {
        return Err(StageError::Convert(ConvertError::Malformed(
            NOT_A_GEOMETRY_SHADER.into(),
        )));
    };
    let selects = super::selects(&geometry.reflection);
    let slots = super::drawn_slots(&geometry.reflection) as usize;
    let culled = super::cull_distances(&geometry.reflection);
    let cull = match culled {
        0 => None,
        _ => {
            let shader = backend.create_shader(&super::geometry_cull_program()?)?;
            Some(GeometryCull {
                program: backend.create_program(&Program::Compute(&shader))?,
                cull_word: super::record_words(slots),
                distances: culled,
            })
        }
    };
    let shader = backend.create_shader(&geometry.modules[0])?;
    Ok(Geometry {
        program: backend.create_program(&Program::Compute(&shader))?,
        passthrough: geometry.modules[1].clone(),
        input_vertices: input_vertices.into(),
        max_vertices: max_output_vertices,
        instances,
        output: output_vertices.into(),
        streams: streams.into(),
        out_words: super::record_words(slots + super::distance_slots(culled) as usize),
        out_locations: slots as u32,
        selects,
        cull,
        numbered: geometry
            .reflection
            .inputs
            .iter()
            .any(|v| v.system == Some(wcontainers_gpu::SystemValue::PrimitiveId)),
    })
}

/// The stream output program, and the declaration's elements located in the records of the stage
/// whose `outputs` are streamed.
fn stream_parts<B: Gpu>(
    backend: &mut B,
    so: &StreamOutput,
    outputs: &[StageValue],
    streams: u32,
) -> Result<Stream<B>, StageError> {
    let declared = stream_declaration(so, outputs, streams)?;
    let shader = backend.create_shader(&super::stream_program()?)?;
    Ok(Stream {
        program: backend.create_program(&Program::Compute(&shader))?,
        entries: declared.entries,
        strides: declared.strides,
        buffer_streams: declared.buffer_streams,
    })
}

/// `so`'s elements located in the records of the stage whose `outputs` are streamed, `streams` the
/// streams it writes.
pub(super) fn stream_declaration(
    so: &StreamOutput,
    outputs: &[StageValue],
    streams: u32,
) -> Result<StreamDeclaration, StageError> {
    let locations = super::locations(outputs);
    let mut ends = [0u32; STREAM_BUFFERS];
    let mut entries = Vec::new();
    let mut buffer_streams: [Option<u8>; STREAM_BUFFERS] = [None; STREAM_BUFFERS];
    for e in &so.entries {
        let (buffer, count, start) = (
            usize::from(e.slot),
            u32::from(e.component_count),
            u32::from(e.start_component),
        );
        if buffer >= STREAM_BUFFERS || count == 0 || start + count > crate::build::VEC4 {
            return Err(StageError::Declaration);
        }
        // A buffer takes one stream's elements, and only streams the shader writes.
        if u32::from(e.stream) >= streams || buffer_streams[buffer].is_some_and(|s| s != e.stream) {
            return Err(StageError::Declaration);
        }
        buffer_streams[buffer] = Some(e.stream);
        if let Some(name) = &e.semantic {
            let value = outputs
                .iter()
                .find(|v| {
                    v.stream == e.stream
                        && v.semantic.eq_ignore_ascii_case(name)
                        && v.semantic_index == e.semantic_index
                })
                .ok_or(StageError::Declaration)?;
            let (slot, first) = match value.system {
                Some(SystemValue::Position) => (0, start),
                None => {
                    let i = locations.iter().position(|&l| l == value.location).unwrap();
                    (1 + i as u32, u32::from(value.component) + start)
                }
                Some(_) => return Err(StageError::Unsupported(STREAM_OUTPUT_SYSTEM_VALUE)),
            };
            if first + count > crate::build::VEC4 {
                return Err(StageError::Declaration);
            }
            let packed = buffer as u32 | first << PACKED_FIRST_SHIFT | count << PACKED_COUNT_SHIFT;
            entries.push((e.stream, [packed, ends[buffer], slot]));
        }
        ends[buffer] += WORD * count;
    }
    if entries.len() > STREAM_ENTRIES {
        return Err(StageError::Unsupported(STREAM_OUTPUT_ENTRIES));
    }
    // A buffer's stride holds its elements; a single buffer declared without one is packed.
    let mut strides = so.strides;
    for b in 0..STREAM_BUFFERS {
        if strides[b] == 0 {
            strides[b] = ends[b];
        } else if strides[b] < ends[b] || !strides[b].is_multiple_of(WORD) {
            return Err(StageError::Declaration);
        }
    }
    Ok(StreamDeclaration {
        entries,
        strides,
        buffer_streams: buffer_streams.map(|s| s.unwrap_or(0)),
    })
}

/// The list of primitives of `n` vertices.
/// The draw's index buffer as the storage words a compute program reads: bytes per index, bytes from
/// the bound words' start to the API's offset, and the bound range.
fn index_words<'a, B: Gpu>(
    index: &StageIndex<'a, B>,
    alignment: u32,
) -> Result<(u32, u32, BufferRange<&'a B::Buffer>), StageError> {
    let bytes = match index.format {
        IndexFormat::U16 => size_of::<u16>() as u32,
        IndexFormat::U32 => size_of::<u32>() as u32,
        IndexFormat::U8 => return Err(StageError::Unsupported(STAGE_INDEX_U8)),
    };
    let alignment = u64::from(alignment);
    let start = index.offset - index.offset % alignment;
    let size = (index.size - start) & !(u64::from(WORD) - 1);
    let range = BufferRange {
        buffer: index.buffer,
        offset: start,
        size,
    };
    Ok((bytes, (index.offset - start) as u32, range))
}

/// The pass-through's vertex layout: records of `words` words in stream 0, read as `slots + 1`
/// four-word slots from location 0.
pub(super) fn record_layout(words: u32, slots: u32) -> VertexLayoutDesc {
    let mut desc = VertexLayoutDesc {
        streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: words * WORD,
        step: StepMode::Vertex,
    });
    for slot in 0..=slots {
        desc.attributes[slot as usize] = Some(VertexAttribute {
            location: slot,
            stream: 0,
            offset: super::RECORD_SLOT_BYTES * slot,
            format: VertexFormat::Uint32x4,
        });
    }
    desc
}

pub(super) fn list_topology(n: u32) -> Topology {
    match n {
        1 => Topology::PointList,
        LINE_VERTICES => Topology::LineList,
        _ => Topology::TriangleList,
    }
}

pub(super) fn words_as_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Writes a stage's values into the data ring, for a set made before the ring's next write.
fn ring_values<B: Gpu>(backend: &mut B, bytes: &[u8]) -> Result<(B::Buffer, u64), GpuError> {
    let align = backend.caps().limits.min_uniform_offset_alignment;
    let mut offsets = [0u64];
    let ring = backend.write_buffer_ring(RingKind::Data, &[bytes], align, &mut offsets)?;
    Ok((ring, offsets[0]))
}

/// The binding of values [`ring_values`] wrote at `at`.
fn values_entry<'a, B: Gpu>(at: &'a (B::Buffer, u64), bytes: &[u8]) -> BindingEntry<'a, B> {
    BindingEntry {
        binding: VALUES_BINDING,
        stages: StageSet::COMPUTE,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer: &at.0,
            offset: at.1,
            size: bytes.len() as u64,
        }),
    }
}

/// A storage binding of the first `size` bytes of `buffer`, for the helper's compute programs.
fn storage<B: Gpu>(
    binding: u32,
    buffer: &B::Buffer,
    size: u64,
    writable: bool,
) -> BindingEntry<'_, B> {
    BindingEntry {
        binding,
        stages: StageSet::COMPUTE,
        resource: BindingResource::StorageBuffer {
            range: BufferRange {
                buffer,
                offset: 0,
                size,
            },
            writable,
        },
    }
}

/// Makes `owned` a buffer of at least `size` bytes.
pub(super) fn grow<B: Gpu>(
    backend: &mut B,
    owned: &mut Option<Owned<B>>,
    size: u64,
    usage: BufferUsage,
) -> Result<(), GpuError> {
    if owned.as_ref().is_some_and(|o| o.size >= size) {
        return Ok(());
    }
    let size = size.max(UNUSED_RANGE_BYTES).next_power_of_two();
    let buffer = backend.create_buffer(&BufferDesc {
        size,
        usage,
        intent: MapIntent::Rarely,
    })?;
    if let Some(old) = owned.take() {
        backend.destroy(wcontainers_gpu::Object::Buffer(old.buffer));
    }
    *owned = Some(Owned { buffer, size });
    Ok(())
}
