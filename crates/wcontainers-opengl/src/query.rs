//! Occlusion queries: whether any sample passed, from the pass tracker's queries, one an
//! interface query a pass the guest query ran in; and counts of the primitives stream output
//! wrote, from the stage helper's statistics.

use std::ffi::c_uint;
use std::ops::Range;

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, MapIntent, Object, PassTracker, QueryKind, ReadSource,
    ReadState, STORAGE_WORD,
};
use wcontainers_shaders::stage::STREAM_BUFFERS;

use crate::context::Context;
use crate::gallium::*;
use crate::state::{refuse, refused};

/// Interface queries in one set. A guest query begun with less than half of a set left takes a
/// new set, so a query may run in this many passes over half before the set is full.
const OCCLUSION_QUERY_BATCH: u32 = 4096;

/// The occlusion sets a context made, the last of them the one queries begin in.
pub struct Sets<B: Backend> {
    sets: Vec<B::QuerySet>,
    /// Interface queries of the last set already taken.
    used: u32,
}

impl<B: Backend> Default for Sets<B> {
    fn default() -> Self {
        Sets {
            sets: Vec::new(),
            used: 0,
        }
    }
}

impl<B: Backend> Sets<B> {
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
            .ok_or_else(|| "no occlusion set".into())
    }
}

/// Where a query's answer is.
enum Answer<B: Backend> {
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

pub struct Query<B: Backend> {
    answer: Option<Answer<B>>,
    /// Paused while st/mesa draws for itself (its blits and clears).
    paused: bool,
    /// For a count of primitives streamed, the count.
    emitted: Option<Emitted<B>>,
}

/// Streamed draws whose counts a count keeps on the device before it reads them back and sums
/// them.
const EMITTED_SLOTS: u64 = 1024;
/// The stage helper's statistics word of stream 0's primitives written.
const WRITTEN_WORD: u64 = STREAM_BUFFERS as u64;

/// Primitives written to stream output while the count ran: each streamed draw's, copied from the
/// stage helper's statistics into a slot, read back and summed when asked for (or when the slots
/// are full).
pub struct Emitted<B: Backend> {
    slots: B::Buffer,
    used: u64,
    sum: u64,
}

impl<B: Backend> Emitted<B> {
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
pub fn count_emitted<B: Backend>(
    counting: Option<*mut pipe_query>,
    backend: &mut B,
    statistics: &B::Buffer,
) -> Result<(), String> {
    let Some(query) = counting else {
        return Ok(());
    };
    // SAFETY: a running count lives until it ends.
    let Some(emitted) = (unsafe { of::<B>(query) }).emitted.as_mut() else {
        return refused(format_args!("a count that is not one"));
    };
    if emitted.used == EMITTED_SLOTS {
        emitted.gather(backend)?;
    }
    backend.copy_buffer(
        statistics,
        WRITTEN_WORD * STORAGE_WORD,
        &emitted.slots,
        emitted.used * STORAGE_WORD,
        STORAGE_WORD,
    );
    emitted.used += 1;
    Ok(())
}

/// `pipe_context::create_query`: occlusion predicates and counts of primitives streamed; an
/// exact sample count needs `OCCLUSION_COUNT` or the counting replay, which is not built.
pub unsafe extern "C" fn create_query<B: Backend>(
    pipe: *mut pipe_context,
    kind: c_uint,
    _: c_uint,
) -> *mut pipe_query {
    let query = |emitted| {
        Box::into_raw(Box::new(Query::<B> {
            answer: None,
            paused: false,
            emitted,
        }))
        .cast()
    };
    match kind as pipe_query_type {
        PIPE_QUERY_OCCLUSION_PREDICATE | PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE => query(None),
        PIPE_QUERY_PRIMITIVES_EMITTED => {
            // SAFETY: Mesa passes this driver's context.
            let context = unsafe { Context::<B>::of(pipe) };
            let slots = context
                .screen()
                .backend()
                .create_buffer(&BufferDesc {
                    size: EMITTED_SLOTS * STORAGE_WORD,
                    usage: BufferUsage::COPY_DST.union(BufferUsage::READ_BACK),
                    intent: MapIntent::Rarely,
                })
                .unwrap_or_else(|e| refuse(format!("a count of primitives: {e:?}")));
            query(Some(Emitted {
                slots,
                used: 0,
                sum: 0,
            }))
        }
        _ => refuse(format!(
            "a query of Gallium kind {kind} (only occlusion predicates are built)"
        )),
    }
}

/// # Safety
/// `query` is one `create_query` made, not yet destroyed.
unsafe fn of<'a, B: Backend>(query: *mut pipe_query) -> &'a mut Query<B> {
    // SAFETY: the caller's.
    unsafe { &mut *query.cast::<Query<B>>() }
}

pub unsafe extern "C" fn destroy_query<B: Backend>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) {
    // SAFETY: Mesa destroys a query it made once.
    let query = unsafe { Box::from_raw(query.cast::<Query<B>>()) };
    if let Some(emitted) = query.emitted {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        context.screen().retire(Object::Buffer(emitted.slots));
    }
}

pub unsafe extern "C" fn begin_query<B: Backend>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) -> bool {
    // SAFETY: Mesa passes this driver's context and a query of it.
    let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
    if let Some(emitted) = &mut query.emitted {
        (emitted.used, emitted.sum) = (0, 0);
        context.counting = Some(std::ptr::from_mut(query).cast());
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
}

pub unsafe extern "C" fn end_query<B: Backend>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) -> bool {
    // SAFETY: as above.
    let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
    if query.emitted.is_some() {
        context.counting = None;
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
    true
}

pub unsafe extern "C" fn get_query_result<B: Backend>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
    wait: bool,
    result: *mut pipe_query_result,
) -> bool {
    // SAFETY: as above; Mesa gives a result to write.
    let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
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
        // SAFETY: as above.
        unsafe { (*result).u64_ = emitted.sum };
        return true;
    }
    let passed = match answer(context, query, wait) {
        Ok(Some(passed)) => passed,
        Ok(None) => return false,
        Err(reason) => refuse(format!("an occlusion query's result: {reason}")),
    };
    // SAFETY: as above.
    unsafe { (*result).b = passed };
    true
}

/// Whether any sample passed, once known (reading back, and waiting if asked to).
fn answer<B: Backend>(
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
/// query takes meanwhile.
pub unsafe extern "C" fn set_active_query_state<B: Backend>(pipe: *mut pipe_context, enable: bool) {
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
}
