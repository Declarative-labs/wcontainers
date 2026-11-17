//! Asynchronous queries. An event is done when the fence of a submit after its end is done. An
//! occlusion predicate counts in stretches: the back end runs one occlusion query at a time inside a
//! pass, so every begin and end of the program's closes the open stretch, whose result belongs to
//! every predicate running during it (`QueriesSegmented` where others run on), and a predicate's
//! answer is whether any of its stretches saw a sample pass. Counted occlusion queries count
//! exactly through the interface's occlusion counter, likewise in stretches between the program's
//! begins and ends. Timestamps, which the back ends do not write, are answered as Direct3D answers
//! a timestamp counter that was not continuous: the disjoint query says so, and programs drop the
//! stamps it covers.

use std::cell::RefCell;
use std::rc::Rc;

use wcontainers_gpu::{Backend, Object, QueryKind, ReadState, SlowPath};

use crate::device::Device;
use crate::error::{Error, Result, not_implemented};
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
    pub const SO_STATISTICS_STREAM3: u32 = 15;
    pub const SO_OVERFLOW_PREDICATE_STREAM3: u32 = 16;
}

/// Slots of one occlusion set: WebGPU's most.
const SET_SLOTS: u32 = 4096;

/// The bytes `GetData` writes for a query of `kind`, or none for a kind Direct3D 11 does not have.
pub fn data_size(kind: u32) -> Option<u32> {
    Some(match kind {
        kind::EVENT | kind::OCCLUSION_PREDICATE => 4,
        kind::OCCLUSION | kind::TIMESTAMP => 8,
        kind::TIMESTAMP_DISJOINT => 16,
        kind::PIPELINE_STATISTICS => 88,
        kind::SO_STATISTICS..=kind::SO_STATISTICS_STREAM3 if (kind - 6).is_multiple_of(2) => 16,
        kind::SO_OVERFLOW_PREDICATE..=kind::SO_OVERFLOW_PREDICATE_STREAM3 => 4,
        _ => return None,
    })
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
    /// Ended after `submits` submits; its stretches and, once asked for, their read backs.
    Ended {
        submits: u64,
        parts: Vec<Part<B>>,
        reads: Option<Vec<B::Readback>>,
    },
    /// The answer: the bytes `GetData` gives, as words.
    Done([u64; 2]),
}

pub struct Query<B: Backend> {
    /// `D3D11_QUERY_DESC`'s query and misc flags.
    pub kind: u32,
    pub misc: u32,
    state: RefCell<State<B>>,
    /// A timestamp's time, in nanoseconds from the device's making.
    stamp: std::cell::Cell<u64>,
}

/// The timestamps' frequency: nanoseconds.
const NANOSECONDS: u64 = 1_000_000_000;

/// The device's occlusion sets and the predicates running.
pub(crate) struct Queries<B: Backend> {
    /// Occlusion predicates begun and not ended.
    running: Vec<Rc<Query<B>>>,
    /// Counted occlusion queries begun and not ended.
    pub(crate) counted: Vec<Rc<Query<B>>>,
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
    fence: Option<B::Fence>,
}

impl<B: Backend> Queries<B> {
    pub fn new() -> Self {
        Queries {
            running: Vec::new(),
            counted: Vec::new(),
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
        match kind {
            kind::EVENT | kind::OCCLUSION | kind::TIMESTAMP | kind::TIMESTAMP_DISJOINT => {}
            kind::OCCLUSION_PREDICATE => self.queries.used = true,
            kind::PIPELINE_STATISTICS => return not_implemented("pipeline statistics queries"),
            _ => return not_implemented("stream output queries"),
        }
        Ok(Rc::new(Query {
            kind,
            misc,
            state: RefCell::new(State::Fresh),
            stamp: std::cell::Cell::new(0),
        }))
    }

    /// `Begin`: starts an occlusion query again from nothing; events and timestamps have no begin,
    /// and a disjoint query's begin only marks it running.
    pub fn begin_query(&mut self, q: &Rc<Query<B>>) {
        match q.kind {
            kind::EVENT | kind::TIMESTAMP => return,
            kind::TIMESTAMP_DISJOINT => {
                *q.state.borrow_mut() = State::Running(Vec::new());
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
            *q.state.borrow_mut() = State::Done([count, 0]);
            if was && !self.queries.counted.is_empty() {
                let r = self.counter.begin(&mut self.backend, &mut self.passes);
                let _ = self.gpu(r);
            }
            return;
        }
        if q.kind == kind::TIMESTAMP {
            // The time of the end, answered once the work before it is done.
            q.stamp.set(self.queries.made.elapsed().as_nanos() as u64);
        }
        let parts = if matches!(
            q.kind,
            kind::EVENT | kind::TIMESTAMP | kind::TIMESTAMP_DISJOINT
        ) {
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
    pub fn query_data(&mut self, q: &Rc<Query<B>>, flush: bool) -> Result<Option<[u64; 2]>> {
        self.live()?;
        let submits = match &*q.state.borrow() {
            State::Fresh | State::Running(_) | State::Counting(_) => {
                return Err(Error::InvalidCall("a query not ended"));
            }
            State::Done(v) => return Ok(Some(*v)),
            State::Ended { submits, .. } => *submits,
        };
        if matches!(
            q.kind,
            kind::EVENT | kind::TIMESTAMP | kind::TIMESTAMP_DISJOINT
        ) {
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
                kind::TIMESTAMP => [q.stamp.get(), 0],
                kind::TIMESTAMP_DISJOINT => {
                    static WARNED: std::sync::Once = std::sync::Once::new();
                    WARNED.call_once(|| {
                        eprintln!(
                            "wcontainers direct3d 11: timestamps reported disjoint: the back end writes none"
                        )
                    });
                    [NANOSECONDS, 1]
                }
                _ => [1, 0],
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
            let mut bytes = [0u8; 8];
            self.backend.finish_read(r, &mut bytes);
            any |= u64::from(u64::from_le_bytes(bytes) != 0);
        }
        *q.state.borrow_mut() = State::Done([any, 0]);
        Ok(Some([any, 0]))
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
