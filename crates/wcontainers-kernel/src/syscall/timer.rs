//! Timers of every kind on the kernel's one timer queue: interval timers (`setitimer`), POSIX timers
//! (`timer_create`) with every notification, and timer descriptors (`timerfd_create`).
//!
//! A timer on a wall clock waits in the queue by when it fires on the monotonic clock, which is where
//! "wake me at this time" comes from; one on a CPU clock is looked at whenever a thread of its
//! process comes back from running, and the running thread's turn ends when it is due.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::rc::{Rc, Weak};
use core::cell::RefCell;

use super::{Args, guest};
use crate::file::{Anon, FileRef, OpenFile};
use crate::linux::general::*;
use crate::linux_extra::{
    CLOCKFD, CLOCKFD_MASK, CPUCLOCK_CLOCK_MASK, CPUCLOCK_MAX, CPUCLOCK_PERTHREAD_MASK,
};
use crate::process::{Channel, Pid, Queued, Tid};
use crate::{Context, Errno, Kernel};

/// What a timer's time is measured on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Clock {
    /// The monotonic clock, or the wall clock with its times turned into the monotonic clock's
    /// (moved when the wall clock is set, for a timer on the wall clock's base).
    Wall,
    /// A process's CPU time.
    Process(Pid),
    /// A thread's CPU time.
    Thread(Tid),
}

/// Whom a POSIX timer tells when it fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notify {
    None,
    Process { signal: u32, value: u64 },
    Thread { tid: Tid, signal: u32, value: u64 },
}

#[derive(Clone, Debug)]
enum Owner {
    Posix {
        pid: Pid,
        id: i32,
        notify: Notify,
    },
    Interval {
        pid: Pid,
        which: u32,
    },
    Descriptor {
        file: Weak<RefCell<OpenFile>>,
    },
    /// A thread sleeping on a CPU clock (`do_cpu_nanosleep`), with the time its call was given.
    Sleep {
        tid: Tid,
        asked: u64,
    },
    /// The end of a TCP connection's deferral (TCP_DEFER_ACCEPT) on a listener, or a held
    /// connection's client sending again to a listener gone.
    Deferral {
        listener: Weak<RefCell<crate::syscall::socket::Socket>>,
    },
}

#[derive(Clone, Debug)]
struct Timer {
    owner: Owner,
    clock: Clock,
    /// When it next fires on its clock; none while disarmed.
    expires: Option<u64>,
    interval: u64,
    /// Expirations missed since its signal was sent, and those its last signal carried.
    overrun: u64,
    overrun_last: u64,
    /// A descriptor's expirations not yet read, and whether it fired and waits to be rearmed by a
    /// read.
    ticks: u64,
    expired: bool,
    /// The clock id it was made on, for `gettime`'s questions.
    clock_id: i32,
    /// Counts each `settime`; a signal sent before the last one is dropped when taken
    /// (`it_signal_seq`).
    sequence: u64,
    /// The count when its signal now queued was sent.
    sent_at: Option<u64>,
    /// A POSIX timer's place for its signal in its maker's count (its preallocated `sigqueue`).
    #[allow(dead_code, reason = "held for what its drop does")]
    charge: Option<crate::process::Charge>,
    /// On the wall clock's base (`HRTIMER_BASE_REALTIME`): its expiry moves when the wall clock is
    /// set. Set by `settime`.
    wall: bool,
    /// A descriptor set with TFD_TIMER_CANCEL_ON_SET (`might_cancel`), and whether a setting of the
    /// wall clock has canceled it since (`moffs` at `KTIME_MAX`).
    might_cancel: bool,
    canceled: bool,
    /// A descriptor's flags as its last `settime` gave them (`settime_flags`), which fdinfo shows.
    settime_flags: u32,
    /// A periodic POSIX timer whose signal was ignored when it fired, kept until the signal's
    /// handler stops being SIG_IGN (Linux's `ignored_posix_timers`).
    ignored: bool,
}

/// Every timer in the container.
#[derive(Clone, Default)]
pub(crate) struct Timers {
    all: BTreeMap<u64, Timer>,
    last: u64,
    /// Armed wall clock timers by when they fire on the monotonic clock.
    due: BTreeSet<(u64, u64)>,
    /// Armed CPU clock timers.
    cpu: BTreeSet<u64>,
}

impl Timers {
    fn add(&mut self, timer: Timer) -> u64 {
        // Descriptors whose files are gone take their timers with them.
        let dead: alloc::vec::Vec<u64> = self
            .all
            .iter()
            .filter(|(_, timer)| matches!(&timer.owner, Owner::Descriptor { file } if file.strong_count() == 0))
            .map(|(id, _)| *id)
            .collect();
        for id in dead {
            self.remove(id);
        }
        self.last += 1;
        self.all.insert(self.last, timer);
        self.last
    }

    fn remove(&mut self, id: u64) -> Option<Timer> {
        self.disarm(id);
        self.all.remove(&id)
    }

    fn disarm(&mut self, id: u64) {
        let Some(timer) = self.all.get_mut(&id) else {
            return;
        };
        if let Some(at) = timer.expires.take() {
            self.due.remove(&(at, id));
        }
        self.cpu.remove(&id);
    }

    /// Arms a timer at `at` on its clock.
    fn arm(&mut self, id: u64, at: u64) {
        self.disarm(id);
        let timer = self.all.get_mut(&id).expect("timer exists");
        timer.expires = Some(at);
        let silent = matches!(
            timer.owner,
            Owner::Posix {
                notify: Notify::None,
                ..
            }
        );
        // A timer that tells no one never needs to fire (`common_timer_set`).
        if silent {
            return;
        }
        match timer.clock {
            Clock::Wall => {
                self.due.insert((at, id));
            }
            _ => {
                self.cpu.insert(id);
            }
        }
    }

    /// The earliest time a wall clock timer fires.
    pub fn earliest(&self) -> Option<u64> {
        self.due.first().map(|(at, _)| *at)
    }

    fn posix(&self, pid: Pid, timer: i32) -> Option<u64> {
        self.all
            .iter()
            .find(|(_, found)| matches!(found.owner, Owner::Posix { pid: owner, id, .. } if owner == pid && id == timer))
            .map(|(id, _)| *id)
    }

    fn interval(&self, pid: Pid, which: u32) -> Option<u64> {
        self.all
            .iter()
            .find(|(_, found)| matches!(found.owner, Owner::Interval { pid: owner, which: kind } if owner == pid && kind == which))
            .map(|(id, _)| *id)
    }

    fn descriptor(&self, file: &FileRef) -> Option<u64> {
        match &file.borrow().anon {
            Some(Anon::Timer(id)) => Some(*id),
            _ => None,
        }
    }
}

/// A clock's time now: the monotonic clock for wall clock timers, CPU time for the others.
fn now(kernel: &Kernel, clock: Clock) -> Option<u64> {
    match clock {
        Clock::Wall => Some(kernel.entry_now),
        Clock::Process(pid) => kernel
            .processes
            .contains_key(&pid)
            .then(|| super::time::process_cpu_time(kernel, pid)),
        Clock::Thread(tid) => kernel.threads.get(&tid).map(|thread| thread.runtime),
    }
}

/// How many intervals bring a time past `now` (`hrtimer_forward`), and the time they bring it to.
fn forward(expires: u64, now: u64, interval: u64) -> (u64, u64) {
    if interval == 0 || expires > now {
        return (0, expires);
    }
    let count = (now - expires) / interval + 1;
    (count, expires + count * interval)
}

/// A thread came back from running: its process's CPU time limit is looked at, and CPU timers of
/// its process and its own that are due fire.
pub(crate) fn cpu_check(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid) {
    let pid = kernel.threads[&tid].process;
    cpu_limit(kernel, cx, pid);
    if kernel.timers.cpu.is_empty() || !kernel.threads.contains_key(&tid) {
        return;
    }
    let due: alloc::vec::Vec<u64> = kernel
        .timers
        .cpu
        .iter()
        .copied()
        .filter(|id| {
            let timer = &kernel.timers.all[id];
            (timer.clock == Clock::Process(pid) || timer.clock == Clock::Thread(tid))
                && timer
                    .expires
                    .zip(now(kernel, timer.clock))
                    .is_some_and(|(at, now)| at <= now)
        })
        .collect();
    for id in due {
        fire(kernel, cx, id);
    }
}

/// RLIMIT_CPU, as Linux's `check_process_timers`: at the hard limit the process is killed; at the
/// soft limit it is sent SIGXCPU and the soft limit goes up a second.
fn cpu_limit(kernel: &mut Kernel, cx: &mut Context<'_>, pid: Pid) {
    let Some((soft, hard)) = cpu_limit_times(kernel, pid) else {
        return;
    };
    let used = super::time::process_cpu_time(kernel, pid);
    if hard.is_some_and(|hard| used >= hard) {
        kernel.send_to_process(cx, pid, SIGKILL, crate::signal::kernel_info(SIGKILL));
    } else if used >= soft {
        let limit =
            &mut kernel.processes.get_mut(&pid).expect("process").limits[RLIMIT_CPU as usize];
        limit.current += 1;
        kernel.send_to_process(cx, pid, SIGXCPU, crate::signal::kernel_info(SIGXCPU));
    }
}

/// A process's RLIMIT_CPU soft and hard limits in nanoseconds of CPU time, if a soft one is set.
fn cpu_limit_times(kernel: &Kernel, pid: Pid) -> Option<(u64, Option<u64>)> {
    let limit = kernel.processes[&pid].limits[RLIMIT_CPU as usize];
    let infinite = RLIM_INFINITY as u64;
    let nanoseconds = |seconds: u64| seconds.saturating_mul(1_000_000_000);
    (limit.current != infinite).then(|| {
        (
            nanoseconds(limit.current),
            (limit.maximum != infinite).then(|| nanoseconds(limit.maximum)),
        )
    })
}

/// How long the thread may run before its process reaches its CPU time limit or a CPU timer of its
/// own or its process's is due.
pub(crate) fn cpu_slice(kernel: &Kernel, tid: Tid) -> Option<u64> {
    let pid = kernel.threads[&tid].process;
    let limit = cpu_limit_times(kernel, pid).map(|(soft, hard)| {
        soft.min(hard.unwrap_or(u64::MAX))
            .saturating_sub(super::time::process_cpu_time(kernel, pid))
            .max(1)
    });
    if kernel.timers.cpu.is_empty() {
        return limit;
    }
    kernel
        .timers
        .cpu
        .iter()
        .filter_map(|id| {
            let timer = &kernel.timers.all[id];
            if timer.clock != Clock::Process(pid) && timer.clock != Clock::Thread(tid) {
                return None;
            }
            Some(
                timer
                    .expires?
                    .saturating_sub(now(kernel, timer.clock)?)
                    .max(1),
            )
        })
        .chain(limit)
        .min()
}

/// Fires every wall clock timer due by the time the kernel was entered.
pub(crate) fn fire_due(kernel: &mut Kernel, cx: &mut Context<'_>) {
    while let Some(&(at, id)) = kernel.timers.due.first() {
        if at > kernel.entry_now {
            break;
        }
        fire(kernel, cx, id);
    }
}

/// The information of a POSIX timer's signal (SI_TIMER).
fn timer_info(signal: u32, id: i32, overrun: u64, value: u64) -> [u8; 128] {
    let mut info = crate::signal::kernel_info(signal);
    info[8..12].copy_from_slice(&SI_TIMER.to_le_bytes());
    info[16..20].copy_from_slice(&id.to_le_bytes());
    info[20..24].copy_from_slice(&(overrun.min(i32::MAX as u64) as i32).to_le_bytes());
    info[24..32].copy_from_slice(&value.to_le_bytes());
    info
}

/// Whether a POSIX timer's signal is still queued, not yet taken.
fn signal_queued(kernel: &Kernel, pid: Pid, id: i32, notify: Notify) -> bool {
    let is_ours = |queued: &Queued| {
        queued.timer
            && i32::from_le_bytes(queued.info[16..20].try_into().expect("four bytes")) == id
    };
    let pending = match notify {
        Notify::Thread { tid, .. } => kernel
            .threads
            .get(&tid)
            .map(|thread| &thread.pending)
            .or_else(|| {
                let process = kernel.processes.get(&pid)?;
                process.away.get(&tid).map(|away| &away.pending)
            }),
        _ => kernel.processes.get(&pid).map(|process| &process.pending),
    };
    pending.is_some_and(|pending| pending.queue.iter().any(is_ours))
}

/// A timer's time has come.
fn fire(kernel: &mut Kernel, cx: &mut Context<'_>, id: u64) {
    let timer = kernel.timers.all[&id].clone();
    let Some(expires) = timer.expires else {
        return;
    };
    let clock_now = now(kernel, timer.clock).unwrap_or(expires);
    kernel.timers.disarm(id);
    match timer.owner {
        Owner::Posix {
            pid,
            id: number,
            notify,
        } => {
            // A periodic one is rearmed when its signal is taken (`posixtimer_deliver_signal`)
            // and waits until then; a one-shot one is done (`posix_timer_queue_signal`).
            let periodic = timer.interval > 0;
            kernel.timers.all.get_mut(&id).expect("timer").expires = periodic.then_some(expires);
            let (target, signal, value) = match notify {
                Notify::None => return,
                Notify::Process { signal, value } => (None, signal, value),
                Notify::Thread { tid, signal, value } => (Some(tid), signal, value),
            };
            if signal_queued(kernel, pid, number, notify) {
                kernel.timers.all.get_mut(&id).expect("timer").overrun += 1;
                return;
            }
            let entry = kernel.timers.all.get_mut(&id).expect("timer");
            entry.sent_at = Some(entry.sequence);
            let live = match target {
                Some(tid) => kernel.task_of(pid, tid),
                None => kernel
                    .processes
                    .get(&pid)
                    .is_some_and(|p| p.ended.is_none()),
            };
            if !live {
                return;
            }
            // An ignored signal leaves a periodic timer waiting for it to be caught again, and a
            // one-shot one drops it (`posixtimer_send_sigqueue`).
            let ignored = kernel.timer_signal_ignored(target, pid, signal);
            kernel.timers.all.get_mut(&id).expect("timer").ignored = ignored && periodic;
            if !ignored {
                let info = timer_info(signal, number, 0, value);
                kernel.send_from_timer(cx, target, pid, signal, info);
            }
        }
        Owner::Interval { pid, which } => {
            let signal = match which {
                ITIMER_REAL => SIGALRM,
                ITIMER_VIRTUAL => SIGVTALRM,
                _ => SIGPROF,
            };
            if which == ITIMER_REAL {
                // Rearmed when SIGALRM is taken (`dequeue_signal`).
                kernel.timers.all.get_mut(&id).expect("timer").expires = Some(expires);
            } else if timer.interval > 0 {
                kernel.timers.arm(id, expires + timer.interval);
            }
            if kernel
                .processes
                .get(&pid)
                .is_some_and(|p| p.ended.is_none())
            {
                kernel.send_to_process(cx, pid, signal, crate::signal::kernel_info(signal));
            }
        }
        Owner::Descriptor { file } => {
            // Rearmed by the read that takes its ticks (`timerfd_read_iter`).
            let timer = kernel.timers.all.get_mut(&id).expect("timer");
            timer.expires = Some(expires);
            timer.ticks += 1;
            timer.expired = true;
            let _ = clock_now;
            if let Some(file) = file.upgrade() {
                kernel.wake(Channel::Object(Rc::as_ptr(&file) as *const () as usize));
            }
        }
        // Disarmed, which tells the sleeper its time has come.
        Owner::Sleep { tid, .. } => kernel.wake_thread(tid),
        Owner::Deferral { listener } => {
            kernel.timers.remove(id);
            if let Some(listener) = listener.upgrade() {
                crate::syscall::socket::deferrals_due(kernel, cx, &listener);
            }
        }
    }
}

/// `signal` of `pid` stopped being SIG_IGN: the periodic timers that fired while it was ignored
/// send it now (`posixtimer_sig_unignore`).
pub(crate) fn unignore(kernel: &mut Kernel, cx: &mut Context<'_>, pid: Pid, signal: u32) {
    let waiting: alloc::vec::Vec<(u64, i32, Notify)> = kernel
        .timers
        .all
        .iter()
        .filter_map(|(id, timer)| match timer.owner {
            Owner::Posix {
                pid: owner,
                id: number,
                notify,
            } if owner == pid && timer.ignored => Some((*id, number, notify)),
            _ => None,
        })
        .collect();
    for (id, number, notify) in waiting {
        let (target, sends, value) = match notify {
            Notify::None => continue,
            Notify::Process { signal, value } => (None, signal, value),
            Notify::Thread { tid, signal, value } => (Some(tid), signal, value),
        };
        if sends != signal {
            continue;
        }
        kernel.timers.all.get_mut(&id).expect("timer").ignored = false;
        if target.is_none_or(|tid| kernel.task_of(pid, tid)) {
            let info = timer_info(signal, number, 0, value);
            kernel.send_from_timer(cx, target, pid, signal, info);
        }
    }
}

/// A signal was taken: a POSIX timer's carries the expirations it missed and the timer is rearmed;
/// SIGALRM rearms the interval timer that sends it. Returns false for a POSIX timer's signal that
/// is dropped instead, its timer gone or set again since it was sent.
pub(crate) fn taken(kernel: &mut Kernel, pid: Pid, queued: &mut Queued) -> bool {
    let code = i32::from_le_bytes(queued.info[8..12].try_into().expect("four bytes"));
    let found = if queued.timer {
        let number = i32::from_le_bytes(queued.info[16..20].try_into().expect("four bytes"));
        let Some(id) = kernel.timers.posix(pid, number) else {
            return false;
        };
        let timer = kernel.timers.all.get_mut(&id).expect("timer");
        if timer.sent_at.take() != Some(timer.sequence) {
            return false;
        }
        Some(id)
    } else if queued.signal == SIGALRM {
        kernel.timers.interval(pid, ITIMER_REAL)
    } else {
        None
    };
    let Some(id) = found else {
        return true;
    };
    let timer = kernel.timers.all[&id].clone();
    let (Some(expires), Some(clock_now)) = (timer.expires, now(kernel, timer.clock)) else {
        return true;
    };
    let queued_by_timer = matches!(&timer.owner, Owner::Interval { .. })
        && (kernel.timers.due.contains(&(expires, id)) || kernel.timers.cpu.contains(&id));
    if queued_by_timer {
        return true;
    }
    let (count, next) = forward(expires, clock_now, timer.interval);
    if code == SI_TIMER {
        let overrun = timer.overrun + count.saturating_sub(1);
        queued.info[20..24].copy_from_slice(&(overrun.min(i32::MAX as u64) as i32).to_le_bytes());
        let entry = kernel.timers.all.get_mut(&id).expect("timer");
        entry.overrun_last = overrun;
        entry.overrun = 0;
    }
    if timer.interval > 0 {
        kernel.timers.arm(id, next);
    } else {
        kernel.timers.all.get_mut(&id).expect("timer").expires = None;
    }
    true
}

/// A process ended: its timers go. With `exec`, only its POSIX timers.
pub(crate) fn process_end(kernel: &mut Kernel, pid: Pid, exec: bool) {
    let ids: alloc::vec::Vec<u64> = kernel
        .timers
        .all
        .iter()
        .filter(|(_, timer)| match timer.owner {
            Owner::Posix { pid: owner, .. } => owner == pid,
            Owner::Interval { pid: owner, .. } => owner == pid && !exec,
            Owner::Descriptor { .. } | Owner::Sleep { .. } | Owner::Deferral { .. } => false,
        })
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        kernel.timers.remove(id);
    }
}

/// Starts the timer of a sleep on a CPU clock, which wakes `tid` when the clock reaches `until`;
/// `asked` is the time the call was given.
pub(super) fn start_cpu_sleep(
    kernel: &mut Kernel,
    tid: Tid,
    clock: Clock,
    until: u64,
    asked: u64,
) -> u64 {
    let id = kernel.timers.add(Timer {
        owner: Owner::Sleep { tid, asked },
        clock,
        expires: None,
        interval: 0,
        overrun: 0,
        overrun_last: 0,
        ticks: 0,
        expired: false,
        clock_id: 0,
        sequence: 0,
        sent_at: None,
        charge: None,
        wall: false,
        might_cancel: false,
        settime_flags: 0,
        canceled: false,
        ignored: false,
    });
    kernel.timers.arm(id, until);
    id
}

/// Starts a timer that ends a listener's connection deferral at `until` on the monotonic clock
/// (or, on a client, sends again what its gone listener dropped).
pub(crate) fn start_deferral(
    kernel: &mut Kernel,
    listener: Weak<RefCell<crate::syscall::socket::Socket>>,
    until: u64,
) {
    let id = kernel.timers.add(Timer {
        owner: Owner::Deferral { listener },
        clock: Clock::Wall,
        expires: None,
        interval: 0,
        overrun: 0,
        overrun_last: 0,
        ticks: 0,
        expired: false,
        clock_id: CLOCK_MONOTONIC as i32,
        sequence: 0,
        sent_at: None,
        charge: None,
        wall: false,
        might_cancel: false,
        settime_flags: 0,
        canceled: false,
        ignored: false,
    });
    kernel.timers.arm(id, until);
}

/// What is left of a sleep on a CPU clock: 0 once its timer fired or its clock reached the time; the
/// time the call was given when the clock's task is gone (`posix_cpu_timer_set` fails and leaves it).
pub(super) fn cpu_sleep_left(kernel: &Kernel, id: u64) -> u64 {
    let timer = &kernel.timers.all[&id];
    let Some(expires) = timer.expires else {
        return 0;
    };
    match now(kernel, timer.clock) {
        Some(clock_now) => expires.saturating_sub(clock_now),
        None => match timer.owner {
            Owner::Sleep { asked, .. } => asked,
            _ => 0,
        },
    }
}

/// When a sleep on a CPU clock ends on its clock; none once its timer fired.
pub(super) fn cpu_sleep_until(kernel: &Kernel, id: u64) -> Option<u64> {
    kernel.timers.all[&id].expires
}

/// A CPU clock's time now; none when its task is gone.
pub(super) fn cpu_clock_now(kernel: &Kernel, clock: Clock) -> Option<u64> {
    now(kernel, clock)
}

/// Ends a sleep on a CPU clock: its timer goes.
pub(super) fn end_cpu_sleep(kernel: &mut Kernel, id: u64) {
    kernel.timers.remove(id);
}

/// A thread ended: the timer of a sleep it was in goes.
pub(crate) fn thread_end(kernel: &mut Kernel, tid: Tid) {
    let sleeping = kernel
        .timers
        .all
        .iter()
        .find(|(_, timer)| matches!(timer.owner, Owner::Sleep { tid: owner, .. } if owner == tid))
        .map(|(id, _)| *id);
    if let Some(id) = sleeping {
        kernel.timers.remove(id);
    }
}

/// A time as a `__kernel_timespec`.
fn timespec(nanoseconds: u64) -> __kernel_timespec {
    __kernel_timespec {
        tv_sec: (nanoseconds / 1_000_000_000) as i64,
        tv_nsec: (nanoseconds % 1_000_000_000) as i64,
    }
}

fn checked_nanoseconds(seconds: i64, nanoseconds: i64) -> Result<u64, Errno> {
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        return Err(Errno::EINVAL);
    }
    Ok((seconds as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(nanoseconds as u64))
}

fn write_itimerspec(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    at: u64,
    (value, interval): (u64, u64),
) -> Result<(), Errno> {
    let out = __kernel_itimerspec {
        it_interval: timespec(interval),
        it_value: timespec(value),
    };
    guest(kernel, cx, tid).write_value(at, &out)
}

/// Turns a time given to `settime` into the timer's clock: relative or absolute, the wall clock's
/// absolute times into the monotonic clock's.
fn expiry(kernel: &Kernel, cx: &mut Context<'_>, timer: &Timer, value: u64, absolute: bool) -> u64 {
    let clock_now = now(kernel, timer.clock).unwrap_or(0);
    if !absolute {
        return clock_now.saturating_add(value);
    }
    match (timer.clock, timer.clock_id as u32) {
        (Clock::Wall, CLOCK_REALTIME | CLOCK_REALTIME_ALARM | CLOCK_TAI) => {
            clock_now.saturating_add(value.saturating_sub(cx.host.realtime_nanoseconds()))
        }
        _ => value,
    }
}

/// What is left of a timer and its interval, as `gettime` reports them; a periodic one that has
/// fired and waits to be rearmed reports its next expiry, a one-shot one that has fired nothing.
fn remaining(kernel: &Kernel, id: u64) -> (u64, u64) {
    let timer = &kernel.timers.all[&id];
    let Some(expires) = timer.expires else {
        return (0, timer.interval);
    };
    let clock_now = now(kernel, timer.clock).unwrap_or(expires);
    let (_, expires) = forward(expires, clock_now, timer.interval);
    let left = expires.saturating_sub(clock_now);
    let silent = matches!(
        timer.owner,
        Owner::Posix {
            notify: Notify::None,
            ..
        }
    );
    if left == 0 && timer.interval == 0 {
        // Due but not yet taken: Linux shows a nanosecond, except for one that tells no one.
        return (u64::from(!silent), 0);
    }
    (left, timer.interval)
}

/// `clockid_to_kclock` and each clock's `timer_create`: the clock a POSIX timer runs on.
fn timer_clock(kernel: &Kernel, tid: Tid, id: i32) -> Result<Clock, Errno> {
    if id < 0 {
        if id & CLOCKFD_MASK == CLOCKFD {
            return Err(Errno::EOPNOTSUPP);
        }
        return Ok(Clock::Process(0));
    }
    match id as u32 {
        CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_BOOTTIME | CLOCK_TAI | CLOCK_REALTIME_ALARM
        | CLOCK_BOOTTIME_ALARM => Ok(Clock::Wall),
        CLOCK_PROCESS_CPUTIME_ID => Ok(Clock::Process(kernel.threads[&tid].process)),
        CLOCK_THREAD_CPUTIME_ID => Ok(Clock::Thread(tid)),
        CLOCK_MONOTONIC_RAW | CLOCK_REALTIME_COARSE | CLOCK_MONOTONIC_COARSE => {
            Err(Errno::EOPNOTSUPP)
        }
        _ => Err(Errno::EINVAL),
    }
}

/// `posix_cpu_timer_create`'s `pid_for_clock`: whose CPU time a clock id below zero names.
fn cpu_clock(kernel: &Kernel, tid: Tid, id: i32) -> Option<Clock> {
    if id & CPUCLOCK_CLOCK_MASK >= CPUCLOCK_MAX {
        return None;
    }
    let pid = !(id >> 3);
    let process = kernel.threads[&tid].process;
    if id & CPUCLOCK_PERTHREAD_MASK != 0 {
        let thread = if pid == 0 { tid } else { pid };
        return kernel
            .threads
            .get(&thread)
            .filter(|found| found.process == process)
            .map(|_| Clock::Thread(thread));
    }
    if pid == 0 {
        return Some(Clock::Process(process));
    }
    kernel
        .processes
        .get(&pid)
        .filter(|found| found.ended.is_none())
        .map(|_| Clock::Process(pid))
}

/// `timer_create(clockid, sevp, timerid)`.
pub(super) fn timer_create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [clock_id, event, out, ..]: Args,
) -> Result<u64, Errno> {
    let clock_id = clock_id as i32;
    let mut raw = [0u8; size_of::<sigevent>()];
    if event != 0 {
        let bytes = guest(kernel, cx, tid).read(event, raw.len() as u64)?;
        raw.copy_from_slice(&bytes);
    }
    let mut clock = timer_clock(kernel, tid, clock_id)?;
    let charge = kernel.timer_charge(tid).ok_or(Errno::EAGAIN)?;
    // The number is taken, and the next one counts on, even if the rest fails.
    let pid = kernel.threads[&tid].process;
    let number = {
        let process = kernel.processes.get_mut(&pid).expect("process exists");
        let number = process.next_timer;
        process.next_timer = (number + 1) & i32::MAX;
        number
    };
    let notify = if event == 0 {
        Notify::Process {
            signal: SIGALRM,
            value: number as u64,
        }
    } else {
        let field = |offset: usize| {
            i32::from_le_bytes(raw[offset..offset + 4].try_into().expect("four bytes"))
        };
        let value = u64::from_le_bytes(raw[0..8].try_into().expect("eight bytes"));
        let signal = field(core::mem::offset_of!(sigevent, sigev_signo));
        let how = field(core::mem::offset_of!(sigevent, sigev_notify)) as u32;
        let thread = field(core::mem::offset_of!(sigevent, _sigev_un));
        let valid_signal = signal > 0 && signal as u32 <= crate::process::SIGNALS as u32;
        // `good_sigevent`.
        match how {
            SIGEV_NONE => Notify::None,
            SIGEV_SIGNAL | SIGEV_THREAD if valid_signal => Notify::Process {
                signal: signal as u32,
                value,
            },
            _ if how == SIGEV_SIGNAL | SIGEV_THREAD_ID
                && valid_signal
                && kernel
                    .threads
                    .get(&thread)
                    .is_some_and(|found| found.process == pid) =>
            {
                Notify::Thread {
                    tid: thread,
                    signal: signal as u32,
                    value,
                }
            }
            _ => return Err(Errno::EINVAL),
        }
    };
    guest(kernel, cx, tid).write_value(out, &number)?;
    if clock == Clock::Process(0) {
        clock = cpu_clock(kernel, tid, clock_id).ok_or(Errno::EINVAL)?;
    }
    if matches!(clock_id as u32, CLOCK_REALTIME_ALARM | CLOCK_BOOTTIME_ALARM)
        && !kernel.credentials(tid).has(CAP_WAKE_ALARM)
    {
        return Err(Errno::EPERM);
    }
    kernel.timers.add(Timer {
        owner: Owner::Posix {
            pid,
            id: number,
            notify,
        },
        clock,
        expires: None,
        interval: 0,
        overrun: 0,
        overrun_last: 0,
        ticks: 0,
        expired: false,
        sequence: 0,
        sent_at: None,
        charge: Some(charge),
        wall: false,
        might_cancel: false,
        settime_flags: 0,
        canceled: false,
        ignored: false,
        clock_id,
    });
    Ok(0)
}

fn posix_timer(kernel: &Kernel, tid: Tid, number: u64) -> Result<u64, Errno> {
    kernel
        .timers
        .posix(kernel.threads[&tid].process, number as i32)
        .filter(|_| number <= i32::MAX as u64)
        .ok_or(Errno::EINVAL)
}

/// `timer_settime(timerid, flags, new, old)`.
pub(super) fn timer_settime(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [number, flags, new, old, ..]: Args,
) -> Result<u64, Errno> {
    if new == 0 {
        return Err(Errno::EINVAL);
    }
    let value: __kernel_itimerspec = guest(kernel, cx, tid).read_value(new)?;
    let value_ns = checked_nanoseconds(value.it_value.tv_sec, value.it_value.tv_nsec)?;
    let interval = checked_nanoseconds(value.it_interval.tv_sec, value.it_interval.tv_nsec)?;
    let id = posix_timer(kernel, tid, number)?;
    let before = remaining(kernel, id);
    set(
        kernel,
        cx,
        id,
        value_ns,
        interval,
        flags as u32 & TIMER_ABSTIME != 0,
    );
    if old != 0 {
        write_itimerspec(kernel, cx, tid, old, before)?;
    }
    Ok(0)
}

/// Arms or disarms a timer and starts its overrun count again (`posix_timer_set_common`).
fn set(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    id: u64,
    value: u64,
    interval: u64,
    absolute: bool,
) {
    kernel.timers.disarm(id);
    let timer = kernel.timers.all.get_mut(&id).expect("timer");
    timer.interval = if value == 0 { 0 } else { interval };
    timer.overrun = 0;
    timer.overrun_last = 0;
    timer.sequence += 1;
    timer.ticks = 0;
    timer.expired = false;
    // A descriptor's timer on the wall clock stays on its base, a POSIX timer only when its time
    // is absolute (`common_hrtimer_arm`), and one on TAI always.
    timer.wall = match (&timer.owner, timer.clock_id as u32) {
        (Owner::Descriptor { .. }, CLOCK_REALTIME | CLOCK_REALTIME_ALARM) => true,
        (Owner::Posix { .. }, CLOCK_REALTIME | CLOCK_REALTIME_ALARM) => absolute,
        (Owner::Posix { .. }, CLOCK_TAI) => true,
        _ => false,
    };
    if value == 0 {
        return;
    }
    let timer = kernel.timers.all[&id].clone();
    let at = expiry(kernel, cx, &timer, value, absolute);
    kernel.timers.arm(id, at);
}

pub(super) fn timer_gettime(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [number, out, ..]: Args,
) -> Result<u64, Errno> {
    let id = posix_timer(kernel, tid, number)?;
    let left = remaining(kernel, id);
    write_itimerspec(kernel, cx, tid, out, left)?;
    Ok(0)
}

pub(super) fn timer_getoverrun(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [number, ..]: Args,
) -> Result<u64, Errno> {
    let id = posix_timer(kernel, tid, number)?;
    Ok(kernel.timers.all[&id].overrun_last.min(i32::MAX as u64))
}

/// `timer_delete`: the timer goes; a signal of it not yet taken stays pending and is dropped when
/// taken.
pub(super) fn timer_delete(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [number, ..]: Args,
) -> Result<u64, Errno> {
    let id = posix_timer(kernel, tid, number)?;
    kernel.timers.remove(id);
    Ok(0)
}

/// An interval timer's value and interval, as `getitimer` rounds them to microseconds.
fn interval_value(kernel: &Kernel, pid: Pid, which: u32) -> (u64, u64) {
    let Some(id) = kernel.timers.interval(pid, which) else {
        return (0, 0);
    };
    let timer = &kernel.timers.all[&id];
    let Some(expires) = timer.expires else {
        return (0, timer.interval);
    };
    // The real one fired and waiting for SIGALRM to be taken is not running (`hrtimer_active`).
    if !kernel.timers.due.contains(&(expires, id)) && !kernel.timers.cpu.contains(&id) {
        return (0, timer.interval);
    }
    let clock_now = now(kernel, timer.clock).unwrap_or(expires);
    // Due and not yet sent shows a little time left: a microsecond for the real one, a tick for
    // the CPU ones (`itimer_get_remtime`, `get_cpu_itimer`).
    let left = match expires.checked_sub(clock_now).filter(|left| *left > 0) {
        Some(left) => left,
        None if which == ITIMER_REAL => 1000,
        None => 4_000_000,
    };
    (left, timer.interval)
}

fn write_itimerval(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    at: u64,
    (value, interval): (u64, u64),
) -> Result<(), Errno> {
    let timeval = |nanoseconds: u64| __kernel_old_timeval {
        tv_sec: (nanoseconds / 1_000_000_000) as i64,
        tv_usec: (nanoseconds % 1_000_000_000 / 1000) as i64,
    };
    let out = __kernel_old_itimerval {
        it_interval: timeval(interval),
        it_value: timeval(value),
    };
    guest(kernel, cx, tid).write_value(at, &out)
}

pub(super) fn getitimer(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [which, out, ..]: Args,
) -> Result<u64, Errno> {
    let which = which as u32;
    if which > ITIMER_PROF {
        return Err(Errno::EINVAL);
    }
    let value = interval_value(kernel, kernel.threads[&tid].process, which);
    write_itimerval(kernel, cx, tid, out, value)?;
    Ok(0)
}

/// `setitimer(which, new, old)`: no new value disarms it.
pub(super) fn setitimer(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [which, new, old, ..]: Args,
) -> Result<u64, Errno> {
    let (mut value, mut interval) = (0, 0);
    if new != 0 {
        let given: __kernel_old_itimerval = guest(kernel, cx, tid).read_value(new)?;
        let micro = |time: __kernel_old_timeval| {
            if time.tv_sec < 0 || !(0..1_000_000).contains(&time.tv_usec) {
                return Err(Errno::EINVAL);
            }
            Ok((time.tv_sec as u64)
                .saturating_mul(1_000_000_000)
                .saturating_add(time.tv_usec as u64 * 1000))
        };
        value = micro(given.it_value)?;
        interval = micro(given.it_interval)?;
    }
    let which = which as u32;
    if which > ITIMER_PROF {
        return Err(Errno::EINVAL);
    }
    let pid = kernel.threads[&tid].process;
    let before = interval_value(kernel, pid, which);
    let id = match kernel.timers.interval(pid, which) {
        Some(id) => id,
        None => kernel.timers.add(Timer {
            owner: Owner::Interval { pid, which },
            clock: if which == ITIMER_REAL {
                Clock::Wall
            } else {
                Clock::Process(pid)
            },
            expires: None,
            interval: 0,
            overrun: 0,
            overrun_last: 0,
            ticks: 0,
            expired: false,
            sequence: 0,
            sent_at: None,
            charge: None,
            clock_id: CLOCK_MONOTONIC as i32,
            wall: false,
            might_cancel: false,
            settime_flags: 0,
            canceled: false,
            ignored: false,
        }),
    };
    set(kernel, cx, id, value, interval, false);
    if old != 0 {
        write_itimerval(kernel, cx, tid, old, before)?;
    }
    Ok(0)
}

/// What `/proc/<pid>/fd` shows for a timer descriptor.
const LINK: &[u8] = b"anon_inode:[timerfd]";

pub(super) fn timerfd_create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [clock_id, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (clock_id, flags) = (clock_id as i32, flags as u32);
    let known = [
        CLOCK_MONOTONIC,
        CLOCK_REALTIME,
        CLOCK_REALTIME_ALARM,
        CLOCK_BOOTTIME,
        CLOCK_BOOTTIME_ALARM,
    ];
    if flags & !TFD_CREATE_FLAGS != 0 || clock_id < 0 || !known.contains(&(clock_id as u32)) {
        return Err(Errno::EINVAL);
    }
    let alarm = matches!(clock_id as u32, CLOCK_REALTIME_ALARM | CLOCK_BOOTTIME_ALARM);
    if alarm && !kernel.credentials(tid).has(CAP_WAKE_ALARM) {
        return Err(Errno::EPERM);
    }
    let node = super::anon_node(kernel, cx);
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(node, O_RDWR | (flags & O_NONBLOCK), LINK.to_vec(), opener);
    let id = kernel.timers.add(Timer {
        owner: Owner::Descriptor {
            file: Rc::downgrade(&file),
        },
        clock: Clock::Wall,
        expires: None,
        interval: 0,
        overrun: 0,
        overrun_last: 0,
        ticks: 0,
        expired: false,
        sequence: 0,
        sent_at: None,
        charge: None,
        wall: false,
        might_cancel: false,
        settime_flags: 0,
        canceled: false,
        ignored: false,
        clock_id,
    });
    file.borrow_mut().anon = Some(Anon::Timer(id));
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let added = kernel
        .files_mut(tid)
        .add(file, flags & TFD_CLOEXEC != 0, 0, limit);
    match added {
        Ok(fd) => Ok(fd as u64),
        Err(error) => {
            kernel.timers.remove(id);
            Err(error)
        }
    }
}

fn timer_descriptor(kernel: &Kernel, tid: Tid, fd: u64) -> Result<u64, Errno> {
    let file = super::io::open_file(kernel, tid, fd)?;
    kernel.timers.descriptor(&file).ok_or(Errno::EINVAL)
}

/// A descriptor's timer that fired and has an interval moves on past now, counting what it missed
/// into its ticks (`timerfd_read_iter`, `timerfd_gettime`).
fn catch_up(kernel: &mut Kernel, id: u64) {
    let timer = &kernel.timers.all[&id];
    if !timer.expired || timer.interval == 0 {
        return;
    }
    let (Some(expires), Some(clock_now)) = (timer.expires, now(kernel, timer.clock)) else {
        return;
    };
    let (count, next) = forward(expires, clock_now, timer.interval);
    kernel.timers.arm(id, next);
    let timer = kernel.timers.all.get_mut(&id).expect("timer");
    timer.ticks += count.saturating_sub(1);
    timer.expired = false;
}

pub(super) fn timerfd_settime(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, flags, new, old, ..]: Args,
) -> Result<u64, Errno> {
    let value: __kernel_itimerspec = guest(kernel, cx, tid).read_value(new)?;
    let flags = flags as u32;
    if flags & !(TFD_SETTIME_FLAGS | TFD_TIMER_CANCEL_ON_SET) != 0 {
        return Err(Errno::EINVAL);
    }
    let value_ns = checked_nanoseconds(value.it_value.tv_sec, value.it_value.tv_nsec)?;
    let interval = checked_nanoseconds(value.it_interval.tv_sec, value.it_interval.tv_nsec)?;
    let id = timer_descriptor(kernel, tid, fd)?;
    let alarm = matches!(
        kernel.timers.all[&id].clock_id as u32,
        CLOCK_REALTIME_ALARM | CLOCK_BOOTTIME_ALARM
    );
    if alarm && !kernel.credentials(tid).has(CAP_WAKE_ALARM) {
        return Err(Errno::EPERM);
    }
    // `timerfd_setup_cancel`: an absolute time on the wall clock may be canceled by its setting.
    let wall_clock = matches!(
        kernel.timers.all[&id].clock_id as u32,
        CLOCK_REALTIME | CLOCK_REALTIME_ALARM
    );
    let timer = kernel.timers.all.get_mut(&id).expect("timer");
    timer.might_cancel =
        wall_clock && flags & TFD_TIMER_ABSTIME != 0 && flags & TFD_TIMER_CANCEL_ON_SET != 0;
    timer.settime_flags = flags & (TFD_SETTIME_FLAGS | TFD_TIMER_CANCEL_ON_SET);
    if kernel.timers.all[&id].expired && kernel.timers.all[&id].interval > 0 {
        let timer = &kernel.timers.all[&id];
        let (expires, clock_now) = (timer.expires.unwrap_or(0), kernel.entry_now);
        let (_, next) = forward(expires, clock_now, timer.interval);
        kernel.timers.all.get_mut(&id).expect("timer").expires = Some(next);
    }
    let before = descriptor_remaining(kernel, id);
    set(
        kernel,
        cx,
        id,
        value_ns,
        interval,
        flags & TFD_TIMER_ABSTIME != 0,
    );
    if old != 0 {
        write_itimerspec(kernel, cx, tid, old, before)?;
    }
    // An armed timer whose clock was set since is told so, and stays armed.
    if value_ns != 0 && canceled(kernel, id) {
        return Err(Errno::ECANCELED);
    }
    Ok(0)
}

/// `timerfd_canceled`: whether a setting of the wall clock canceled the descriptor since it last
/// said so; saying so clears it.
fn canceled(kernel: &mut Kernel, id: u64) -> bool {
    let timer = kernel.timers.all.get_mut(&id).expect("timer");
    if !timer.might_cancel || !timer.canceled {
        return false;
    }
    timer.canceled = false;
    true
}

/// `clock_was_set`: the wall clock moved by `delta` nanoseconds against the monotonic clock.
/// Timers on its base move with it (and fire if their time has come), and each descriptor that may
/// be canceled is, and becomes readable (`timerfd_clock_was_set`).
pub(crate) fn clock_was_set(kernel: &mut Kernel, delta: i64) {
    let ids: alloc::vec::Vec<u64> = kernel.timers.all.keys().copied().collect();
    for id in ids {
        let timer = &kernel.timers.all[&id];
        if timer.wall
            && let Some(expires) = timer.expires
        {
            let moved = (i128::from(expires) - i128::from(delta)).clamp(0, i128::from(u64::MAX));
            kernel.timers.arm(id, moved as u64);
        }
        let timer = kernel.timers.all.get_mut(&id).expect("timer");
        if !timer.might_cancel {
            continue;
        }
        timer.canceled = true;
        timer.ticks += 1;
        if let Owner::Descriptor { file } = &timer.owner
            && let Some(file) = file.upgrade()
        {
            kernel.wake(Channel::Object(Rc::as_ptr(&file) as *const () as usize));
        }
    }
}

/// `timerfd_get_remaining`: what is left, never below zero.
fn descriptor_remaining(kernel: &Kernel, id: u64) -> (u64, u64) {
    let timer = &kernel.timers.all[&id];
    let left = timer
        .expires
        .map_or(0, |expires| expires.saturating_sub(kernel.entry_now));
    (left, timer.interval)
}

pub(super) fn timerfd_gettime(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, out, ..]: Args,
) -> Result<u64, Errno> {
    let id = timer_descriptor(kernel, tid, fd)?;
    catch_up(kernel, id);
    let left = descriptor_remaining(kernel, id);
    write_itimerspec(kernel, cx, tid, out, left)?;
    Ok(0)
}

/// `timerfd_show`: a descriptor's clock, expirations not yet read, flags, time left and interval,
/// as they stand (no catching up on missed expirations, unlike `gettime`).
pub(super) fn fdinfo(kernel: &Kernel, id: u64) -> alloc::string::String {
    let timer = &kernel.timers.all[&id];
    let (left, interval) = descriptor_remaining(kernel, id);
    alloc::format!(
        "clockid: {}\nticks: {}\nsettime flags: 0{:o}\nit_value: ({}, {})\nit_interval: ({}, {})\n",
        timer.clock_id,
        timer.ticks,
        timer.settime_flags,
        left / 1_000_000_000,
        left % 1_000_000_000,
        interval / 1_000_000_000,
        interval % 1_000_000_000,
    )
}

/// The descriptor's timer, if the file is one.
pub(super) fn of(file: &OpenFile) -> Option<u64> {
    match &file.anon {
        Some(Anon::Timer(id)) => Some(*id),
        _ => None,
    }
}

/// `timerfd_read_iter`: the expirations so far as eight bytes, waiting for one unless nonblocking.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let id = of(&file.borrow()).expect("a timer descriptor");
    // Eight bytes in all, across the buffers (measured with readv).
    if parts.iter().map(|(_, length)| length).sum::<u64>() < 8 {
        return Err(Errno::EINVAL);
    }
    if kernel.timers.all[&id].ticks == 0 {
        if super::io::nonblocking(kernel, tid, file) {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.park(tid, Channel::Object(Rc::as_ptr(file) as *const () as usize));
        return Err(Errno::PARKED);
    }
    // A setting of the wall clock drops what was counted, without rearming.
    if canceled(kernel, id) {
        let timer = kernel.timers.all.get_mut(&id).expect("timer");
        timer.ticks = 0;
        timer.expired = false;
        return Err(Errno::ECANCELED);
    }
    catch_up(kernel, id);
    let timer = kernel.timers.all.get_mut(&id).expect("timer");
    let ticks = core::mem::take(&mut timer.ticks);
    timer.expired = false;
    super::signalfd::store(kernel, cx, tid, parts, 0, &ticks.to_le_bytes())?;
    Ok(8)
}

/// `timerfd_poll`: readable once it has fired.
pub(super) fn poll(kernel: &Kernel, id: u64) -> u16 {
    if kernel
        .timers
        .all
        .get(&id)
        .is_some_and(|timer| timer.ticks > 0)
    {
        (POLLIN | POLLRDNORM) as u16
    } else {
        0
    }
}

/// `TFD_IOC_SET_TICKS`: sets the expirations not yet read.
pub(super) fn set_ticks(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    at: u64,
) -> Result<u64, Errno> {
    let id = of(&file.borrow()).expect("a timer descriptor");
    let ticks: u64 = guest(kernel, cx, tid).read_value(at)?;
    if ticks == 0 {
        return Err(Errno::EINVAL);
    }
    if canceled(kernel, id) {
        return Err(Errno::ECANCELED);
    }
    kernel.timers.all.get_mut(&id).expect("timer").ticks = ticks;
    kernel.wake(Channel::Object(Rc::as_ptr(file) as *const () as usize));
    Ok(0)
}
