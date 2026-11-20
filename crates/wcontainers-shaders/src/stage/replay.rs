//! A geometry shader on a back end with transform feedback and without compute (WebGL 2): the vertex
//! stage is captured into records, the records are copied into a texture, and the geometry shader's
//! replay form (`VariantFlags::GEOMETRY_REPLAY`) is captured in turn, one vertex per corner of each
//! output primitive slot; the pass-through draws what it captured, and stream output is written on
//! the host from it.

use super::helper::{
    FinalDraw, InputTopology, LINE_VERTICES, NOT_A_GEOMETRY_SHADER, Owned, STAGE_INDIRECT,
    STREAM_BUFFERS, STREAM_FILLED_SIZE, StageError, StreamDeclaration, StreamOutput, StreamTargets,
    grow, list_topology, record_layout, stream_declaration, words_as_bytes,
};
use super::{
    LINE_ADJACENCY_VERTICES, SLOT_BYTES, STAGE_SET, TRIANGLE_ADJACENCY_VERTICES, TRIANGLE_VERTICES,
    VALUES_BINDING, VERTICES_BINDING, word,
};
use crate::build::{VEC4, WORD};
use crate::{ConvertError, Converted};
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendDesc, BufferRange, BufferUsage, ColorWrites,
    CopyBufferTexture, CullMode, DepthStencilDesc, DrawCall, DrawRange, Extent, Features, FillMode,
    Format, FrontFace, GpuError, IndexBinding, MAX_BINDING_SETS, MAX_COLOR_TARGETS, Object, Origin,
    Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend, RingKind, ShaderModule,
    ShaderStage, SlowPath, StageExtras, StageSet, TextureDesc, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, Topology, VertexBinding, VertexLayoutDesc, ViewDimension,
    Viewport, WriteMode,
};

/// A strip with adjacency, which the capture cannot list.
const ADJACENT_STRIP: &str = "GeometryReplayAdjacentStrip";
/// An indexed strip: its restart indices make the captured primitives' count known on the GPU alone.
const INDEXED_STRIP: &str = "GeometryReplayIndexedStrip";
/// A record wider than one capture target holds.
const RECORD_COMPONENTS: &str = "GeometryReplayComponents";
/// Stream output from a geometry shader writing streams past the first, which the replay leaves out.
const STREAMS_PAST_THE_FIRST: &str = "GeometryReplayStreams";
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
/// capture through [`Self::passthrough`].
pub struct GeometryReplay<B: Backend> {
    capture: B::Program,
    replay: B::Program,
    passthrough: ShaderModule,
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
    no_streams: B::VertexLayout,
    input_vertices: u32,
    /// Vertices per output primitive: 1, 2 or 3.
    output: u32,
    /// Vertices the replay draws per input primitive: every corner of every slot of every geometry
    /// instance.
    primitive_vertices: u32,
    /// Four-word slots per vertex record, and per output record.
    in_slots: u32,
    out_slots: u32,
    /// Words per captured output vertex: its record, then whether its slot's primitive is whole.
    out_words: u32,
    selects: bool,
    stream: Option<StreamDeclaration>,
    captured: Option<Owned<B>>,
    records: Option<Records<B>>,
    out: Option<Owned<B>>,
}

impl<B: Backend> GeometryReplay<B> {
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
        if !backend
            .caps()
            .features
            .contains(Features::TRANSFORM_FEEDBACK)
        {
            return Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK).into());
        }
        let in_slots = 1 + super::locations(&geometry.reflection.inputs).len() as u32;
        let selects = super::selects(&geometry.reflection);
        let distances = super::clip_distances(&geometry.reflection);
        let out_slots = 1
            + super::locations(&geometry.reflection.outputs).len() as u32
            + u32::from(selects)
            + super::distance_slots(distances);
        let out_words = out_slots * VEC4 + 1;
        if (in_slots * VEC4).max(out_words) > backend.caps().limits.capture_components {
            return Err(StageError::Unsupported(RECORD_COMPONENTS));
        }
        let stream = match stream_output {
            Some(_) if streams > 1 => return Err(StageError::Unsupported(STREAMS_PAST_THE_FIRST)),
            Some(so) => Some(stream_declaration(so, &geometry.reflection.outputs, 1)?),
            None => None,
        };
        let output = u32::from(output_vertices);
        let primitive_vertices =
            instances.max(1) * super::replay_slots(max_output_vertices, output) * output;
        let capture = super::capture_program(vertex, geometry)?;
        let capture = render_program(backend, &capture)?;
        let replay = render_program(backend, &geometry.modules[0])?;
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
        // Strips are captured as lists with the first vertex provoking, the order the replay reads.
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
            depth_minus_one: false,
            sample_mask: !0,
        })?;
        let no_streams = backend.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        })?;
        Ok(GeometryReplay {
            capture,
            replay,
            passthrough: geometry.modules[1].clone(),
            blend,
            depth_stencil,
            raster,
            no_streams,
            input_vertices: input_vertices.into(),
            output,
            primitive_vertices,
            in_slots,
            out_slots,
            out_words,
            selects,
            stream,
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
        self.selects
    }

    /// The vertex layout of that draw.
    pub fn vertex_layout(&self) -> VertexLayoutDesc {
        record_layout(self.out_words, self.out_slots - 1)
    }

    /// Records the captures of one API draw (outside a pass), and returns what the front draws next;
    /// `None` when the draw makes no primitive.
    pub fn run(
        &mut self,
        backend: &mut B,
        draw: ReplayDraw<'_, B>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
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
        if n != self.input_vertices {
            return Err(StageError::TopologyMismatch);
        }
        if strip && base_vertex.is_some() {
            return Err(StageError::Unsupported(INDEXED_STRIP));
        }
        let primitives = match strip {
            false => count / n,
            true => (count + 1).saturating_sub(n),
        };
        if primitives == 0 || instances == 0 {
            return Ok(None);
        }
        backend.slow_paths().hit(SlowPath::GeometryAsReplay);
        let limits = backend.caps().limits;
        let too_many = |_| GpuError::OutOfMemory;

        // The vertex stage's records, a list's trailing vertices left out so that each instance's
        // records follow the last's.
        let listed = primitives.checked_mul(n).ok_or(GpuError::OutOfMemory)?;
        let slots = u64::from(listed) * u64::from(instances) * u64::from(self.in_slots);
        let slots = u32::try_from(slots).map_err(too_many)?;
        let width = self.records_for(backend, slots, limits.max_texture_2d)?;
        let rows = slots.div_ceil(width);
        let records_bytes = u64::from(width) * u64::from(rows) * u64::from(SLOT_BYTES);
        let captured_bytes = u64::from(slots) * u64::from(SLOT_BYTES);
        let usage = BufferUsage::CAPTURE | BufferUsage::COPY_SRC;
        grow(backend, &mut self.captured, records_bytes, usage)?;
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
                blend: &self.blend,
                depth_stencil: &self.depth_stencil,
                raster: &self.raster,
                vertex_layout: draw.vertex_layout,
                bindings: draw.vertex_sets,
                vertex_buffers: draw.vertex_buffers,
                index: draw.index,
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
        let records = self.records.as_ref().unwrap();
        backend.copy_buffer_to_texture(
            &CopyBufferTexture {
                buffer: captured,
                offset: 0,
                row_pitch: width * SLOT_BYTES,
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

        // The replay: every corner of every output slot of every geometry invocation, per API
        // instance.
        let vertices = primitives
            .checked_mul(self.primitive_vertices)
            .ok_or(GpuError::OutOfMemory)?;
        let out_bytes = u64::from(vertices)
            * u64::from(instances)
            * u64::from(self.out_words)
            * u64::from(WORD);
        grow(
            backend,
            &mut self.out,
            out_bytes,
            BufferUsage::CAPTURE | BufferUsage::VERTEX | BufferUsage::COPY_SRC,
        )?;
        let mut values = [0u32; word::COUNT_OF as usize];
        values[word::COUNT as usize] = count;
        values[word::FIRST as usize] = first;
        values[word::FIRST_INSTANCE as usize] = first_instance;
        values[word::INSTANCES as usize] = instances;
        values[word::PRIMITIVES as usize] = primitives;
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
        let out = &self.out.as_ref().unwrap().buffer;
        backend.capture_vertices(
            &DrawCall {
                program: &self.replay,
                blend: &self.blend,
                depth_stencil: &self.depth_stencil,
                raster: &self.raster,
                vertex_layout: &self.no_streams,
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
        let count = vertices
            .checked_mul(instances)
            .ok_or(GpuError::OutOfMemory)?;
        if let (Some(declared), Some(targets)) = (&self.stream, &draw.stream_output) {
            let out = &self.out.as_ref().unwrap().buffer;
            stream_out(
                backend,
                declared,
                targets,
                out,
                out_bytes,
                self.out_words,
                self.output,
            )?;
        }
        let out = &self.out.as_ref().unwrap().buffer;
        Ok(Some(FinalDraw {
            buffer: out.clone(),
            index_offset: None,
            topology: list_topology(self.output),
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
        backend.destroy(Object::Program(self.replay));
        backend.destroy(Object::Blend(self.blend));
        backend.destroy(Object::DepthStencil(self.depth_stencil));
        backend.destroy(Object::Raster(self.raster));
        backend.destroy(Object::VertexLayout(self.no_streams));
        for owned in [self.captured, self.out].into_iter().flatten() {
            backend.destroy(Object::Buffer(owned.buffer));
        }
        if let Some(r) = self.records {
            backend.destroy(Object::TextureView(r.view));
            backend.destroy(Object::Texture(r.texture));
        }
    }
}

/// Writes stream 0's output on the host ([`SlowPath::StreamOutAfterReplay`]) from the replay's
/// capture `out` (`out_bytes`, `out_words` words a vertex, its last saying whether the vertex's
/// primitive is whole), as Direct3D's stream output stage: the whole primitives in order, each
/// written only while every bound buffer holds it, and the filled sizes and counts advanced.
fn stream_out<B: Backend>(
    backend: &mut B,
    declared: &StreamDeclaration,
    targets: &StreamTargets<'_, B>,
    out: &B::Buffer,
    out_bytes: u64,
    out_words: u32,
    vertices: u32,
) -> Result<(), StageError> {
    backend.slow_paths().hit(SlowPath::StreamOutAfterReplay);
    let filled_words = STREAM_BUFFERS + 2;
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

    // The primitives every bound buffer of stream 0 holds past its filled size.
    let mut strides = [0u64; STREAM_BUFFERS];
    let mut capacity = None;
    for (b, target) in targets.buffers.iter().enumerate() {
        if let Some(t) = target
            && declared.buffer_streams[b] == 0
            && declared.strides[b] != 0
        {
            strides[b] = u64::from(declared.strides[b]);
            let room =
                t.size.saturating_sub(u64::from(filled[b])) / (strides[b] * u64::from(vertices));
            capacity = Some(capacity.map_or(room, |c: u64| c.min(room)));
        }
    }
    let capacity = capacity.unwrap_or(0);
    let record = out_words as usize;
    let whole = record - 1;
    let first_vertices: Vec<usize> = captured
        .chunks_exact(record * vertices as usize)
        .enumerate()
        .filter(|(_, p)| p[whole] != 0)
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
                for e in declared.elements(0).filter(|e| e.buffer == b) {
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
    filled[STREAM_BUFFERS] = written as u32;
    filled[STREAM_BUFFERS + 1] = made as u32;
    backend.write_buffer(
        targets.filled.buffer,
        targets.filled.offset,
        &words_as_bytes(&filled),
        WriteMode::Ordered,
    );
    Ok(())
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
