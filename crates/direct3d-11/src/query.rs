//! Asynchronous queries. An event is done when the fence of a submit after its end is done. An
//! occlusion predicate counts in stretches: the back end runs one occlusion query at a time inside a
//! pass, so every begin and end of the program's closes the open stretch, whose result belongs to
//! every predicate running during it (`QueriesSegmented` where others run on), and a predicate's
//! answer is whether any of its stretches saw a sample pass. Counted occlusion queries count
//! exactly through the interface's occlusion counter, likewise in stretches between the program's
//! begins and ends, each stretch's count written into a buffer read back once the GPU has it.
//! Timestamps, which the back ends do not write, are answered as Direct3D answers
//! a timestamp counter that was not continuous: the disjoint query says so, and programs drop the
//! stamps it covers. Pipeline statistics are counted by the front from the work it issues (plan
//! 4.12): input assembler, vertex shader and clipper counts from the draws' arguments, as a GPU
//! without vertex reuse or clipping splits counts them, compute invocations from the dispatches,
//! and pixel shader invocations by a second occlusion counter replaying the draws with the depth
//! and stencil tests off through a program counting one per pixel (one per sample where the pixel
//! shader runs per sample). Stream output statistics and overflow predicates add up what the stage
//! helper counted per geometry stream at each streaming draw.

use std::cell::RefCell;
use std::rc::Rc;

use wcontainers_abi::NANOSECONDS_PER_SECOND;
use wcontainers_directx_graphics_infrastructure::d3d::{
    PipelineStatistics, StreamOutputStatistics,
};
use wcontainers_gpu::{
    Gpu, BufferDesc, BufferUsage, MapIntent, Object, QueryKind, ReadSource, ReadState, SlowPath,
};
use wcontainers_shaders::stage::STREAMS;

use crate::device::Device;
use crate::error::{Error, Result, WARNING_PREFIX};
use crate::object::Kept;

/// `D3D11_QUERY`.
pub mod kind {
    pub const EVENT: u32 = 0;
    pub const OCCLUSION: u32 = 1;
    pub const TIMESTAMP: u32 = 2;
    pub const TIMESTAMP_DISJOINT: u32 = 3;
    pub const PIPELINE_STATISTICS: u32 = 4;
    pub const OCCLUSION_PREDICATE: u32 = 5;
    pub const SO_STATISTICS: u32 = 6;
    pub const SO_OVERFLOW_PREDICATE: u32 = 7;
    pub const SO_STATISTICS_STREAM0: u32 = 8;
    pub const SO_STATISTICS_STREAM3: u32 = 14;
    pub const SO_OVERFLOW_PREDICATE_STREAM3: u32 = 15;
}

/// What `GetData` gives, as the words of its bytes: the largest answer is the pipeline
/// statistics'.
pub type Answer = [u64; ANSWER_WORDS];
const ANSWER_WORDS: usize = size_of::<PipelineStatistics>() / size_of::<u64>();

/// The words of a short answer.
fn short(words: &[u64]) -> Answer {
    let mut a = [0; ANSWER_WORDS];
    a[..words.len()].copy_from_slice(words);
    a
}

/// `D3D11_QUERY_DATA_PIPELINE_STATISTICS`'s members, by word.
pub mod statistic {
    use std::mem::offset_of;

    use wcontainers_directx_graphics_infrastructure::d3d::PipelineStatistics;

    /// The bytes of one member: every member is a u64 count.
    const STATISTIC_BYTES: usize = size_of::<u64>();
    pub const IA_VERTICES: usize = offset_of!(PipelineStatistics, ia_vertices) / STATISTIC_BYTES;
    pub const IA_PRIMITIVES: usize =
        offset_of!(PipelineStatistics, ia_primitives) / STATISTIC_BYTES;
    pub const VS_INVOCATIONS: usize =
        offset_of!(PipelineStatistics, vs_invocations) / STATISTIC_BYTES;
    pub const C_INVOCATIONS: usize =
        offset_of!(PipelineStatistics, c_invocations) / STATISTIC_BYTES;
    pub const C_PRIMITIVES: usize = offset_of!(PipelineStatistics, c_primitives) / STATISTIC_BYTES;
    pub const PS_INVOCATIONS: usize =
        offset_of!(PipelineStatistics, ps_invocations) / STATISTIC_BYTES;
    pub const CS_INVOCATIONS: usize =
        offset_of!(PipelineStatistics, cs_invocations) / STATISTIC_BYTES;
}

/// Slots of one occlusion set: WebGPU's most.
const SET_SLOTS: u32 = 4096;

/// The bytes of a counted stretch's sum: the occlusion counter writes a u64.
const SUM_BYTES: u64 = size_of::<u64>() as u64;

/// Counted stretches' sums one buffer holds, so a frame of many stretches makes few buffers: as
/// many as an occlusion set's slots.
const SUMS_PER_BUFFER: u64 = SET_SLOTS as u64;

/// The bytes `GetData` writes for a query of `kind`, or none for a kind Direct3D 11 does not have.
pub fn data_size(kind: u32) -> Option<u32> {
    const BOOL_BYTES: usize = size_of::<i32>();
    const WORD_BYTES: usize = size_of::<u64>();
    let bytes = match kind {
        kind::EVENT | kind::OCCLUSION_PREDICATE => BOOL_BYTES,
        kind::OCCLUSION | kind::TIMESTAMP => WORD_BYTES,
        kind::TIMESTAMP_DISJOINT => 2 * WORD_BYTES,
        kind::PIPELINE_STATISTICS => size_of::<Answer>(),
        kind::SO_STATISTICS..=kind::SO_STATISTICS_STREAM3
            if (kind - kind::SO_STATISTICS).is_multiple_of(2) =>
        {
            size_of::<StreamOutputStatistics>()
        }
        kind::SO_OVERFLOW_PREDICATE..=kind::SO_OVERFLOW_PREDICATE_STREAM3 => BOOL_BYTES,
        _ => return None,
    };
    Some(bytes as u32)
}

/// Whether `kind` is a stream output statistics query or overflow predicate.
fn is_stream_output(kind: u32) -> bool {
    (kind::SO_STATISTICS..=kind::SO_OVERFLOW_PREDICATE_STREAM3).contains(&kind)
}

/// The geometry stream a stream output query of `kind` counts, or `None` for the predicate of
/// every stream (`SO_OVERFLOW_PREDICATE`); `SO_STATISTICS` is stream 0's.
fn stream_of(kind: u32) -> Option<usize> {
    match kind {
        kind::SO_STATISTICS => Some(0),
        kind::SO_OVERFLOW_PREDICATE => None,
        k => Some(((k - kind::SO_STATISTICS_STREAM0) / 2) as usize),
    }
}

/// Primitives written and needed, per geometry stream.
pub(crate) type Streamed = [[u64; 2]; STREAMS];

impl<B: Gpu> Queries<B> {
    /// Whether any query counting draws runs.
    pub(crate) fn running_any(&self) -> bool {
        !(self.running.is_empty()
            && self.counted.is_empty()
            && self.statistics.is_empty()
            && self.streams.is_empty())
    }

    /// Whether a stream output query runs.
    pub(crate) fn streams_running(&self) -> bool {
        !self.streams.is_empty()
    }

    /// Adds a draw's primitives written and needed per stream to the running stream output queries.
    pub(crate) fn add_streamed(&mut self, counts: &Streamed) {
        for q in &self.streams {
            let mut sums = q.streamed.get();
            for (sum, count) in sums.iter_mut().zip(counts) {
                sum[0] += count[0];
                sum[1] += count[1];
            }
            q.streamed.set(sums);
        }
    }
}

/// Whether `kind` answers from the work's end alone, with no stretch of its own: events,
/// timestamps, the disjoint query and stream output's queries.
fn answered_at_end(kind: u32) -> bool {
    matches!(
        kind,
        kind::EVENT | kind::TIMESTAMP | kind::TIMESTAMP_DISJOINT
    ) || is_stream_output(kind)
}

/// An occlusion set, shared by the stretches counted into it.
type Set<B> = Rc<Kept<B, <B as Gpu>::QuerySet>>;

/// A buffer of counted stretches' sums, shared by the stretches written into it.
type Sums<B> = Rc<Kept<B, <B as Gpu>::Buffer>>;

/// A stretch of an occlusion query: a slot of a set (predicates), or the place in a buffer the
/// occlusion counter wrote a counted query's stretch into.
enum Part<B: Gpu> {
    Slot { set: Set<B>, slot: u32 },
    Sum { sums: Sums<B>, offset: u64 },
}

enum State<B: Gpu> {
    /// Never ended.
    Fresh,
    /// An occlusion query begun and not yet ended: the stretches it ran in so far.
    Running(Vec<Part<B>>),
    /// A pipeline statistics query begun and not yet ended: what it counted so far.
    Statistics(Answer),
    /// Ended after `submits` submits; its stretches and, once asked for, their read backs.
    Ended {
        submits: u64,
        parts: Vec<Part<B>>,
        reads: Option<Vec<B::Readback>>,
    },
    /// The answer: the bytes `GetData` gives, as words.
    Done(Answer),
}

pub struct Query<B: Gpu> {
    /// `D3D11_QUERY_DESC`'s query and misc flags.
    pub kind: u32,
    pub misc: u32,
    state: RefCell<State<B>>,
    /// A timestamp's time, in nanoseconds from the device's making.
    stamp: std::cell::Cell<u64>,
    /// A stream output query's primitives written and needed, per stream.
    streamed: std::cell::Cell<Streamed>,
}

/// `D3D11_QUERY_MISC_PREDICATEHINT`.
pub const PREDICATE_HINT: u32 = 1;

/// The timestamps' frequency: nanoseconds, the host's clocks' unit.
const NANOSECONDS: u64 = NANOSECONDS_PER_SECOND;

/// Refused: a query kind Direct3D 11 does not define.
const NOT_A_QUERY: &str = "not a query Direct3D 11 has";
/// Refused: a per-stream stream output query below 11_0.
const STREAM_QUERIES_BELOW_11_0: &str = "per-stream stream output queries need feature level 11_0";

/// The device's occlusion sets and the predicates running.
pub(crate) struct Queries<B: Gpu> {
    /// Occlusion predicates begun and not ended.
    running: Vec<Rc<Query<B>>>,
    /// Counted occlusion queries begun and not ended.
    pub(crate) counted: Vec<Rc<Query<B>>>,
    /// Pipeline statistics queries begun and not ended.
    pub(crate) statistics: Vec<Rc<Query<B>>>,
    /// Stream output queries begun and not ended.
    streams: Vec<Rc<Query<B>>>,
    /// When the device was made: timestamps count nanoseconds from it.
    made: std::time::Instant,
    /// Whether the program made an occlusion predicate: from then on every pass counts into a set,
    /// so a begin never has to split a pass.
    used: bool,
    /// The set passes count into, and its next free slot.
    set: Option<(Set<B>, u32)>,
    /// The buffer counted stretches write their sums into, and its next free offset.
    sums: Option<(Sums<B>, u64)>,
    /// The set the open pass counts into, and the slot of its open stretch.
    pub(crate) pass_set: Option<Set<B>>,
    stretch: Option<u32>,
    /// Submits so far, and the fence of the last.
    submits: u64,
    pub(crate) fence: Option<B::Fence>,
}

impl<B: Gpu> Queries<B> {
    pub fn new() -> Self {
        Queries {
            running: Vec::new(),
            counted: Vec::new(),
            statistics: Vec::new(),
            streams: Vec::new(),
            made: std::time::Instant::now(),
            used: false,
            set: None,
            sums: None,
            pass_set: None,
            stretch: None,
            submits: 0,
            fence: None,
        }
    }
}

impl<B: Gpu> Device<B> {
    pub fn create_query(&mut self, kind: u32, misc: u32) -> Result<Rc<Query<B>>> {
        if data_size(kind).is_none() || misc & !1 != 0 {
            return Err(Error::InvalidArg(NOT_A_QUERY));
        }
        if kind >= kind::SO_STATISTICS_STREAM0 && self.feature_level() < crate::desc::level::L11_0 {
            return Err(Error::InvalidArg(STREAM_QUERIES_BELOW_11_0));
        }
        match kind {
            kind::EVENT | kind::OCCLUSION | kind::TIMESTAMP | kind::TIMESTAMP_DISJOINT => {}
            kind::OCCLUSION_PREDICATE => self.queries.used = true,
            _ => {}
        }
        Ok(Rc::new(Query {
            kind,
            misc,
            state: RefCell::new(State::Fresh),
            stamp: std::cell::Cell::new(0),
            streamed: std::cell::Cell::new(Streamed::default()),
        }))
    }

    /// `Begin`: starts an occlusion query again from nothing; events and timestamps have no begin,
    /// and a disjoint query's begin only marks it running.
    pub fn begin_query(&mut self, q: &Rc<Query<B>>) {
        match q.kind {
            kind::EVENT | kind::TIMESTAMP => return,
            k if answered_at_end(k) => {
                *q.state.borrow_mut() = State::Running(Vec::new());
                q.streamed.set(Streamed::default());
                if is_stream_output(k) && !self.queries.streams.iter().any(|r| Rc::ptr_eq(r, q)) {
                    self.queries.streams.push(q.clone());
                }
                return;
            }
            kind::OCCLUSION => {
                if let Err(e) = self.count_stretch() {
                    self.lose(e);
                    return;
                }
                if !self.queries.counted.iter().any(|r| Rc::ptr_eq(r, q)) {
                    self.queries.counted.push(q.clone());
                }
                *q.state.borrow_mut() = State::Running(Vec::new());
                let r = self.counter.begin(&mut self.backend, &mut self.passes);
                let _ = self.gpu(r);
                return;
            }
            kind::PIPELINE_STATISTICS => {
                if let Err(e) = self.count_pixel_stretch() {
                    self.lose(e);
                    return;
                }
                if !self.queries.statistics.iter().any(|r| Rc::ptr_eq(r, q)) {
                    self.queries.statistics.push(q.clone());
                }
                *q.state.borrow_mut() = State::Statistics([0; ANSWER_WORDS]);
                let r = self
                    .pixel_counter
                    .begin(&mut self.backend, &mut self.passes);
                let _ = self.gpu(r);
                return;
            }
            _ => {}
        }
        self.close_stretch(true);
        if !self.queries.running.iter().any(|r| Rc::ptr_eq(r, q)) {
            self.queries.running.push(q.clone());
        }
        *q.state.borrow_mut() = State::Running(Vec::new());
    }

    /// `End`: an event or timestamp marks the work recorded so far; an occlusion query stops
    /// counting (one never begun ends with nothing counted).
    pub fn end_query(&mut self, q: &Rc<Query<B>>) {
        if q.kind == kind::OCCLUSION {
            let was = self.queries.counted.iter().any(|r| Rc::ptr_eq(r, q));
            if was && let Err(e) = self.count_stretch() {
                self.lose(e);
            }
            self.queries.counted.retain(|r| !Rc::ptr_eq(r, q));
            let parts = match std::mem::replace(&mut *q.state.borrow_mut(), State::Fresh) {
                State::Running(parts) => parts,
                _ => Vec::new(),
            };
            // The count is read back from here, so it goes to the GPU with the next submit.
            let reads = match self.start_reads(&parts) {
                Ok(reads) => reads,
                Err(e) => {
                    self.lose(e);
                    Vec::new()
                }
            };
            *q.state.borrow_mut() = State::Ended {
                submits: self.queries.submits,
                parts,
                reads: Some(reads),
            };
            if was && !self.queries.counted.is_empty() {
                let r = self.counter.begin(&mut self.backend, &mut self.passes);
                let _ = self.gpu(r);
            }
            return;
        }
        if q.kind == kind::PIPELINE_STATISTICS {
            let was = self.queries.statistics.iter().any(|r| Rc::ptr_eq(r, q));
            if was && let Err(e) = self.count_pixel_stretch() {
                self.lose(e);
            }
            self.queries.statistics.retain(|r| !Rc::ptr_eq(r, q));
            let counts = match *q.state.borrow() {
                State::Statistics(c) => c,
                _ => [0; ANSWER_WORDS],
            };
            *q.state.borrow_mut() = State::Done(counts);
            if was && !self.queries.statistics.is_empty() {
                let r = self
                    .pixel_counter
                    .begin(&mut self.backend, &mut self.passes);
                let _ = self.gpu(r);
            }
            return;
        }
        if q.kind == kind::TIMESTAMP {
            // The time of the end, answered once the work before it is done.
            q.stamp.set(self.queries.made.elapsed().as_nanos() as u64);
        }
        let parts = if answered_at_end(q.kind) {
            self.queries.streams.retain(|r| !Rc::ptr_eq(r, q));
            Vec::new()
        } else {
            self.close_stretch(true);
            self.queries.running.retain(|r| !Rc::ptr_eq(r, q));
            match std::mem::replace(&mut *q.state.borrow_mut(), State::Fresh) {
                State::Running(parts) => parts,
                _ => Vec::new(),
            }
        };
        *q.state.borrow_mut() = State::Ended {
            submits: self.queries.submits,
            parts,
            reads: None,
        };
    }

    /// `GetData`: the answer once the GPU has it, as the words of its bytes, `None` before.
    /// `flush` submits what the answer waits on (Direct3D does unless
    /// `D3D11_ASYNC_GETDATA_DONOTFLUSH`).
    pub fn query_data(&mut self, q: &Rc<Query<B>>, flush: bool) -> Result<Option<Answer>> {
        self.live()?;
        let submits = match &*q.state.borrow() {
            State::Fresh | State::Running(_) | State::Statistics(_) => {
                return Err(Error::InvalidCall("a query not ended"));
            }
            State::Done(v) => return Ok(Some(*v)),
            State::Ended { submits, .. } => *submits,
        };
        if answered_at_end(q.kind) {
            if self.queries.submits == submits {
                if !flush {
                    return Ok(None);
                }
                self.flush();
            }
            let fence = self.queries.fence.clone().expect("a submit's fence");
            if !self.backend.fence_done(&fence) {
                return Ok(None);
            }
            let answer = match q.kind {
                kind::TIMESTAMP => short(&[q.stamp.get()]),
                kind::TIMESTAMP_DISJOINT => {
                    static WARNED: std::sync::Once = std::sync::Once::new();
                    WARNED.call_once(|| {
                        eprintln!(
                            "{WARNING_PREFIX}: timestamps reported disjoint: the back end writes none"
                        )
                    });
                    short(&[NANOSECONDS, 1])
                }
                kind::EVENT => short(&[1]),
                k if is_stream_output(k) && (k - kind::SO_STATISTICS).is_multiple_of(2) => {
                    short(&q.streamed.get()[stream_of(k).expect("a stream's statistics")])
                }
                k if is_stream_output(k) => {
                    let sums = q.streamed.get();
                    let overflowed = match stream_of(k) {
                        Some(s) => sums[s][1] > sums[s][0],
                        None => sums.iter().any(|[written, needed]| needed > written),
                    };
                    short(&[u64::from(overflowed)])
                }
                _ => short(&[]),
            };
            *q.state.borrow_mut() = State::Done(answer);
            return Ok(Some(answer));
        }
        let started = matches!(&*q.state.borrow(), State::Ended { reads: Some(_), .. });
        if !started {
            // Results are resolved outside a pass.
            if self.ctx_pass_open() && !flush {
                return Ok(None);
            }
            self.settle();
            let reads = match &*q.state.borrow() {
                State::Ended { parts, .. } => self.start_reads(parts)?,
                _ => Vec::new(),
            };
            if let State::Ended {
                reads: r, submits, ..
            } = &mut *q.state.borrow_mut()
            {
                *r = Some(reads);
                *submits = self.queries.submits;
            }
        }
        let (reads, read_at) = match &mut *q.state.borrow_mut() {
            State::Ended { reads, submits, .. } => (reads.take().unwrap_or_default(), *submits),
            _ => (Vec::new(), self.queries.submits),
        };
        if reads
            .iter()
            .any(|r| self.backend.read_state(r) == ReadState::Pending)
        {
            if let State::Ended { reads: r, .. } = &mut *q.state.borrow_mut() {
                *r = Some(reads);
            }
            // Reads started by an earlier call that did not flush are sent now.
            if flush && read_at == self.queries.submits {
                self.flush();
            }
            return Ok(None);
        }
        // A counted query sums its stretches; a predicate is whether any saw a sample pass.
        let mut answer = 0;
        for r in reads {
            let mut bytes = [0u8; size_of::<u64>()];
            self.backend.finish_read(r, &mut bytes);
            let n = u64::from_le_bytes(bytes);
            answer = match q.kind {
                kind::OCCLUSION => answer + n,
                _ => answer | u64::from(n != 0),
            };
        }
        *q.state.borrow_mut() = State::Done(short(&[answer]));
        Ok(Some(short(&[answer])))
    }

    /// The answer of an ended query, waiting for the GPU where it is not there yet
    /// (`PredicationWait`); none for a query not ended.
    pub(crate) fn wait_answer(&mut self, q: &Rc<Query<B>>) -> Result<Option<Answer>> {
        let mut waited = false;
        loop {
            match self.query_data(q, true) {
                Ok(Some(v)) => return Ok(Some(v)),
                Ok(None) => {}
                Err(Error::InvalidCall(_)) => return Ok(None),
                Err(e) => return Err(e),
            }
            if !waited {
                waited = true;
                self.backend.slow_paths().hit(SlowPath::PredicationWait);
            }
            match &*q.state.borrow() {
                State::Ended {
                    reads: Some(reads), ..
                } => {
                    for r in reads {
                        self.backend.wait_read(r);
                    }
                }
                _ => {
                    if let Some(f) = self.queries.fence.clone() {
                        self.backend.wait_fence(&f);
                    }
                }
            }
        }
    }

    /// Starts reading back each stretch's result, outside a pass.
    fn start_reads(&mut self, parts: &[Part<B>]) -> Result<Vec<B::Readback>> {
        let mut reads = Vec::with_capacity(parts.len());
        for p in parts {
            let r = match p {
                Part::Slot { set, slot } => self.backend.read_queries(&set.handle, *slot, 1),
                Part::Sum { sums, offset } => self.backend.read_back(&ReadSource::Buffer {
                    buffer: &sums.handle,
                    offset: *offset,
                    size: SUM_BYTES,
                }),
            };
            reads.push(self.gpu(r)?);
        }
        Ok(reads)
    }

    /// Ends the occlusion counter's stretch, if counted queries run, writing its count into a
    /// buffer each running query reads back at its end.
    fn count_stretch(&mut self) -> Result<()> {
        if self.queries.counted.is_empty() {
            return Ok(());
        }
        self.end_pass();
        let (sums, offset) = match self.queries.sums.take() {
            Some((sums, offset)) if offset < SUMS_PER_BUFFER * SUM_BYTES => (sums, offset),
            _ => {
                let made = self.backend.create_buffer(&BufferDesc {
                    size: SUMS_PER_BUFFER * SUM_BYTES,
                    usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                    intent: MapIntent::Rarely,
                });
                let made = self.gpu(made)?;
                let id = self.id();
                (
                    Rc::new(Kept::new(made, id, Object::Buffer, &self.graveyard)),
                    0,
                )
            }
        };
        let r = self
            .counter
            .end_into(&mut self.backend, &mut self.passes, &sums.handle, offset);
        self.gpu(r)?;
        for q in &self.queries.counted {
            if let State::Running(parts) = &mut *q.state.borrow_mut() {
                parts.push(Part::Sum {
                    sums: sums.clone(),
                    offset,
                });
            }
        }
        self.queries.sums = Some((sums, offset + SUM_BYTES));
        Ok(())
    }

    /// Ends the pixel counter's stretch, if pipeline statistics queries run, adding its count to
    /// each one's pixel shader invocations.
    fn count_pixel_stretch(&mut self) -> Result<()> {
        if self.queries.statistics.is_empty() {
            return Ok(());
        }
        self.end_pass();
        let n = self.pixel_counter.end(&mut self.backend, &mut self.passes);
        let n = self.gpu(n)?;
        self.add_statistics(&[(statistic::PS_INVOCATIONS, n)]);
        Ok(())
    }

    /// Adds `counts` (member, count) to every running pipeline statistics query.
    pub(crate) fn add_statistics(&mut self, counts: &[(usize, u64)]) {
        for q in &self.queries.statistics {
            if let State::Statistics(s) = &mut *q.state.borrow_mut() {
                for &(i, n) in counts {
                    s[i] += n;
                }
            }
        }
    }

    /// The occlusion set a pass opening now counts into, if any predicate may run in it.
    pub(crate) fn pass_occlusion(&mut self) -> Result<Option<Set<B>>> {
        if !self.queries.used {
            return Ok(None);
        }
        let fresh = self
            .queries
            .set
            .as_ref()
            .is_none_or(|(_, next)| *next >= SET_SLOTS);
        if fresh {
            let set = self
                .backend
                .create_query_set(QueryKind::OcclusionAny, SET_SLOTS);
            let set = self.gpu(set)?;
            let id = self.id();
            let kept = Rc::new(Kept::new(set, id, Object::QuerySet, &self.graveyard));
            self.queries.set = Some((kept, 0));
        }
        let set = self.queries.set.as_ref().map(|(s, _)| s.clone());
        self.queries.pass_set = set.clone();
        Ok(set)
    }

    /// Before a draw in an open pass: opens a stretch if a predicate runs and none is open. A pass
    /// whose set has no slot left, or that began without one, ends, and the draw opens the next on
    /// a fresh set (`QuerySetSplitPass`).
    pub(crate) fn open_stretch(&mut self) -> bool {
        if self.queries.running.is_empty() || self.queries.stretch.is_some() {
            return true;
        }
        if let (Some(pass_set), Some((set, next))) = (&self.queries.pass_set, &mut self.queries.set)
            && Rc::ptr_eq(set, pass_set)
            && *next < SET_SLOTS
        {
            let slot = *next;
            *next += 1;
            self.backend.begin_query(slot);
            self.queries.stretch = Some(slot);
            return true;
        }
        self.backend.slow_paths().hit(SlowPath::QuerySetSplitPass);
        false
    }

    /// Closes the open stretch, giving it to every predicate running. `segmented` says the program
    /// began or ended a predicate inside it while others run on.
    pub(crate) fn close_stretch(&mut self, segmented: bool) {
        let Some(slot) = self.queries.stretch.take() else {
            return;
        };
        self.backend.end_query();
        if segmented && self.queries.running.len() > 1 {
            self.backend.slow_paths().hit(SlowPath::QueriesSegmented);
        }
        let set = self.queries.pass_set.clone().expect("the open pass's set");
        for q in &self.queries.running {
            if let State::Running(parts) = &mut *q.state.borrow_mut() {
                parts.push(Part::Slot {
                    set: set.clone(),
                    slot,
                });
            }
        }
    }

    /// After a submit: its fence.
    pub(crate) fn submitted(&mut self, fence: B::Fence) {
        self.queries.submits += 1;
        self.queries.fence = Some(fence);
    }
}
