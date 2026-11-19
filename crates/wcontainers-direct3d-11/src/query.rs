//! Asynchronous queries. An event is done when the fence of a submit after its end is done. An
//! occlusion predicate counts in stretches: the back end runs one occlusion query at a time inside a
//! pass, so every begin and end of the program's closes the open stretch, whose result belongs to
//! every predicate running during it (`QueriesSegmented` where others run on), and a predicate's
//! answer is whether any of its stretches saw a sample pass. Counted occlusion queries count
//! exactly through the interface's occlusion counter, likewise in stretches between the program's
//! begins and ends. Timestamps, which the back ends do not write, are answered as Direct3D answers
//! a timestamp counter that was not continuous: the disjoint query says so, and programs drop the
//! stamps it covers. Pipeline statistics are counted by the front from the work it issues (plan
//! 4.12): input assembler, vertex shader and clipper counts from the draws' arguments, as a GPU
//! without vertex reuse or clipping splits counts them, compute invocations from the dispatches,
//! and pixel shader invocations by a second occlusion counter replaying the draws with the depth
//! and stencil tests off through a program counting one per pixel (one per sample where the pixel
//! shader runs per sample). Stream output statistics and overflow predicates of stream 0 add up
//! what the stage helper counted at each streaming draw; only stream 0 is ever written, so the
//! others answer zero and false.

use std::cell::RefCell;
use std::rc::Rc;

use wcontainers_gpu::{Backend, Object, QueryKind, ReadState, SlowPath};

use crate::device::Device;
use crate::error::{Error, Result};
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
/// statistics' eleven.
pub type Answer = [u64; ANSWER_WORDS];
const ANSWER_WORDS: usize = statistic::CS_INVOCATIONS + 1;

/// The words of a short answer.
fn short(words: &[u64]) -> Answer {
    let mut a = [0; ANSWER_WORDS];
    a[..words.len()].copy_from_slice(words);
    a
}

/// `D3D11_QUERY_DATA_PIPELINE_STATISTICS`'s members, by word.
pub mod statistic {
    pub const IA_VERTICES: usize = 0;
    pub const IA_PRIMITIVES: usize = 1;
    pub const VS_INVOCATIONS: usize = 2;
    pub const C_INVOCATIONS: usize = 5;
    pub const C_PRIMITIVES: usize = 6;
    pub const PS_INVOCATIONS: usize = 7;
    pub const CS_INVOCATIONS: usize = 10;
}

/// Slots of one occlusion set: WebGPU's most.
const SET_SLOTS: u32 = 4096;

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
            2 * WORD_BYTES
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

/// Whether `kind` is a stream output query of stream 0 (or of all streams).
fn of_stream_zero(kind: u32) -> bool {
    (kind::SO_STATISTICS..=kind::SO_STATISTICS_STREAM0 + 1).contains(&kind)
}

impl<B: Backend> Queries<B> {
    /// Whether any query counting draws runs.
    pub(crate) fn running_any(&self) -> bool {
        !(self.running.is_empty()
            && self.counted.is_empty()
            && self.statistics.is_empty()
            && self.streams.is_empty())
    }

    /// Whether a stream output query of stream 0 runs.
    pub(crate) fn streams_running(&self) -> bool {
        !self.streams.is_empty()
    }

    /// Adds a draw's primitives written and needed to the running stream output queries.
    pub(crate) fn add_streamed(&mut self, written: u64, needed: u64) {
        for q in &self.streams {
            let [w, n] = q.streamed.get();
            q.streamed.set([w + written, n + needed]);
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
type Set<B> = Rc<Kept<B, <B as Backend>::QuerySet>>;

/// A stretch of an occlusion predicate: a slot of a set.
struct Part<B: Backend> {
    set: Set<B>,
    slot: u32,
}

enum State<B: Backend> {
    /// Never ended.
    Fresh,
    /// Begun and not yet ended: the stretches it ran in so far.
    Running(Vec<Part<B>>),
    /// A counted occlusion query begun and not yet ended: the samples its stretches counted.
    Counting(u64),
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

pub struct Query<B: Backend> {
    /// `D3D11_QUERY_DESC`'s query and misc flags.
    pub kind: u32,
    pub misc: u32,
    state: RefCell<State<B>>,
    /// A timestamp's time, in nanoseconds from the device's making.
    stamp: std::cell::Cell<u64>,
    /// A stream output query's primitives written and needed.
    streamed: std::cell::Cell<[u64; 2]>,
}

/// `D3D11_QUERY_MISC_PREDICATEHINT`.
pub const PREDICATE_HINT: u32 = 1;

/// The timestamps' frequency: nanoseconds.
const NANOSECONDS: u64 = 1_000_000_000;

/// The device's occlusion sets and the predicates running.
pub(crate) struct Queries<B: Backend> {
    /// Occlusion predicates begun and not ended.
    running: Vec<Rc<Query<B>>>,
    /// Counted occlusion queries begun and not ended.
    pub(crate) counted: Vec<Rc<Query<B>>>,
    /// Pipeline statistics queries begun and not ended.
    pub(crate) statistics: Vec<Rc<Query<B>>>,
    /// Stream output queries of stream 0 begun and not ended.
    streams: Vec<Rc<Query<B>>>,
    /// When the device was made: timestamps count nanoseconds from it.
    made: std::time::Instant,
    /// Whether the program made an occlusion predicate: from then on every pass counts into a set,
    /// so a begin never has to split a pass.
    used: bool,
    /// The set passes count into, and its next free slot.
    set: Option<(Set<B>, u32)>,
    /// The set the open pass counts into, and the slot of its open stretch.
    pub(crate) pass_set: Option<Set<B>>,
    stretch: Option<u32>,
    /// Submits so far, and the fence of the last.
    submits: u64,
    pub(crate) fence: Option<B::Fence>,
}

impl<B: Backend> Queries<B> {
    pub fn new() -> Self {
        Queries {
            running: Vec::new(),
            counted: Vec::new(),
            statistics: Vec::new(),
            streams: Vec::new(),
            made: std::time::Instant::now(),
            used: false,
            set: None,
            pass_set: None,
            stretch: None,
            submits: 0,
            fence: None,
        }
    }
}

impl<B: Backend> Device<B> {
    pub fn create_query(&mut self, kind: u32, misc: u32) -> Result<Rc<Query<B>>> {
        if data_size(kind).is_none() || misc & !1 != 0 {
            return Err(Error::InvalidArg("not a query Direct3D 11 has"));
        }
        if kind >= kind::SO_STATISTICS_STREAM0 && self.feature_level() < crate::desc::level::L11_0 {
            return Err(Error::InvalidArg(
                "per-stream stream output queries need feature level 11_0",
            ));
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
            streamed: std::cell::Cell::new([0; 2]),
        }))
    }

    /// `Begin`: starts an occlusion query again from nothing; events and timestamps have no begin,
    /// and a disjoint query's begin only marks it running.
    pub fn begin_query(&mut self, q: &Rc<Query<B>>) {
        match q.kind {
            kind::EVENT | kind::TIMESTAMP => return,
            k if answered_at_end(k) => {
                *q.state.borrow_mut() = State::Running(Vec::new());
                q.streamed.set([0; 2]);
                if of_stream_zero(k) && !self.queries.streams.iter().any(|r| Rc::ptr_eq(r, q)) {
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
                *q.state.borrow_mut() = State::Counting(0);
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
            let count = match *q.state.borrow() {
                State::Counting(n) => n,
                _ => 0,
            };
            *q.state.borrow_mut() = State::Done(short(&[count]));
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
            State::Fresh | State::Running(_) | State::Counting(_) | State::Statistics(_) => {
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
                            "wcontainers direct3d 11: timestamps reported disjoint: the back end writes none"
                        )
                    });
                    short(&[NANOSECONDS, 1])
                }
                kind::EVENT => short(&[1]),
                k if is_stream_output(k) && (k - kind::SO_STATISTICS).is_multiple_of(2) => {
                    short(&q.streamed.get())
                }
                k if is_stream_output(k) => {
                    let [written, needed] = q.streamed.get();
                    short(&[u64::from(needed > written)])
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
            let mut reads = Vec::new();
            if let State::Ended { parts, .. } = &*q.state.borrow() {
                for p in parts {
                    let r = self.backend.read_queries(&p.set.handle, p.slot, 1);
                    reads.push(self.gpu(r)?);
                }
            }
            if let State::Ended { reads: r, .. } = &mut *q.state.borrow_mut() {
                *r = Some(reads);
            }
            if flush {
                self.flush();
            }
        }
        let reads = match &mut *q.state.borrow_mut() {
            State::Ended { reads, .. } => reads.take().unwrap_or_default(),
            _ => Vec::new(),
        };
        if reads
            .iter()
            .any(|r| self.backend.read_state(r) == ReadState::Pending)
        {
            if let State::Ended { reads: r, .. } = &mut *q.state.borrow_mut() {
                *r = Some(reads);
            }
            return Ok(None);
        }
        let mut any = 0;
        for r in reads {
            let mut bytes = [0u8; size_of::<u64>()];
            self.backend.finish_read(r, &mut bytes);
            any |= u64::from(u64::from_le_bytes(bytes) != 0);
        }
        *q.state.borrow_mut() = State::Done(short(&[any]));
        Ok(Some(short(&[any])))
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

    /// Ends the occlusion counter's stretch, if counted queries run, adding its count to each.
    fn count_stretch(&mut self) -> Result<()> {
        if self.queries.counted.is_empty() {
            return Ok(());
        }
        self.end_pass();
        let n = self.counter.end(&mut self.backend, &mut self.passes);
        let n = self.gpu(n)?;
        for q in &self.queries.counted {
            if let State::Counting(sum) = &mut *q.state.borrow_mut() {
                *sum += n;
            }
        }
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
    /// whose set has no slot left ends, and the draw opens the next on a fresh set.
    pub(crate) fn open_stretch(&mut self) -> bool {
        if self.queries.running.is_empty() || self.queries.stretch.is_some() {
            return true;
        }
        let Some(pass_set) = &self.queries.pass_set else {
            return false;
        };
        match &mut self.queries.set {
            Some((set, next)) if Rc::ptr_eq(set, pass_set) && *next < SET_SLOTS => {
                let slot = *next;
                *next += 1;
                self.backend.begin_query(slot);
                self.queries.stretch = Some(slot);
                true
            }
            _ => false,
        }
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
                parts.push(Part {
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
