//! Who runs next: Linux's Completely Fair Scheduler for one CPU (`kernel/sched/fair.c` as it was
//! before EEVDF), without the parts that spread threads across cores.
//!
//! Each thread's time is weighted by its nice value into a virtual runtime; the runnable thread with
//! the least runs next. A thread that wakes is placed no further back than half a scheduling period
//! behind the least, so it runs almost at once.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::linux::general::{SCHED_BATCH, SCHED_IDLE};
use crate::linux_extra::{MAX_NICE, MIN_NICE};
use crate::process::{Channel, Schedule, Tid};

/// `sched_prio_to_weight`: the weight of nice -20 to 19.
const WEIGHTS: [u64; 40] = [
    88761, 71755, 56483, 46273, 36291, 29154, 23254, 18705, 14949, 11916, 9548, 7620, 6100, 4904,
    3906, 3121, 2501, 1991, 1586, 1277, 1024, 820, 655, 526, 423, 335, 272, 215, 172, 137, 110, 87,
    70, 56, 45, 36, 29, 23, 18, 15,
];

/// The weight of nice 0 (`NICE_0_LOAD`).
const NICE_0: u64 = 1024;
/// `WEIGHT_IDLEPRIO`, for `SCHED_IDLE`.
const IDLE_WEIGHT: u64 = 3;

/// The scheduling period while few threads run (`sysctl_sched_latency`), the least a thread runs
/// (`sysctl_sched_min_granularity`), and how far ahead a woken thread must be to take the CPU
/// (`sysctl_sched_wakeup_granularity`); one CPU, so unscaled.
const LATENCY: u64 = 6_000_000;
const MIN_GRANULARITY: u64 = 750_000;
const WAKEUP_GRANULARITY: u64 = 1_000_000;
/// How many threads fit in one period at the least granularity (`sched_nr_latency`).
const NR_LATENCY: u64 = LATENCY / MIN_GRANULARITY;

/// A thread's weight for its policy and nice value.
pub(crate) fn weight(schedule: &Schedule) -> u64 {
    if schedule.policy == SCHED_IDLE {
        return IDLE_WEIGHT;
    }
    WEIGHTS[(schedule.nice.clamp(MIN_NICE, MAX_NICE) - MIN_NICE) as usize]
}

/// The run queue: runnable threads that are not running, by virtual runtime.
#[derive(Clone, Default)]
pub(crate) struct RunQueue {
    queue: BTreeSet<(u64, u64, Tid)>,
    /// Where each queued thread is in `queue`.
    positions: BTreeMap<Tid, (u64, u64)>,
    /// A number that keeps equal runtimes in the order they came.
    sequence: u64,
    /// `min_vruntime`: never goes backwards.
    pub min_vruntime: u64,
    /// The sum of the queued threads' weights and the running one's.
    pub load: u64,
}

impl RunQueue {
    pub(crate) fn insert(&mut self, tid: Tid, vruntime: u64, weight: u64) {
        debug_assert!(
            !self.positions.contains_key(&tid),
            "thread {tid} queued twice"
        );
        self.sequence += 1;
        self.queue.insert((vruntime, self.sequence, tid));
        self.positions.insert(tid, (vruntime, self.sequence));
        self.load += weight;
    }

    /// Takes a thread out, wherever it is; returns whether it was there.
    pub(crate) fn remove(&mut self, tid: Tid, weight: u64) -> bool {
        let Some((vruntime, sequence)) = self.positions.remove(&tid) else {
            return false;
        };
        self.queue.remove(&(vruntime, sequence, tid));
        self.load -= weight;
        true
    }

    pub(crate) fn contains(&self, tid: Tid) -> bool {
        self.positions.contains_key(&tid)
    }

    /// The thread with the least virtual runtime, taken out of the queue.
    pub(crate) fn pop(&mut self, weight_of: impl Fn(Tid) -> u64) -> Option<Tid> {
        let entry = self.queue.pop_first()?;
        self.positions.remove(&entry.2);
        self.load -= weight_of(entry.2);
        Some(entry.2)
    }

    /// The least virtual runtime queued.
    pub(crate) fn leftmost(&self) -> Option<(u64, Tid)> {
        self.queue.first().map(|entry| (entry.0, entry.2))
    }

    /// The greatest virtual runtime queued.
    pub(crate) fn rightmost(&self) -> Option<u64> {
        self.queue.last().map(|entry| entry.0)
    }

    pub(crate) fn len(&self) -> usize {
        self.queue.len()
    }

    /// `update_min_vruntime`, with `current` the running thread's virtual runtime if one runs.
    pub(crate) fn update_min(&mut self, current: Option<u64>) {
        let least = match (current, self.leftmost()) {
            (Some(current), Some((left, _))) => current.min(left),
            (Some(current), None) => current,
            (None, Some((left, _))) => left,
            (None, None) => return,
        };
        self.min_vruntime = self.min_vruntime.max(least);
    }

    /// Where a thread that wakes is put (`place_entity` with `GENTLE_FAIR_SLEEPERS`): no further
    /// back than half a period behind the least.
    pub(crate) fn place_woken(&self, vruntime: u64) -> u64 {
        vruntime.max(self.min_vruntime.saturating_sub(LATENCY / 2))
    }

    /// Where a new child is put (`place_entity` with `START_DEBIT`): a slice behind the least, so
    /// it does not take the CPU from those already waiting.
    pub(crate) fn place_new(&self, weight: u64, running: usize) -> u64 {
        self.min_vruntime + scale(slice(weight, self.load + weight, running + 1), weight)
    }
}

/// Parked threads by what they wait on and by deadline, so a wake finds its waiters without
/// looking at every thread.
#[derive(Clone, Default)]
pub(crate) struct Parked {
    by_channel: BTreeMap<Channel, BTreeSet<Tid>>,
    deadlines: BTreeSet<(u64, Tid)>,
}

impl Parked {
    pub(crate) fn insert(&mut self, tid: Tid, channel: Channel, deadline: Option<u64>) {
        self.by_channel.entry(channel).or_default().insert(tid);
        if let Some(at) = deadline {
            self.deadlines.insert((at, tid));
        }
    }

    pub(crate) fn remove(&mut self, tid: Tid, channel: Channel, deadline: Option<u64>) {
        if let Some(waiters) = self.by_channel.get_mut(&channel) {
            waiters.remove(&tid);
            if waiters.is_empty() {
                self.by_channel.remove(&channel);
            }
        }
        if let Some(at) = deadline {
            self.deadlines.remove(&(at, tid));
        }
    }

    /// The threads parked on any channel `wanted` accepts, in thread order.
    pub(crate) fn on(&self, wanted: impl Fn(&Channel) -> bool) -> Vec<Tid> {
        let mut found: Vec<Tid> = self
            .by_channel
            .iter()
            .filter(|(channel, _)| wanted(channel))
            .flat_map(|(_, waiters)| waiters.iter().copied())
            .collect();
        found.sort_unstable();
        found
    }

    /// The threads parked on exactly `channel` and on `also`, in thread order.
    pub(crate) fn on_either(&self, channel: Channel, also: Option<Channel>) -> Vec<Tid> {
        let mut found: Vec<Tid> = [Some(channel), also]
            .into_iter()
            .flatten()
            .filter_map(|channel| self.by_channel.get(&channel))
            .flat_map(|waiters| waiters.iter().copied())
            .collect();
        found.sort_unstable();
        found
    }

    pub(crate) fn earliest(&self) -> Option<u64> {
        self.deadlines.first().map(|(at, _)| *at)
    }

    /// The threads whose deadline is at or before `now`, in thread order.
    pub(crate) fn due(&self, now: u64) -> Vec<Tid> {
        let mut found: Vec<Tid> = self
            .deadlines
            .iter()
            .take_while(|(at, _)| *at <= now)
            .map(|(_, tid)| *tid)
            .collect();
        found.sort_unstable();
        found
    }
}

/// The share of a period a thread of `weight` gets among `running` threads of total `load`
/// (`sched_slice`).
pub(crate) fn slice(weight: u64, load: u64, running: usize) -> u64 {
    let running = running as u64;
    let period = if running > NR_LATENCY {
        running * MIN_GRANULARITY
    } else {
        LATENCY
    };
    if load == 0 {
        return period;
    }
    (period as u128 * weight as u128 / load as u128) as u64
}

/// Real time run, as virtual runtime for a thread of `weight` (`calc_delta_fair`).
pub(crate) fn scale(delta: u64, weight: u64) -> u64 {
    if weight == NICE_0 {
        return delta;
    }
    (delta as u128 * NICE_0 as u128 / weight as u128) as u64
}

/// Whether a woken thread takes the CPU from the running one (`check_preempt_wakeup` and
/// `wakeup_preempt_entity`): it must be more than the granularity, in the woken thread's terms,
/// behind; a batch or idle thread never takes it from a normal one.
pub(crate) fn preempts(
    current: u64,
    current_schedule: &Schedule,
    woken: u64,
    woken_schedule: &Schedule,
) -> bool {
    if current_schedule.policy == SCHED_IDLE && woken_schedule.policy != SCHED_IDLE {
        return true;
    }
    if matches!(woken_schedule.policy, SCHED_BATCH | SCHED_IDLE) {
        return false;
    }
    let granularity = scale(WAKEUP_GRANULARITY, weight(woken_schedule));
    current > woken && current - woken > granularity
}
