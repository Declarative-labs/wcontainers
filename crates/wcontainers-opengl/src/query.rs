//! Occlusion queries: whether any sample passed, from the pass tracker's queries, one an
//! interface query a pass the guest query ran in.

use std::ffi::c_uint;
use std::ops::Range;

use wcontainers_gpu::{Backend, Object, PassTracker, QueryKind, ReadState};

use crate::context::Context;
use crate::gallium::*;
use crate::state::refuse;

/// Interface queries in one set. A guest query begun with less than half of a set left takes a
/// new set, so a query may run in this many passes over half before the set is full.
const QUERIES_PER_SET: u32 = 4096;

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
        if self.sets.is_empty() || self.used >= QUERIES_PER_SET / 2 {
            let set = backend
                .create_query_set(QueryKind::OcclusionAny, QUERIES_PER_SET)
                .map_err(|e| format!("an occlusion set: {e:?}"))?;
            tracker
                .set_occlusion(backend, set.clone(), QUERIES_PER_SET)
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
}

/// `pipe_context::create_query`: occlusion predicates; an exact sample count needs
/// `OCCLUSION_COUNT` or the counting replay, which is not built.
pub unsafe extern "C" fn create_query<B: Backend>(
    _: *mut pipe_context,
    kind: c_uint,
    _: c_uint,
) -> *mut pipe_query {
    match kind as pipe_query_type {
        PIPE_QUERY_OCCLUSION_PREDICATE | PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE => {
            Box::into_raw(Box::new(Query::<B> {
                answer: None,
                paused: false,
            }))
            .cast()
        }
        _ => refuse(&format!(
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

pub unsafe extern "C" fn destroy_query<B: Backend>(_: *mut pipe_context, query: *mut pipe_query) {
    // SAFETY: Mesa destroys a query it made once.
    drop(unsafe { Box::from_raw(query.cast::<Query<B>>()) });
}

pub unsafe extern "C" fn begin_query<B: Backend>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) -> bool {
    // SAFETY: Mesa passes this driver's context and a query of it.
    let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
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
        Err(reason) => refuse(&format!("an occlusion query: {reason}")),
    }
}

pub unsafe extern "C" fn end_query<B: Backend>(
    pipe: *mut pipe_context,
    query: *mut pipe_query,
) -> bool {
    // SAFETY: as above.
    let (context, query) = unsafe { (Context::<B>::of(pipe), of::<B>(query)) };
    let Some(Answer::Running { set, first }) = query.answer.take() else {
        refuse("an occlusion query ended that is not running");
    };
    let mut backend = context.screen().backend();
    let end = if query.paused {
        context.queries.used
    } else {
        match context.tracker.end_query(&mut backend) {
            Ok(range) => range.end,
            Err(error) => refuse(&format!("an occlusion query: {error:?}")),
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
    let passed = match answer(context, query, wait) {
        Ok(Some(passed)) => passed,
        Ok(None) => return false,
        Err(reason) => refuse(&format!("an occlusion query's result: {reason}")),
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
            _ => Err("a query that has not ended".into()),
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
        refuse(&format!("pausing an occlusion query: {error:?}"));
    }
    query.paused = !enable;
}
