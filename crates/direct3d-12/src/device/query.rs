//! Query heaps, `ResolveQueryData` and `SetPredication` (plan 5.2 and 4.12): binary occlusion as the
//! back end's any-sample queries turned into exactly 0 or 1 after the resolve, the values the front
//! counts itself (exact occlusion, pipeline and stream output statistics) written into a buffer of
//! the heap's as each query ends, and predicated draws and dispatches recorded as indirect ones whose
//! counts a small compute program zeroes when the predicate skips them, so nothing waits.

use wcontainers_gpu::{
    BindingEntry, BindingResource, BufferRange, INDEX_FORMATS, MAX_BINDING_SETS, QueryKind,
    RingKind, SLOT_KINDS, ShaderModule, SlotKind,
};

use super::bind::CONSTANT_REGISTER;
use super::*;
use wcontainers_directx_graphics_infrastructure::d3d::{
    DrawIndexedArguments, PipelineStatistics, StreamOutputStatistics,
};

/// `D3D12_QUERY_HEAP_TYPE` values.
const HEAP_OCCLUSION: u32 = 0;
const HEAP_TIMESTAMP: u32 = 1;
const HEAP_VIDEO_DECODE_STATISTICS: u32 = 4;
const HEAP_COPY_QUEUE_TIMESTAMP: u32 = 5;
const HEAP_PIPELINE_STATISTICS1: u32 = 7;
pub(super) const HEAP_PIPELINE_STATISTICS: u32 = 2;
pub(super) const HEAP_SO_STATISTICS: u32 = 3;
/// `D3D12_QUERY_TYPE` values.
pub const QUERY_OCCLUSION: u32 = 0;
pub const QUERY_BINARY_OCCLUSION: u32 = 1;
pub const QUERY_TIMESTAMP: u32 = 2;
pub const QUERY_PIPELINE_STATISTICS: u32 = 3;
/// `SO_STATISTICS_STREAM0` to `STREAM3`.
pub const QUERY_SO_STATISTICS: std::ops::RangeInclusive<u32> = 4..=7;

/// An exact occlusion count, the value of `D3D12_QUERY_TYPE_OCCLUSION`.
type OcclusionCount = u64;

/// The bytes of each answer `ResolveQueryData` writes of the back end's queries (occlusion and
/// timestamps, both 64-bit).
const ANSWER_BYTES: u64 = size_of::<u64>() as u64;

/// The value `SetPredication` tests.
type PredicateValue = u64;
const PREDICATE_BYTES: u64 = size_of::<PredicateValue>() as u64;

/// The bytes of one query's value the front writes, by heap type.
fn value_size(heap: u32) -> Option<u64> {
    let size = match heap {
        HEAP_OCCLUSION => size_of::<OcclusionCount>(),
        HEAP_PIPELINE_STATISTICS => size_of::<PipelineStatistics>(),
        HEAP_SO_STATISTICS => size_of::<StreamOutputStatistics>(),
        _ => return None,
    };
    Some(size as u64)
}

/// The bytes of one counted field of a query's value.
const FIELD_BYTES: u64 = size_of::<u64>() as u64;

/// One argument record of a predicated draw or dispatch, as large as the largest arguments (an
/// indexed draw's), the count first.
const RECORD: u64 = size_of::<DrawIndexedArguments>() as u64;
const RECORD_WORDS: u64 = RECORD / STORAGE_WORD;

/// The binding of the buffer the front's own programs read: the first texture slot.
pub(super) const OWN_SOURCE: u32 = SLOT_KINDS[SlotKind::Texture as usize].first_binding;
/// The binding of the buffer they write: the first storage slot.
pub(super) const OWN_RESULT: u32 = SLOT_KINDS[SlotKind::Storage as usize].first_binding;
/// The binding of their uniform block of parameters.
pub(super) const OWN_PARAMETERS: u32 = SLOT_KINDS[SlotKind::Uniform as usize].first_binding;
/// The threads of each of their workgroups.
pub(super) const OWN_GROUP: u32 = 64;
/// Their entry point.
pub(super) const OWN_ENTRY: &str = wcontainers_shaders::ENTRY_POINT;

/// Turns resolved any-sample answers into exactly 0 or 1, in place: an answer is two words.
fn binary_source() -> String {
    let words = ANSWER_BYTES / STORAGE_WORD;
    format!(
        "
@group(0) @binding({OWN_RESULT}) var<storage, read_write> values: array<u32>;
struct Range {{ first: u32, count: u32 }}
@group(0) @binding({OWN_PARAMETERS}) var<uniform> range: Range;
@compute @workgroup_size({OWN_GROUP})
fn {OWN_ENTRY}(@builtin(global_invocation_id) id: vec3u) {{
    if id.x >= range.count {{ return; }}
    let i = range.first + id.x * {words}u;
    values[i] = select(0u, 1u, (values[i] | values[i + 1u]) != 0u);
    values[i + 1u] = 0u;
}}
"
    )
}

/// Zeroes the count of each argument record the predicate skips: the predicate is two words.
fn predicate_source() -> String {
    format!(
        "
@group(0) @binding({OWN_SOURCE}) var<storage, read> predicate: array<u32>;
@group(0) @binding({OWN_RESULT}) var<storage, read_write> records: array<u32>;
struct Test {{ word: u32, skip_if_zero: u32, count: u32 }}
@group(0) @binding({OWN_PARAMETERS}) var<uniform> test: Test;
@compute @workgroup_size({OWN_GROUP})
fn {OWN_ENTRY}(@builtin(global_invocation_id) id: vec3u) {{
    if id.x >= test.count {{ return; }}
    let zero = (predicate[test.word] | predicate[test.word + 1u]) == 0u;
    if zero == (test.skip_if_zero != 0u) {{ records[id.x * {RECORD_WORDS}u] = 0u; }}
}}
"
    )
}

/// A query heap: its `D3D12_QUERY_HEAP_TYPE`, the interface query set of binary occlusion and
/// timestamps, and the values the front writes as each query ends.
pub(super) struct QueryHeap<B: Gpu> {
    set: Option<B::QuerySet>,
    kind: u32,
    count: u32,
    values: Option<B::Buffer>,
}

/// The predicate set by `SetPredication` while a list replays.
pub(super) struct Predicate<B: Gpu> {
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

impl<B: Gpu> Predicate<B> {
    /// Whether its value is still to be read back.
    pub(super) fn value_unknown(&self) -> bool {
        self.value.is_none()
    }
}

/// The argument records of the draws and dispatches `commands` make, bundles' included.
fn argument_words(commands: &[Command], words: &mut Vec<u32>) {
    for c in commands {
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
            Command::Bundle(ref inner) => argument_words(inner, words),
            _ => {}
        }
    }
}

/// The front's own compute programs.
#[derive(Clone, Copy)]
pub(super) enum Own {
    Binary,
    Predicate,
    /// Zeroes the commands of `ExecuteIndirect` past its count (`indirect.rs`).
    Expand,
    /// Rewrites a strip's indices to restart at another value (`device.rs`, `draw`).
    Restart,
}

impl Own {
    const ALL: &[Own] = &[Own::Binary, Own::Predicate, Own::Expand, Own::Restart];

    fn source(self) -> String {
        match self {
            Own::Binary => binary_source(),
            Own::Predicate => predicate_source(),
            Own::Expand => super::indirect::expand_source(),
            Own::Restart => restart_source(),
        }
    }
}

/// The front's own compute programs, made at first use.
pub(super) struct Programs<B: Gpu>([Option<B::Program>; Own::ALL.len()]);

/// 32-bit indices of a strip from 16-bit (`wide` 0) or 32-bit ones, `cut` becoming the 32-bit
/// restart value.
fn restart_source() -> String {
    let short = IndexFormat::U16;
    let (bits, short_max) = (
        INDEX_FORMATS[short as usize].size * u8::BITS,
        INDEX_FORMATS[short as usize].restart_value,
    );
    let restart = INDEX_FORMATS[IndexFormat::U32 as usize].restart_value;
    format!(
        "
@group(0) @binding({OWN_SOURCE}) var<storage, read> src: array<u32>;
@group(0) @binding({OWN_RESULT}) var<storage, read_write> dst: array<u32>;
struct Restart {{ first: u32, count: u32, wide: u32, cut: u32 }}
@group(0) @binding({OWN_PARAMETERS}) var<uniform> p: Restart;
@compute @workgroup_size({OWN_GROUP})
fn {OWN_ENTRY}(@builtin(global_invocation_id) id: vec3u) {{
    if id.x >= p.count {{ return; }}
    let i = p.first + id.x;
    var v: u32;
    if p.wide == 0u {{
        v = (src[i >> 1u] >> ((i & 1u) * {bits}u)) & {short_max}u;
    }} else {{
        v = src[i];
    }}
    if p.cut != 0u && v == p.cut {{ v = {restart}u; }}
    dst[id.x] = v;
}}
"
    )
}

impl<B: Gpu> Default for Programs<B> {
    fn default() -> Self {
        Programs(std::array::from_fn(|_| None))
    }
}

impl<B: Gpu> Device<B> {
    /// `CreateQueryHeap`.
    pub fn create_query_heap(&mut self, kind: u32, count: u32) -> Result<QueryHeapId, Error> {
        self.live()?;
        let set = match kind {
            HEAP_OCCLUSION => Some(
                self.backend
                    .create_query_set(QueryKind::OcclusionAny, count)?,
            ),
            // Every queue's work runs in one submission order, so copy queues write timestamps
            // as direct ones do.
            HEAP_TIMESTAMP | HEAP_COPY_QUEUE_TIMESTAMP
                if self.backend.caps().features.contains(Features::TIMESTAMPS) =>
            {
                Some(self.backend.create_query_set(QueryKind::Timestamp, count)?)
            }
            HEAP_TIMESTAMP | HEAP_COPY_QUEUE_TIMESTAMP => {
                return Err(Error::NotImplemented("timestamps on this back end"));
            }
            HEAP_PIPELINE_STATISTICS | HEAP_SO_STATISTICS => None,
            HEAP_VIDEO_DECODE_STATISTICS => {
                return Err(Error::NotImplemented("video decode statistics queries"));
            }
            HEAP_PIPELINE_STATISTICS1 => {
                return Err(Error::NotImplemented("mesh shader pipeline statistics"));
            }
            _ => return Err(Error::Invalid("unknown query heap type")),
        };
        let values = match value_size(kind) {
            Some(size) => match self.backend.create_buffer(&BufferDesc {
                size: u64::from(count.max(1)) * size,
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            }) {
                Ok(b) => Some(b),
                Err(e) => {
                    if let Some(s) = set {
                        self.backend.destroy(Object::QuerySet(s));
                    }
                    return Err(e.into());
                }
            },
            None => None,
        };
        Ok(QueryHeapId(self.query_heaps.insert(QueryHeap {
            set,
            kind,
            count,
            values,
        }) as u32))
    }

    pub fn destroy_query_heap(&mut self, id: QueryHeapId) {
        if let Some(h) = self.query_heaps.try_remove(id.0 as usize) {
            if let Some(s) = h.set {
                self.backend.destroy(Object::QuerySet(s));
            }
            if let Some(v) = h.values {
                self.backend.destroy(Object::Buffer(v));
            }
        }
    }

    /// The type of query heap `heap` if query `index` lies in it.
    pub(super) fn heap_kind(&self, heap: QueryHeapId, index: u32) -> Result<u32, Error> {
        match self.query_heaps.get(heap.0 as usize) {
            Some(h) if index < h.count => Ok(h.kind),
            Some(_) => Err(Error::Invalid("a query past its heap's end")),
            None => Err(Error::Invalid("no such query heap")),
        }
    }

    /// The front's compute program `which`, made once.
    fn own_program(&mut self, which: Own) -> Result<B::Program, Error> {
        if let Some(p) = &self.programs.0[which as usize] {
            return Ok(p.clone());
        }
        let module = ShaderModule::from_wgsl(&which.source(), OWN_ENTRY, ShaderStage::Compute)?;
        let shader = self.backend.create_shader(&module)?;
        let program = self.backend.create_program(&Program::Compute(&shader));
        self.backend.destroy(Object::Shader(shader));
        let program = program?;
        self.programs.0[which as usize] = Some(program.clone());
        Ok(program)
    }

    /// Runs one of the front's programs over `groups` workgroups with a uniform block and storage
    /// buffers at their binding numbers.
    pub(super) fn run_own(
        &mut self,
        which: Own,
        uniform: &[u32],
        storage: &[(u32, &B::Buffer, u64, bool)],
        threads: u32,
    ) -> Result<(), Error> {
        let program = self.own_program(which)?;
        let bytes: Vec<u8> = uniform.iter().flat_map(|v| v.to_le_bytes()).collect();
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offset = [0];
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &[&bytes], align, &mut offset)?;
        let mut entries = vec![BindingEntry {
            binding: OWN_PARAMETERS,
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &ring,
                offset: offset[0],
                size: (bytes.len() as u64).next_multiple_of(CONSTANT_REGISTER),
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
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = Default::default();
        bindings[0] = Some(&set);
        self.backend.dispatch(&DispatchCall {
            program: &program,
            bindings,
            immediates: &[],
            groups: DispatchGroups::Direct([threads.div_ceil(OWN_GROUP), 1, 1]),
        })?;
        self.backend.destroy(Object::BindingSet(set));
        Ok(())
    }

    /// A buffer of the front's own for the length of one replay.
    pub(super) fn scratch(&mut self, size: u64, usage: BufferUsage) -> Result<B::Buffer, Error> {
        let buffer = self.backend.create_buffer(&BufferDesc {
            size: size.next_multiple_of(STORAGE_WORD),
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
        let written = match (h.kind, kind) {
            (HEAP_OCCLUSION, QUERY_BINARY_OCCLUSION)
            | (HEAP_TIMESTAMP | HEAP_COPY_QUEUE_TIMESTAMP, QUERY_TIMESTAMP) => false,
            (HEAP_OCCLUSION, QUERY_OCCLUSION)
            | (HEAP_PIPELINE_STATISTICS, QUERY_PIPELINE_STATISTICS) => true,
            (HEAP_SO_STATISTICS, k) if QUERY_SO_STATISTICS.contains(&k) => true,
            _ => return Err(Error::Invalid("a query type its heap does not hold")),
        };
        if count == 0 {
            return Ok(());
        }
        if written {
            let each = value_size(h.kind).expect("a heap of written values");
            let size = u64::from(count) * each;
            let values = h.values.clone().expect("a heap of written values");
            let (target, heap_type) = self.buffer(dst)?;
            let target = target.clone();
            self.backend
                .copy_buffer(&values, u64::from(first) * each, &target, dst_offset, size);
            if heap_type.reads_back() {
                self.read_into(dst, dst_offset, size)?;
            }
            return Ok(());
        }
        let size = u64::from(count) * ANSWER_BYTES;
        let set = h.set.clone().expect("a heap with a query set");
        let scratch = self.scratch(
            size,
            BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        )?;
        self.backend
            .resolve_queries(&set, first, count, &scratch, 0)?;
        if kind == QUERY_BINARY_OCCLUSION {
            self.run_own(
                Own::Binary,
                &[0, count],
                &[(OWN_RESULT, &scratch, size, true)],
                count,
            )?;
        }
        let (target, heap_type) = self.buffer(dst)?;
        let target = target.clone();
        self.backend
            .copy_buffer(&scratch, 0, &target, dst_offset, size);
        self.backend.destroy(Object::Buffer(scratch));
        if heap_type.reads_back() {
            self.read_into(dst, dst_offset, size)?;
        }
        Ok(())
    }

    /// Writes the value of query `index` of `heap`, which the front counted, in order with the work
    /// recorded around it: the bytes of the heap type's value.
    pub(super) fn write_value(
        &mut self,
        heap: QueryHeapId,
        index: u32,
        value: &[u8],
    ) -> Result<(), Error> {
        let h = self
            .query_heaps
            .get(heap.0 as usize)
            .ok_or(Error::Invalid("no such query heap"))?;
        match (&h.values, value_size(h.kind)) {
            (Some(v), Some(size)) if index < h.count && size == value.len() as u64 => {
                let v = v.clone();
                self.backend
                    .write_buffer(&v, u64::from(index) * size, value, WriteMode::Ordered);
                Ok(())
            }
            _ => Err(Error::Invalid("a query of another heap type")),
        }
    }

    /// Copies the u64 at the start of `from` into the field at byte `field` of the value of query
    /// `index` of `heap`, in order with the work recorded around it.
    pub(super) fn copy_into_value(
        &mut self,
        heap: QueryHeapId,
        index: u32,
        field: usize,
        from: &B::Buffer,
    ) -> Result<(), Error> {
        let (v, at) = self.value_at(heap, index, field)?;
        self.backend.copy_buffer(from, 0, &v, at, FIELD_BYTES);
        Ok(())
    }

    /// The buffer holding the values of `heap` and the offset of the field at byte `field` of query
    /// `index`'s.
    pub(super) fn value_at(
        &self,
        heap: QueryHeapId,
        index: u32,
        field: usize,
    ) -> Result<(B::Buffer, u64), Error> {
        let h = self
            .query_heaps
            .get(heap.0 as usize)
            .ok_or(Error::Invalid("no such query heap"))?;
        match (&h.values, value_size(h.kind)) {
            (Some(v), Some(size)) if index < h.count && field as u64 + FIELD_BYTES <= size => {
                Ok((v.clone(), u64::from(index) * size + field as u64))
            }
            _ => Err(Error::Invalid("a query of another heap type")),
        }
    }

    /// The query set of an occlusion heap, for a pass to count in.
    pub(super) fn occlusion_set(&self, heap: QueryHeapId) -> Result<B::QuerySet, Error> {
        match self.query_heaps.get(heap.0 as usize) {
            Some(h) if h.kind == HEAP_OCCLUSION => Ok(h.set.clone().expect("an occlusion set")),
            _ => Err(Error::Invalid(
                "an occlusion query in a heap of another kind",
            )),
        }
    }

    /// `EndQuery` of a timestamp.
    pub(super) fn timestamp(&mut self, heap: QueryHeapId, index: u32) -> Result<(), Error> {
        match self.query_heaps.get(heap.0 as usize) {
            Some(h)
                if matches!(h.kind, HEAP_TIMESTAMP | HEAP_COPY_QUEUE_TIMESTAMP)
                    && index < h.count =>
            {
                let set = h.set.clone().expect("a timestamp set");
                self.backend.write_timestamp(&set, index)?;
                Ok(())
            }
            _ => Err(Error::Invalid("a timestamp in a heap of another kind")),
        }
    }

    /// `GetClockCalibration`: a GPU timestamp and the CPU clock `now` read at about one moment.
    /// The work submitted so far is let finish, `now` is read just before a lone timestamp is
    /// submitted, and that timestamp is read back (`ReadBackWait`): the GPU's value is later by
    /// the submission's latency.
    pub fn clock_calibration<T>(&mut self, now: impl FnOnce() -> T) -> Result<(u64, T), Error> {
        self.live()?;
        let set = self.backend.create_query_set(QueryKind::Timestamp, 1)?;
        let idle = self.backend.submit();
        self.backend.wait_fence(&idle);
        let read = self
            .backend
            .write_timestamp(&set, 0)
            .and_then(|()| self.backend.read_queries(&set, 0, 1));
        let read = match read {
            Ok(r) => r,
            Err(e) => {
                self.backend.destroy(Object::QuerySet(set));
                return Err(e.into());
            }
        };
        let cpu = now();
        self.backend.submit();
        if self.backend.read_state(&read) == ReadState::Pending {
            self.backend.record_slow_path(SlowPath::ReadBackWait);
            self.backend.wait_read(&read);
        }
        let mut bytes = [0u8; size_of::<u64>()];
        self.backend.finish_read(read, &mut bytes);
        self.backend.destroy(Object::QuerySet(set));
        Ok((u64::from_le_bytes(bytes), cpu))
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
        if op > 1 || !offset.is_multiple_of(PREDICATE_BYTES) {
            return Err(Error::Invalid("a predicate's operation or offset"));
        }
        let (buffer, _) = self.buffer(resource)?;
        let buffer = buffer.clone();
        let width = self.resources[resource.0 as usize].desc.width;
        if offset + PREDICATE_BYTES > width {
            return Err(Error::Invalid("a predicate past its buffer's end"));
        }
        let mut words: Vec<u32> = Vec::new();
        argument_words(covered, &mut words);
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
        if !caps
            .features
            .contains(Features::COMPUTE | Features::INDIRECT)
        {
            return Ok(predicate);
        }
        self.backend.record_slow_path(SlowPath::PredicationIndirect);
        self.upload(resource)?;
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let records = self.scratch(
            bytes.len() as u64,
            BufferUsage::STORAGE | BufferUsage::INDIRECT | BufferUsage::COPY_DST,
        )?;
        self.backend
            .write_buffer(&records, 0, &bytes, WriteMode::Ordered);
        let count = (words.len() as u64 / RECORD_WORDS) as u32;
        self.run_own(
            Own::Predicate,
            &[(offset / STORAGE_WORD) as u32, u32::from(op == 0), count],
            &[
                (
                    OWN_SOURCE,
                    &buffer,
                    width.next_multiple_of(STORAGE_WORD),
                    false,
                ),
                (OWN_RESULT, &records, bytes.len() as u64, true),
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
                self.backend.record_slow_path(SlowPath::PredicationWait);
                self.upload(p.resource)?;
                let (buffer, _) = self.buffer(p.resource)?;
                let buffer = buffer.clone();
                let scratch = self.scratch(
                    PREDICATE_BYTES,
                    BufferUsage::COPY_DST | BufferUsage::READ_BACK,
                )?;
                self.backend
                    .copy_buffer(&buffer, p.offset, &scratch, 0, PREDICATE_BYTES);
                let mut bytes = [0u8; size_of::<PredicateValue>()];
                self.backend.read_back_now(
                    &ReadSource::Buffer {
                        buffer: &scratch,
                        offset: 0,
                        size: PREDICATE_BYTES,
                    },
                    &mut bytes,
                )?;
                self.backend.destroy(Object::Buffer(scratch));
                let v = PredicateValue::from_le_bytes(bytes);
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
