//! Occlusion queries: whether any sample passed, from the pass tracker's queries, one an
//! interface query a pass the guest query ran in; exact counts of the samples that passed, from
//! the interface's occlusion counter; counts of the primitives stream output
//! wrote or made, from the stage helper's statistics or counted on the host; and timestamps and times elapsed, from the
//! interface's timestamps.

use std::ffi::c_uint;
use std::ops::Range;

use wcontainers_gpu::{
    Gpu, Object, STORAGE_WORD, passes::PassTracker, resource::BufferDesc, resource::BufferUsage,
    resource::MapIntent, resource::QueryKind, resource::ReadSource, resource::ReadState,
    slow::SlowPath,
};
use wcontainers_shaders::stage::STREAM_BUFFERS;

use crate::context::Context;
use crate::gallium::*;
use crate::screen::DRIVER_NAME;
use crate::state::{refuse, refused};

/// Interface queries in one set. A guest query begun with less than half of a set left takes a
/// new set, so a query may run in this many passes over half before the set is full.
const OCCLUSION_QUERY_BATCH: u32 = 4096;

/// The occlusion sets a context made, the last of them the one queries begin in.
pub struct Sets<B: Gpu> {
    sets: Vec<B::QuerySet>,
    /// Interface queries of the last set already taken.
    used: u32,
}

impl<B: Gpu> Default for Sets<B> {
    fn default() -> Self {
        Sets {
            sets: Vec::new(),
            used: 0,
        }
    }
}

impl<B: Gpu> Sets<B> {
    /// Gives every set back, with the context.
    pub fn release(&mut self, screen: &crate::screen::Screen<B>) {
        for set in self.sets.drain(..) {
            screen.retire(Object::QuerySet(set));
        }
    }

    /// The set a query begins in, made when the last one is half used.
    fn for_next(
        &mut self,
        backend: &mut B,
        tracker: &mut PassTracker<B>,
    ) -> Result<B::QuerySet, String> {
        if self.sets.is_empty() || self.used >= OCCLUSION_QUERY_BATCH / 2 {
            let set = backend
                .create_query_set(QueryKind::OcclusionAny, OCCLUSION_QUERY_BATCH)
                .map_err(|e| format!("an occlusion set: {e:?}"))?;
            tracker
                .set_occlusion(backend, set.clone(), OCCLUSION_QUERY_BATCH)
                .map_err(|e| format!("{e:?}"))?;
            self.sets.push(set);
            self.used = 0;
        }
        self.sets
            .last()
            .cloned()
            .map_or_else(|| refused(format_args!("no occlusion set")), Ok)
    }
}

/// Where a query's answer is.
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

pub struct Query<B: Gpu> {
    answer: Option<Answer<B>>,
    /// Paused while st/mesa draws for itself (its blits and clears).
    paused: bool,
    /// For a count of primitives streamed, the count.
    emitted: Option<Emitted<B>>,
    /// For a timestamp or a time elapsed, its timestamps.
    timer: Option<Timer<B>>,
    /// For an exact count of samples, its count.
    counted: Option<Counted<B>>,
}

/// An exact count of the samples that passed (`SAMPLES_PASSED`), which the interface's occlusion
/// counter writes into `slot` at the query's end (`OcclusionCountReplay`), read back once asked
/// for.
pub struct Counted<B: Gpu> {
    slot: B::Buffer,
    read: Option<B::Readback>,
    known: Option<u64>,
}

/// The bytes of a count the occlusion counter writes.
const COUNT_BYTES: u64 = size_of::<u64>() as u64;

/// A counted query's count, once known (reading back, and waiting if asked to).
fn counted_value<B: Gpu>(
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    counted: &mut Counted<B>,
    wait: bool,
) -> Result<Option<u64>, String> {
    if let Some(known) = counted.known {
        return Ok(Some(known));
    }
    let read = match counted.read.take() {
        Some(read) => read,
        None => {
            tracker.close(backend).map_err(|e| format!("{e:?}"))?;
            backend
                .read_back(&ReadSource::Buffer {
                    buffer: &counted.slot,
                    offset: 0,
                    size: COUNT_BYTES,
                })
                .map_err(|e| format!("{e:?}"))?
        }
    };
    if backend.read_state(&read) == ReadState::Pending {
        if !wait {
            counted.read = Some(read);
            return Ok(None);
        }
        backend.wait_read(&read);
    }
    let mut bytes = [0; COUNT_BYTES as usize];
    backend.finish_read(read, &mut bytes);
    let value = u64::from_le_bytes(bytes);
    counted.known = Some(value);
    Ok(Some(value))
}

/// A timer's two timestamps, at its begin and its end (a timestamp query writes only the end's),
/// read back once asked for.
pub struct Timer<B: Gpu> {
    set: B::QuerySet,
    elapsed: bool,
    read: Option<B::Readback>,
    known: Option<u64>,
}

/// Where a timer's timestamps are in its set.
const TIMER_BEGIN: u32 = 0;
const TIMER_END: u32 = 1;
const TIMER_STAMPS: u32 = 2;

/// Writes the GPU time into query `index` of `set`: timestamps are written between passes, so
/// the open pass ends first.
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

/// The timestamps of `set` from `first`, `count` of them in nanoseconds, waited for.
fn stamps_now<B: Gpu>(
    backend: &mut B,
    set: &B::QuerySet,
    first: u32,
    count: u32,
) -> Result<Vec<u64>, String> {
    let read = backend
        .read_queries(set, first, count)
        .map_err(|e| format!("{e:?}"))?;
    backend.wait_read(&read);
    Ok(finished_stamps(backend, read, count))
}

fn finished_stamps<B: Gpu>(backend: &mut B, read: B::Readback, count: u32) -> Vec<u64> {
    let mut bytes = vec![0; count as usize * size_of::<u64>()];
    backend.finish_read(read, &mut bytes);
    let (stamps, _) = bytes.as_chunks::<{ size_of::<u64>() }>();
    stamps.iter().map(|&s| u64::from_le_bytes(s)).collect()
}

/// `pipe_context::get_timestamp`: the GPU time now, on the clock timer queries read, written after
/// the work recorded so far and waited for.
pub unsafe extern "C" fn get_timestamp<B: Gpu>(pipe: *mut pipe_context) -> u64 {
    crate::state::guest_call(pipe, 0, move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        let screen = context.screen();
        let mut backend = screen.backend();
        let set = backend
            .create_query_set(QueryKind::Timestamp, 1)
            .unwrap_or_else(|e| refuse(format!("the GPU time: {e:?}")));
        let now = stamp(&mut context.tracker, &mut *backend, &set, 0)
            .and_then(|()| stamps_now(&mut *backend, &set, 0, 1));
        screen.retire(Object::QuerySet(set));
        match now {
            Ok(stamps) => stamps.first().copied().unwrap_or_default(),
            Err(reason) => refuse(format!("the GPU time: {reason}")),
        }
    })
}

/// Streamed draws whose counts a count keeps on the device before it reads them back and sums
/// them.
const EMITTED_SLOTS: u64 = 1024;
/// The stage helper's statistics words of stream 0's primitives written and made.
const WRITTEN_WORD: u64 = STREAM_BUFFERS as u64;
const MADE_WORD: u64 = WRITTEN_WORD + 1;

/// Primitives written to stream output, or made, while the count ran: each streamed draw's,
/// copied from the stage helper's statistics into a slot, read back and summed when asked for (or
/// when the slots are full); and for a count of those made, the primitives of each draw the stage
/// helper did not run, counted on the host.
pub struct Emitted<B: Gpu> {
    slots: B::Buffer,
    used: u64,
    sum: u64,
    /// The statistics word copied: `WRITTEN_WORD` or `MADE_WORD`.
    word: u64,
}

impl<B: Gpu> Emitted<B> {
    /// The slots' counts summed into `sum`, read back now (outside a pass).
    fn gather(&mut self, backend: &mut B) -> Result<(), String> {
        if self.used == 0 {
            return Ok(());
        }
        let mut words = vec![0; (self.used * STORAGE_WORD) as usize];
        backend
            .read_back_now(
                &ReadSource::Buffer {
                    buffer: &self.slots,
                    offset: 0,
                    size: self.used * STORAGE_WORD,
                },
                &mut words,
            )
            .map_err(|e| format!("{e:?}"))?;
        let (counts, _) = words.as_chunks::<{ size_of::<u32>() }>();
        self.sum += counts
            .iter()
            .map(|&w| u64::from(u32::from_le_bytes(w)))
            .sum::<u64>();
        self.used = 0;
        Ok(())
    }
}

/// After a streamed draw: the running count takes its primitives written from `statistics`, the
/// stage helper's block (outside a pass).
pub fn count_emitted<B: Gpu>(
    counting: &[*mut pipe_query],
    backend: &mut B,
    statistics: &B::Buffer,
) -> Result<(), String> {
    for &query in counting {
        // SAFETY: a running count lives until it ends.
        let Some(emitted) = (unsafe { of::<B>(query) }).emitted.as_mut() else {
            return refused(format_args!("a count that is not one"));
        };
        if emitted.used == EMITTED_SLOTS {
            emitted.gather(backend)?;
        }
        backend.copy_buffer(
            statistics,
            emitted.word * STORAGE_WORD,
            &emitted.slots,
            emitted.used * STORAGE_WORD,
            STORAGE_WORD,
        );
        emitted.used += 1;
    }
    Ok(())
}

/// The primitives `count` vertices of `mode` make, ignoring restart.
fn primitives(mode: mesa_prim, count: u32) -> u32 {
    /// Vertices a primitive of each kind of list takes.
    const LINE: u32 = 2;
    const TRIANGLE: u32 = 3;
    const LINE_ADJACENCY: u32 = 4;
    const TRIANGLE_ADJACENCY: u32 = 6;
    let past = |first: u32| count.saturating_sub(first - 1);
    match mode {
        MESA_PRIM_POINTS => count,
        MESA_PRIM_LINES => count / LINE,
        MESA_PRIM_LINE_LOOP if count >= LINE => count,
        MESA_PRIM_LINE_STRIP => past(LINE),
        MESA_PRIM_TRIANGLES => count / TRIANGLE,
        MESA_PRIM_TRIANGLE_STRIP | MESA_PRIM_TRIANGLE_FAN => past(TRIANGLE),
        MESA_PRIM_LINES_ADJACENCY => count / LINE_ADJACENCY,
        MESA_PRIM_LINE_STRIP_ADJACENCY => past(LINE_ADJACENCY),
        MESA_PRIM_TRIANGLES_ADJACENCY => count / TRIANGLE_ADJACENCY,
        MESA_PRIM_TRIANGLE_STRIP_ADJACENCY => past(TRIANGLE_ADJACENCY) / LINE,
        _ => 0,
    }
}

/// After a draw not streamed: each running count of primitives made takes the draw's, counted on
/// the host from its ranges; a draw through a geometry or tessellation stage (`staged`) was counted
/// by the stage helper. A draw restarting its strips, whose count needs its indices read, is
/// refused by name.
pub fn count_drawn<B: Gpu>(
    counting: &[*mut pipe_query],
    info: &pipe_draw_info,
    draws: &[pipe_draw_start_count_bias],
    staged: bool,
) -> Result<(), String> {
    for &query in counting {
        // SAFETY: a running count lives until it ends.
        let Some(emitted) = (unsafe { of::<B>(query) }).emitted.as_mut() else {
            continue;
        };
        // The stage helper counted what a geometry or tessellation stage made (`count_emitted`).
        if emitted.word != MADE_WORD || staged {
            continue;
        }
        if info.primitive_restart() && info.index_size > 0 {
            return refused(format_args!(
                "a count of primitives made by a draw with primitive restart"
            ));
        }
        let mode = mesa_prim::from(info.mode);
        let made: u64 = draws
            .iter()
            .map(|d| u64::from(primitives(mode, d.count)))
            .sum();
        emitted.sum += made * u64::from(info.instance_count);
    }
    Ok(())
}

/// `pipe_context::create_query`: occlusion predicates and exact counts, counts of primitives
/// streamed and timers.
pub unsafe extern "C" fn create_query<B: Gpu>(
    pipe: *mut pipe_context,
    kind: c_uint,
    _: c_uint,
) -> *mut pipe_query {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        let made = |emitted, timer, counted| {
            Box::into_raw(Box::new(Query::<B> {
                answer: None,
                paused: false,
                emitted,
                timer,
                counted,
            }))
            .cast()
        };
        let query = |emitted| made(emitted, None, None);
        match kind as pipe_query_type {
            PIPE_QUERY_OCCLUSION_PREDICATE | PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE => {
                query(None)
            }
            PIPE_QUERY_OCCLUSION_COUNTER => {
                let slot = context
                    .screen()
                    .backend()
                    .create_buffer(&BufferDesc {
                        size: COUNT_BYTES,
                        usage: BufferUsage::COPY_DST.union(BufferUsage::READ_BACK),
                        intent: MapIntent::Rarely,
                    })
                    .unwrap_or_else(|e| refuse(format!("a count of samples: {e:?}")));
                made(
                    None,
                    None,
                    Some(Counted {
                        slot,
                        read: None,
                        known: None,
                    }),
                )
            }
            PIPE_QUERY_PRIMITIVES_EMITTED | PIPE_QUERY_PRIMITIVES_GENERATED => {
                let slots = context
                    .screen()
                    .backend()
                    .create_buffer(&BufferDesc {
                        size: EMITTED_SLOTS * STORAGE_WORD,
                        usage: BufferUsage::COPY_DST.union(BufferUsage::READ_BACK),
                        intent: MapIntent::Rarely,
                    })
                    .unwrap_or_else(|e| refuse(format!("a count of primitives: {e:?}")));
                let word = if kind as pipe_query_type == PIPE_QUERY_PRIMITIVES_GENERATED {
                    MADE_WORD
                } else {
                    WRITTEN_WORD
                };
                query(Some(Emitted {
                    slots,
                    used: 0,
                    sum: 0,
                    word,
                }))
            }
            PIPE_QUERY_TIMESTAMP | PIPE_QUERY_TIME_ELAPSED => {
                let set = context
                    .screen()
                    .backend()
                    .create_query_set(QueryKind::Timestamp, TIMER_STAMPS)
                    .unwrap_or_else(|e| refuse(format!("a timer: {e:?}")));
                made(
                    None,
                    Some(Timer {
                        set,
                        elapsed: kind as pipe_query_type == PIPE_QUERY_TIME_ELAPSED,
                        read: None,
                        known: None,
                    }),
                    None,
                )
            }
            // The rest: the API's own error, st/mesa's out of memory for a query it could not
            // make.
            _ => {
                eprintln!(
                    "{}: not built: a query of Gallium kind {kind} (occlusion predicates and \
                     counts, counts of primitives streamed and timers are built)",
                    DRIVER_NAME.to_string_lossy()
                );
                std::ptr::null_mut()
            }
        }
    })
}

/// # Safety
/// `query` is one `create_query` made, not yet destroyed.
unsafe fn of<'a, B: Gpu>(query: *mut pipe_query) -> &'a mut Query<B> {
    // SAFETY: the caller's.
    unsafe { &mut *query.cast::<Query<B>>() }
}

pub unsafe extern "C" fn destroy_query<B: Gpu>(pipe: *mut pipe_context, query: *mut pipe_query) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        context.unread.retain(|&q| q != query);
        context.counting.retain(|&q| q != query);
        // SAFETY: Mesa destroys a query it made once.
        let query = unsafe { Box::from_raw(query.cast::<Query<B>>()) };
        if let Some(emitted) = query.emitted {
            context.screen().retire(Object::Buffer(emitted.slots));
        }
        if let Some(timer) = query.timer {
            context.screen().retire(Object::QuerySet(timer.set));
        }
        if let Some(counted) = query.counted {
            context.screen().retire(Object::Buffer(counted.slot));
        }
    })
}

pub unsafe extern "C" fn begin_query<B: Gpu>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) -> bool {
    crate::state::guest_call(pipe, false, move || {
        // SAFETY: Mesa passes this driver's context and a query of it.
        let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
        if let Some(emitted) = &mut query.emitted {
            (emitted.used, emitted.sum) = (0, 0);
            context.counting.push(std::ptr::from_mut(query).cast());
            return true;
        }
        if let Some(timer) = &mut query.timer {
            (timer.read, timer.known) = (None, None);
            if timer.elapsed {
                let mut backend = context.screen().backend();
                if let Err(reason) =
                    stamp(&mut context.tracker, &mut backend, &timer.set, TIMER_BEGIN)
                {
                    refuse(format!("a timer's begin: {reason}"));
                }
            }
            return true;
        }
        if let Some(counted) = &mut query.counted {
            (counted.read, counted.known) = (None, None);
            let mut backend = context.screen().backend();
            if let Err(error) = context.counter.begin(&mut backend, &mut context.tracker) {
                refuse(format!("a count of samples: {error:?}"));
            }
            query.paused = false;
            context.running = Some(std::ptr::from_mut(query).cast());
            context.counts_samples = true;
            return true;
        }
        let screen = context.screen();
        let mut backend = screen.backend();
        let begun = context
            .queries
            .for_next(&mut backend, &mut context.tracker)
            .and_then(|set| {
                context
                    .tracker
                    .begin_query(&mut backend)
                    .map_err(|e| format!("{e:?}"))?;
                Ok(set)
            });
        match begun {
            Ok(set) => {
                query.answer = Some(Answer::Running {
                    set,
                    first: context.queries.used,
                });
                context.running = Some(std::ptr::from_mut(query).cast());
                true
            }
            Err(reason) => refuse(format!("an occlusion query: {reason}")),
        }
    })
}

pub unsafe extern "C" fn end_query<B: Gpu>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) -> bool {
    crate::state::guest_call(pipe, false, move || {
        // SAFETY: as above.
        let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
        if query.emitted.is_some() {
            let ended: *mut pipe_query = std::ptr::from_mut(query).cast();
            context.counting.retain(|&q| q != ended);
            return true;
        }
        if let Some(timer) = &mut query.timer {
            (timer.read, timer.known) = (None, None);
            let mut backend = context.screen().backend();
            if let Err(reason) = stamp(&mut context.tracker, &mut backend, &timer.set, TIMER_END) {
                refuse(format!("a timer's end: {reason}"));
            }
            return true;
        }
        if let Some(counted) = &query.counted {
            let mut backend = context.screen().backend();
            let ended =
                context
                    .counter
                    .end_into(&mut backend, &mut context.tracker, &counted.slot, 0);
            if let Err(error) = ended {
                refuse(format!("a count of samples: {error:?}"));
            }
            context.running = None;
            context.counts_samples = false;
            query.paused = false;
            return true;
        }
        let Some(Answer::Running { set, first }) = query.answer.take() else {
            refuse(format_args!("an occlusion query ended that is not running"));
        };
        let mut backend = context.screen().backend();
        let end = if query.paused {
            context.queries.used
        } else {
            match context.tracker.end_query(&mut backend) {
                Ok(range) => range.end,
                Err(error) => refuse(format!("an occlusion query: {error:?}")),
            }
        };
        context.queries.used = end;
        context.running = None;
        query.paused = false;
        query.answer = Some(Answer::Ended {
            set,
            queries: first..end,
            read: None,
        });
        context.unread.push(std::ptr::from_mut(query).cast());
        true
    })
}

/// Starts the read back of every query ended since the last flush (outside a pass).
pub fn start_reads<B: Gpu>(context: &mut Context<B>, backend: &mut B) {
    for query in std::mem::take(&mut context.unread) {
        // SAFETY: a query in the list is live: destroying one takes it out.
        let query = unsafe { of::<B>(query) };
        if let Some(Answer::Ended { set, queries, read }) = &mut query.answer
            && read.is_none()
            && !Range::is_empty(queries)
        {
            match backend.read_queries(set, queries.start, queries.len() as u32) {
                Ok(started) => *read = Some(started),
                Err(error) => refuse(format!("an occlusion query's read back: {error:?}")),
            }
        }
    }
}

pub unsafe extern "C" fn get_query_result<B: Gpu>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
    wait: bool,
    result: *mut pipe_query_result,
) -> bool {
    // A refused answer is zero, and ready, as a lost context's answers are.
    let known = crate::state::guest_call(pipe, Some(pipe_query_result { u64_: 0 }), move || {
        // SAFETY: as above.
        let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
        query_result(context, query, wait)
    });
    let Some(known) = known else {
        return false;
    };
    // SAFETY: Mesa gives a result to write.
    unsafe { *result = known };
    true
}

/// A query's answer, once known (waiting for it if asked to).
fn query_result<B: Gpu>(
    context: &mut Context<B>,
    query: &mut Query<B>,
    wait: bool,
) -> Option<pipe_query_result> {
    if let Some(emitted) = &mut query.emitted {
        let mut backend = context.screen().backend();
        let gathered = context
            .tracker
            .close(&mut backend)
            .map_err(|e| format!("{e:?}"))
            .and_then(|()| emitted.gather(&mut backend));
        if let Err(reason) = gathered {
            refuse(format!("a count of primitives: {reason}"));
        }
        return Some(pipe_query_result { u64_: emitted.sum });
    }
    if let Some(timer) = &mut query.timer {
        let mut backend = context.screen().backend();
        return match timer_value(&mut context.tracker, &mut backend, timer, wait) {
            Ok(value) => value.map(|u64_| pipe_query_result { u64_ }),
            Err(reason) => refuse(format!("a timer's result: {reason}")),
        };
    }
    if let Some(counted) = &mut query.counted {
        let mut backend = context.screen().backend();
        return match counted_value(&mut context.tracker, &mut backend, counted, wait) {
            Ok(value) => value.map(|u64_| pipe_query_result { u64_ }),
            Err(reason) => refuse(format!("a count of samples' result: {reason}")),
        };
    }
    match answer(context, query, wait) {
        Ok(passed) => passed.map(|b| pipe_query_result { b }),
        Err(reason) => refuse(format!("an occlusion query's result: {reason}")),
    }
}

/// `pipe_context::render_condition`: draws and clears from now on are skipped while `query`'s
/// answer equals `condition`; no query lifts it. Every mode waits for the answer (the waiting
/// modes as they ask; the others may draw before it is known and are given it).
pub unsafe extern "C" fn render_condition<B: Gpu>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
    condition: bool,
    _: pipe_render_cond_flag,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        context.condition = (!query.is_null()).then_some((query, condition));
    })
}

/// Whether the render condition lets the next draw or clear through, its occlusion answer waited
/// for (`PredicationWait`).
pub fn condition_holds<B: Gpu>(context: &mut Context<B>) -> bool {
    let Some((query, condition)) = context.condition else {
        return true;
    };
    // SAFETY: a query st/mesa conditions on lives while the condition is set.
    let query = unsafe { of::<B>(query) };
    if query.emitted.is_some() || query.timer.is_some() {
        refuse(format_args!(
            "a render condition on a query other than an occlusion query"
        ));
    }
    context
        .screen()
        .backend()
        .record_slow_path(SlowPath::PredicationWait);
    if let Some(counted) = &mut query.counted {
        let mut backend = context.screen().backend();
        return match counted_value(&mut context.tracker, &mut backend, counted, true) {
            Ok(Some(count)) => (count != 0) != condition,
            Ok(None) => true,
            Err(reason) => refuse(format!("a render condition's count: {reason}")),
        };
    }
    match answer(context, query, true) {
        Ok(Some(passed)) => passed != condition,
        Ok(None) => true,
        Err(reason) => refuse(format!("a render condition's answer: {reason}")),
    }
}

/// A timer's nanoseconds (a timestamp, or the time between its begin and end), once known
/// (reading back, and waiting if asked to).
fn timer_value<B: Gpu>(
    tracker: &mut PassTracker<B>,
    backend: &mut B,
    timer: &mut Timer<B>,
    wait: bool,
) -> Result<Option<u64>, String> {
    if let Some(known) = timer.known {
        return Ok(Some(known));
    }
    let read = match timer.read.take() {
        Some(read) => read,
        None => {
            tracker.close(backend).map_err(|e| format!("{e:?}"))?;
            backend
                .read_queries(&timer.set, TIMER_BEGIN, TIMER_STAMPS)
                .map_err(|e| format!("{e:?}"))?
        }
    };
    if backend.read_state(&read) == ReadState::Pending {
        if !wait {
            timer.read = Some(read);
            return Ok(None);
        }
        backend.wait_read(&read);
    }
    let stamps = finished_stamps(backend, read, TIMER_STAMPS);
    let (begin, end) = (stamps[TIMER_BEGIN as usize], stamps[TIMER_END as usize]);
    let value = if timer.elapsed {
        end.saturating_sub(begin)
    } else {
        end
    };
    timer.known = Some(value);
    Ok(Some(value))
}

/// Whether any sample passed, once known (reading back, and waiting if asked to).
fn answer<B: Gpu>(
    context: &mut Context<B>,
    query: &mut Query<B>,
    wait: bool,
) -> Result<Option<bool>, String> {
    let mut backend = context.screen().backend();
    let Some(Answer::Ended { set, queries, read }) = &mut query.answer else {
        return match query.answer {
            Some(Answer::Known(passed)) => Ok(Some(passed)),
            _ => refused(format_args!("a query that has not ended")),
        };
    };
    if Range::is_empty(queries) {
        query.answer = Some(Answer::Known(false));
        return Ok(Some(false));
    }
    if read.is_none() {
        context
            .tracker
            .close(&mut backend)
            .map_err(|e| format!("{e:?}"))?;
        *read = Some(
            backend
                .read_queries(set, queries.start, queries.len() as u32)
                .map_err(|e| format!("{e:?}"))?,
        );
    }
    let readback = read.take().ok_or("the read back")?;
    if backend.read_state(&readback) == ReadState::Pending {
        if !wait {
            *read = Some(readback);
            return Ok(None);
        }
        backend.wait_read(&readback);
    }
    let mut counts = vec![0; queries.len() * size_of::<u64>()];
    backend.finish_read(readback, &mut counts);
    let passed = counts.iter().any(|&byte| byte != 0);
    query.answer = Some(Answer::Known(passed));
    Ok(Some(passed))
}

/// `pipe_context::set_active_query_state`: st/mesa's own draws (blits, clears) count in no query;
/// the running query's interface queries stop and go on from the next index, which no other
/// query takes meanwhile, and a running count of samples keeps none of them.
pub unsafe extern "C" fn set_active_query_state<B: Gpu>(pipe: *mut pipe_context, enable: bool) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        let Some(running) = context.running else {
            return;
        };
        // SAFETY: a running query lives until it ends.
        let query = unsafe { of::<B>(running) };
        if query.paused != enable {
            return;
        }
        if query.counted.is_some() {
            query.paused = !enable;
            context.counts_samples = enable;
            return;
        }
        let mut backend = context.screen().backend();
        let changed = if enable {
            context.tracker.begin_query(&mut backend)
        } else {
            context.tracker.end_query(&mut backend).map(|range| {
                context.queries.used = range.end;
            })
        };
        if let Err(error) = changed {
            refuse(format!("pausing an occlusion query: {error:?}"));
        }
        query.paused = !enable;
    })
}
