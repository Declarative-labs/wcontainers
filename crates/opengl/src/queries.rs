//! Queries on the Gallium trait: whether any sample passed (`OCCLUSION_PREDICATE`), from the pass
//! tracker's interface queries, one a pass the query ran in; exact counts of the samples that passed
//! (`OCCLUSION_COUNTER`), from the interface's occlusion counter; times elapsed and timestamps, from
//! the interface's timestamps. A query the trait hands over begun is an [`ActiveQuery`] holding the
//! same state as the [`Query`] it was; the context keeps the running one for st/mesa's pauses
//! (`set_active_query_state`) and the one a render condition reads.

use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard};

use wcontainers_gallium::values::QueryType;
use wcontainers_gpu::{
    Gpu, Object, occlusion::OcclusionCounter, passes::PassTracker, resource::BufferDesc,
    resource::BufferUsage, resource::MapIntent, resource::QueryKind, resource::ReadSource,
    resource::ReadState, slow::SlowPath,
};

use crate::guest::refused;
use crate::screen::whole;
use crate::states::{Retired, retire};

/// Interface queries in one set. A query begun with less than half of a set left takes a new set,
/// so a query may run in this many passes over half before the set is full.
const OCCLUSION_QUERY_BATCH: u32 = 4096;

/// Where a timer's timestamps are in its set.
const TIMER_BEGIN: u32 = 0;
const TIMER_END: u32 = 1;
const TIMER_STAMPS: u32 = 2;

/// The bytes of a count the occlusion counter writes, and of a query's result.
const COUNT_BYTES: u64 = size_of::<u64>() as u64;

/// What each query reads its answer from.
enum Kind<B: Gpu> {
    /// Whether any sample passed: the interface queries of the passes it ran in.
    Predicate(Option<Answer<B>>),
    /// The exact count of samples that passed, which the occlusion counter writes into `slot` at
    /// the query's end, read back once asked for.
    Counted {
        slot: B::Buffer,
        read: Option<B::Readback>,
        known: Option<u64>,
    },
    /// The time between its begin and its end, in nanoseconds.
    Elapsed(Timer<B>),
}

/// Where a predicate's answer is.
enum Answer<B: Gpu> {
    /// Begun, not yet ended: the interface queries from its first.
    Running {
        set: B::QuerySet,
        first: u32,
    },
    /// Ended: its interface queries, read back once asked for.
    Ended {
        set: B::QuerySet,
        queries: Range<u32>,
        read: Option<B::Readback>,
    },
    Known(bool),
}

/// A timer's timestamps in its own set, read back once asked for.
struct Timer<B: Gpu> {
    set: B::QuerySet,
    read: Option<B::Readback>,
    known: Option<u64>,
}

/// A query's state, shared by the value st/mesa holds and the context while it runs.
struct Held<B: Gpu> {
    kind: Kind<B>,
    /// Paused while st/mesa draws for itself (its blits and clears).
    paused: bool,
}

/// A query made and not running (the trait's `Query`).
pub struct Query<B: Gpu>(Arc<Mutex<Held<B>>>);

/// A query between its begin and its end (the trait's `ActiveQuery`).
pub struct ActiveQuery<B: Gpu>(Arc<Mutex<Held<B>>>);

/// A query only ended, a timestamp (the trait's `EndOnlyQuery`): the GPU time at its end.
pub struct EndOnlyQuery<B: Gpu>(Mutex<Timer<B>>);

/// What a context keeps of its queries: the occlusion sets it made (the last the one predicates
/// begin in), its occlusion counter, the query running, and the render condition.
pub struct Queries<B: Gpu> {
    sets: Vec<B::QuerySet>,
    /// Interface queries of the last set already taken.
    used: u32,
    counter: OcclusionCounter<B>,
    running: Option<Arc<Mutex<Held<B>>>>,
    /// The query draws and clears are skipped by while its answer equals the flag.
    condition: Option<(Arc<Mutex<Held<B>>>, bool)>,
}

impl<B: Gpu> Default for Queries<B> {
    fn default() -> Self {
        Queries {
            sets: Vec::new(),
            used: 0,
            counter: OcclusionCounter::default(),
            running: None,
            condition: None,
        }
    }
}

fn held<B: Gpu>(state: &Mutex<Held<B>>) -> MutexGuard<'_, Held<B>> {
    whole(state)
}

/// `pipe_context.create_query`: occlusion predicates (the conservative kind too), exact counts of
/// samples and times elapsed; None for the rest, which st/mesa reports as out of memory.
pub fn create<B: Gpu>(backend: &mut B, kind: QueryType) -> Result<Option<Query<B>>, String> {
    let kind = match kind {
        QueryType::PIPE_QUERY_OCCLUSION_PREDICATE
        | QueryType::PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE => Kind::Predicate(None),
        QueryType::PIPE_QUERY_OCCLUSION_COUNTER => Kind::Counted {
            slot: backend
                .create_buffer(&BufferDesc {
                    size: COUNT_BYTES,
                    usage: BufferUsage::COPY_DST.union(BufferUsage::READ_BACK),
                    intent: MapIntent::Rarely,
                })
                .map_err(|e| format!("a count of samples: {e:?}"))?,
            read: None,
            known: None,
        },
        QueryType::PIPE_QUERY_TIME_ELAPSED => Kind::Elapsed(timer(backend)?),
        _ => return Ok(None),
    };
    Ok(Some(Query(Arc::new(Mutex::new(Held {
        kind,
        paused: false,
    })))))
}

/// `pipe_context.create_query` of a query only ended: a timestamp; None for the rest.
pub fn create_end_only<B: Gpu>(
    backend: &mut B,
    kind: QueryType,
) -> Result<Option<EndOnlyQuery<B>>, String> {
    if kind != QueryType::PIPE_QUERY_TIMESTAMP {
        return Ok(None);
    }
    Ok(Some(EndOnlyQuery(Mutex::new(timer(backend)?))))
}

fn timer<B: Gpu>(backend: &mut B) -> Result<Timer<B>, String> {
    Ok(Timer {
        set: backend
            .create_query_set(QueryKind::Timestamp, TIMER_STAMPS)
            .map_err(|e| format!("a timer: {e:?}"))?,
        read: None,
        known: None,
    })
}

/// What a query gives back to the back end when st/mesa destroys it.
fn release<B: Gpu>(retired: &Retired<B>, kind: &Kind<B>) {
    match kind {
        Kind::Counted { slot, .. } => retire(retired, Object::Buffer(slot.clone())),
        Kind::Elapsed(timer) => retire(retired, Object::QuerySet(timer.set.clone())),
        Kind::Predicate(_) => {}
    }
}

impl<B: Gpu> Query<B> {
    /// A second handle on the same query, which a refused call hands back in its place.
    pub fn handle(&self) -> Query<B> {
        Query(Arc::clone(&self.0))
    }

    /// Gives its buffers and sets back; the context forgets it as its render condition.
    pub fn destroy(self, queries: &mut Queries<B>, retired: &Retired<B>) {
        if queries
            .condition
            .as_ref()
            .is_some_and(|(q, _)| Arc::ptr_eq(q, &self.0))
        {
            queries.condition = None;
        }
        release(retired, &held(&self.0).kind);
    }
}

impl<B: Gpu> ActiveQuery<B> {
    /// A second handle on the same query, which a refused call hands back in its place.
    pub fn handle(&self) -> ActiveQuery<B> {
        ActiveQuery(Arc::clone(&self.0))
    }

    /// Destroyed while running: it stops running first, and gives its buffers and sets back
    /// whether or not it could end.
    pub fn destroy(
        self,
        queries: &mut Queries<B>,
        tracker: &mut PassTracker<B>,
        backend: &mut B,
        retired: &Retired<B>,
    ) {
        match end(self, queries, tracker, backend) {
            Ok(query) => query.destroy(queries, retired),
            Err((active, _)) => {
                queries.running = None;
                release(retired, &held(&active.0).kind);
            }
        }
    }
}

impl<B: Gpu> EndOnlyQuery<B> {
    pub fn destroy(self, retired: &Retired<B>) {
        retire(retired, Object::QuerySet(whole(&self.0).set.clone()));
    }
}

/// `pipe_context.begin_query`: the query runs from here, in the passes recorded until it ends.
pub fn begin<B: Gpu>(
    query: Query<B>,
    queries: &mut Queries<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
) -> Result<ActiveQuery<B>, (Query<B>, String)> {
    if queries.running.is_some() {
        return Err((query, "a query begun while another runs".into()));
    }
    let begun = {
        let mut state = held(&query.0);
        state.paused = false;
        match &mut state.kind {
            Kind::Elapsed(timer) => {
                (timer.read, timer.known) = (None, None);
                stamp(tracker, backend, &timer.set, TIMER_BEGIN)
            }
            Kind::Counted { read, known, .. } => {
                (*read, *known) = (None, None);
                queries
                    .counter
                    .begin(backend, tracker)
                    .map_err(|e| format!("a count of samples: {e:?}"))
            }
            Kind::Predicate(answer) => predicate_set(queries, tracker, backend).and_then(|set| {
                tracker
                    .begin_query(backend)
                    .map_err(|e| format!("an occlusion query: {e:?}"))?;
                *answer = Some(Answer::Running {
                    set,
                    first: queries.used,
                });
                Ok(())
            }),
        }
    };
    match begun {
        Ok(()) => {
            queries.running = Some(Arc::clone(&query.0));
            Ok(ActiveQuery(query.0))
        }
        Err(reason) => Err((query, reason)),
    }
}

/// The set a predicate begins in, made when the last one is half used.
fn predicate_set<B: Gpu>(
    queries: &mut Queries<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
) -> Result<B::QuerySet, String> {
    if queries.sets.is_empty() || queries.used >= OCCLUSION_QUERY_BATCH / 2 {
        let set = backend
            .create_query_set(QueryKind::OcclusionAny, OCCLUSION_QUERY_BATCH)
            .map_err(|e| format!("an occlusion set: {e:?}"))?;
        tracker
            .set_occlusion(backend, set.clone(), OCCLUSION_QUERY_BATCH)
            .map_err(|e| format!("{e:?}"))?;
        queries.sets.push(set);
        queries.used = 0;
    }
    queries
        .sets
        .last()
        .cloned()
        .map_or_else(|| refused(format_args!("no occlusion set")), Ok)
}

/// `pipe_context.end_query`: the query stops running; its answer is read back once asked for.
pub fn end<B: Gpu>(
    query: ActiveQuery<B>,
    queries: &mut Queries<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
) -> Result<Query<B>, (ActiveQuery<B>, String)> {
    let ended = {
        let mut state = held(&query.0);
        let paused = state.paused;
        match &mut state.kind {
            Kind::Elapsed(timer) => stamp(tracker, backend, &timer.set, TIMER_END),
            Kind::Counted { slot, .. } => queries
                .counter
                .end_into(backend, tracker, slot, 0)
                .map_err(|e| format!("a count of samples: {e:?}")),
            Kind::Predicate(answer) => match answer.take() {
                Some(Answer::Running { set, first }) => {
                    let end = if paused {
                        Ok(queries.used)
                    } else {
                        tracker.end_query(backend).map(|range| range.end)
                    };
                    match end {
                        Ok(end) => {
                            queries.used = end;
                            *answer = Some(Answer::Ended {
                                set,
                                queries: first..end,
                                read: None,
                            });
                            Ok(())
                        }
                        Err(error) => {
                            *answer = Some(Answer::Running { set, first });
                            Err(format!("an occlusion query: {error:?}"))
                        }
                    }
                }
                other => {
                    *answer = other;
                    Err("an occlusion query ended that is not running".into())
                }
            },
        }
    };
    match ended {
        Ok(()) => {
            held(&query.0).paused = false;
            queries.running = None;
            Ok(Query(query.0))
        }
        Err(reason) => Err((query, reason)),
    }
}

/// `pipe_context.end_query` of a timestamp: the GPU time after the work recorded so far.
pub fn end_only<B: Gpu>(
    query: &EndOnlyQuery<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
) -> Result<(), String> {
    let mut timer = whole(&query.0);
    (timer.read, timer.known) = (None, None);
    stamp(tracker, backend, &timer.set, TIMER_END)
}

/// Writes the GPU time into query `index` of `set`: timestamps are written between passes, so the
/// open pass ends first.
fn stamp<B: Gpu>(
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    set: &B::QuerySet,
    index: u32,
) -> Result<(), String> {
    tracker.close(backend).map_err(|e| format!("{e:?}"))?;
    backend
        .write_timestamp(set, index)
        .map_err(|e| format!("{e:?}"))
}

/// `pipe_context.get_timestamp`: the GPU time now, written after the work recorded so far and
/// waited for.
pub fn now<B: Gpu>(
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    retired: &Retired<B>,
) -> Result<u64, String> {
    let set = backend
        .create_query_set(QueryKind::Timestamp, 1)
        .map_err(|e| format!("the GPU time: {e:?}"))?;
    let now = stamp(tracker, backend, &set, 0).and_then(|()| {
        let read = backend
            .read_queries(&set, 0, 1)
            .map_err(|e| format!("{e:?}"))?;
        backend.wait_read(&read);
        Ok(stamps(backend, read, 1)[0])
    });
    retire(retired, Object::QuerySet(set));
    now
}

fn stamps<B: Gpu>(backend: &mut B, read: B::Readback, count: u32) -> Vec<u64> {
    let mut bytes = vec![0; count as usize * size_of::<u64>()];
    backend.finish_read(read, &mut bytes);
    bytes
        .as_chunks::<{ size_of::<u64>() }>()
        .0
        .iter()
        .map(|&s| u64::from_le_bytes(s))
        .collect()
}

/// `pipe_context.get_query_result`: the answer written into `result` as Gallium's
/// `pipe_query_result` (a u64, whose low byte is the bool a predicate answers), once known
/// (reading back, and waiting if asked to); false while it is not.
pub fn result<B: Gpu>(
    query: &Query<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    wait: bool,
    result: &mut [u8],
) -> Result<bool, String> {
    let value = value(&mut held(&query.0), tracker, backend, wait)?;
    Ok(value.is_some_and(|v| written(v, result)))
}

/// A timestamp's result, as [`result`] writes one.
pub fn end_only_result<B: Gpu>(
    query: &EndOnlyQuery<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    wait: bool,
    result: &mut [u8],
) -> Result<bool, String> {
    let value = timer_value(&mut whole(&query.0), tracker, backend, wait, false)?;
    Ok(value.is_some_and(|v| written(v, result)))
}

/// Writes `value` as a `pipe_query_result`; false where `result` cannot hold one.
fn written(value: u64, result: &mut [u8]) -> bool {
    match result.get_mut(..COUNT_BYTES as usize) {
        Some(to) => {
            to.copy_from_slice(&value.to_le_bytes());
            true
        }
        None => false,
    }
}

/// A query's answer as a u64 (a predicate's 1 or 0), once known.
fn value<B: Gpu>(
    state: &mut Held<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    wait: bool,
) -> Result<Option<u64>, String> {
    match &mut state.kind {
        Kind::Elapsed(timer) => timer_value(timer, tracker, backend, wait, true),
        Kind::Counted { slot, read, known } => {
            if let Some(known) = known {
                return Ok(Some(*known));
            }
            let pending = match read.take() {
                Some(pending) => pending,
                None => {
                    tracker.close(backend).map_err(|e| format!("{e:?}"))?;
                    backend
                        .read_back(&ReadSource::Buffer {
                            buffer: slot,
                            offset: 0,
                            size: COUNT_BYTES,
                        })
                        .map_err(|e| format!("{e:?}"))?
                }
            };
            let Some(bytes) = finished(backend, pending, read, wait, COUNT_BYTES as usize) else {
                return Ok(None);
            };
            let count = u64::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| "a count of the wrong size".to_string())?,
            );
            *known = Some(count);
            Ok(Some(count))
        }
        Kind::Predicate(answer) => Ok(passed(answer, tracker, backend, wait)?.map(u64::from)),
    }
}

/// The bytes of `pending`, once ready (waited for if asked to); kept in `kept` while not.
fn finished<B: Gpu>(
    backend: &mut B,
    pending: B::Readback,
    kept: &mut Option<B::Readback>,
    wait: bool,
    len: usize,
) -> Option<Vec<u8>> {
    if backend.read_state(&pending) == ReadState::Pending {
        if !wait {
            *kept = Some(pending);
            return None;
        }
        backend.wait_read(&pending);
    }
    let mut bytes = vec![0; len];
    backend.finish_read(pending, &mut bytes);
    Some(bytes)
}

/// A timer's nanoseconds: the time between its begin and end where `elapsed`, else its end's
/// timestamp, once known.
fn timer_value<B: Gpu>(
    timer: &mut Timer<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    wait: bool,
    elapsed: bool,
) -> Result<Option<u64>, String> {
    if let Some(known) = timer.known {
        return Ok(Some(known));
    }
    let pending = match timer.read.take() {
        Some(pending) => pending,
        None => {
            tracker.close(backend).map_err(|e| format!("{e:?}"))?;
            backend
                .read_queries(&timer.set, TIMER_BEGIN, TIMER_STAMPS)
                .map_err(|e| format!("{e:?}"))?
        }
    };
    if backend.read_state(&pending) == ReadState::Pending {
        if !wait {
            timer.read = Some(pending);
            return Ok(None);
        }
        backend.wait_read(&pending);
    }
    let stamps = stamps(backend, pending, TIMER_STAMPS);
    let (begin, end) = (stamps[TIMER_BEGIN as usize], stamps[TIMER_END as usize]);
    let value = if elapsed {
        end.saturating_sub(begin)
    } else {
        end
    };
    timer.known = Some(value);
    Ok(Some(value))
}

/// Whether any sample passed, once known.
fn passed<B: Gpu>(
    answer: &mut Option<Answer<B>>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    wait: bool,
) -> Result<Option<bool>, String> {
    let Some(Answer::Ended { set, queries, read }) = answer else {
        return match answer {
            Some(Answer::Known(passed)) => Ok(Some(*passed)),
            _ => refused(format_args!("the answer of a query that has not ended")),
        };
    };
    if Range::is_empty(queries) {
        *answer = Some(Answer::Known(false));
        return Ok(Some(false));
    }
    let count = queries.len();
    let pending = match read.take() {
        Some(pending) => pending,
        None => {
            tracker.close(backend).map_err(|e| format!("{e:?}"))?;
            backend
                .read_queries(set, queries.start, count as u32)
                .map_err(|e| format!("{e:?}"))?
        }
    };
    let Some(counts) = finished(backend, pending, read, wait, count * size_of::<u64>()) else {
        return Ok(None);
    };
    let passed = counts.iter().any(|&byte| byte != 0);
    *answer = Some(Answer::Known(passed));
    Ok(Some(passed))
}

/// `pipe_context.render_condition`: draws and clears from now on are skipped while `query`'s
/// answer equals `condition`; none lifts it.
pub fn condition<B: Gpu>(queries: &mut Queries<B>, query: Option<&Query<B>>, condition: bool) {
    queries.condition = query.map(|q| (Arc::clone(&q.0), condition));
}

/// Whether the render condition lets the next draw or clear through, its answer waited for
/// (`PredicationWait`); a timer conditions nothing.
pub fn condition_holds<B: Gpu>(
    queries: &mut Queries<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
) -> Result<bool, String> {
    let Some((query, condition)) = &queries.condition else {
        return Ok(true);
    };
    let mut state = held(query);
    if matches!(state.kind, Kind::Elapsed(_)) {
        return refused(format_args!(
            "a render condition on a query other than an occlusion query"
        ));
    }
    backend.record_slow_path(SlowPath::PredicationWait);
    Ok(match value(&mut state, tracker, backend, true)? {
        Some(answer) => (answer != 0) != *condition,
        None => true,
    })
}

/// `pipe_context.set_active_query_state`: st/mesa's own draws (blits, clears) count in no query;
/// the running predicate's interface queries stop and go on from the next index, which no other
/// query takes meanwhile, and a running count of samples keeps none of them.
pub fn set_active<B: Gpu>(
    queries: &mut Queries<B>,
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    enable: bool,
) -> Result<(), String> {
    let Some(running) = &queries.running else {
        return Ok(());
    };
    let mut state = held(running);
    if state.paused != enable {
        return Ok(());
    }
    if matches!(state.kind, Kind::Predicate(_)) {
        if enable {
            tracker.begin_query(backend).map_err(|e| format!("{e:?}"))?;
        } else {
            queries.used = tracker
                .end_query(backend)
                .map_err(|e| format!("{e:?}"))?
                .end;
        }
    }
    state.paused = !enable;
    Ok(())
}

impl<B: Gpu> Queries<B> {
    /// Gives every set back, with the context.
    pub fn release(&mut self, retired: &Retired<B>) {
        for set in self.sets.drain(..) {
            retire(retired, Object::QuerySet(set));
        }
    }
}
