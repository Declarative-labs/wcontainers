//! Futexes, the waiting primitive locks are built on, as `kernel/futex` keeps them: waiters queued on
//! a futex's key in the order they came, woken, requeued or handed a priority inheriting lock, and
//! what a thread's end does to the locks it holds (its robust list and its thread id address).
//!
//! A waiter parks on [`Channel::Futex`] with its call's deadline; whoever wakes it takes it off its
//! queue first, so when its call runs again it knows it was woken because it is no longer queued.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use smallvec::SmallVec;
use wcontainers_abi::{PAGE_SIZE, Permissions};

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::memory::Backing;
use crate::process::{Channel, Restart, Tid};
use crate::{Context, Errno, Kernel};

/// What a futex is known by (`union futex_key`): a private one, or a shared one in a private
/// mapping, by its memory and address; a shared one in a shared mapping by the file's inode and
/// the offset in it, so every process that maps it finds the same futex.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Key {
    Private(u32, u64),
    Shared(usize, u64),
}

#[derive(Clone, Copy, Debug)]
struct Waiter {
    tid: Tid,
    /// Which of its call's futexes this is (a vector wait has several).
    index: u8,
    /// The address it waits at, in its own memory.
    address: u64,
    bitset: u32,
    /// Waiting for a priority inheriting lock (`FUTEX_LOCK_PI`).
    pi: bool,
    /// Waiting on a condition to be requeued onto this priority inheriting lock
    /// (`FUTEX_WAIT_REQUEUE_PI`).
    requeue_pi: Option<Key>,
}

/// Every futex with waiters.
#[derive(Clone, Default)]
pub(crate) struct Futexes {
    /// Each futex's waiters, the first to be woken first (all threads here have the same
    /// priority, so Linux's priority list keeps them in the order they came).
    queues: BTreeMap<Key, VecDeque<Waiter>>,
    /// The owner the kernel knows of each priority inheriting futex with waiters (`pi_state`).
    pi_owners: BTreeMap<Key, Tid>,
}

/// A thread's futex wait, kept in its entry (Linux's `futex_q`s): the futex each of its call's
/// entries is queued on, and which of them a wake, requeue or hand-over took off its queue.
#[derive(Clone, Default, Debug)]
pub(crate) struct Wait {
    keys: SmallVec<[Key; 1]>,
    /// One bit per entry (a vector has at most `FUTEX_WAITV_MAX`, 128).
    woken: u128,
}

impl Wait {
    /// The highest entry taken off its queue, as `futex_unqueue_multiple` answers.
    fn last_woken(&self) -> Option<usize> {
        (self.woken != 0).then(|| 127 - self.woken.leading_zeros() as usize)
    }
}

/// Queues a thread's next entry on a futex.
fn queue(kernel: &mut Kernel, key: Key, mut waiter: Waiter) {
    let wait = &mut kernel.thread_mut(waiter.tid).futex;
    waiter.index = wait.keys.len() as u8;
    wait.keys.push(key);
    kernel
        .futexes
        .queues
        .entry(key)
        .or_default()
        .push_back(waiter);
}

/// Takes a waiter out of a futex's queue by its place in it.
fn remove(kernel: &mut Kernel, key: Key, position: usize) -> Option<Waiter> {
    let queue = kernel.futexes.queues.get_mut(&key)?;
    let waiter = queue.remove(position)?;
    if queue.is_empty() {
        kernel.futexes.queues.remove(&key);
        kernel.futexes.pi_owners.remove(&key);
    }
    Some(waiter)
}

/// Takes a waiter off a futex's queue as woken (`__futex_unqueue`), which its call sees when it
/// runs again.
fn take(kernel: &mut Kernel, key: Key, position: usize) -> Option<Waiter> {
    let waiter = remove(kernel, key, position)?;
    if let Some(thread) = kernel.threads.get_mut(&waiter.tid) {
        thread.futex.woken |= 1 << waiter.index;
    }
    Some(waiter)
}

/// Moves a waiter to the end of another futex's queue (`requeue_futex`), as `waiter` now is.
fn move_waiter(kernel: &mut Kernel, key: Key, position: usize, to: Key, waiter: Waiter) {
    remove(kernel, key, position);
    if let Some(thread) = kernel.threads.get_mut(&waiter.tid) {
        thread.futex.keys[usize::from(waiter.index)] = to;
    }
    kernel
        .futexes
        .queues
        .entry(to)
        .or_default()
        .push_back(waiter);
}

/// Ends a thread's futex wait: its entries still queued are taken off. Returns the highest entry
/// that had been taken off by a wake, if any was.
fn finish(kernel: &mut Kernel, tid: Tid) -> Option<usize> {
    let wait = core::mem::take(&mut kernel.thread_mut(tid).futex);
    for (index, key) in wait.keys.iter().enumerate() {
        if wait.woken & (1 << index) != 0 {
            continue;
        }
        let position = kernel.futexes.queues.get(key).and_then(|queue| {
            queue
                .iter()
                .position(|waiter| waiter.tid == tid && usize::from(waiter.index) == index)
        });
        if let Some(position) = position {
            remove(kernel, *key, position);
        }
    }
    wait.last_woken()
}

/// The first waiter of a futex.
fn top(kernel: &Kernel, key: Key) -> Option<Waiter> {
    kernel
        .futexes
        .queues
        .get(&key)
        .and_then(|queue| queue.front().copied())
}

/// Takes a thread's waits away, as its end does.
pub(crate) fn forget(kernel: &mut Kernel, tid: Tid) {
    finish(kernel, tid);
}

/// `get_futex_key`: the address must be aligned and in user space; a shared futex's page must be
/// mapped and readable, or writable for the calls that change the word (`FUTEX_WRITE`). A page of
/// a file is known by the file: one in a shared mapping, or a private one read-only and not yet
/// copied. A private page the futex cannot write is otherwise EFAULT (the zero page, or an
/// anonymous page reached read-only); a writable one is the process's own.
fn key_of(
    kernel: &Kernel,
    tid: Tid,
    address: u64,
    shared: bool,
    write: bool,
) -> Result<Key, Errno> {
    if !address.is_multiple_of(4) {
        return Err(Errno::EINVAL);
    }
    if !crate::user::access_ok(address, 4) {
        return Err(Errno::EFAULT);
    }
    let process = kernel.process_of(tid);
    if !shared {
        return Ok(Key::Private(process.memory.0, address));
    }
    let map = process.map.borrow();
    let mapping = map.find(address).ok_or(Errno::EFAULT)?;
    let allows = mapping.page_permissions(address);
    let writable = allows.contains(Permissions::WRITE);
    if !allows.contains(Permissions::READ) || (write && !writable) {
        return Err(Errno::EFAULT);
    }
    match &mapping.backing {
        Backing::File {
            node,
            offset,
            copied,
            ..
        } => {
            let at = offset + (address - mapping.start);
            if mapping.shared || (!writable && !copied.contains(&(at / PAGE_SIZE))) {
                Ok(Key::Shared(
                    alloc::rc::Rc::as_ptr(node) as *const () as usize,
                    at,
                ))
            } else if writable {
                Ok(Key::Private(process.memory.0, address))
            } else {
                Err(Errno::EFAULT)
            }
        }
        _ if writable => Ok(Key::Private(process.memory.0, address)),
        _ => Err(Errno::EFAULT),
    }
}

fn read_word(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
) -> Result<u32, Errno> {
    guest(kernel, cx, tid).read_value::<u32>(address)
}

fn write_word(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    value: u32,
) -> Result<(), Errno> {
    let mut guest = guest(kernel, cx, tid);
    // An atomic change needs the page writable, as the CPU's exclusive store does.
    if guest.accessible(address, 4, wcontainers_abi::Permissions::WRITE) < 4 {
        return Err(Errno::EFAULT);
    }
    guest.write_value(address, &value)
}

/// Wakes a waiter taken off its queue.
fn wake_waiter(kernel: &mut Kernel, tid: Tid) {
    if kernel
        .threads
        .get(&tid)
        .is_some_and(|thread| thread.call.wait == Some(Channel::Futex))
    {
        kernel.wake_thread(tid);
    }
}

/// When a wait ends anyway: none, or a time on the monotonic clock.
#[derive(Clone, Copy)]
enum Timeout {
    None,
    /// Relative to now, on the monotonic clock (`FUTEX_WAIT`).
    Relative(u64),
    /// An absolute time on the monotonic or the wall clock.
    Absolute {
        at: u64,
        realtime: bool,
    },
}

fn read_timespec(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
) -> Result<u64, Errno> {
    let value: __kernel_timespec = guest(kernel, cx, tid).read_value(address)?;
    // `timespec64_valid`.
    if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
        return Err(Errno::EINVAL);
    }
    Ok((value.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(value.tv_nsec as u64))
}

/// The call's deadline on the kernel's monotonic clock, set at its first run.
fn deadline(cx: &mut Context<'_>, timeout: Timeout, slack: u64) -> Option<u64> {
    let now = cx.monotonic();
    let at = match timeout {
        Timeout::None => return None,
        Timeout::Relative(after) => now.saturating_add(after),
        Timeout::Absolute { at, realtime: true } => {
            now.saturating_add(at.saturating_sub(cx.host.realtime_nanoseconds()))
        }
        Timeout::Absolute { at, .. } => at,
    };
    Some(at.saturating_add(slack))
}

/// `futex(uaddr, op, val, timeout or val2, uaddr2, val3)`.
pub(super) fn futex(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, op, value, timeout, address2, value3]: Args,
) -> Outcome {
    let op = op as u32;
    let (value, value3) = (value as u32, value3 as u32);
    let command = op & FUTEX_CMD_MASK as u32;
    let shared = op & FUTEX_PRIVATE_FLAG == 0;
    let realtime = op & FUTEX_CLOCK_REALTIME != 0;
    let started = kernel.threads[&tid].call.started;
    // A wait already queued runs again to see how it ended.
    if started {
        return match command {
            FUTEX_LOCK_PI | FUTEX_LOCK_PI2 => lock_pi_again(kernel, cx, tid),
            FUTEX_WAIT_REQUEUE_PI => wait_requeue_pi_again(kernel, cx, tid, address2, shared),
            _ => wait_again(kernel, cx, tid),
        };
    }
    // `sys_futex`: the timeout is read and checked before anything else.
    let has_timeout = matches!(
        command,
        FUTEX_WAIT | FUTEX_LOCK_PI | FUTEX_LOCK_PI2 | FUTEX_WAIT_BITSET | FUTEX_WAIT_REQUEUE_PI
    );
    let mut wait_until = Timeout::None;
    if timeout != 0 && has_timeout {
        let time = match read_timespec(kernel, cx, tid, timeout) {
            Ok(time) => time,
            Err(error) => return Err(error).into(),
        };
        wait_until = match command {
            FUTEX_WAIT => Timeout::Relative(time),
            FUTEX_LOCK_PI => Timeout::Absolute {
                at: time,
                realtime: true,
            },
            _ => Timeout::Absolute { at: time, realtime },
        };
    }
    // `do_futex`.
    if realtime
        && !matches!(
            command,
            FUTEX_WAIT_BITSET | FUTEX_WAIT_REQUEUE_PI | FUTEX_LOCK_PI2
        )
    {
        return Err(Errno::ENOSYS).into();
    }
    let slack = kernel.threads[&tid].timer_slack;
    let count2 = timeout as u32 as i32;
    match command {
        FUTEX_WAIT => wait(
            kernel,
            cx,
            tid,
            address,
            shared,
            value,
            FUTEX_BITSET_MATCH_ANY,
            wait_until,
            slack,
            true,
        ),
        FUTEX_WAIT_BITSET => wait(
            kernel, cx, tid, address, shared, value, value3, wait_until, slack, true,
        ),
        FUTEX_WAKE => wake(
            kernel,
            tid,
            address,
            shared,
            value as i32,
            FUTEX_BITSET_MATCH_ANY,
            false,
        )
        .into(),
        FUTEX_WAKE_BITSET => wake(kernel, tid, address, shared, value as i32, value3, false).into(),
        FUTEX_REQUEUE => requeue(
            kernel,
            cx,
            tid,
            address,
            address2,
            shared,
            value as i32,
            count2,
            None,
            false,
        )
        .into(),
        FUTEX_CMP_REQUEUE => requeue(
            kernel,
            cx,
            tid,
            address,
            address2,
            shared,
            value as i32,
            count2,
            Some(value3),
            false,
        )
        .into(),
        FUTEX_CMP_REQUEUE_PI => requeue(
            kernel,
            cx,
            tid,
            address,
            address2,
            shared,
            value as i32,
            count2,
            Some(value3),
            true,
        )
        .into(),
        FUTEX_WAKE_OP => wake_op(
            kernel,
            cx,
            tid,
            address,
            address2,
            shared,
            value as i32,
            count2,
            value3,
        )
        .into(),
        FUTEX_LOCK_PI | FUTEX_LOCK_PI2 => {
            lock_pi(kernel, cx, tid, address, shared, wait_until, false)
        }
        FUTEX_TRYLOCK_PI => lock_pi(kernel, cx, tid, address, shared, Timeout::None, true),
        FUTEX_UNLOCK_PI => unlock_pi(kernel, cx, tid, address, shared).into(),
        FUTEX_WAIT_REQUEUE_PI => wait_requeue_pi(
            kernel, cx, tid, address, address2, shared, value, wait_until,
        ),
        _ => Err(Errno::ENOSYS).into(),
    }
}

/// `futex_wait`: queues the caller if the futex still holds `value`, until a wake whose bits meet
/// `bitset`, the deadline or a signal.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn wait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    shared: bool,
    value: u32,
    bitset: u32,
    timeout: Timeout,
    slack: u64,
    restartable: bool,
) -> Outcome {
    if bitset == 0 {
        return Err(Errno::EINVAL).into();
    }
    let key = match key_of(kernel, tid, address, shared, false) {
        Ok(key) => key,
        Err(error) => return Err(error).into(),
    };
    match read_word(kernel, cx, tid, address) {
        Ok(found) if found == value => {}
        Ok(_) => return Err(Errno::EAGAIN).into(),
        Err(error) => return Err(error).into(),
    }
    let until = deadline(cx, timeout, slack);
    queue(
        kernel,
        key,
        Waiter {
            tid,
            index: 0,
            address,
            bitset,
            pi: false,
            requeue_pi: None,
        },
    );
    let thread = kernel.thread_mut(tid);
    thread.call.started = true;
    thread.call.deadline = until;
    // What `restart_syscall` needs to wait again after a handler-less signal.
    thread.restart = until.filter(|_| restartable).map(|until| Restart::Futex {
        address,
        value,
        bitset,
        shared,
        until,
    });
    wait_again(kernel, cx, tid)
}

/// A queued wait's next look: woken if it is off its queue, else its deadline, a signal, or park.
fn wait_again(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid) -> Outcome {
    if kernel.threads[&tid].futex.woken != 0 {
        finish(kernel, tid);
        return Outcome::Return(0);
    }
    let call = kernel.threads[&tid].call;
    if call.deadline.is_some_and(|at| at <= cx.monotonic()) {
        finish(kernel, tid);
        return Err(Errno::ETIMEDOUT).into();
    }
    if kernel.signal_pending(tid) {
        finish(kernel, tid);
        // With a timeout, the wait goes on from `restart_syscall` until the same time.
        if matches!(kernel.threads[&tid].restart, Some(Restart::Futex { .. })) {
            return Err(Errno::ERESTART_RESTARTBLOCK).into();
        }
        return Err(Errno::ERESTARTSYS).into();
    }
    kernel.park(tid, Channel::Futex);
    Outcome::Parked
}

/// `restart_syscall` for a futex wait a signal interrupted (`futex_wait_restart`): it waits again,
/// reading the futex anew, until the same deadline.
pub(super) fn restart(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    restart: Restart,
) -> Outcome {
    let Restart::Futex {
        address,
        value,
        bitset,
        shared,
        until,
    } = restart
    else {
        unreachable!("a futex's restart");
    };
    let outcome = if kernel.threads[&tid].call.started {
        wait_again(kernel, cx, tid)
    } else {
        wait(
            kernel,
            cx,
            tid,
            address,
            shared,
            value,
            bitset,
            Timeout::Absolute {
                at: until,
                realtime: false,
            },
            0,
            true,
        )
    };
    let interrupted = Errno::ERESTART_RESTARTBLOCK.to_return();
    if matches!(outcome, Outcome::Return(value) if value != interrupted) {
        kernel.thread_mut(tid).restart = None;
    }
    outcome
}

/// `futex_wake`: wakes up to `count` waiters whose bits meet `bitset` (one at least, unless
/// `strict` and `count` is 0).
pub(crate) fn wake(
    kernel: &mut Kernel,
    tid: Tid,
    address: u64,
    shared: bool,
    count: i32,
    bitset: u32,
    strict: bool,
) -> Result<u64, Errno> {
    if bitset == 0 {
        return Err(Errno::EINVAL);
    }
    let key = key_of(kernel, tid, address, shared, false)?;
    if strict && count == 0 {
        return Ok(0);
    }
    wake_key(kernel, key, count, bitset)
}

fn wake_key(kernel: &mut Kernel, key: Key, count: i32, bitset: u32) -> Result<u64, Errno> {
    let mut woken = 0i64;
    let mut position = 0;
    while let Some(waiter) = kernel
        .futexes
        .queues
        .get(&key)
        .and_then(|queue| queue.get(position).copied())
    {
        // A priority inheriting waiter is not for a plain wake; those found before it are woken.
        if waiter.pi || waiter.requeue_pi.is_some() {
            return Err(Errno::EINVAL);
        }
        if waiter.bitset & bitset == 0 {
            position += 1;
            continue;
        }
        take(kernel, key, position);
        wake_waiter(kernel, waiter.tid);
        woken += 1;
        if woken >= i64::from(count) {
            break;
        }
    }
    Ok(woken as u64)
}

/// `futex_requeue`: wakes up to `wake` waiters of the first futex and moves up to `requeue` more
/// to the second, if the first still holds `compare` when one is given. Returns how many it woke
/// and moved.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn requeue(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    address2: u64,
    shared: bool,
    wake: i32,
    requeue: i32,
    compare: Option<u32>,
    pi: bool,
) -> Result<u64, Errno> {
    if wake < 0 || requeue < 0 {
        return Err(Errno::EINVAL);
    }
    // Two distinct futexes, and only the one waiter that takes the lock may be woken.
    if pi && (address == address2 || wake != 1) {
        return Err(Errno::EINVAL);
    }
    let key = key_of(kernel, tid, address, shared, false)?;
    let key2 = key_of(kernel, tid, address2, shared, pi)?;
    if pi && key == key2 {
        return Err(Errno::EINVAL);
    }
    if let Some(compare) = compare
        && read_word(kernel, cx, tid, address)? != compare
    {
        return Err(Errno::EAGAIN);
    }
    if pi {
        return requeue_pi(kernel, cx, tid, key, key2, address2, requeue);
    }
    requeue_plain(kernel, key, key2, wake, requeue)
}

/// The plain part of `futex_requeue`: the first `wake` waiters woken, the next `requeue` moved.
fn requeue_plain(
    kernel: &mut Kernel,
    key: Key,
    key2: Key,
    wake: i32,
    requeue: i32,
) -> Result<u64, Errno> {
    let mut count = 0i64;
    // Waiters requeued onto their own futex keep their places.
    let mut position = 0;
    while let Some(waiter) = kernel
        .futexes
        .queues
        .get(&key)
        .and_then(|queue| queue.get(position).copied())
    {
        if count - i64::from(wake) >= i64::from(requeue) {
            break;
        }
        if waiter.pi || waiter.requeue_pi.is_some() {
            return Err(Errno::EINVAL);
        }
        count += 1;
        if count <= i64::from(wake) {
            take(kernel, key, position);
            wake_waiter(kernel, waiter.tid);
        } else if key == key2 {
            position += 1;
        } else {
            move_waiter(kernel, key, position, key2, waiter);
        }
    }
    Ok(count as u64)
}

/// `futex_atomic_op_inuser`: changes the word at `address` as `op` says and compares what it held.
fn atomic_op(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    encoded: u32,
) -> Result<bool, Errno> {
    let op = (encoded & 0x7000_0000) >> 28;
    let compare = (encoded & 0x0f00_0000) >> 24;
    let mut argument = (((encoded & 0x00ff_f000) >> 12) << 20) as i32 >> 20;
    let compared = ((encoded & 0xfff) << 20) as i32 >> 20;
    if encoded & (FUTEX_OP_OPARG_SHIFT << 28) != 0 {
        argument = 1 << (argument & 31);
    }
    if !crate::user::access_ok(address, 4) {
        return Err(Errno::EFAULT);
    }
    // An unknown operation is refused before the word is touched.
    if op > FUTEX_OP_XOR {
        return Err(Errno::ENOSYS);
    }
    let old = read_word(kernel, cx, tid, address)?;
    let new = match op {
        FUTEX_OP_SET => argument as u32,
        FUTEX_OP_ADD => old.wrapping_add(argument as u32),
        FUTEX_OP_OR => old | argument as u32,
        FUTEX_OP_ANDN => old & !(argument as u32),
        FUTEX_OP_XOR => old ^ argument as u32,
        _ => return Err(Errno::ENOSYS),
    };
    write_word(kernel, cx, tid, address, new)?;
    let old = old as i32;
    Ok(match compare {
        FUTEX_OP_CMP_EQ => old == compared,
        FUTEX_OP_CMP_NE => old != compared,
        FUTEX_OP_CMP_LT => old < compared,
        FUTEX_OP_CMP_GE => old >= compared,
        FUTEX_OP_CMP_LE => old <= compared,
        FUTEX_OP_CMP_GT => old > compared,
        _ => return Err(Errno::ENOSYS),
    })
}

/// `futex_wake_op`: changes the second futex, wakes up to `wake` waiters of the first, and if the
/// comparison held up to `wake2` of the second.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn wake_op(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    address2: u64,
    shared: bool,
    wake: i32,
    wake2: i32,
    op: u32,
) -> Result<u64, Errno> {
    let key = key_of(kernel, tid, address, shared, false)?;
    let key2 = key_of(kernel, tid, address2, shared, true)?;
    let held = atomic_op(kernel, cx, tid, address2, op)?;
    let mut woken = wake_key(kernel, key, wake, FUTEX_BITSET_MATCH_ANY)?;
    if held {
        woken += wake_key(kernel, key2, wake2, FUTEX_BITSET_MATCH_ANY)?;
    }
    Ok(woken)
}

/// `futex_lock_pi_atomic`: takes the lock if it is free (1), or marks it as having waiters and
/// finds its owner (0); EDEADLK if the caller holds it.
fn lock_pi_atomic(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    key: Key,
    set_waiters: bool,
) -> Result<bool, Errno> {
    let value = read_word(kernel, cx, tid, address)?;
    if value & FUTEX_TID_MASK == tid as u32 {
        return Err(Errno::EDEADLK);
    }
    if top(kernel, key).is_some() {
        // `attach_to_pi_state`: the word must agree with the owner the kernel knows.
        let owner = kernel.futexes.pi_owners.get(&key).copied();
        let named = (value & FUTEX_TID_MASK) as Tid;
        let agrees = match owner {
            Some(owner) => named == owner,
            None => value & FUTEX_OWNER_DIED != 0 && named == 0,
        };
        if !agrees || kernel.futexes.queues[&key].iter().any(|waiter| !waiter.pi) {
            return Err(Errno::EINVAL);
        }
        return Ok(false);
    }
    if value & FUTEX_TID_MASK == 0 {
        let mut new = (value & FUTEX_OWNER_DIED) | tid as u32;
        if set_waiters {
            new |= FUTEX_WAITERS;
        }
        write_word(kernel, cx, tid, address, new)?;
        return Ok(true);
    }
    write_word(kernel, cx, tid, address, value | FUTEX_WAITERS)?;
    // `attach_to_pi_owner`: the owner must be a live thread.
    let owner = (value & FUTEX_TID_MASK) as Tid;
    if !kernel.threads.contains_key(&owner) {
        return Err(Errno::ESRCH);
    }
    kernel.futexes.pi_owners.insert(key, owner);
    Ok(false)
}

/// `futex_lock_pi`: takes the lock, or waits for its owner to hand it over. Here every thread has
/// the same priority, so the owner's priority is never raised.
fn lock_pi(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    shared: bool,
    timeout: Timeout,
    trylock: bool,
) -> Outcome {
    let key = match key_of(kernel, tid, address, shared, true) {
        Ok(key) => key,
        Err(error) => return Err(error).into(),
    };
    match lock_pi_atomic(kernel, cx, tid, address, key, false) {
        Ok(true) => return Outcome::Return(0),
        Ok(false) => {}
        Err(error) => return Err(error).into(),
    }
    if trylock {
        if top(kernel, key).is_none() {
            kernel.futexes.pi_owners.remove(&key);
        }
        return Err(Errno::EAGAIN).into();
    }
    let until = deadline(cx, timeout, 0);
    queue(
        kernel,
        key,
        Waiter {
            tid,
            index: 0,
            address,
            bitset: FUTEX_BITSET_MATCH_ANY,
            pi: true,
            requeue_pi: None,
        },
    );
    let call = &mut kernel.thread_mut(tid).call;
    call.started = true;
    call.deadline = until;
    lock_pi_again(kernel, cx, tid)
}

/// A queued lock's next look: handed the lock if it is off its queue, else its deadline, a signal
/// (the call starts again, `ERESTARTNOINTR`), or park.
fn lock_pi_again(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid) -> Outcome {
    if kernel.threads[&tid].futex.woken != 0 {
        finish(kernel, tid);
        return Outcome::Return(0);
    }
    if kernel.threads[&tid]
        .call
        .deadline
        .is_some_and(|at| at <= cx.monotonic())
    {
        finish(kernel, tid);
        return Err(Errno::ETIMEDOUT).into();
    }
    if kernel.signal_pending(tid) {
        finish(kernel, tid);
        return Err(Errno::ERESTARTNOINTR).into();
    }
    kernel.park(tid, Channel::Futex);
    Outcome::Parked
}

/// Hands a priority inheriting lock to its first waiter (`wake_futex_pi`): the word names it, with
/// the waiters bit always kept and an owner's death kept if `keep` holds it.
fn hand_over(kernel: &mut Kernel, cx: &mut Context<'_>, key: Key, keep: u32) -> Result<(), Errno> {
    let Some(next) = top(kernel, key) else {
        return Ok(());
    };
    let word = keep | FUTEX_WAITERS | next.tid as u32;
    write_word(kernel, cx, next.tid, next.address, word)?;
    take(kernel, key, 0);
    if top(kernel, key).is_some() {
        kernel.futexes.pi_owners.insert(key, next.tid);
    }
    wake_waiter(kernel, next.tid);
    Ok(())
}

/// `futex_unlock_pi`: only the owner may unlock; the first waiter is handed the lock, or the word is
/// cleared.
fn unlock_pi(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    shared: bool,
) -> Result<u64, Errno> {
    let value = read_word(kernel, cx, tid, address)?;
    if value & FUTEX_TID_MASK != tid as u32 {
        return Err(Errno::EPERM);
    }
    let key = key_of(kernel, tid, address, shared, true)?;
    if let Some(top) = top(kernel, key) {
        if !top.pi || kernel.futexes.pi_owners.get(&key) != Some(&tid) {
            return Err(Errno::EINVAL);
        }
        hand_over(kernel, cx, key, 0)?;
        return Ok(0);
    }
    write_word(kernel, cx, tid, address, 0)?;
    Ok(0)
}

/// `futex_wait_requeue_pi`: waits on a condition until it is requeued onto the lock at `address2`
/// and handed it.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn wait_requeue_pi(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    address2: u64,
    shared: bool,
    value: u32,
    timeout: Timeout,
) -> Outcome {
    if address == address2 {
        return Err(Errno::EINVAL).into();
    }
    // The lock's key, then the condition's and its value, then whether they are one futex.
    let (key2, key) = match key_of(kernel, tid, address2, shared, true)
        .and_then(|key2| Ok((key2, key_of(kernel, tid, address, shared, false)?)))
    {
        Ok(keys) => keys,
        Err(error) => return Err(error).into(),
    };
    match read_word(kernel, cx, tid, address) {
        Ok(found) if found == value => {}
        Ok(_) => return Err(Errno::EAGAIN).into(),
        Err(error) => return Err(error).into(),
    }
    if key == key2 {
        return Err(Errno::EINVAL).into();
    }
    let until = deadline(cx, timeout, kernel.threads[&tid].timer_slack);
    queue(
        kernel,
        key,
        Waiter {
            tid,
            index: 0,
            address,
            bitset: FUTEX_BITSET_MATCH_ANY,
            pi: false,
            requeue_pi: Some(key2),
        },
    );
    let call = &mut kernel.thread_mut(tid).call;
    call.started = true;
    call.deadline = until;
    wait_requeue_pi_again(kernel, cx, tid, address2, shared)
}

/// A condition waiter's next look: on the condition still, it may time out or be signalled
/// (`ERESTARTNOINTR`); requeued onto the lock it waits as a lock's waiter; handed the lock, it has
/// it.
fn wait_requeue_pi_again(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address2: u64,
    shared: bool,
) -> Outcome {
    let wait = &kernel.threads[&tid].futex;
    if wait.woken != 0 {
        finish(kernel, tid);
        return Outcome::Return(0);
    }
    let on_lock = wait.keys.first().copied() == key_of(kernel, tid, address2, shared, true).ok();
    let timed_out = kernel.threads[&tid]
        .call
        .deadline
        .is_some_and(|at| at <= cx.monotonic());
    if timed_out || kernel.signal_pending(tid) {
        finish(kernel, tid);
        return Err(match (timed_out, on_lock) {
            (true, _) => Errno::ETIMEDOUT,
            // Still on the condition: the wait ends as a plain one does.
            (false, false) => Errno::ERESTARTNOINTR,
            // Already on the lock: the word has changed, so a restart would only find that.
            (false, true) => Errno::EAGAIN,
        })
        .into();
    }
    kernel.park(tid, Channel::Futex);
    Outcome::Parked
}

/// `futex_requeue` with priority inheritance: the first waiter takes the lock if it is free, the
/// rest move to the lock's queue as its waiters.
fn requeue_pi(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    key: Key,
    key2: Key,
    address2: u64,
    requeue: i32,
) -> Result<u64, Errno> {
    // `futex_proxy_trylock_atomic` reads the lock's word first, waiters or not.
    read_word(kernel, cx, tid, address2)?;
    let mut count = 0i64;
    let mut first = true;
    // Each waiter is taken or moved, so the next is always the first.
    while let Some(waiter) = top(kernel, key) {
        if count > i64::from(requeue) {
            break;
        }
        if waiter.requeue_pi != Some(key2) {
            return Err(Errno::EINVAL);
        }
        if first {
            first = false;
            // `futex_proxy_trylock_atomic`: the top waiter takes the lock if it is free.
            let value = read_word(kernel, cx, tid, address2)?;
            if value & FUTEX_TID_MASK == 0 && top(kernel, key2).is_none() {
                let more = kernel.futexes.queues[&key].len() > 1;
                let new = (value & FUTEX_OWNER_DIED)
                    | waiter.tid as u32
                    | if more { FUTEX_WAITERS } else { 0 };
                write_word(kernel, cx, tid, address2, new)?;
                take(kernel, key, 0);
                kernel.futexes.pi_owners.insert(key2, waiter.tid);
                wake_waiter(kernel, waiter.tid);
                count += 1;
                continue;
            }
        }
        // Onto the lock's queue: its word says it has waiters, and the kernel knows its owner.
        let value = read_word(kernel, cx, tid, address2)?;
        let owner = (value & FUTEX_TID_MASK) as Tid;
        if !kernel.futexes.pi_owners.contains_key(&key2) {
            if !kernel.threads.contains_key(&owner) {
                return Err(Errno::ESRCH);
            }
            kernel.futexes.pi_owners.insert(key2, owner);
        }
        write_word(kernel, cx, tid, address2, value | FUTEX_WAITERS)?;
        move_waiter(
            kernel,
            key,
            0,
            key2,
            Waiter {
                address: address2,
                pi: true,
                requeue_pi: None,
                ..waiter
            },
        );
        count += 1;
    }
    Ok(count as u64)
}

/// A thread's end as `futex_exit_release` and `mm_release` see it: the locks on its robust list
/// marked as their owner's death and a waiter woken, the priority inheriting locks it holds handed
/// on marked the same, and its thread id address cleared and woken while others still use the
/// memory.
pub(crate) fn thread_end(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    others_use_memory: bool,
) {
    forget(kernel, tid);
    let (head, clear) = {
        let thread = &kernel.threads[&tid];
        (thread.robust_list.0, thread.clear_child_tid)
    };
    if head != 0 {
        robust_list(kernel, cx, tid, head);
    }
    let held: Vec<Key> = kernel
        .futexes
        .pi_owners
        .iter()
        .filter(|(_, owner)| **owner == tid)
        .map(|(key, _)| *key)
        .collect();
    for key in held {
        kernel.futexes.pi_owners.remove(&key);
        // The next owner finds no owner to take over from (`__fixup_pi_state_owner`).
        let _ = hand_over(kernel, cx, key, FUTEX_OWNER_DIED);
    }
    if clear != 0 && others_use_memory && guest(kernel, cx, tid).write_value(clear, &0u32).is_ok() {
        let _ = wake(kernel, tid, clear, true, 1, FUTEX_BITSET_MATCH_ANY, false);
    }
}

/// The most entries `exit_robust_list` follows (`ROBUST_LIST_LIMIT`).
const ROBUST_LIST_LIMIT: usize = 2048;

/// `exit_robust_list`: each lock on the list (and the one being taken or let go) that the thread
/// holds is marked as its owner's death, and a plain one with waiters wakes one.
fn robust_list(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid, head: u64) {
    let read = |kernel: &mut Kernel, cx: &mut Context<'_>, at: u64| {
        guest(kernel, cx, tid).read_value::<u64>(at).ok()
    };
    let Some(first) = read(kernel, cx, head) else {
        return;
    };
    let Some(offset) = read(kernel, cx, head + 8) else {
        return;
    };
    let Some(pending) = read(kernel, cx, head + 16) else {
        return;
    };
    let (pending, pending_pi) = (pending & !1, pending & 1 != 0);
    let (mut entry, mut pi) = (first & !1, first & 1 != 0);
    let mut limit = ROBUST_LIST_LIMIT;
    while entry != head {
        let next = read(kernel, cx, entry);
        if entry != pending && !owner_died(kernel, cx, tid, entry.wrapping_add(offset), pi, false) {
            return;
        }
        let Some(next) = next else {
            return;
        };
        (entry, pi) = (next & !1, next & 1 != 0);
        limit -= 1;
        if limit == 0 {
            break;
        }
    }
    if pending != 0 {
        owner_died(
            kernel,
            cx,
            tid,
            pending.wrapping_add(offset),
            pending_pi,
            true,
        );
    }
}

/// `handle_futex_death`; false when the walk must stop.
fn owner_died(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    pi: bool,
    pending: bool,
) -> bool {
    if !address.is_multiple_of(4) {
        return false;
    }
    let Ok(value) = read_word(kernel, cx, tid, address) else {
        return false;
    };
    // A plain lock let go just before the end may have a waiter that missed its wake.
    if pending && !pi && value == 0 {
        let _ = wake(kernel, tid, address, true, 1, FUTEX_BITSET_MATCH_ANY, false);
        return true;
    }
    if value & FUTEX_TID_MASK != tid as u32 {
        return true;
    }
    let marked = (value & FUTEX_WAITERS) | FUTEX_OWNER_DIED;
    if write_word(kernel, cx, tid, address, marked).is_err() {
        return false;
    }
    if !pi && value & FUTEX_WAITERS != 0 {
        let _ = wake(kernel, tid, address, true, 1, FUTEX_BITSET_MATCH_ANY, false);
    }
    true
}

/// The futex2 flags (`futex2_to_flags` and `futex_flags_valid`): only 32 bit futexes; the NUMA
/// forms are not built. Returns whether the futex is shared.
fn futex2_flags(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    number: u32,
    flags: u32,
) -> Result<bool, Errno> {
    if flags & !(FUTEX2_SIZE_MASK | FUTEX2_NUMA | FUTEX2_MPOL | FUTEX2_PRIVATE) != 0 {
        return Err(Errno::EINVAL);
    }
    if flags & FUTEX2_SIZE_MASK != FUTEX2_SIZE_U32
        || flags & FUTEX2_MPOL != 0 && flags & FUTEX2_NUMA == 0
    {
        return Err(Errno::EINVAL);
    }
    if flags & (FUTEX2_NUMA | FUTEX2_MPOL) != 0 {
        return Err(kernel
            .counters
            .refuse_flag(cx.host, number, flags.into(), Errno::EINVAL));
    }
    Ok(flags & FUTEX2_PRIVATE == 0)
}

/// `futex2_setup_timeout`: an absolute time on the wall or the monotonic clock.
fn futex2_timeout(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    timeout: u64,
    clock: u64,
) -> Result<Timeout, Errno> {
    if timeout == 0 {
        return Ok(Timeout::None);
    }
    let realtime = match clock as u32 {
        CLOCK_REALTIME => true,
        CLOCK_MONOTONIC => false,
        _ => return Err(Errno::EINVAL),
    };
    let at = read_timespec(kernel, cx, tid, timeout)?;
    Ok(Timeout::Absolute { at, realtime })
}

/// `futex_wake(uaddr, mask, nr, flags)`.
pub(super) fn futex_wake(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, mask, count, flags, ..]: Args,
) -> Result<u64, Errno> {
    let shared = futex2_flags(kernel, cx, __NR_futex_wake, flags as u32)?;
    if mask >> 32 != 0 {
        return Err(Errno::EINVAL);
    }
    wake(
        kernel,
        tid,
        address,
        shared,
        count as i32,
        mask as u32,
        true,
    )
}

/// `futex_wait(uaddr, val, mask, flags, timeout, clockid)`.
pub(super) fn futex_wait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, value, mask, flags, timeout, clock]: Args,
) -> Outcome {
    if kernel.threads[&tid].call.started {
        return wait_again(kernel, cx, tid);
    }
    let shared = match futex2_flags(kernel, cx, __NR_futex_wait, flags as u32) {
        Ok(shared) => shared,
        Err(error) => return Err(error).into(),
    };
    if value >> 32 != 0 || mask >> 32 != 0 {
        return Err(Errno::EINVAL).into();
    }
    let timeout = match futex2_timeout(kernel, cx, tid, timeout, clock) {
        Ok(timeout) => timeout,
        Err(error) => return Err(error).into(),
    };
    wait(
        kernel,
        cx,
        tid,
        address,
        shared,
        value as u32,
        mask as u32,
        timeout,
        0,
        false,
    )
}

/// One entry of a futex2 vector (`futex_parse_waitv`): its address, value and whether it is shared.
fn parse_waitv(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    number: u32,
    at: u64,
) -> Result<(u64, u32, bool), Errno> {
    let entry: futex_waitv = guest(kernel, cx, tid).read_value(at)?;
    if entry.flags & !(FUTEX2_SIZE_MASK | FUTEX2_NUMA | FUTEX2_MPOL | FUTEX2_PRIVATE) != 0
        || entry.__reserved != 0
    {
        return Err(Errno::EINVAL);
    }
    let shared = futex2_flags(kernel, cx, number, entry.flags)?;
    if entry.val >> 32 != 0 {
        return Err(Errno::EINVAL);
    }
    Ok((entry.uaddr, entry.val as u32, shared))
}

/// `futex_waitv(waiters, nr_futexes, flags, timeout, clockid)`: waits on several futexes at once
/// and answers the index of one that was woken.
pub(super) fn futex_waitv(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [vector, count, flags, timeout, clock, _]: Args,
) -> Outcome {
    if kernel.threads[&tid].call.started {
        return waitv_again(kernel, cx, tid);
    }
    let count = count as u32;
    if flags as u32 != 0 || count == 0 || count > FUTEX_WAITV_MAX || vector == 0 {
        return Err(Errno::EINVAL).into();
    }
    let result = (|| {
        let timeout = futex2_timeout(kernel, cx, tid, timeout, clock)?;
        let mut entries = Vec::new();
        for index in 0..u64::from(count) {
            let at = vector + index * size_of::<futex_waitv>() as u64;
            entries.push(parse_waitv(kernel, cx, tid, __NR_futex_waitv, at)?);
        }
        // `futex_wait_multiple_setup`: every key, then every value, before any is queued.
        let mut keys = Vec::new();
        for (address, _, shared) in &entries {
            keys.push(key_of(kernel, tid, *address, *shared, false)?);
        }
        for (address, value, _) in &entries {
            if read_word(kernel, cx, tid, *address)? != *value {
                return Err(Errno::EAGAIN);
            }
        }
        Ok((timeout, keys, entries))
    })();
    let (timeout, keys, entries) = match result {
        Ok(found) => found,
        Err(error) => return Err(error).into(),
    };
    for (key, (address, ..)) in keys.into_iter().zip(entries) {
        queue(
            kernel,
            key,
            Waiter {
                tid,
                index: 0,
                address,
                bitset: FUTEX_BITSET_MATCH_ANY,
                pi: false,
                requeue_pi: None,
            },
        );
    }
    let until = deadline(cx, timeout, 0);
    let call = &mut kernel.thread_mut(tid).call;
    call.started = true;
    call.deadline = until;
    waitv_again(kernel, cx, tid)
}

/// A vector wait's next look: the last of its futexes found woken, its deadline, a signal, or park.
fn waitv_again(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid) -> Outcome {
    if kernel.threads[&tid].futex.woken != 0 {
        let index = finish(kernel, tid).unwrap_or(0);
        return Outcome::Return(index as u64);
    }
    if kernel.threads[&tid]
        .call
        .deadline
        .is_some_and(|at| at <= cx.monotonic())
    {
        finish(kernel, tid);
        return Err(Errno::ETIMEDOUT).into();
    }
    if kernel.signal_pending(tid) {
        finish(kernel, tid);
        return Err(Errno::ERESTARTSYS).into();
    }
    kernel.park(tid, Channel::Futex);
    Outcome::Parked
}

/// `futex_requeue(waiters, flags, nr_wake, nr_requeue)`: the two futexes from a vector of two.
pub(super) fn futex_requeue(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [vector, flags, wake, requeue_count, ..]: Args,
) -> Result<u64, Errno> {
    if flags as u32 != 0 || vector == 0 {
        return Err(Errno::EINVAL);
    }
    let (address, value, shared) = parse_waitv(kernel, cx, tid, __NR_futex_requeue, vector)?;
    let (address2, _, shared2) = parse_waitv(
        kernel,
        cx,
        tid,
        __NR_futex_requeue,
        vector + size_of::<futex_waitv>() as u64,
    )?;
    // Both futexes must have the same flags, as `sys_futex`'s requeue has.
    if shared != shared2 {
        return Err(Errno::EINVAL);
    }
    let (wake, requeue_count) = (wake as i32, requeue_count as i32);
    if wake < 0 || requeue_count < 0 {
        return Err(Errno::EINVAL);
    }
    let key = key_of(kernel, tid, address, shared, false)?;
    let key2 = key_of(kernel, tid, address2, shared2, false)?;
    if read_word(kernel, cx, tid, address)? != value {
        return Err(Errno::EAGAIN);
    }
    requeue_plain(kernel, key, key2, wake, requeue_count)
}
