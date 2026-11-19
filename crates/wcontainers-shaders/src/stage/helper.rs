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
    Backend, BindingEntry, BindingResource, BufferDesc, BufferRange, BufferUsage, DispatchCall,
    DispatchGroups, DrawRange, GpuError, IndexFormat, MAX_BINDING_SETS, MapIntent, Program,
    RingKind, ShaderModule, SlowPath, StageExtras, StageSet, StageValue, StepMode, SystemValue,
    Topology, VertexAttribute, VertexFormat, VertexLayoutDesc, VertexStream,
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

/// Where one draw's stream output goes.
#[derive(Debug)]
pub struct StreamTargets<'a, B: Backend> {
    /// The bound buffers (made with [`BufferUsage::STORAGE`]), each written from its range's offset
    /// (aligned as storage bindings are) plus its filled size.
    pub buffers: [Option<BufferRange<&'a B::Buffer>>; STREAM_BUFFERS],
    /// Eight u32 (storage, writable, made with [`BufferUsage::STORAGE`]): the bytes each buffer is
    /// filled to, where the draw appends and which it advances (the front writes them for
    /// Direct3D's explicit offsets and keeps them for `DrawAuto`), then the primitives the draw wrote
    /// and those it made (Direct3D's stream output statistics).
    pub filled: BufferRange<&'a B::Buffer>,
}

/// The draw's index buffer, which the vertex stage's compute form reads as storage: made with
/// [`BufferUsage::STORAGE`] as well as [`BufferUsage::INDEX`].
#[derive(Debug)]
pub struct StageIndex<'a, B: Backend> {
    pub buffer: &'a B::Buffer,
    /// The buffer's size in bytes, a multiple of 4.
    pub size: u64,
    pub offset: u64,
    pub format: IndexFormat,
}

/// One API draw through the stage helper.
#[derive(Debug)]
pub struct StageDraw<'a, B: Backend> {
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
pub struct DomainSets<'a, B: Backend> {
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
/// says how it differs.
#[derive(Debug)]
pub struct FinalDraw<B: Backend> {
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
pub struct TessellatedDraw<B: Backend> {
    pub arguments: B::Buffer,
    pub patches: BufferRange<B::Buffer>,
    pub points: BufferRange<B::Buffer>,
}

/// Buffers the helper owns, grown as draws need.
pub(super) struct Owned<B: Backend> {
    pub(super) buffer: B::Buffer,
    pub(super) size: u64,
}

/// The geometry stage's compute program and what its output is.
struct Geometry<B: Backend> {
    program: B::Program,
    passthrough: ShaderModule,
    input_vertices: u32,
    max_vertices: u32,
    instances: u32,
    /// Vertices per output primitive: 1, 2 or 3.
    output: u32,
    out_words: u32,
    /// Vertex attributes after the position: the output locations, then the layer and viewport slot
    /// when the shader writes either.
    out_locations: u32,
    selects: bool,
}

/// The hull stage's compute program, the tessellator, and the patch records between them.
struct Hull<B: Backend> {
    program: B::Program,
    tessellator: Tessellator<B>,
    tessellation: Tessellation,
    input_points: u32,
    /// Words per patch record.
    patch_words: u32,
    patches: Option<Owned<B>>,
}

/// The domain stage's compute form, which runs when a geometry stage or stream output follows it.
struct Domain<B: Backend> {
    program: B::Program,
    /// Words per record it writes.
    words: u32,
    records: Option<Owned<B>>,
}

/// The cull program and where the records hold what it reads.
struct Cull<B: Backend> {
    program: B::Program,
    passthrough: ShaderModule,
    /// Slots the pass-through reads after the position: the locations and the clip distances.
    drawn_slots: u32,
    /// The word of a record where the cull distances start, and their count.
    cull_word: u32,
    distances: u32,
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
    pad: [u32; 2],
}

/// The line program and what its corners hold.
struct Lines<B: Backend> {
    program: B::Program,
    passthrough: ShaderModule,
    /// Slots the pass-through reads after the position: the locations, the clip distances and the
    /// coverage.
    drawn_slots: u32,
    interpolated: u32,
    linear: u32,
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
    pad: u32,
    half_width: f32,
    half_height: f32,
    reach_across: f32,
    reach_along: f32,
    pad_end: [u32; 2],
}

/// The line program's bindings: its values, the vertex records, and the corners followed by the index
/// list.
const LINE_VALUES_BINDING: u32 = 0;
const LINE_RECORDS_BINDING: u32 = 1;
const LINE_CORNERS_BINDING: u32 = 2;
/// Corners of a line's rectangle, and the indices of its two triangles.
const LINE_CORNERS: u32 = 4;
const LINE_INDICES: u32 = 2 * TRIANGLE_VERTICES;
/// The vertices of a line.
const LINE_VERTICES: u32 = 2;
/// A draw on the line route of another topology than lines.
const LINE_TOPOLOGY: &str = "AntialiasedLinesTopology";
/// The line route run without the viewport's size, or another route with it.
const LINE_VIEWPORT: &str = "the viewport's size, given to the line route alone";

/// The cull program's bindings: its values, and the records followed by the index list.
const CULL_VALUES_BINDING: u32 = 0;
const CULL_DATA_BINDING: u32 = 1;
/// Refusals by name, and what is malformed in a call.
const WITHOUT_WORK: &str = "the stage helper without work";
const GEOMETRY_TRIANGLE_STRIP_ADJACENCY: &str = "GeometryTriangleStripAdjacency";
const STAGE_INDEX_U8: &str = "StageIndexU8";
const STAGE_INDEXED_STRIP: &str = "StageIndexedStrip";
const STAGE_INDIRECT: &str = "StageIndirect";
const DOMAIN_SETS: &str = "the sets of the stages after a domain stage";
const GEOMETRY_PRIMITIVE_ID_AFTER_DOMAIN: &str = "GeometryPrimitiveIdAfterDomain";
const NOT_A_DOMAIN_SHADER: &str = "not a domain shader";
const NOT_A_HULL_SHADER: &str = "not a hull shader";
const NOT_A_GEOMETRY_SHADER: &str = "not a geometry shader";
const STREAM_OUTPUT_ENTRIES: &str = "StreamOutputEntries";
const STREAM_OUTPUT_STREAMS: &str = "StreamOutputStreams";
const STREAM_OUTPUT_SYSTEM_VALUE: &str = "StreamOutputSystemValue";
/// Stream output of a vertex stage drawn with adjacency.
const STREAM_OUTPUT_ADJACENCY: &str = "StreamOutputAdjacency";
/// A vertex stage writing cull distances, drawn with adjacency, which Direct3D draws without the
/// adjacent vertices.
const CULL_DISTANCE_ADJACENCY: &str = "CullDistanceAdjacency";
/// Stream output from a vertex stage writing cull distances whose records do not hold its reflected
/// locations first.
const CULL_DISTANCE_STREAM_LOCATIONS: &str = "CullDistanceStreamLocations";

/// The stream output program and its declaration's elements.
struct Stream<B: Backend> {
    program: B::Program,
    /// Each element's first words in the program's table ([`StreamEntry`]).
    entries: Vec<[u32; ENTRY_WORDS]>,
    strides: [u32; STREAM_BUFFERS],
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
    pad: u32,
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
const fn word_at(offset: usize) -> usize {
    offset / size_of::<u32>()
}

/// The stand-in buffer's ranges: the index words, then the stream output buffers, each at a multiple
/// of the adapter's storage offset alignment.
const UNUSED_RANGES: u64 = 1 + STREAM_BUFFERS as u64;
/// The bytes of each.
const UNUSED_RANGE_BYTES: u64 = super::SLOT_BYTES as u64;

/// A vertex shader, and a geometry shader or stream output or both, or a hull shader and the
/// tessellator (then the domain shader too, when a geometry shader or stream output follows it), run
/// as compute work before the draw ([`SlowPath::GeometryAsCompute`],
/// [`SlowPath::StreamOutAsCompute`], [`SlowPath::TessellationAsCompute`]).
pub struct Stages<B: Backend> {
    vertex: B::Program,
    geometry: Option<Geometry<B>>,
    hull: Option<Hull<B>>,
    domain: Option<Domain<B>>,
    stream: Option<Stream<B>>,
    cull: Option<Cull<B>>,
    lines: Option<Lines<B>>,
    /// Words per vertex record.
    in_words: u32,
    records: Option<Owned<B>>,
    out: Option<Owned<B>>,
    /// Stands in for bindings a draw does not use ([`UNUSED_RANGES`] storage ranges).
    unused: B::Buffer,
    /// Bytes between its ranges.
    unused_stride: u64,
}

impl<B: Backend> Stages<B> {
    /// Makes the compute programs of `vertex` (converted with every input pulled), of `geometry` if
    /// given, and of stream output from the last of them if declared.
    pub fn new(
        backend: &mut B,
        vertex: &Converted,
        geometry: Option<&Converted>,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        if geometry.is_none() && stream_output.is_none() {
            return Err(StageError::Convert(ConvertError::Malformed(
                WITHOUT_WORK.into(),
            )));
        }
        // The vertex records hold what the next stage reads: the geometry shader's inputs, or every
        // output of a vertex shader whose output is streamed.
        let (read, last) = match geometry {
            Some(g) => (
                super::locations(&g.reflection.inputs),
                &g.reflection.outputs,
            ),
            None => (
                super::locations(&vertex.reflection.outputs),
                &vertex.reflection.outputs,
            ),
        };
        let shader = backend.create_shader(&super::vertex_program_for(vertex, &read)?)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let geometry = match geometry {
            Some(g) => Some(geometry_parts(backend, g)?),
            None => None,
        };
        let stream = match stream_output {
            Some(so) => Some(stream_parts(backend, so, last)?),
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
            geometry,
            hull: None,
            domain: None,
            stream,
            cull: None,
            lines: None,
            in_words: super::record_words(read.len()),
            records: None,
            out: None,
            unused,
            unused_stride,
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
        let programs = super::cull_programs(vertex)?;
        let shader = backend.create_shader(&programs.compute)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let stream = match stream_output {
            Some(so) => {
                let reflected = super::locations(&vertex.reflection.outputs);
                if !programs.locations.starts_with(&reflected) {
                    return Err(StageError::Unsupported(CULL_DISTANCE_STREAM_LOCATIONS));
                }
                Some(stream_parts(backend, so, &vertex.reflection.outputs)?)
            }
            None => None,
        };
        let shader = backend.create_shader(&super::cull_program()?)?;
        let program = backend.create_program(&Program::Compute(&shader))?;
        let drawn_slots =
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
            geometry: None,
            hull: None,
            domain: None,
            stream,
            cull: Some(Cull {
                program,
                passthrough: programs.passthrough,
                drawn_slots,
                cull_word: (1 + drawn_slots) * crate::build::VEC4,
                distances: programs.cull_distances,
            }),
            lines: None,
            in_words: super::record_words(
                (drawn_slots + super::distance_slots(programs.cull_distances)) as usize,
            ),
            records: None,
            out: None,
            unused,
            unused_stride,
        })
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled) drawing its lines
    /// antialiased for a pixel program converted with `pixel` (its key, with
    /// `VariantFlags::LINE_COVERAGE`): each draw through [`Self::run_antialiased_lines`] runs it and
    /// writes a rectangle around each line, which the final draw draws through [`Self::passthrough`]
    /// with culling off ([`SlowPath::AntialiasedLinesAsQuads`]).
    pub fn antialiased_lines(
        backend: &mut B,
        vertex: &Converted,
        pixel: &wcontainers_gpu::VariantKey,
    ) -> Result<Self, StageError> {
        let programs = super::line_programs(vertex, pixel)?;
        let shader = backend.create_shader(&programs.compute)?;
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
            }),
            in_words: super::record_words(record_slots as usize),
            records: None,
            out: None,
            unused,
            unused_stride,
        })
    }

    /// Makes the compute programs of `vertex` (converted with every input pulled) and `hull`, and the
    /// tessellator, for draws whose final draw runs `domain` ([`TessellatedDraw`]).
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
    /// geometry shader the final draw is the geometry stage's, without one the tessellated draw.
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
        let shader = backend.create_shader(&super::vertex_program_for(vertex, &read)?)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let shader = backend.create_shader(&hull.modules[0])?;
        let hull_program = backend.create_program(&Program::Compute(&shader))?;
        let slot = crate::build::VEC4;
        let patch_words = u32::from(output_control_points) * slot * u32::from(point_locations)
            + slot * u32::from(constant_locations)
            + tess::FACTOR_WORDS;
        // After the domain stage: its records hold what the next stage reads.
        let tessellated_vertices = tess::output_vertices(output_primitive);
        let (geometry, domain_stage, stream) = match (geometry, stream_output) {
            (None, None) => (None, None, None),
            _ => {
                let (read, last) =
                    match geometry {
                        Some(g) => {
                            if g.reflection.inputs.iter().any(|v| {
                                v.system == Some(wcontainers_gpu::SystemValue::PrimitiveId)
                            }) {
                                // Its primitive would be the patch's, which the domain's records lack.
                                return Err(StageError::Unsupported(
                                    GEOMETRY_PRIMITIVE_ID_AFTER_DOMAIN,
                                ));
                            }
                            (
                                super::locations(&g.reflection.inputs),
                                &g.reflection.outputs,
                            )
                        }
                        None => (
                            super::locations(&domain.reflection.outputs),
                            &domain.reflection.outputs,
                        ),
                    };
                let shader = backend.create_shader(&super::domain_program_for(domain, &read)?)?;
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
                let stream = match stream_output {
                    Some(so) => Some(stream_parts(backend, so, last)?),
                    None => None,
                };
                let domain = Domain {
                    program,
                    words: super::record_words(read.len()),
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
            cull: None,
            lines: None,
            in_words: super::record_words(read.len()),
            records: None,
            out: None,
            unused,
            unused_stride,
        })
    }

    /// Releases everything the helper made.
    pub fn destroy(self, backend: &mut B) {
        use wcontainers_gpu::Object;
        backend.destroy(Object::Program(self.vertex));
        if let Some(g) = self.geometry {
            backend.destroy(Object::Program(g.program));
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
        for owned in [self.records, self.out].into_iter().flatten() {
            backend.destroy(Object::Buffer(owned.buffer));
        }
        backend.destroy(Object::Buffer(self.unused));
    }

    /// The vertex program of the draw after a geometry stage or the cull program, for the front's
    /// program with its pixel program.
    pub fn passthrough(&self) -> Option<&ShaderModule> {
        match (&self.geometry, &self.cull, &self.lines) {
            (Some(g), _, _) => Some(&g.passthrough),
            (None, Some(c), _) => Some(&c.passthrough),
            (None, None, Some(l)) => Some(&l.passthrough),
            (None, None, None) => None,
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
        let (words, slots) = match (&self.geometry, &self.cull, &self.lines) {
            (Some(g), _, _) => (g.out_words, g.out_locations),
            (None, Some(c), _) => (self.in_words, c.drawn_slots),
            (None, None, Some(l)) => (self.in_words + crate::build::VEC4, l.drawn_slots),
            (None, None, None) => return None,
        };
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
                offset: super::SLOT_BYTES * slot,
                format: VertexFormat::Uint32x4,
            });
        }
        Some(desc)
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
    /// `viewport` pixels wide and high.
    pub fn run_antialiased_lines(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
        viewport: [f32; 2],
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        self.run_with(backend, draw, None, Some(viewport))
    }

    /// [`Self::run`] for a helper made by [`Self::tessellated_with`] with a geometry shader or stream
    /// output: `draw.geometry_sets` are the hull stage's, `later` the stages' after it.
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
        viewport: Option<[f32; 2]>,
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
            T::TriangleStripAdjacency => {
                return Err(StageError::Unsupported(GEOMETRY_TRIANGLE_STRIP_ADJACENCY));
            }
            T::PatchList(n) => (u32::from(n), false),
        };
        let patches = matches!(draw.topology, T::PatchList(_));
        match (&self.geometry, &self.hull) {
            (_, Some(h)) if !patches || n < h.input_points => {
                return Err(StageError::TopologyMismatch);
            }
            (_, None) if patches => return Err(StageError::TopologyMismatch),
            (Some(g), None) if n != g.input_vertices => return Err(StageError::TopologyMismatch),
            (None, None)
                if self.lines.is_some() && !matches!(draw.topology, T::LineList | T::LineStrip) =>
            {
                return Err(StageError::Unsupported(LINE_TOPOLOGY));
            }
            (None, None) if n > TRIANGLE_VERTICES && self.cull.is_some() => {
                return Err(StageError::Unsupported(CULL_DISTANCE_ADJACENCY));
            }
            (None, None) if n > TRIANGLE_VERTICES => {
                return Err(StageError::Unsupported(STREAM_OUTPUT_ADJACENCY));
            }
            _ => {}
        }
        if strip && draw.index.is_some() {
            // Direct3D's indexed strips restart at the largest index, which needs a pass over the
            // indices to number the primitives.
            return Err(StageError::Unsupported(STAGE_INDEXED_STRIP));
        }
        let primitives = match strip {
            false => count / n,
            true => (count + 1).saturating_sub(n),
        };
        let streamed = self.stream.is_some() && draw.stream_output.is_some();
        let drawn = self.geometry.is_some()
            || self.hull.is_some()
            || self.cull.is_some()
            || self.lines.is_some();
        if primitives == 0 || instances == 0 || (!drawn && !streamed) {
            return Ok(None);
        }
        let limits = backend.caps().limits;
        let record_count = u64::from(count) * u64::from(instances);
        let record_bytes = record_count * u64::from(self.in_words) * u64::from(WORD);
        // On the cull route the records end with the empty record, then the index list the final draw
        // draws.
        let (empty, list_base, records_size, records_usage) = match &self.cull {
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
                let vertex_words = invocations * u64::from(g.max_vertices) * u64::from(g.out_words);
                let index_words = invocations * 2 * u64::from(g.max_vertices);
                let index_base = u32::try_from(vertex_words).map_err(|_| GpuError::OutOfMemory)?;
                let usage = BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX;
                grow(
                    backend,
                    &mut self.out,
                    (vertex_words + index_words) * u64::from(WORD),
                    usage,
                )?;
                (
                    invocations,
                    (vertex_words + index_words) * u64::from(WORD),
                    index_base,
                )
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
                None => (0, 0, 0),
            },
        };

        let (index_bytes, index_offset, indices) = match &draw.index {
            Some(index) => {
                let bytes = match index.format {
                    IndexFormat::U16 => size_of::<u16>() as u32,
                    IndexFormat::U32 => size_of::<u32>() as u32,
                    IndexFormat::U8 => return Err(StageError::Unsupported(STAGE_INDEX_U8)),
                };
                let align = u64::from(limits.min_storage_offset_alignment);
                let start = index.offset - index.offset % align;
                let size = (index.size - start) & !(u64::from(WORD) - 1);
                let range = BufferRange {
                    buffer: index.buffer,
                    offset: start,
                    size,
                };
                (bytes, (index.offset - start) as u32, range)
            }
            None => (0, 0, self.unused_range(0)),
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
        let mut offsets = [0u64; 2];
        let ring = backend.write_buffer_ring(
            RingKind::Data,
            &[&bytes, &after_bytes],
            limits.min_uniform_offset_alignment,
            &mut offsets,
        )?;
        let uniform = |part: usize| BindingEntry {
            binding: VALUES_BINDING,
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ring,
                offset: offsets[part],
                size: bytes.len() as u64,
            }),
        };
        let records = &self.records.as_ref().unwrap().buffer;
        let vertex_set = backend.create_binding_set(&[
            uniform(0),
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
        if let Some(c) = &self.cull {
            backend.slow_paths().hit(SlowPath::CullDistanceAsGeometry);
            let mut cv = [0u32; word_at(size_of::<CullValues>())];
            let at = |offset| word_at(offset);
            cv[at(offset_of!(CullValues, units))] = primitives * instances;
            cv[at(offset_of!(CullValues, primitives))] = primitives;
            cv[at(offset_of!(CullValues, count))] = count;
            cv[at(offset_of!(CullValues, vertices))] = n;
            cv[at(offset_of!(CullValues, strip))] = strip.into();
            cv[at(offset_of!(CullValues, record_words))] = self.in_words;
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
                topology: match n {
                    1 => Topology::PointList,
                    2 => Topology::LineList,
                    _ => Topology::TriangleList,
                },
                count: primitives * instances * n,
                tessellated: None,
            });
        }
        if let (Some(l), Some([width, height])) = (&self.lines, viewport) {
            backend.slow_paths().hit(SlowPath::AntialiasedLinesAsQuads);
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
            lv[at(offset_of!(LineValues, half_width))] = (width / 2.0).to_bits();
            lv[at(offset_of!(LineValues, half_height))] = (height / 2.0).to_bits();
            lv[at(offset_of!(LineValues, reach_across))] =
                crate::variant::LINE_REACH_ACROSS.to_bits();
            lv[at(offset_of!(LineValues, reach_along))] =
                crate::variant::LINE_REACH_ALONG.to_bits();
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
        if let Some(h) = &mut self.hull {
            backend.slow_paths().hit(SlowPath::TessellationAsCompute);
            let patch_bytes = (u64::from(tess::HEADER_WORDS)
                + u64::from(patch_total) * u64::from(h.patch_words))
                * u64::from(WORD);
            grow(backend, &mut h.patches, patch_bytes, BufferUsage::STORAGE)?;
            let patches = &h.patches.as_ref().unwrap().buffer;
            let hull_set = backend.create_binding_set(&[
                uniform(0),
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
        let mut domain_records = None;
        if let (Some(d), Some((points, _, _))) = (&mut self.domain, listed) {
            let bytes = u64::from(points) * u64::from(d.words) * u64::from(WORD);
            grow(backend, &mut d.records, bytes, BufferUsage::STORAGE)?;
        }
        if let (Some(d), Some(later), Some(h), Some((points, _, _))) =
            (&self.domain, later, &self.hull, listed)
        {
            let bytes = u64::from(points) * u64::from(d.words) * u64::from(WORD);
            let out = &d.records.as_ref().unwrap().buffer;
            let (_, tess_points, _) = h.tessellator.owned();
            let patches = &h.patches.as_ref().unwrap();
            let set = backend.create_binding_set(&[
                uniform(1),
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
            _ => None,
        };

        if let Some(g) = &self.geometry {
            backend.slow_paths().hit(SlowPath::GeometryAsCompute);
            let out = &self.out.as_ref().unwrap().buffer;
            let (input, input_bytes, part, sets) = match (domain_records, later) {
                (Some((r, bytes)), Some(later)) => (r, bytes, 1, later.geometry_sets),
                _ => (records, record_bytes, 0, draw.geometry_sets),
            };
            let geometry_set = backend.create_binding_set(&[
                uniform(part),
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
            ])?;
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
        }

        if let (Some(stream), Some(targets)) = (&self.stream, &draw.stream_output) {
            backend.slow_paths().hit(SlowPath::StreamOutAsCompute);
            let mut so = [0u32; word_at(size_of::<StreamValues>())];
            let at = |offset| word_at(offset);
            let (data, data_bytes) = match (&self.geometry, domain_records, listed) {
                (Some(g), _, _) => {
                    so[at(offset_of!(StreamValues, units))] = invocations as u32;
                    so[at(offset_of!(StreamValues, geometry))] = 1;
                    so[at(offset_of!(StreamValues, vertices))] = g.output;
                    so[at(offset_of!(StreamValues, slots))] = 2 * g.max_vertices;
                    so[at(offset_of!(StreamValues, index_base))] = index_base;
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
                    so[at(offset_of!(StreamValues, record_words))] = self.in_words;
                    (records, record_bytes)
                }
            };
            so[at(offset_of!(StreamValues, primitives))] = primitives;
            so[at(offset_of!(StreamValues, count))] = count;
            // Elements of buffers not bound are left out; such a buffer neither holds nor limits.
            let mut entries = 0;
            for e in &stream.entries {
                let buffer = (e[0] & u32::from(u8::MAX)) as usize;
                if targets.buffers[buffer].is_none() {
                    continue;
                }
                let first =
                    at(offset_of!(StreamValues, table) + size_of::<StreamEntry>() * entries);
                so[first..first + ENTRY_WORDS].copy_from_slice(e);
                entries += 1;
            }
            so[at(offset_of!(StreamValues, entries))] = entries as u32;
            for (b, target) in targets.buffers.iter().enumerate() {
                if let Some(t) = target {
                    so[at(offset_of!(StreamValues, strides)) + b] = stream.strides[b];
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
                program: &stream.program,
                bindings: sets,
                immediates: &[],
                groups: DispatchGroups::Direct([1, 1, 1]),
            })?;
        }
        Ok(last)
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
fn geometry_parts<B: Backend>(
    backend: &mut B,
    geometry: &Converted,
) -> Result<Geometry<B>, StageError> {
    let StageExtras::Geometry {
        input_vertices,
        max_output_vertices,
        output_vertices,
        instances,
        ..
    } = geometry.reflection.extras
    else {
        return Err(StageError::Convert(ConvertError::Malformed(
            NOT_A_GEOMETRY_SHADER.into(),
        )));
    };
    let selects = super::selects(&geometry.reflection);
    let distances = super::clip_distances(&geometry.reflection);
    let slots = super::locations(&geometry.reflection.outputs).len()
        + usize::from(selects)
        + super::distance_slots(distances) as usize;
    let shader = backend.create_shader(&geometry.modules[0])?;
    Ok(Geometry {
        program: backend.create_program(&Program::Compute(&shader))?,
        passthrough: geometry.modules[1].clone(),
        input_vertices: input_vertices.into(),
        max_vertices: max_output_vertices,
        instances,
        output: output_vertices.into(),
        out_words: super::record_words(slots),
        out_locations: slots as u32,
        selects,
    })
}

/// The stream output program, and the declaration's elements located in the records of the stage
/// whose `outputs` are streamed.
fn stream_parts<B: Backend>(
    backend: &mut B,
    so: &StreamOutput,
    outputs: &[StageValue],
) -> Result<Stream<B>, StageError> {
    let locations = super::locations(outputs);
    let mut ends = [0u32; STREAM_BUFFERS];
    let mut entries = Vec::new();
    for e in &so.entries {
        if e.stream != 0 {
            return Err(StageError::Unsupported(STREAM_OUTPUT_STREAMS));
        }
        let (buffer, count, start) = (
            usize::from(e.slot),
            u32::from(e.component_count),
            u32::from(e.start_component),
        );
        if buffer >= STREAM_BUFFERS || count == 0 || start + count > crate::build::VEC4 {
            return Err(StageError::Declaration);
        }
        if let Some(name) = &e.semantic {
            let value = outputs
                .iter()
                .find(|v| {
                    v.semantic.eq_ignore_ascii_case(name) && v.semantic_index == e.semantic_index
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
            entries.push([packed, ends[buffer], slot]);
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
    let shader = backend.create_shader(&super::stream_program()?)?;
    Ok(Stream {
        program: backend.create_program(&Program::Compute(&shader))?,
        entries,
        strides,
    })
}

fn words_as_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// A storage binding of the first `size` bytes of `buffer`, for the helper's compute programs.
fn storage<B: Backend>(
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
pub(super) fn grow<B: Backend>(
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
