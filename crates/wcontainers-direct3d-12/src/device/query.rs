//! Query heaps, `ResolveQueryData` and `SetPredication` (plan 5.2 and 4.12): binary occlusion as the
//! back end's any-sample queries turned into exactly 0 or 1 after the resolve, and predicated draws
//! and dispatches recorded as indirect ones whose counts a small compute program zeroes when the
//! predicate skips them, so nothing waits.

use wcontainers_gpu::{
    BindingEntry, BindingResource, BufferRange, QueryKind, RingKind, ShaderModule,
};

use super::*;

/// `D3D12_QUERY_HEAP_TYPE` values.
const HEAP_OCCLUSION: u32 = 0;
const HEAP_TIMESTAMP: u32 = 1;
/// `D3D12_QUERY_TYPE` values.
pub const QUERY_OCCLUSION: u32 = 0;
pub const QUERY_BINARY_OCCLUSION: u32 = 1;
pub const QUERY_TIMESTAMP: u32 = 2;

/// One argument record of a predicated draw or dispatch: five words, the count first.
const RECORD: u64 = 20;

/// Turns resolved any-sample answers into exactly 0 or 1, in place.
const BINARY: &str = "
@group(0) @binding(160) var<storage, read_write> values: array<u32>;
struct Range { first: u32, count: u32 }
@group(0) @binding(0) var<uniform> range: Range;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= range.count { return; }
    let i = range.first + id.x * 2u;
    values[i] = select(0u, 1u, (values[i] | values[i + 1u]) != 0u);
    values[i + 1u] = 0u;
}
";

/// Zeroes the count of each argument record the predicate skips.
const PREDICATE: &str = "
@group(0) @binding(32) var<storage, read> predicate: array<u32>;
@group(0) @binding(160) var<storage, read_write> records: array<u32>;
struct Test { word: u32, skip_if_zero: u32, count: u32 }
@group(0) @binding(0) var<uniform> test: Test;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= test.count { return; }
    let zero = (predicate[test.word] | predicate[test.word + 1u]) == 0u;
    if zero == (test.skip_if_zero != 0u) { records[id.x * 5u] = 0u; }
}
";

/// A query heap: its interface query set and `D3D12_QUERY_HEAP_TYPE`.
pub(super) struct QueryHeap<B: Backend> {
    set: B::QuerySet,
    kind: u32,
    count: u32,
}

/// The predicate set by `SetPredication` while a list replays.
pub(super) struct Predicate<B: Backend> {
    resource: ResourceId,
    offset: u64,
    /// `D3D12_PREDICATION_OP`: 0 skips when the value is zero, 1 when it is not.
    op: u32,
    /// The argument records of the draws and dispatches it covers, in order, and the next one.
    records: Option<B::Buffer>,
    next: u64,
    /// The value, once read back for an operation that has no indirect form.
    value: Option<u64>,
}

/// The front's own compute programs, made at first use.
pub(super) struct Programs<B: Backend> {
    binary: Option<B::Program>,
    predicate: Option<B::Program>,
}

impl<B: Backend> Default for Programs<B> {
    fn default() -> Self {
        Programs {
            binary: None,
            predicate: None,
        }
    }
}

impl<B: Backend> Device<B> {
    /// `CreateQueryHeap`.
    pub fn create_query_heap(&mut self, kind: u32, count: u32) -> Result<QueryHeapId, Error> {
        self.live()?;
        let set = match kind {
            HEAP_OCCLUSION => self
                .backend
                .create_query_set(QueryKind::OcclusionAny, count)?,
            HEAP_TIMESTAMP if self.backend.caps().has(Features::TIMESTAMPS) => {
                self.backend.create_query_set(QueryKind::Timestamp, count)?
            }
            HEAP_TIMESTAMP => return Err(Error::NotImplemented("timestamps on this back end")),
            _ => {
                return Err(Error::NotImplemented(
                    "pipeline and stream output statistics queries",
                ));
            }
        };
        Ok(QueryHeapId(
            self.query_heaps.insert(QueryHeap { set, kind, count }) as u32,
        ))
    }

    pub fn destroy_query_heap(&mut self, id: QueryHeapId) {
        if let Some(h) = self.query_heaps.try_remove(id.0 as usize) {
            self.backend.destroy(Object::QuerySet(h.set));
        }
    }

    /// The front's compute program `source`, made once.
    fn own_program(&mut self, predicate: bool) -> Result<B::Program, Error> {
        let slot = if predicate {
            &self.programs.predicate
        } else {
            &self.programs.binary
        };
        if let Some(p) = slot {
            return Ok(p.clone());
        }
        let source = if predicate { PREDICATE } else { BINARY };
        let module = ShaderModule::from_wgsl(source, "main", ShaderStage::Compute)?;
        let shader = self.backend.create_shader(&module)?;
        let program = self.backend.create_program(&Program::Compute(&shader));
        self.backend.destroy(Object::Shader(shader));
        let program = program?;
        if predicate {
            self.programs.predicate = Some(program.clone());
        } else {
            self.programs.binary = Some(program.clone());
        }
        Ok(program)
    }

    /// Runs one of the front's programs over `groups` workgroups with a uniform block and storage
    /// buffers at their binding numbers.
    fn run_own(
        &mut self,
        predicate: bool,
        uniform: &[u32],
        storage: &[(u32, &B::Buffer, u64, bool)],
        threads: u32,
    ) -> Result<(), Error> {
        let program = self.own_program(predicate)?;
        let bytes: Vec<u8> = uniform.iter().flat_map(|v| v.to_le_bytes()).collect();
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offset = [0];
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &[&bytes], align, &mut offset)?;
        let mut entries = vec![BindingEntry {
            binding: 0,
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ring,
                offset: offset[0],
                size: bytes.len().next_multiple_of(16) as u64,
            }),
        }];
        for (binding, buffer, size, writable) in storage {
            entries.push(BindingEntry {
                binding: *binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: *buffer,
                        offset: 0,
                        size: *size,
                    },
                    writable: *writable,
                },
            });
        }
        let set = self.backend.create_binding_set(&entries)?;
        let mut bindings: [Option<&B::BindingSet>; wcontainers_gpu::MAX_BINDING_SETS] =
            Default::default();
        bindings[0] = Some(&set);
        self.backend.dispatch(&DispatchCall {
            program: &program,
            bindings,
            immediates: &[],
            groups: DispatchGroups::Direct([threads.div_ceil(64), 1, 1]),
        })?;
        self.backend.destroy(Object::BindingSet(set));
        Ok(())
    }

    /// A buffer of the front's own for the length of one replay.
    fn scratch(&mut self, size: u64, usage: BufferUsage) -> Result<B::Buffer, Error> {
        let buffer = self.backend.create_buffer(&BufferDesc {
            size: size.next_multiple_of(4),
            usage,
            intent: MapIntent::Rarely,
        })?;
        Ok(buffer)
    }

    /// `ResolveQueryData`, outside a pass: the answers go into a buffer of the front's, binary
    /// occlusion answers become 0 or 1 there, and they are copied into the destination.
    pub(super) fn resolve_query_data(
        &mut self,
        heap: QueryHeapId,
        kind: u32,
        first: u32,
        count: u32,
        dst: ResourceId,
        dst_offset: u64,
    ) -> Result<(), Error> {
        let h = self
            .query_heaps
            .get(heap.0 as usize)
            .ok_or(Error::Invalid("no such query heap"))?;
        if first.checked_add(count).is_none_or(|end| end > h.count) {
            return Err(Error::Invalid("resolving queries past the heap's end"));
        }
        match (h.kind, kind) {
            (HEAP_OCCLUSION, QUERY_BINARY_OCCLUSION) | (HEAP_TIMESTAMP, QUERY_TIMESTAMP) => {}
            (HEAP_OCCLUSION, QUERY_OCCLUSION) => {
                return Err(Error::NotImplemented("counted occlusion queries"));
            }
            _ => return Err(Error::Invalid("a query type its heap does not hold")),
        }
        if count == 0 {
            return Ok(());
        }
        let set = h.set.clone();
        let size = u64::from(count) * 8;
        let scratch = self.scratch(
            size,
            BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        )?;
        self.backend
            .resolve_queries(&set, first, count, &scratch, 0)?;
        if kind == QUERY_BINARY_OCCLUSION {
            self.run_own(false, &[0, count], &[(160, &scratch, size, true)], count)?;
        }
        let (target, heap_type) = self.buffer(dst)?;
        let target = target.clone();
        self.backend
            .copy_buffer(&scratch, 0, &target, dst_offset, size);
        self.backend.destroy(Object::Buffer(scratch));
        if heap_type == HeapType::Readback {
            self.read_into(dst, dst_offset, size)?;
        }
        Ok(())
    }

    /// The query set of an occlusion heap, for a pass to count in.
    pub(super) fn occlusion_set(&self, heap: QueryHeapId) -> Result<B::QuerySet, Error> {
        match self.query_heaps.get(heap.0 as usize) {
            Some(h) if h.kind == HEAP_OCCLUSION => Ok(h.set.clone()),
            _ => Err(Error::Invalid(
                "an occlusion query in a heap of another kind",
            )),
        }
    }

    /// `EndQuery` of a timestamp.
    pub(super) fn timestamp(&mut self, heap: QueryHeapId, index: u32) -> Result<(), Error> {
        match self.query_heaps.get(heap.0 as usize) {
            Some(h) if h.kind == HEAP_TIMESTAMP && index < h.count => {
                let set = h.set.clone();
                self.backend.write_timestamp(&set, index)?;
                Ok(())
            }
            _ => Err(Error::Invalid("a timestamp in a heap of another kind")),
        }
    }

    /// `SetPredication(resource, offset, op)` before the commands in `covered`: the argument
    /// records of the draws and dispatches among them, with their counts zeroed on the GPU where
    /// the predicate skips them.
    pub(super) fn predicate(
        &mut self,
        resource: ResourceId,
        offset: u64,
        op: u32,
        covered: &[Command],
    ) -> Result<Predicate<B>, Error> {
        if op > 1 || !offset.is_multiple_of(8) {
            return Err(Error::Invalid("a predicate's operation or offset"));
        }
        let (buffer, _) = self.buffer(resource)?;
        let buffer = buffer.clone();
        let width = self.resources[resource.0 as usize].desc.width;
        if offset + 8 > width {
            return Err(Error::Invalid("a predicate past its buffer's end"));
        }
        let mut words: Vec<u32> = Vec::new();
        for c in covered {
            match *c {
                Command::Draw {
                    vertices,
                    instances,
                    first_vertex,
                    first_instance,
                } => words.extend([vertices, instances, first_vertex, first_instance, 0]),
                Command::DrawIndexed {
                    indices,
                    instances,
                    first_index,
                    base_vertex,
                    first_instance,
                } => words.extend([
                    indices,
                    instances,
                    first_index,
                    base_vertex as u32,
                    first_instance,
                ]),
                Command::Dispatch(g) => words.extend([g[0], g[1], g[2], 0, 0]),
                _ => {}
            }
        }
        let mut predicate = Predicate {
            resource,
            offset,
            op,
            records: None,
            next: 0,
            value: None,
        };
        if words.is_empty() {
            return Ok(predicate);
        }
        let caps = self.backend.caps();
        if !caps.has(Features::COMPUTE | Features::INDIRECT) {
            return Ok(predicate);
        }
        self.backend.slow_paths().hit(SlowPath::PredicationIndirect);
        self.upload(resource);
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let records = self.scratch(
            bytes.len() as u64,
            BufferUsage::STORAGE | BufferUsage::INDIRECT | BufferUsage::COPY_DST,
        )?;
        self.backend
            .write_buffer(&records, 0, &bytes, WriteMode::Ordered);
        let count = (words.len() / 5) as u32;
        self.run_own(
            true,
            &[(offset / 4) as u32, u32::from(op == 0), count],
            &[
                (32, &buffer, width.next_multiple_of(4), false),
                (160, &records, bytes.len() as u64, true),
            ],
            count,
        )?;
        predicate.records = Some(records);
        Ok(predicate)
    }

    /// Whether the predicate skips an operation that has no indirect form, reading its value back
    /// (`PredicationWait`), once per `SetPredication`.
    pub(super) fn skips(&mut self, p: &mut Predicate<B>) -> Result<bool, Error> {
        let value = match p.value {
            Some(v) => v,
            None => {
                self.backend.slow_paths().hit(SlowPath::PredicationWait);
                self.upload(p.resource);
                let (buffer, _) = self.buffer(p.resource)?;
                let buffer = buffer.clone();
                let scratch = self.scratch(8, BufferUsage::COPY_DST | BufferUsage::READ_BACK)?;
                self.backend.copy_buffer(&buffer, p.offset, &scratch, 0, 8);
                let mut bytes = [0u8; 8];
                self.backend.read_back_now(
                    &ReadSource::Buffer {
                        buffer: &scratch,
                        offset: 0,
                        size: 8,
                    },
                    &mut bytes,
                )?;
                self.backend.destroy(Object::Buffer(scratch));
                let v = u64::from_le_bytes(bytes);
                p.value = Some(v);
                v
            }
        };
        Ok((value == 0) == (p.op == 0))
    }

    /// The next argument record of a predicated draw or dispatch: its buffer and offset.
    pub(super) fn next_record(p: &mut Predicate<B>) -> Option<(B::Buffer, u64)> {
        let records = p.records.clone()?;
        let at = p.next * RECORD;
        p.next += 1;
        Some((records, at))
    }

    /// Releases a predicate's records once the work using them is recorded.
    pub(super) fn drop_predicate(&mut self, p: Predicate<B>) {
        if let Some(r) = p.records {
            self.backend.destroy(Object::Buffer(r));
        }
    }
}
