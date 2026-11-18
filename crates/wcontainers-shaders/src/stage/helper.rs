//! The stage helper's run time: it makes the compute programs once per pair of vertex and geometry
//! shaders, and for each API draw records the two dispatches and says what the front draws after them.

use super::{GEOMETRY_BINDING, INDICES_BINDING, VALUES_BINDING, VERTICES_BINDING, WORKGROUP, word};
use crate::{ConvertError, Converted};
use alloc::vec::Vec;
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferDesc, BufferRange, BufferUsage, DispatchCall,
    DispatchGroups, DrawRange, GpuError, IndexFormat, MAX_BINDING_SETS, MapIntent, Program,
    RingKind, ShaderModule, SlowPath, StageExtras, StageSet, StepMode, Topology, VertexAttribute,
    VertexFormat, VertexLayoutDesc, VertexStream,
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
    /// The geometry stage's sets: its slots in set 0, its helper constants in set 3.
    pub geometry_sets: [Option<&'a B::BindingSet>; MAX_BINDING_SETS],
    pub index: Option<StageIndex<'a, B>>,
    pub topology: InputTopology,
    /// A direct draw's range.
    pub range: DrawRange<'a, B>,
}

/// What the front draws after [`Stages::run`], in its pass: the program it made of
/// [`Stages::passthrough`] and its pixel program, with [`Stages::vertex_layout`], `buffer` as vertex
/// stream 0 at offset 0, and with indices from `buffer` at `index_offset` (u32, strips restarting)
/// when given; `count` vertices or indices, one instance.
#[derive(Debug)]
pub struct FinalDraw<B: Backend> {
    pub buffer: B::Buffer,
    pub index_offset: Option<u64>,
    pub topology: Topology,
    pub count: u32,
}

/// Buffers the helper owns, grown as draws need.
struct Owned<B: Backend> {
    buffer: B::Buffer,
    size: u64,
}

/// A vertex shader and a geometry shader, run as compute work before the draw
/// ([`SlowPath::GeometryAsCompute`]).
pub struct Stages<B: Backend> {
    vertex: B::Program,
    geometry: B::Program,
    passthrough: ShaderModule,
    input_vertices: u32,
    max_vertices: u32,
    instances: u32,
    strips: bool,
    output: Topology,
    in_words: u32,
    out_words: u32,
    out_locations: u32,
    records: Option<Owned<B>>,
    out: Option<Owned<B>>,
    no_indices: B::Buffer,
}

impl<B: Backend> Stages<B> {
    /// Makes the compute programs of `vertex` (converted with every input pulled) and `geometry`.
    pub fn new(
        backend: &mut B,
        vertex: &Converted,
        geometry: &Converted,
    ) -> Result<Self, StageError> {
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
        let inputs = super::locations(&geometry.reflection.inputs);
        let outputs = super::locations(&geometry.reflection.outputs);
        let vertex_shader = backend.create_shader(&super::vertex_program(vertex, geometry)?)?;
        let geometry_shader = backend.create_shader(&geometry.modules[0])?;
        let vertex_program = backend.create_program(&Program::Compute(&vertex_shader))?;
        let geometry_program = backend.create_program(&Program::Compute(&geometry_shader))?;
        let no_indices = backend.create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::STORAGE,
            intent: MapIntent::Rarely,
        })?;
        Ok(Stages {
            vertex: vertex_program,
            geometry: geometry_program,
            passthrough: geometry.modules[1].clone(),
            input_vertices: input_vertices.into(),
            max_vertices: max_output_vertices,
            instances,
            strips: output_vertices != 1,
            output: match output_vertices {
                1 => Topology::PointList,
                2 => Topology::LineStrip,
                _ => Topology::TriangleStrip,
            },
            in_words: super::record_words(inputs.len()),
            out_words: super::record_words(outputs.len()),
            out_locations: outputs.len() as u32,
            records: None,
            out: None,
            no_indices,
        })
    }

    /// The vertex program of the draw after the geometry stage, for the front's program with its pixel
    /// program.
    pub fn passthrough(&self) -> &ShaderModule {
        &self.passthrough
    }

    /// The vertex layout of that draw.
    pub fn vertex_layout(&self) -> VertexLayoutDesc {
        let mut desc = VertexLayoutDesc {
            streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
            attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        };
        desc.streams[0] = Some(VertexStream {
            stride: self.out_words * 4,
            step: StepMode::Vertex,
        });
        for slot in 0..=self.out_locations {
            desc.attributes[slot as usize] = Some(VertexAttribute {
                location: slot,
                stream: 0,
                offset: 16 * slot,
                format: VertexFormat::Uint32x4,
            });
        }
        desc
    }

    /// Records the vertex and geometry stages' dispatches for one API draw (outside a pass), and
    /// returns what the front draws next; `None` when the draw makes no primitive.
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
            _ => return Err(StageError::Unsupported("GeometryIndirect")),
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
        };
        if n != self.input_vertices {
            return Err(StageError::TopologyMismatch);
        }
        if strip && draw.index.is_some() {
            // Direct3D's indexed strips restart at the largest index, which needs a pass over the
            // indices to number the primitives.
            return Err(StageError::Unsupported("GeometryIndexedStrip"));
        }
        let primitives = match strip {
            false => count / n,
            true => (count + 1).saturating_sub(n),
        };
        if primitives == 0 || instances == 0 {
            return Ok(None);
        }
        backend.slow_paths().hit(SlowPath::GeometryAsCompute);
        let limits = backend.caps().limits;
        let invocations = primitives * self.instances;
        let record_bytes = u64::from(count) * u64::from(instances) * u64::from(self.in_words) * 4;
        let slots = u64::from(invocations) * u64::from(instances);
        let vertex_words = slots * u64::from(self.max_vertices) * u64::from(self.out_words);
        let index_words = if self.strips {
            slots * 2 * u64::from(self.max_vertices)
        } else {
            0
        };
        let out_bytes = (vertex_words + index_words) * 4;
        let index_base = u32::try_from(vertex_words).map_err(|_| GpuError::OutOfMemory)?;
        grow(
            backend,
            &mut self.records,
            record_bytes,
            BufferUsage::STORAGE,
        )?;
        let usage = BufferUsage::STORAGE | BufferUsage::VERTEX | BufferUsage::INDEX;
        grow(backend, &mut self.out, out_bytes, usage)?;

        let (index_bytes, index_offset, indices) = match &draw.index {
            Some(index) => {
                let bytes = match index.format {
                    IndexFormat::U16 => 2,
                    IndexFormat::U32 => 4,
                    IndexFormat::U8 => return Err(StageError::Unsupported("GeometryIndexU8")),
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
            None => (
                0,
                0,
                BufferRange {
                    buffer: &self.no_indices,
                    offset: 0,
                    size: 16,
                },
            ),
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
        let bytes: Vec<u8> = values.iter().flat_map(|w| w.to_le_bytes()).collect();
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
        let records = self.records.as_ref().unwrap();
        let out = self.out.as_ref().unwrap();
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
            storage(VERTICES_BINDING, &records.buffer, record_bytes, true),
        ])?;
        let geometry_set = backend.create_binding_set(&[
            uniform(),
            storage(VERTICES_BINDING, &records.buffer, record_bytes, false),
            storage(GEOMETRY_BINDING, &out.buffer, out_bytes, true),
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
        let mut sets = draw.geometry_sets;
        sets[super::STAGE_SET as usize] = Some(&geometry_set);
        backend.dispatch(&DispatchCall {
            program: &self.geometry,
            bindings: sets,
            immediates: &[],
            groups: groups(invocations),
        })?;
        let total = u32::try_from(slots * u64::from(self.max_vertices))
            .map_err(|_| GpuError::OutOfMemory)?;
        Ok(Some(FinalDraw {
            buffer: out.buffer.clone(),
            index_offset: self.strips.then_some(vertex_words * 4),
            topology: self.output,
            count: if self.strips { 2 * total } else { total },
        }))
    }
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
fn grow<B: Backend>(
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
