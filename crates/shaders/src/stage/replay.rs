//! A geometry shader on a back end with transform feedback and without compute (WebGL 2): the vertex
//! stage is captured into records, the records are copied into a texture, and the geometry shader's
//! replay form (`VariantFlags::GEOMETRY_REPLAY`) is captured in turn, one vertex per corner of each
//! output primitive slot; the pass-through draws what it captured, and stream output is written on
//! the host from it. A geometry shader writing several streams is replayed once a stream for its
//! stream output, stream 0 last for the draw. Stream output from a vertex shader alone is written on
//! the host from the vertex stage's capture, which the pass-through draws. Alpha-antialiased lines
//! take the vertex stage's capture through the line replay program, whose captured rectangles the
//! line pass-through draws. Cull distances and primitive numbers take the vertex stage's capture
//! through the primitive replay program, which writes a dropped primitive's vertices outside every
//! clip volume, or each vertex with its primitive's number.

use super::helper::{
    CULL_DISTANCE_ADJACENCY, FinalDraw, InputTopology, LINE_INDICES, LINE_TOPOLOGY, LINE_VERTICES,
    LINE_VIEWPORT, NOT_A_GEOMETRY_SHADER, Owned, PRIMITIVE_ID_ADJACENCY, STAGE_INDIRECT,
    STREAM_BUFFERS, STREAM_FILLED_SIZE, STREAM_OUTPUT_ADJACENCY, StageError, StreamDeclaration,
    StreamOutput, StreamTargets, grow, list_topology, record_layout, stream_declaration, word_at,
    words_as_bytes,
};
use super::{
    LINE_ADJACENCY_VERTICES, RECORD_SLOT_BYTES, STAGE_SET, TRIANGLE_ADJACENCY_VERTICES,
    TRIANGLE_VERTICES, VALUES_BINDING, VERTICES_BINDING, word,
};
use crate::build::{VEC4, WORD};
use crate::{ConvertError, Converted};
use core::mem::{offset_of, size_of};
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendDesc, BufferRange, BufferUsage, ColorWrites,
    CopyBufferTexture, CullMode, DepthStencilDesc, DrawCall, DrawRange, Extent, Features, FillMode,
    Format, FrontFace, GpuError, IndexBinding, IndexFormat, MAX_BINDING_SETS, MAX_COLOR_TARGETS,
    Object, Origin, Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend, RingKind,
    ShaderModule, ShaderStage, SlowPath, StageExtras, StageSet, TextureDesc, TextureDimension,
    TextureRegion, TextureUsage, TextureViewDesc, Topology, VariantKey, VertexBinding,
    VertexLayoutDesc, ViewDimension, Viewport, WriteMode,
};

/// A strip with adjacency, which the capture cannot list.
const ADJACENT_STRIP: &str = "GeometryReplayAdjacentStrip";
/// A record wider than one capture target holds.
const RECORD_COMPONENTS: &str = "GeometryReplayComponents";
/// Stream output of a vertex program whose records do not hold its reflected locations first.
const STREAMED_LOCATIONS: &str = "StreamOutCaptureLocations";
/// A replay form holding an array of arrays (a DXIL geometry shader's input arrays of rows, one per
/// vertex), which GLSL ES 3.00, the language of the back ends the replay serves, cannot declare.
const ARRAY_OF_ARRAYS: &str = "GeometryReplayArrayOfArrays";
/// A geometry shader converted without `VariantFlags::GEOMETRY_REPLAY`.
const NOT_A_REPLAY_FORM: &str = "a geometry shader converted without its replay form";
/// The format of the records' texture: one four-word record slot a texel.
const RECORD_FORMAT: Format = Format::Rgba32Uint;
/// The one viewport a captured draw names; nothing is drawn through it.
const CAPTURE_VIEWPORT: Viewport = Viewport {
    x: 0,
    y: 0,
    width: 1.0f32.to_bits(),
    height: 1.0f32.to_bits(),
    min_depth: 0,
    max_depth: 1.0f32.to_bits(),
};

/// The line replay program's values (`line_replay.wgsl`'s `Values`), read through its layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct LineReplayValues {
    interpolated: u32,
    linear: u32,
    near_w: f32,
    pad: u32,
    half_width: f32,
    half_height: f32,
    reach_across: f32,
    reach_along: f32,
}

/// The primitive replay program's values (`primitive_replay.wgsl`'s `Values`), read through its
/// layout.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct PrimitiveReplayValues {
    vertices: u32,
    cull_count: u32,
    primitives: u32,
    numbered: u32,
}

/// One API draw through [`GeometryReplay`].
#[derive(Debug)]
pub struct ReplayDraw<'a, B: Backend> {
    /// The vertex stage's sets, layout, streams, indices and immediates, as for the API draw.
    pub vertex_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
    pub vertex_layout: &'a B::VertexLayout,
    pub vertex_buffers: &'a [Option<VertexBinding<'a, B>>],
    pub index: Option<IndexBinding<'a, B>>,
    pub immediates: &'a [u8],
    /// The geometry stage's sets, made visible to the vertex stage: its slots in set 0, its helper
    /// constants in set 3. Set 1 is the route's own.
    pub geometry_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
    pub topology: InputTopology,
    /// A direct draw's range.
    pub range: DrawRange<'a, B>,
    /// Where stream output goes, for a route made with a declaration; `None` when no buffer is bound.
    pub stream_output: Option<StreamTargets<'a, B>>,
}

/// The records' texture, `width` texels a row.
struct Records<B: Backend> {
    texture: B::Texture,
    view: B::TextureView,
    width: u32,
    height: u32,
}

/// A vertex shader and a geometry shader run by replay ([`SlowPath::GeometryAsReplay`]): each draw
/// captures the vertex stage, then the geometry shader's replay form, and the front draws the second
/// capture through [`Self::passthrough`]. Made by [`Self::streamed`], the vertex stage alone, whose
/// stream output is written from its capture, which the front draws the same way.
pub struct GeometryReplay<B: Backend> {
    capture: B::Program,
    /// The geometry shader's replay; `None` for a vertex stage alone.
    geometry: Option<Replayed<B>>,
    /// The line replay, for a vertex stage drawing alpha-antialiased lines.
    lines: Option<ReplayedLines<B>>,
    /// The primitive replay, for a vertex stage writing cull distances or a pixel stage reading its
    /// primitive's number.
    primitives: Option<ReplayedPrimitives<B>>,
    passthrough: ShaderModule,
    states: CaptureStates<B>,
    /// Four-word slots per vertex record.
    in_slots: u32,
    stream: Option<StreamDeclaration>,
    captured: Option<Owned<B>>,
    records: Option<Records<B>>,
    out: Option<Owned<B>>,
}

/// The geometry shader's replay form and the shape of what it captures.
struct Replayed<B: Backend> {
    program: B::Program,
    no_streams: B::VertexLayout,
    input_vertices: u32,
    /// Vertices per output primitive: 1, 2 or 3.
    output: u32,
    /// Vertices the replay draws per input primitive: every corner of every slot of every geometry
    /// instance.
    primitive_vertices: u32,
    /// Four-word slots per output record.
    out_slots: u32,
    /// Words per captured output vertex: its record, then whether its slot's primitive is whole.
    out_words: u32,
    selects: bool,
    /// Streams the geometry shader writes, at least one.
    streams: u32,
}

/// The line replay program and which record slots it takes along the line
/// ([`super::LinePrograms`]).
struct ReplayedLines<B: Backend> {
    program: B::Program,
    no_streams: B::VertexLayout,
    interpolated: u32,
    linear: u32,
}

/// The primitive replay program and the shape of what it captures.
struct ReplayedPrimitives<B: Backend> {
    program: B::Program,
    no_streams: B::VertexLayout,
    /// Four-word slots per record it writes, which the pass-through draws.
    drawn_slots: u32,
    distances: u32,
    numbered: bool,
}

/// The states of a captured draw, which draws nothing: no colour writes, no depth test, no culling,
/// the first vertex provoking (the order the replay and stream output read strips in).
struct CaptureStates<B: Backend> {
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
}

fn capture_states<B: Backend>(backend: &mut B) -> Result<CaptureStates<B>, GpuError> {
    let blend = backend.create_blend(&BlendDesc {
        targets: [RenderTargetBlend {
            blend: None,
            writes: ColorWrites::empty(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    })?;
    let depth_stencil = backend.create_depth_stencil(&DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [u8::MAX; 2],
        stencil_write_mask: [u8::MAX; 2],
        depth_bounds: None,
    })?;
    let raster = backend.create_raster(&RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::Clockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::First,
        // Nothing is drawn; the clip depth convention the back end takes.
        depth_minus_one: !backend
            .caps()
            .features
            .contains(Features::CLIP_DEPTH_ZERO_ONE),
        sample_mask: !0,
    })?;
    Ok(CaptureStates {
        blend,
        depth_stencil,
        raster,
    })
}

impl<B: Backend> GeometryReplay<B> {
    /// Makes the capture program of `vertex` (converted, its inputs as drawn) whose output
    /// `stream_output` declares, with no geometry shader: each draw captures the vertex stage, writes
    /// its stream output on the host ([`SlowPath::StreamOutAfterReplay`]), and the front draws the
    /// capture through [`Self::passthrough`] when it draws the stream.
    pub fn streamed(
        backend: &mut B,
        vertex: &Converted,
        stream_output: &StreamOutput,
    ) -> Result<Self, StageError> {
        if !backend
            .caps()
            .features
            .contains(Features::TRANSFORM_FEEDBACK)
        {
            return Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK).into());
        }
        let programs = super::streamed_programs(vertex)?;
        if programs.slots * VEC4 > backend.caps().limits.capture_components {
            return Err(StageError::Unsupported(RECORD_COMPONENTS));
        }
        if !programs
            .locations
            .starts_with(&super::locations(&vertex.reflection.outputs))
        {
            return Err(StageError::Unsupported(STREAMED_LOCATIONS));
        }
        let stream = stream_declaration(stream_output, &vertex.reflection.outputs, 1)?;
        let capture = render_program(backend, &programs.capture)?;
        let states = capture_states(backend)?;
        Ok(GeometryReplay {
            capture,
            geometry: None,
            lines: None,
            primitives: None,
            passthrough: programs.passthrough,
            states,
            in_slots: programs.slots,
            stream: Some(stream),
            captured: None,
            records: None,
            out: None,
        })
    }

    /// Makes the capture program of `vertex` (converted, its inputs as drawn) and the replay program
    /// of `geometry` (converted with `VariantFlags::GEOMETRY_REPLAY`), with `stream_output`'s
    /// declaration if any.
    pub fn new(
        backend: &mut B,
        vertex: &Converted,
        geometry: &Converted,
        stream_output: Option<&StreamOutput>,
    ) -> Result<Self, StageError> {
        let StageExtras::Geometry {
            input_vertices,
            max_output_vertices,
            output_vertices,
            instances,
            streams,
        } = geometry.reflection.extras
        else {
            return Err(ConvertError::Malformed(NOT_A_GEOMETRY_SHADER.into()).into());
        };
        if geometry.modules[0].stage != ShaderStage::Vertex {
            return Err(ConvertError::Malformed(NOT_A_REPLAY_FORM.into()).into());
        }
        if nested_array(&geometry.modules[0].module) {
            return Err(StageError::Unsupported(ARRAY_OF_ARRAYS));
        }
        if !backend
            .caps()
            .features
            .contains(Features::TRANSFORM_FEEDBACK)
        {
            return Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK).into());
        }
        let in_slots = 1 + super::locations(&geometry.reflection.inputs).len() as u32;
        let selects = super::selects(&geometry.reflection);
        let out_slots = 1 + super::drawn_slots(&geometry.reflection);
        let out_words = out_slots * VEC4 + 1;
        if (in_slots * VEC4).max(out_words) > backend.caps().limits.capture_components {
            return Err(StageError::Unsupported(RECORD_COMPONENTS));
        }
        let streams = u32::from(streams.max(1));
        let stream = stream_output
            .map(|so| stream_declaration(so, &geometry.reflection.outputs, streams))
            .transpose()?;
        let output = u32::from(output_vertices);
        let primitive_vertices =
            instances.max(1) * super::replay_slots(max_output_vertices, output) * output;
        let capture = super::capture_program(vertex, geometry)?;
        let capture = render_program(backend, &capture)?;
        let program = render_program(backend, &geometry.modules[0])?;
        let states = capture_states(backend)?;
        let no_streams = backend.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        })?;
        Ok(GeometryReplay {
            capture,
            geometry: Some(Replayed {
                program,
                no_streams,
                input_vertices: input_vertices.into(),
                output,
                primitive_vertices,
                out_slots,
                out_words,
                selects,
                streams,
            }),
            lines: None,
            primitives: None,
            passthrough: geometry.modules[1].clone(),
            states,
            in_slots,
            stream,
            captured: None,
            records: None,
            out: None,
        })
    }

    /// Makes the capture program of `vertex` (converted, its inputs as drawn) and the line replay
    /// program drawing its lines antialiased for a pixel program converted with `pixel` (its key, with
    /// `VariantFlags::LINE_COVERAGE`): each draw through [`Self::run_antialiased_lines`] captures the
    /// vertex stage, then a rectangle around each line, which the front draws through
    /// [`Self::passthrough`] with culling off ([`SlowPath::AntialiasedLinesAsQuads`]).
    pub fn antialiased_lines(
        backend: &mut B,
        vertex: &Converted,
        pixel: &VariantKey,
    ) -> Result<Self, StageError> {
        if !backend
            .caps()
            .features
            .contains(Features::TRANSFORM_FEEDBACK)
        {
            return Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK).into());
        }
        let programs = super::line_capture_programs(vertex, pixel)?;
        let in_slots =
            1 + programs.locations.len() as u32 + super::distance_slots(programs.clip_distances);
        if (in_slots + 1) * VEC4 > backend.caps().limits.capture_components {
            return Err(StageError::Unsupported(RECORD_COMPONENTS));
        }
        let capture = render_program(backend, &programs.records)?;
        let program = render_program(backend, &super::line_replay_program(in_slots + 1)?)?;
        let no_streams = backend.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        })?;
        let states = capture_states(backend)?;
        Ok(GeometryReplay {
            capture,
            geometry: None,
            lines: Some(ReplayedLines {
                program,
                no_streams,
                interpolated: programs.interpolated,
                linear: programs.linear,
            }),
            primitives: None,
            passthrough: programs.passthrough,
            states,
            in_slots,
            stream: None,
            captured: None,
            records: None,
            out: None,
        })
    }

    /// Makes the capture program of `vertex` (converted, its inputs as drawn), which writes cull
    /// distances, and the primitive replay program: each draw captures the vertex stage, then each
    /// primitive's vertices or, where its cull distances drop it, records outside every clip volume,
    /// which the front draws through [`Self::passthrough`] ([`SlowPath::CullDistanceAsGeometry`]).
    pub fn culled(backend: &mut B, vertex: &Converted) -> Result<Self, StageError> {
        if !backend
            .caps()
            .features
            .contains(Features::TRANSFORM_FEEDBACK)
        {
            return Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK).into());
        }
        let programs = super::cull_capture_programs(vertex)?;
        Self::primitives_replayed(backend, programs, false)
    }

    /// Makes the capture program of `vertex` (converted, its inputs as drawn) and the primitive
    /// replay program for a pixel program converted with `pixel` (its key, with
    /// `VariantFlags::PRIMITIVE_ID_FROM_STAGE`) that reads its primitive's number: each draw captures
    /// the vertex stage, then each primitive's vertices with its number, which the front draws
    /// through [`Self::passthrough`] ([`SlowPath::PrimitiveIdAsGeometry`]).
    pub fn primitive_numbered(
        backend: &mut B,
        vertex: &Converted,
        pixel: &VariantKey,
    ) -> Result<Self, StageError> {
        if !backend
            .caps()
            .features
            .contains(Features::TRANSFORM_FEEDBACK)
        {
            return Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK).into());
        }
        let programs = super::primitive_capture_programs(vertex, pixel)?;
        Self::primitives_replayed(backend, programs, true)
    }

    fn primitives_replayed(
        backend: &mut B,
        programs: super::ReplayedPrimitivePrograms,
        numbered: bool,
    ) -> Result<Self, StageError> {
        let copied = programs.drawn_slots - u32::from(numbered);
        let in_slots = copied + super::distance_slots(programs.cull_distances);
        if in_slots.max(programs.drawn_slots) * VEC4 > backend.caps().limits.capture_components {
            return Err(StageError::Unsupported(RECORD_COMPONENTS));
        }
        let capture = render_program(backend, &programs.capture)?;
        let replay = super::primitive_replay_program(programs.drawn_slots, in_slots)?;
        let program = render_program(backend, &replay)?;
        let no_streams = backend.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        })?;
        let states = capture_states(backend)?;
        Ok(GeometryReplay {
            capture,
            geometry: None,
            lines: None,
            primitives: Some(ReplayedPrimitives {
                program,
                no_streams,
                drawn_slots: programs.drawn_slots,
                distances: programs.cull_distances,
                numbered,
            }),
            passthrough: programs.passthrough,
            states,
            in_slots,
            stream: None,
            captured: None,
            records: None,
            out: None,
        })
    }

    /// The vertex program of the draw after the replay, for the front's program with its pixel
    /// program.
    pub fn passthrough(&self) -> &ShaderModule {
        &self.passthrough
    }

    /// Whether the geometry stage writes a layer or viewport, as [`super::Stages::selects`].
    pub fn selects(&self) -> bool {
        self.geometry.as_ref().is_some_and(|g| g.selects)
    }

    /// The vertex layout of that draw.
    pub fn vertex_layout(&self) -> VertexLayoutDesc {
        match (&self.geometry, &self.lines, &self.primitives) {
            (Some(g), ..) => record_layout(g.out_words, g.out_slots - 1),
            (None, Some(_), _) => record_layout((self.in_slots + 1) * VEC4, self.in_slots),
            (None, None, Some(p)) => record_layout(p.drawn_slots * VEC4, p.drawn_slots - 1),
            (None, None, None) => record_layout(self.in_slots * VEC4, self.in_slots - 1),
        }
    }

    /// Records the captures of one API draw (outside a pass), and returns what the front draws next;
    /// `None` when the draw makes no primitive.
    pub fn run(
        &mut self,
        backend: &mut B,
        draw: ReplayDraw<'_, B>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        self.run_with(backend, draw, None)
    }

    /// [`Self::run`] for a route made by [`Self::antialiased_lines`], whose draw's viewport is
    /// `viewport` pixels wide and high.
    pub fn run_antialiased_lines(
        &mut self,
        backend: &mut B,
        draw: ReplayDraw<'_, B>,
        viewport: [f32; 2],
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        self.run_with(backend, draw, Some(viewport))
    }

    fn run_with(
        &mut self,
        backend: &mut B,
        draw: ReplayDraw<'_, B>,
        viewport: Option<[f32; 2]>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
        if self.lines.is_some() != viewport.is_some() {
            return Err(ConvertError::Malformed(LINE_VIEWPORT.into()).into());
        }
        let (first, count, base_vertex, first_instance, instances) = match draw.range {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => (first, count, None, first_instance, instances),
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => (first, count, Some(base_vertex), first_instance, instances),
            _ => return Err(StageError::Unsupported(STAGE_INDIRECT)),
        };
        use InputTopology as T;
        let adjacent = matches!(
            draw.topology,
            T::LineListAdjacency
                | T::TriangleListAdjacency
                | T::LineStripAdjacency
                | T::TriangleStripAdjacency
        );
        if self.lines.is_some() && !matches!(draw.topology, T::LineList | T::LineStrip) {
            return Err(StageError::Unsupported(LINE_TOPOLOGY));
        }
        if let (true, Some(p)) = (adjacent, &self.primitives) {
            return Err(StageError::Unsupported(match p.numbered {
                true => PRIMITIVE_ID_ADJACENCY,
                false => CULL_DISTANCE_ADJACENCY,
            }));
        }
        if adjacent && self.geometry.is_none() {
            return Err(StageError::Unsupported(STREAM_OUTPUT_ADJACENCY));
        }
        // Vertices per input primitive, whether a strip, and what the capture draws: lists with
        // adjacency as points in list order.
        let (n, strip, captured_as) = match draw.topology {
            T::PointList => (1, false, Topology::PointList),
            T::LineList => (LINE_VERTICES, false, Topology::LineList),
            T::LineStrip => (LINE_VERTICES, true, Topology::LineStrip),
            T::TriangleList => (TRIANGLE_VERTICES, false, Topology::TriangleList),
            T::TriangleStrip => (TRIANGLE_VERTICES, true, Topology::TriangleStrip),
            T::LineListAdjacency => (LINE_ADJACENCY_VERTICES, false, Topology::PointList),
            T::TriangleListAdjacency => (TRIANGLE_ADJACENCY_VERTICES, false, Topology::PointList),
            T::LineStripAdjacency | T::TriangleStripAdjacency => {
                return Err(StageError::Unsupported(ADJACENT_STRIP));
            }
            T::PatchList(_) => return Err(StageError::TopologyMismatch),
        };
        match &self.geometry {
            Some(g) if n != g.input_vertices => return Err(StageError::TopologyMismatch),
            _ => {}
        }
        // An indexed strip, whose restart indices end strips, is listed on the host: its indices
        // read back and each strip's primitives listed in Direct3D's order, then captured as a list.
        let listed_strip = match (&draw.index, strip) {
            (Some(index), true) => Some(listed_indices(backend, index, (first, count), n)?),
            _ => None,
        };
        let listed_ring;
        let mut index = draw.index;
        let (first, count, strip, captured_as) = match &listed_strip {
            Some(list) => {
                let bytes = words_as_bytes(list);
                let mut offset = [0u64];
                listed_ring =
                    backend.write_buffer_ring(RingKind::Indices, &[&bytes], WORD, &mut offset)?;
                index = Some(IndexBinding {
                    buffer: &listed_ring,
                    offset: offset[0],
                    format: IndexFormat::U32,
                });
                let count = u32::try_from(list.len()).map_err(|_| GpuError::OutOfMemory)?;
                (0, count, false, list_topology(n))
            }
            None => (first, count, strip, captured_as),
        };
        let primitives = match strip {
            false => count / n,
            true => (count + 1).saturating_sub(n),
        };
        if primitives == 0 || instances == 0 {
            return Ok(None);
        }
        if self.geometry.is_some() {
            backend.slow_paths().hit(SlowPath::GeometryAsReplay);
        }
        let limits = backend.caps().limits;
        let too_many = |_| GpuError::OutOfMemory;

        // The vertex stage's records, a list's trailing vertices left out so that each instance's
        // records follow the last's; held as rows of the records' texture for a replay, drawn
        // without one.
        let listed = primitives.checked_mul(n).ok_or(GpuError::OutOfMemory)?;
        let slots = u64::from(listed) * u64::from(instances) * u64::from(self.in_slots);
        let slots = u32::try_from(slots).map_err(too_many)?;
        let captured_bytes = u64::from(slots) * u64::from(RECORD_SLOT_BYTES);
        let replayed = self.geometry.is_some() || self.lines.is_some() || self.primitives.is_some();
        let (rows, records_bytes, usage) = match replayed {
            true => {
                let width = self.records_for(backend, slots, limits.max_texture_2d)?;
                let rows = slots.div_ceil(width);
                let bytes = u64::from(width) * u64::from(rows) * u64::from(RECORD_SLOT_BYTES);
                (Some((width, rows)), bytes, BufferUsage::COPY_SRC)
            }
            false => (
                None,
                captured_bytes,
                BufferUsage::COPY_SRC | BufferUsage::VERTEX,
            ),
        };
        grow(
            backend,
            &mut self.captured,
            records_bytes,
            BufferUsage::CAPTURE | usage,
        )?;
        let captured = &self.captured.as_ref().unwrap().buffer;
        let captured_count = if strip { count } else { listed };
        let range = match base_vertex {
            Some(base_vertex) => DrawRange::Indexed {
                first,
                count: captured_count,
                base_vertex,
                first_instance,
                instances,
            },
            None => DrawRange::Vertices {
                first,
                count: captured_count,
                first_instance,
                instances,
            },
        };
        backend.capture_vertices(
            &DrawCall {
                program: &self.capture,
                blend: &self.states.blend,
                depth_stencil: &self.states.depth_stencil,
                raster: &self.states.raster,
                vertex_layout: draw.vertex_layout,
                bindings: draw.vertex_sets,
                vertex_buffers: draw.vertex_buffers,
                index,
                topology: captured_as,
                restart: false,
                viewports: &[CAPTURE_VIEWPORT],
                scissors: &[],
                blend_constant: Default::default(),
                stencil_reference: Default::default(),
                immediates: draw.immediates,
                range,
            },
            &[BufferRange {
                buffer: captured,
                offset: 0,
                size: captured_bytes,
            }],
            false,
        )?;
        let drawn = listed.checked_mul(instances).ok_or(GpuError::OutOfMemory)?;
        if let (Some(lines), Some((width, rows)), Some([view_width, view_height])) =
            (&self.lines, rows, viewport)
        {
            // Six corners a line, each its record then the coverage slot, drawn as triangles.
            backend.slow_paths().hit(SlowPath::AntialiasedLinesAsQuads);
            // A back end without clip depth from zero draws vertex programs converted with
            // `CLIP_DEPTH_REWRITE`.
            let clip_zero_one = backend
                .caps()
                .features
                .contains(Features::CLIP_DEPTH_ZERO_ONE);
            let records = self.records.as_ref().unwrap();
            copy_records(backend, captured, records, width, rows);
            let corners = drawn
                .checked_mul(LINE_INDICES / LINE_VERTICES)
                .ok_or(GpuError::OutOfMemory)?;
            let out_words = (self.in_slots + 1) * VEC4;
            let out_bytes = u64::from(corners) * u64::from(out_words) * u64::from(WORD);
            grow(
                backend,
                &mut self.out,
                out_bytes,
                BufferUsage::CAPTURE | BufferUsage::VERTEX,
            )?;
            let mut values = [0u32; word_at(size_of::<LineReplayValues>())];
            let at = |offset| word_at(offset);
            values[at(offset_of!(LineReplayValues, interpolated))] = lines.interpolated;
            values[at(offset_of!(LineReplayValues, linear))] = lines.linear;
            values[at(offset_of!(LineReplayValues, near_w))] =
                f32::from(u8::from(!clip_zero_one)).to_bits();
            values[at(offset_of!(LineReplayValues, half_width))] = (view_width / 2.0).to_bits();
            values[at(offset_of!(LineReplayValues, half_height))] = (view_height / 2.0).to_bits();
            values[at(offset_of!(LineReplayValues, reach_across))] =
                crate::variant::LINE_REACH_ACROSS.to_bits();
            values[at(offset_of!(LineReplayValues, reach_along))] =
                crate::variant::LINE_REACH_ALONG.to_bits();
            let out = &self.out.as_ref().unwrap().buffer;
            let program = (&lines.program, &lines.no_streams);
            let out = BufferRange {
                buffer: out,
                offset: 0,
                size: out_bytes,
            };
            replay_capture(
                backend,
                &self.states,
                program,
                &values,
                records,
                corners,
                out,
            )?;
            return Ok(Some(FinalDraw {
                buffer: out.buffer.clone(),
                index_offset: None,
                topology: Topology::TriangleList,
                count: corners,
                tessellated: None,
            }));
        }
        if let (Some(replayed), Some((width, rows))) = (&self.primitives, rows) {
            // Each primitive's vertices, or where its cull distances drop it the empty record's, or
            // each with its number.
            backend.slow_paths().hit(match replayed.numbered {
                true => SlowPath::PrimitiveIdAsGeometry,
                false => SlowPath::CullDistanceAsGeometry,
            });
            let records = self.records.as_ref().unwrap();
            copy_records(backend, captured, records, width, rows);
            let out_bytes =
                u64::from(drawn) * u64::from(replayed.drawn_slots) * u64::from(RECORD_SLOT_BYTES);
            grow(
                backend,
                &mut self.out,
                out_bytes,
                BufferUsage::CAPTURE | BufferUsage::VERTEX,
            )?;
            let mut values = [0u32; word_at(size_of::<PrimitiveReplayValues>())];
            let at = |offset| word_at(offset);
            values[at(offset_of!(PrimitiveReplayValues, vertices))] = n;
            values[at(offset_of!(PrimitiveReplayValues, cull_count))] = replayed.distances;
            values[at(offset_of!(PrimitiveReplayValues, primitives))] = primitives;
            values[at(offset_of!(PrimitiveReplayValues, numbered))] = replayed.numbered.into();
            let out = BufferRange {
                buffer: &self.out.as_ref().unwrap().buffer,
                offset: 0,
                size: out_bytes,
            };
            let program = (&replayed.program, &replayed.no_streams);
            replay_capture(backend, &self.states, program, &values, records, drawn, out)?;
            return Ok(Some(FinalDraw {
                buffer: out.buffer.clone(),
                index_offset: None,
                topology: list_topology(n),
                count: drawn,
                tessellated: None,
            }));
        }
        let (Some(geometry), Some((width, rows))) = (&self.geometry, rows) else {
            if let (Some(declared), Some(targets)) = (&self.stream, &draw.stream_output) {
                let record = Record {
                    words: self.in_slots * VEC4,
                    whole_word: false,
                };
                stream_out(
                    backend,
                    declared,
                    targets,
                    (captured, captured_bytes, record),
                    n,
                    (0, 1),
                )?;
            }
            return Ok(Some(FinalDraw {
                buffer: captured.clone(),
                index_offset: None,
                topology: list_topology(n),
                count: drawn,
                tessellated: None,
            }));
        };
        let records = self.records.as_ref().unwrap();
        copy_records(backend, captured, records, width, rows);

        // The replay: every corner of every output slot of every geometry invocation, per API
        // instance; of several streams, each later one for its stream output, then stream 0, drawn.
        let vertices = primitives
            .checked_mul(geometry.primitive_vertices)
            .ok_or(GpuError::OutOfMemory)?;
        let out_bytes = u64::from(vertices)
            * u64::from(instances)
            * u64::from(geometry.out_words)
            * u64::from(WORD);
        grow(
            backend,
            &mut self.out,
            out_bytes,
            BufferUsage::CAPTURE | BufferUsage::VERTEX | BufferUsage::COPY_SRC,
        )?;
        let streamed = match (&self.stream, &draw.stream_output) {
            (Some(declared), Some(targets)) => Some((declared, targets)),
            _ => None,
        };
        let replayed: Vec<u32> = match streamed {
            Some(_) => (1..geometry.streams).chain([0]).collect(),
            None => alloc::vec![0],
        };
        let out = &self.out.as_ref().unwrap().buffer;
        for stream in replayed {
            let mut values = [0u32; word::COUNT_OF as usize];
            values[word::COUNT as usize] = count;
            values[word::FIRST as usize] = first;
            values[word::FIRST_INSTANCE as usize] = first_instance;
            values[word::INSTANCES as usize] = instances;
            values[word::PRIMITIVES as usize] = primitives;
            values[word::STREAM as usize] = stream;
            let bytes = words_as_bytes(&values);
            let mut offset = [0u64];
            let ring = backend.write_buffer_ring(
                RingKind::Data,
                &[&bytes],
                limits.min_uniform_offset_alignment,
                &mut offset,
            )?;
            let set = backend.create_binding_set(&[
                BindingEntry {
                    binding: VALUES_BINDING,
                    stages: StageSet::VERTEX,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offset[0],
                        size: bytes.len() as u64,
                    }),
                },
                BindingEntry {
                    binding: VERTICES_BINDING,
                    stages: StageSet::VERTEX,
                    resource: BindingResource::Texture(&records.view),
                },
            ])?;
            let mut sets = draw.geometry_sets;
            sets[STAGE_SET as usize] = Some(&set);
            backend.capture_vertices(
                &DrawCall {
                    program: &geometry.program,
                    blend: &self.states.blend,
                    depth_stencil: &self.states.depth_stencil,
                    raster: &self.states.raster,
                    vertex_layout: &geometry.no_streams,
                    bindings: sets,
                    vertex_buffers: &[],
                    index: None,
                    topology: Topology::PointList,
                    restart: false,
                    viewports: &[CAPTURE_VIEWPORT],
                    scissors: &[],
                    blend_constant: Default::default(),
                    stencil_reference: Default::default(),
                    immediates: &[],
                    range: DrawRange::Vertices {
                        first: 0,
                        count: vertices,
                        first_instance: 0,
                        instances,
                    },
                },
                &[BufferRange {
                    buffer: out,
                    offset: 0,
                    size: out_bytes,
                }],
                false,
            )?;
            if let Some((declared, targets)) = streamed {
                let record = Record {
                    words: geometry.out_words,
                    whole_word: true,
                };
                let streams = (stream, geometry.streams);
                stream_out(
                    backend,
                    declared,
                    targets,
                    (out, out_bytes, record),
                    geometry.output,
                    streams,
                )?;
            }
        }
        let count = vertices
            .checked_mul(instances)
            .ok_or(GpuError::OutOfMemory)?;
        let out = &self.out.as_ref().unwrap().buffer;
        Ok(Some(FinalDraw {
            buffer: out.clone(),
            index_offset: None,
            topology: list_topology(geometry.output),
            count,
            tessellated: None,
        }))
    }

    /// The records' texture, made at least `slots` texels: rows as wide as the slots need, up to the
    /// widest the back end makes. Returns its width.
    fn records_for(&mut self, backend: &mut B, slots: u32, widest: u32) -> Result<u32, GpuError> {
        let fits = self
            .records
            .as_ref()
            .is_some_and(|r| u64::from(r.width) * u64::from(r.height) >= u64::from(slots));
        if !fits {
            let width = slots.next_power_of_two().min(widest);
            let rows = slots.div_ceil(width);
            if rows > widest {
                return Err(GpuError::OutOfMemory);
            }
            let height = rows.next_power_of_two().min(widest);
            let texture = backend.create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format: RECORD_FORMAT,
                width,
                height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })?;
            let view = backend.create_view(
                &texture,
                &TextureViewDesc {
                    format: RECORD_FORMAT,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )?;
            if let Some(old) = self.records.take() {
                backend.destroy(Object::TextureView(old.view));
                backend.destroy(Object::Texture(old.texture));
            }
            self.records = Some(Records {
                texture,
                view,
                width,
                height,
            });
        }
        Ok(self.records.as_ref().unwrap().width)
    }

    /// Releases everything the route made.
    pub fn destroy(self, backend: &mut B) {
        backend.destroy(Object::Program(self.capture));
        backend.destroy(Object::Blend(self.states.blend));
        backend.destroy(Object::DepthStencil(self.states.depth_stencil));
        backend.destroy(Object::Raster(self.states.raster));
        if let Some(g) = self.geometry {
            backend.destroy(Object::Program(g.program));
            backend.destroy(Object::VertexLayout(g.no_streams));
        }
        if let Some(l) = self.lines {
            backend.destroy(Object::Program(l.program));
            backend.destroy(Object::VertexLayout(l.no_streams));
        }
        if let Some(p) = self.primitives {
            backend.destroy(Object::Program(p.program));
            backend.destroy(Object::VertexLayout(p.no_streams));
        }
        for owned in [self.captured, self.out].into_iter().flatten() {
            backend.destroy(Object::Buffer(owned.buffer));
        }
        if let Some(r) = self.records {
            backend.destroy(Object::TextureView(r.view));
            backend.destroy(Object::Texture(r.texture));
        }
    }
}

/// Captures `count` vertices of a replay `program` (with its empty vertex layout), which reads
/// `values` and `records`, into `out`.
fn replay_capture<B: Backend>(
    backend: &mut B,
    states: &CaptureStates<B>,
    (program, no_streams): (&B::Program, &B::VertexLayout),
    values: &[u32],
    records: &Records<B>,
    count: u32,
    out: BufferRange<&B::Buffer>,
) -> Result<(), StageError> {
    let bytes = words_as_bytes(values);
    let mut offset = [0u64];
    let alignment = backend.caps().limits.min_uniform_offset_alignment;
    let ring = backend.write_buffer_ring(RingKind::Data, &[&bytes], alignment, &mut offset)?;
    let set = backend.create_binding_set(&[
        BindingEntry {
            binding: VALUES_BINDING,
            stages: StageSet::VERTEX,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ring,
                offset: offset[0],
                size: bytes.len() as u64,
            }),
        },
        BindingEntry {
            binding: VERTICES_BINDING,
            stages: StageSet::VERTEX,
            resource: BindingResource::Texture(&records.view),
        },
    ])?;
    let mut sets = [None; MAX_BINDING_SETS];
    sets[STAGE_SET as usize] = Some(&set);
    backend.capture_vertices(
        &DrawCall {
            program,
            blend: &states.blend,
            depth_stencil: &states.depth_stencil,
            raster: &states.raster,
            vertex_layout: no_streams,
            bindings: sets,
            vertex_buffers: &[],
            index: None,
            topology: Topology::PointList,
            restart: false,
            viewports: &[CAPTURE_VIEWPORT],
            scissors: &[],
            blend_constant: Default::default(),
            stencil_reference: Default::default(),
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count,
                first_instance: 0,
                instances: 1,
            },
        },
        &[out],
        false,
    )?;
    Ok(())
}

/// Copies `rows` rows of `width` record slots from the vertex stage's capture into the records'
/// texture.
fn copy_records<B: Backend>(
    backend: &mut B,
    captured: &B::Buffer,
    records: &Records<B>,
    width: u32,
    rows: u32,
) {
    backend.copy_buffer_to_texture(
        &CopyBufferTexture {
            buffer: captured,
            offset: 0,
            row_pitch: width * RECORD_SLOT_BYTES,
            rows_per_layer: rows,
        },
        &TextureRegion {
            texture: &records.texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width,
            height: rows,
            depth_or_layers: 1,
        },
    );
}

/// A captured vertex's words, and whether its last says whether the vertex's primitive is whole
/// (the replay's capture) or every primitive is (the vertex stage's).
#[derive(Clone, Copy)]
struct Record {
    words: u32,
    whole_word: bool,
}

/// Writes stream `stream`'s output on the host ([`SlowPath::StreamOutAfterReplay`]) from a capture
/// `out` (`out_bytes`, `record` a vertex, `vertices` a primitive) of that stream, of `streams` the
/// shader writes, as Direct3D's stream output stage: the whole primitives in order, each written only
/// while every bound buffer of the stream holds it, and the filled sizes and the stream's counts
/// advanced.
fn stream_out<B: Backend>(
    backend: &mut B,
    declared: &StreamDeclaration,
    targets: &StreamTargets<'_, B>,
    (out, out_bytes, record): (&B::Buffer, u64, Record),
    vertices: u32,
    (stream, streams): (u32, u32),
) -> Result<(), StageError> {
    backend.slow_paths().hit(SlowPath::StreamOutAfterReplay);
    let filled_words = STREAM_BUFFERS + 2 * streams as usize;
    let filled_bytes = filled_words as u64 * u64::from(WORD);
    if targets.filled.size < filled_bytes {
        return Err(ConvertError::Malformed(STREAM_FILLED_SIZE.into()).into());
    }
    let mut bytes = alloc::vec![0u8; filled_bytes as usize];
    backend.read_back_now(
        &ReadSource::Buffer {
            buffer: targets.filled.buffer,
            offset: targets.filled.offset,
            size: filled_bytes,
        },
        &mut bytes,
    )?;
    let mut filled = words_of(&bytes);
    let mut bytes = alloc::vec![0u8; out_bytes as usize];
    backend.read_back_now(
        &ReadSource::Buffer {
            buffer: out,
            offset: 0,
            size: out_bytes,
        },
        &mut bytes,
    )?;
    let captured = words_of(&bytes);

    // The primitives every bound buffer of the stream holds past its filled size.
    let mut strides = [0u64; STREAM_BUFFERS];
    let mut capacity = None;
    for (b, target) in targets.buffers.iter().enumerate() {
        if let Some(t) = target
            && u32::from(declared.buffer_streams[b]) == stream
            && declared.strides[b] != 0
        {
            strides[b] = u64::from(declared.strides[b]);
            let room =
                t.size.saturating_sub(u64::from(filled[b])) / (strides[b] * u64::from(vertices));
            capacity = Some(capacity.map_or(room, |c: u64| c.min(room)));
        }
    }
    let capacity = capacity.unwrap_or(0);
    let whole = record.whole_word.then(|| record.words as usize - 1);
    let record = record.words as usize;
    let first_vertices: Vec<usize> = captured
        .chunks_exact(record * vertices as usize)
        .enumerate()
        .filter(|(_, p)| whole.is_none_or(|w| p[w] != 0))
        .map(|(i, _)| i * vertices as usize)
        .collect();
    let made = first_vertices.len() as u64;
    let written = made.min(capacity);

    // Each buffer's written bytes, read and patched element by element, so gaps keep their bytes.
    for (b, target) in targets.buffers.iter().enumerate() {
        let Some(t) = target else { continue };
        let length = written * u64::from(vertices) * strides[b];
        if length == 0 {
            continue;
        }
        let at = t.offset + u64::from(filled[b]);
        let mut old = alloc::vec![0u8; length as usize];
        backend.read_back_now(
            &ReadSource::Buffer {
                buffer: t.buffer,
                offset: at,
                size: length,
            },
            &mut old,
        )?;
        let stride = strides[b] as usize;
        for (p, &first) in first_vertices.iter().take(written as usize).enumerate() {
            for k in 0..vertices as usize {
                let vertex = &captured[(first + k) * record..][..record];
                let base = (p * vertices as usize + k) * stride;
                for e in declared.elements(stream as u8).filter(|e| e.buffer == b) {
                    let words = &vertex[e.word as usize..][..e.count as usize];
                    let to = base + e.offset as usize;
                    old[to..to + words.len() * WORD as usize]
                        .copy_from_slice(&words_as_bytes(words));
                }
            }
        }
        backend.write_buffer(t.buffer, at, &old, WriteMode::Ordered);
    }
    for b in 0..STREAM_BUFFERS {
        let advanced = u64::from(filled[b]) + written * u64::from(vertices) * strides[b];
        filled[b] = u32::try_from(advanced).map_err(|_| GpuError::OutOfMemory)?;
    }
    let counts = STREAM_BUFFERS + 2 * stream as usize;
    filled[counts] = written as u32;
    filled[counts + 1] = made as u32;
    backend.write_buffer(
        targets.filled.buffer,
        targets.filled.offset,
        &words_as_bytes(&filled),
        WriteMode::Ordered,
    );
    Ok(())
}

/// The places `first` and `count` of `index`, read back, as an indexed list of the primitives of
/// `vertices` each the strip makes, in the order the stage helper's strip program lists them
/// (`strips.wgsl`): a restart index ends a strip, and odd triangles take their last two vertices
/// swapped. The read covers whole words, as buffer copies do.
fn listed_indices<B: Backend>(
    backend: &mut B,
    index: &IndexBinding<'_, B>,
    (first, count): (u32, u32),
    vertices: u32,
) -> Result<Vec<u32>, StageError> {
    let (size, restart) = match index.format {
        IndexFormat::U8 => (size_of::<u8>() as u64, u32::from(u8::MAX)),
        IndexFormat::U16 => (size_of::<u16>() as u64, u32::from(u16::MAX)),
        IndexFormat::U32 => (size_of::<u32>() as u64, u32::MAX),
    };
    let start = index.offset + u64::from(first) * size;
    let word = u64::from(WORD);
    let from = start / word * word;
    let end = (start + u64::from(count) * size).div_ceil(word) * word;
    let mut bytes =
        alloc::vec![0u8; usize::try_from(end - from).map_err(|_| GpuError::OutOfMemory)?];
    backend.read_back_now(
        &ReadSource::Buffer {
            buffer: index.buffer,
            offset: from,
            size: end - from,
        },
        &mut bytes,
    )?;
    let skip = (start - from) as usize;
    let indices: Vec<u32> = bytes[skip..]
        .chunks_exact(size as usize)
        .take(count as usize)
        .map(|c| c.iter().rev().fold(0, |v, &b| v << u8::BITS | u32::from(b)))
        .collect();
    Ok(listed_strip(&indices, restart, vertices as usize))
}

/// The primitives of `vertices` each that a strip of `indices` makes, as an indexed list (see
/// [`listed_indices`]).
fn listed_strip(indices: &[u32], restart: u32, vertices: usize) -> Vec<u32> {
    let mut list = Vec::new();
    let mut start = 0;
    for (i, &v) in indices.iter().enumerate() {
        if v == restart {
            start = i + 1;
            continue;
        }
        let Some(primitive) = indices.get(i..i + vertices) else {
            break;
        };
        if primitive.contains(&restart) {
            continue;
        }
        let odd = vertices == TRIANGLE_VERTICES as usize && (i - start) % 2 != 0;
        list.extend((0..vertices).map(|k| match odd && k != 0 {
            true => primitive[vertices - k],
            false => primitive[k],
        }));
    }
    list
}

fn words_of(bytes: &[u8]) -> Vec<u32> {
    let (words, _) = bytes.as_chunks::<{ WORD as usize }>();
    words.iter().map(|w| u32::from_le_bytes(*w)).collect()
}

/// A program of `module`'s vertex program alone.
fn render_program<B: Backend>(
    backend: &mut B,
    module: &ShaderModule,
) -> Result<B::Program, GpuError> {
    let shader = backend.create_shader(module)?;
    let program = backend.create_program(&Program::Render {
        vertex: &shader,
        fragment: None,
    });
    backend.destroy(Object::Shader(shader));
    program
}

/// Whether a global variable of `module` is an array of arrays.
fn nested_array(module: &naga::Module) -> bool {
    let array = |ty: naga::Handle<naga::Type>| match module.types[ty].inner {
        naga::TypeInner::Array { base, .. } => Some(base),
        _ => None,
    };
    module
        .global_variables
        .iter()
        .any(|(_, g)| array(g.ty).and_then(array).is_some())
}
