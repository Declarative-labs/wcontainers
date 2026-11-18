//! The stage helper's run time: it makes the compute programs once per pair of vertex and geometry
//! shaders (or a vertex shader whose output is streamed), and for each API draw records the
//! dispatches and says what the front draws after them.

use super::tessellate::{Tessellation, Tessellator};
use super::{
    GEOMETRY_BINDING, INDICES_BINDING, PATCHES_BINDING, VALUES_BINDING, VERTICES_BINDING,
    WORKGROUP, word,
};
use crate::{ConvertError, Converted};
use alloc::string::String;
use alloc::vec::Vec;
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
    /// The draw's topology does not give the geometry shader's input primitive, which Direct3D does not
    /// draw either.
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

/// One element of a stream output declaration (Direct3D's `D3D11_SO_DECLARATION_ENTRY`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOutputEntry {
    pub stream: u8,
    /// The output's semantic; `None` for a gap of `component_count` components.
    pub semantic: Option<String>,
    pub semantic_index: u32,
    pub start_component: u8,
    pub component_count: u8,
    /// The buffer it is written to, 0 to 3.
    pub slot: u8,
}

/// A stream output declaration: its elements, and each buffer's bytes per vertex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOutput {
    pub entries: Vec<StreamOutputEntry>,
    pub strides: [u32; 4],
}

/// Where one draw's stream output goes.
#[derive(Debug)]
pub struct StreamTargets<'a, B: Backend> {
    /// The bound buffers (made with [`BufferUsage::STORAGE`]), each written from its range's offset
    /// (aligned as storage bindings are) plus its filled size.
    pub buffers: [Option<BufferRange<&'a B::Buffer>>; 4],
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
/// vertex streams, indices (u32) from [`FinalDraw::buffer`] at offset 0, as an indexed indirect draw
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
    out_locations: u32,
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

/// The stream output program and its declaration's elements.
struct Stream<B: Backend> {
    program: B::Program,
    /// Each element as the program's table holds it: buffer, first component and count packed,
    /// offset in the buffer's vertex, record slot.
    entries: Vec<[u32; 3]>,
    strides: [u32; 4],
}

/// The program's values before the strides, in words.
const STREAM_WORDS: usize = 12;
/// The elements its table holds.
const STREAM_ENTRIES: usize = 64;

/// A vertex shader, and a geometry shader or stream output or both, or a hull shader and the
/// tessellator, run as compute work before the draw ([`SlowPath::GeometryAsCompute`],
/// [`SlowPath::StreamOutAsCompute`], [`SlowPath::TessellationAsCompute`]).
pub struct Stages<B: Backend> {
    vertex: B::Program,
    geometry: Option<Geometry<B>>,
    hull: Option<Hull<B>>,
    stream: Option<Stream<B>>,
    /// Words per vertex record.
    in_words: u32,
    records: Option<Owned<B>>,
    out: Option<Owned<B>>,
    /// Stands in for bindings a draw does not use: five storage ranges 256 bytes apart (the index
    /// words, the four stream output buffers).
    unused: B::Buffer,
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
                "the stage helper without work".into(),
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
        let unused = backend.create_buffer(&BufferDesc {
            size: 1280,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            geometry,
            hull: None,
            stream,
            in_words: super::record_words(read.len()),
            records: None,
            out: None,
            unused,
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
                "not a hull shader".into(),
            )));
        };
        match domain.reflection.extras {
            StageExtras::Domain { domain: d, .. } if d == hull_domain => {}
            StageExtras::Domain { .. } => return Err(StageError::Declaration),
            _ => {
                return Err(StageError::Convert(ConvertError::Malformed(
                    "not a domain shader".into(),
                )));
            }
        }
        let read = super::locations(&hull.reflection.inputs);
        let shader = backend.create_shader(&super::vertex_program_for(vertex, &read)?)?;
        let vertex_program = backend.create_program(&Program::Compute(&shader))?;
        let shader = backend.create_shader(&hull.modules[0])?;
        let hull_program = backend.create_program(&Program::Compute(&shader))?;
        let patch_words = u32::from(output_control_points) * 4 * u32::from(point_locations)
            + 4 * u32::from(constant_locations)
            + 8;
        let unused = backend.create_buffer(&BufferDesc {
            size: 1280,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            geometry: None,
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
            stream: None,
            in_words: super::record_words(read.len()),
            records: None,
            out: None,
            unused,
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
        if let Some(s) = self.stream {
            backend.destroy(Object::Program(s.program));
        }
        for owned in [self.records, self.out].into_iter().flatten() {
            backend.destroy(Object::Buffer(owned.buffer));
        }
        backend.destroy(Object::Buffer(self.unused));
    }

    /// The vertex program of the draw after a geometry stage, for the front's program with its pixel
    /// program.
    pub fn passthrough(&self) -> Option<&ShaderModule> {
        self.geometry.as_ref().map(|g| &g.passthrough)
    }

    /// The vertex layout of that draw.
    pub fn vertex_layout(&self) -> Option<VertexLayoutDesc> {
        let g = self.geometry.as_ref()?;
        let mut desc = VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        };
        desc.streams[0] = Some(VertexStream {
            stride: g.out_words * 4,
            step: StepMode::Vertex,
        });
        for slot in 0..=g.out_locations {
            desc.attributes[slot as usize] = Some(VertexAttribute {
                location: slot,
                stream: 0,
                offset: 16 * slot,
                format: VertexFormat::Uint32x4,
            });
        }
        Some(desc)
    }

    /// Records the stages' dispatches for one API draw (outside a pass), and returns what the front
    /// draws next after a geometry stage; `None` when the draw makes no primitive, or when there is no
    /// geometry stage (the front draws as it would, its output having been streamed).
    pub fn run(
        &mut self,
        backend: &mut B,
        draw: &StageDraw<'_, B>,
    ) -> Result<Option<FinalDraw<B>>, StageError> {
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
            _ => return Err(StageError::Unsupported("StageIndirect")),
        };
        use InputTopology as T;
        let (n, strip) = match draw.topology {
            T::PointList => (1, false),
            T::LineList => (2, false),
            T::LineStrip => (2, true),
            T::TriangleList => (3, false),
            T::TriangleStrip => (3, true),
            T::LineListAdjacency => (4, false),
            T::LineStripAdjacency => (4, true),
            T::TriangleListAdjacency => (6, false),
            T::TriangleStripAdjacency => {
                return Err(StageError::Unsupported("GeometryTriangleStripAdjacency"));
            }
            T::PatchList(n) => (u32::from(n), false),
        };
        let patches = matches!(draw.topology, T::PatchList(_));
        match (&self.geometry, &self.hull) {
            (_, Some(h)) if !patches || n != h.input_points => {
                return Err(StageError::TopologyMismatch);
            }
            (_, None) if patches => return Err(StageError::TopologyMismatch),
            (Some(g), _) if n != g.input_vertices => return Err(StageError::TopologyMismatch),
            (None, None) if n > 3 => return Err(StageError::Unsupported("StreamOutputAdjacency")),
            _ => {}
        }
        if strip && draw.index.is_some() {
            // Direct3D's indexed strips restart at the largest index, which needs a pass over the
            // indices to number the primitives.
            return Err(StageError::Unsupported("StageIndexedStrip"));
        }
        let primitives = match strip {
            false => count / n,
            true => (count + 1).saturating_sub(n),
        };
        let streamed = self.stream.is_some() && draw.stream_output.is_some();
        let drawn = self.geometry.is_some() || self.hull.is_some();
        if primitives == 0 || instances == 0 || (!drawn && !streamed) {
            return Ok(None);
        }
        let limits = backend.caps().limits;
        let record_bytes = u64::from(count) * u64::from(instances) * u64::from(self.in_words) * 4;
        grow(
            backend,
            &mut self.records,
            record_bytes,
            BufferUsage::STORAGE,
        )?;
        // The geometry output's size and where its index slots start.
        let (invocations, out_bytes, index_base) = match &self.geometry {
            Some(g) => {
                let invocations = u64::from(primitives * g.instances) * u64::from(instances);
                let vertex_words = invocations * u64::from(g.max_vertices) * u64::from(g.out_words);
                let index_words = invocations * 2 * u64::from(g.max_vertices);
                let index_base = u32::try_from(vertex_words).map_err(|_| GpuError::OutOfMemory)?;
                let usage = BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX;
                grow(
                    backend,
                    &mut self.out,
                    (vertex_words + index_words) * 4,
                    usage,
                )?;
                (invocations, (vertex_words + index_words) * 4, index_base)
            }
            None => (0, 0, 0),
        };

        let (index_bytes, index_offset, indices) = match &draw.index {
            Some(index) => {
                let bytes = match index.format {
                    IndexFormat::U16 => 2,
                    IndexFormat::U32 => 4,
                    IndexFormat::U8 => return Err(StageError::Unsupported("StageIndexU8")),
                };
                let align = u64::from(limits.min_storage_offset_alignment);
                let start = index.offset - index.offset % align;
                let size = (index.size - start) & !3;
                let range = BufferRange {
                    buffer: index.buffer,
                    offset: start,
                    size,
                };
                (bytes, (index.offset - start) as u32, range)
            }
            None => (0, 0, self.unused_range(0)),
        };
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
        let bytes = words_as_bytes(&values);
        let mut offsets = [0u64];
        let ring = backend.write_buffer_ring(
            RingKind::Data,
            &[&bytes],
            limits.min_uniform_offset_alignment,
            &mut offsets,
        )?;
        let uniform = || BindingEntry {
            binding: VALUES_BINDING,
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ring,
                offset: offsets[0],
                size: bytes.len() as u64,
            }),
        };
        let records = &self.records.as_ref().unwrap().buffer;
        let vertex_set = backend.create_binding_set(&[
            uniform(),
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
        let groups = |x: u32| DispatchGroups::Direct([x.div_ceil(WORKGROUP), instances, 1]);
        let mut sets = draw.vertex_sets;
        sets[super::STAGE_SET as usize] = Some(&vertex_set);
        backend.dispatch(&DispatchCall {
            program: &self.vertex,
            bindings: sets,
            immediates: &[],
            groups: groups(count),
        })?;

        let mut last = None;
        if let Some(g) = &self.geometry {
            backend.slow_paths().hit(SlowPath::GeometryAsCompute);
            let out = &self.out.as_ref().unwrap().buffer;
            let geometry_set = backend.create_binding_set(&[
                uniform(),
                storage(VERTICES_BINDING, records, record_bytes, false),
                storage(GEOMETRY_BINDING, out, out_bytes, true),
            ])?;
            let mut sets = draw.geometry_sets;
            sets[super::STAGE_SET as usize] = Some(&geometry_set);
            backend.dispatch(&DispatchCall {
                program: &g.program,
                bindings: sets,
                immediates: &[],
                groups: groups(primitives * g.instances),
            })?;
            let total = u32::try_from(invocations * u64::from(g.max_vertices))
                .map_err(|_| GpuError::OutOfMemory)?;
            let strips = g.output != 1;
            last = Some(FinalDraw {
                buffer: out.clone(),
                index_offset: strips.then_some(u64::from(index_base) * 4),
                topology: match g.output {
                    1 => Topology::PointList,
                    2 => Topology::LineStrip,
                    _ => Topology::TriangleStrip,
                },
                count: if strips { 2 * total } else { total },
                tessellated: None,
            });
        }

        if let Some(h) = &mut self.hull {
            backend.slow_paths().hit(SlowPath::TessellationAsCompute);
            let total = primitives
                .checked_mul(instances)
                .ok_or(GpuError::OutOfMemory)?;
            let patch_bytes = (u64::from(super::tess::HEADER_WORDS)
                + u64::from(total) * u64::from(h.patch_words))
                * 4;
            grow(backend, &mut h.patches, patch_bytes, BufferUsage::STORAGE)?;
            let patches = &h.patches.as_ref().unwrap().buffer;
            let hull_set = backend.create_binding_set(&[
                uniform(),
                storage(VERTICES_BINDING, records, record_bytes, false),
                storage(PATCHES_BINDING, patches, patch_bytes, true),
            ])?;
            let mut sets = draw.geometry_sets;
            sets[super::STAGE_SET as usize] = Some(&hull_set);
            backend.dispatch(&DispatchCall {
                program: &h.program,
                bindings: sets,
                immediates: &[],
                groups: groups(primitives),
            })?;
            let hull_range = BufferRange {
                buffer: patches,
                offset: 0,
                size: patch_bytes,
            };
            h.tessellator
                .run(backend, &h.tessellation, hull_range, total, primitives)?;
            let (_, per_indices) = super::tess::patch_bounds(h.tessellation.max_factor);
            let (arguments, points, indices) = h.tessellator.owned();
            last = Some(FinalDraw {
                buffer: indices.buffer.clone(),
                index_offset: Some(0),
                topology: match h.tessellation.output {
                    0 => Topology::PointList,
                    1 => Topology::LineList,
                    _ => Topology::TriangleList,
                },
                count: total.saturating_mul(per_indices),
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

        if let (Some(stream), Some(targets)) = (&self.stream, &draw.stream_output) {
            backend.slow_paths().hit(SlowPath::StreamOutAsCompute);
            let mut so = [0u32; STREAM_WORDS + 8 + 4 * STREAM_ENTRIES];
            let (data, data_bytes) = match &self.geometry {
                Some(g) => {
                    so[0] = invocations as u32;
                    so[1] = 1;
                    so[2] = g.output;
                    so[4] = 2 * g.max_vertices;
                    so[5] = index_base;
                    so[6] = g.out_words;
                    (&self.out.as_ref().unwrap().buffer, out_bytes)
                }
                None => {
                    so[0] = primitives * instances;
                    so[2] = n;
                    so[3] = strip.into();
                    so[6] = self.in_words;
                    (records, record_bytes)
                }
            };
            so[8] = primitives;
            so[9] = count;
            // Elements of buffers not bound are left out; such a buffer neither holds nor limits.
            let mut entries = 0;
            for e in &stream.entries {
                let buffer = (e[0] & 0xff) as usize;
                if targets.buffers[buffer].is_none() {
                    continue;
                }
                let at = STREAM_WORDS + 8 + 4 * entries;
                so[at..at + 3].copy_from_slice(e);
                entries += 1;
            }
            so[7] = entries as u32;
            for (b, target) in targets.buffers.iter().enumerate() {
                if let Some(t) = target {
                    so[STREAM_WORDS + b] = stream.strides[b];
                    so[STREAM_WORDS + 4 + b] = u32::try_from(t.size).unwrap_or(u32::MAX);
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
                    binding: 0,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[0],
                        size: bytes.len() as u64,
                    }),
                },
                storage(1, data, data_bytes, false),
                BindingEntry {
                    binding: 2,
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
                    binding: 3 + b as u32,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range,
                        writable: true,
                    },
                });
            }
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
            offset: 256 * i,
            size: 16,
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
            "not a geometry shader".into(),
        )));
    };
    let outputs = super::locations(&geometry.reflection.outputs);
    let shader = backend.create_shader(&geometry.modules[0])?;
    Ok(Geometry {
        program: backend.create_program(&Program::Compute(&shader))?,
        passthrough: geometry.modules[1].clone(),
        input_vertices: input_vertices.into(),
        max_vertices: max_output_vertices,
        instances,
        output: output_vertices.into(),
        out_words: super::record_words(outputs.len()),
        out_locations: outputs.len() as u32,
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
    let mut ends = [0u32; 4];
    let mut entries = Vec::new();
    for e in &so.entries {
        if e.stream != 0 {
            return Err(StageError::Unsupported("StreamOutputStreams"));
        }
        let (buffer, count, start) = (
            usize::from(e.slot),
            u32::from(e.component_count),
            u32::from(e.start_component),
        );
        if buffer > 3 || count == 0 || start + count > 4 {
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
                Some(_) => return Err(StageError::Unsupported("StreamOutputSystemValue")),
            };
            if first + count > 4 {
                return Err(StageError::Declaration);
            }
            entries.push([buffer as u32 | first << 8 | count << 16, ends[buffer], slot]);
        }
        ends[buffer] += 4 * count;
    }
    if entries.len() > STREAM_ENTRIES {
        return Err(StageError::Unsupported("StreamOutputEntries"));
    }
    // A buffer's stride holds its elements; a single buffer declared without one is packed.
    let mut strides = so.strides;
    for b in 0..4 {
        if strides[b] == 0 {
            strides[b] = ends[b];
        } else if strides[b] < ends[b] || !strides[b].is_multiple_of(4) {
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
    let size = size.max(256).next_power_of_two();
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
