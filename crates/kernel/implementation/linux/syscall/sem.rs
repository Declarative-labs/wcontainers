//! System V semaphores as Linux's `ipc/sem.c` builds them: sets of counters changed by groups of
//! operations that happen whole or wait whole, waiting operations kept as data in Linux's queues and
//! completed in their order by the change that lets them, and adjustments undone when a process ends.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use wcontainers_abi::NANOSECONDS_PER_SECOND;

use zerocopy::{FromBytes, IntoBytes};

use super::Args;
use super::guest;
use super::ipc::{Ids, Perm, get, new_perm};
use crate::linux::general::*;
use crate::linux_extra::*;
use crate::process::{Channel, Pid, Tid};
use crate::sysctl::{SEMMNI_INDEX, SEMMNS_INDEX, SEMMSL_INDEX, SEMOPM_INDEX};
use crate::{Context, Errno, Kernel};

/// One semaphore (`struct sem`) and the waiting operations on it alone.
#[derive(Clone, Default)]
struct Semaphore {
    value: i32,
    /// The process that last changed it or waited it out (`sempid`).
    pid: Pid,
    otime: i64,
    /// Waiting single operations that change it, and that wait for it to be zero.
    alter: Vec<Tid>,
    constant: Vec<Tid>,
}

/// A waiting `semop` (`struct sem_queue`).
#[derive(Clone)]
struct Waiter {
    ops: Vec<sembuf>,
    /// The undo list its adjustments go to, when an operation asks for them.
    undo: Option<u64>,
    pid: Pid,
    /// The operation that could not go on at the last try.
    blocking: usize,
    alter: bool,
    /// Whether an operation may name a semaphore an earlier one changes.
    dupsop: bool,
}

/// A semaphore set (`struct sem_array`).
#[derive(Clone)]
struct Array {
    perm: Perm,
    ctime: i64,
    sems: Vec<Semaphore>,
    /// Waiting operations on several semaphores, and single ones that change a semaphore while
    /// any of those wait.
    alter: Vec<Tid>,
    constant: Vec<Tid>,
    /// How many waiting operations name several semaphores (`complex_count`).
    complex: usize,
    waiters: BTreeMap<Tid, Waiter>,
}

impl Default for Ids<Array> {
    fn default() -> Self {
        Self::new(|array| &array.perm)
    }
}

/// One set's adjustments in an undo list (`struct sem_undo`).
#[derive(Clone)]
struct Undo {
    id: i32,
    adjust: Vec<i32>,
}

/// What a process, and those it shares it with through `CLONE_SYSVSEM`, undoes when the last of
/// them ends; most recently used first (`struct sem_undo_list`).
#[derive(Clone, Default)]
struct UndoList {
    users: u32,
    undos: Vec<Undo>,
}

/// The namespace's semaphore sets and undo lists.
#[derive(Clone, Default)]
pub(crate) struct Semaphores {
    ids: Ids<Array>,
    /// Semaphores in every set (`used_sems`).
    used: u64,
    lists: BTreeMap<u64, UndoList>,
    /// Each thread's undo list (Linux's `task->sysvsem.undo_list`).
    list_of: BTreeMap<Tid, u64>,
    next_list: u64,
    /// How a waiting operation ended while it was parked (Linux's `queue.status`), taken when its
    /// call runs again.
    told: BTreeMap<Tid, Result<(), Errno>>,
}

/// Threads to wake with how their operations ended, in Linux's `wake_q` order.
type Woken = Vec<(Tid, Result<(), Errno>)>;

pub(super) fn semget<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    [key, count, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (key, count, flags) = (key as i32, count as i32, flags as u32);
    let [semmsl, semmns, _, semmni] = kernel.ipc.limits.sem;
    if !(0..=semmsl).contains(&count) {
        return Err(Errno::EINVAL);
    }
    let count = count as usize;
    let credentials = kernel.credentials(tid).clone();
    let sems = &mut kernel.ipc.sem;
    let existing = get(&sems.ids, &credentials, key, flags, |array| {
        if count > array.sems.len() {
            Err(Errno::EINVAL)
        } else {
            Ok(())
        }
    })?;
    if let Some(id) = existing {
        return Ok(id as u64);
    }
    // `newary`.
    if count == 0 {
        return Err(Errno::EINVAL);
    }
    if sems.used as i64 + count as i64 > i64::from(semmns) {
        return Err(Errno::ENOSPC);
    }
    let slot = sems.ids.allocate(semmni as u64)?;
    let perm = new_perm(&credentials, key, flags, slot);
    let id = perm.id;
    let now = kernel.now(cx).seconds;
    let sems = &mut kernel.ipc.sem;
    sems.ids.slots.insert(
        slot.0,
        Array {
            perm,
            ctime: now,
            sems: alloc::vec![Semaphore::default(); count],
            alter: Vec::new(),
            constant: Vec::new(),
            complex: 0,
            waiters: BTreeMap::new(),
        },
    );
    sems.used += count as u64;
    Ok(id as u64)
}

pub(super) fn semop<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    [id, ops, count, ..]: Args,
) -> Result<u64, Errno> {
    operate(kernel, cx, tid, id, ops, count, 0)
}

pub(super) fn semtimedop<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    [id, ops, count, timeout, ..]: Args,
) -> Result<u64, Errno> {
    operate(kernel, cx, tid, id, ops, count, timeout)
}

/// `ksys_semtimedop` and `__do_semtimedop`, in their order. A call that waited runs again from
/// `waited`.
fn operate<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    id: u64,
    address: u64,
    count: u64,
    timeout: u64,
) -> Result<u64, Errno> {
    if kernel.threads[&tid].call.started {
        return waited(kernel, cx, tid).map(|()| 0);
    }
    let timeout = if timeout != 0 {
        Some(guest(kernel, cx, tid).read_value::<__kernel_timespec>(timeout)?)
    } else {
        None
    };
    let (id, count) = (id as i32, count as u32);
    if count > kernel.ipc.limits.sem[SEMOPM_INDEX] as u32 {
        return Err(Errno::E2BIG);
    }
    if count < 1 {
        return Err(Errno::EINVAL);
    }
    let size = core::mem::size_of::<sembuf>() as u64;
    let bytes = guest(kernel, cx, tid).read(address, u64::from(count) * size)?;
    let ops: Vec<sembuf> = bytes
        .chunks_exact(size as usize)
        .map(|chunk| sembuf::read_from_bytes(chunk).expect("one operation"))
        .collect();
    if id < 0 {
        return Err(Errno::EINVAL);
    }
    let deadline = match timeout {
        Some(timeout) => {
            if timeout.tv_sec < 0 || !(0..NANOSECONDS_PER_SECOND as i64).contains(&timeout.tv_nsec)
            {
                return Err(Errno::EINVAL);
            }
            let length = (timeout.tv_sec as u64)
                .saturating_mul(NANOSECONDS_PER_SECOND)
                .saturating_add(timeout.tv_nsec as u64);
            Some(cx.monotonic().saturating_add(length))
        }
        None => None,
    };
    // Which semaphores the operations name, whether they change any, and whether one may name a
    // semaphore an earlier one changes (by number modulo 64, as Linux checks).
    let (mut max, mut undos, mut alter, mut dupsop, mut dup) = (0, false, false, false, 0u64);
    for op in &ops {
        let mask = 1u64 << (op.sem_num % 64);
        max = max.max(usize::from(op.sem_num));
        undos |= op.sem_flg & SEM_UNDO != 0;
        dupsop |= dup & mask != 0;
        if op.sem_op != 0 {
            alter = true;
            dup |= mask;
        }
    }
    let pid = kernel.threads[&tid].process;
    let credentials = kernel.credentials(tid).clone();
    let now = kernel.now(cx).seconds;
    let sems = &mut kernel.ipc.sem;
    let undo = if undos {
        Some(sems.find_alloc_undo(tid, id)?)
    } else {
        None
    };
    let index = sems.ids.checked(id)?;
    let array = sems.ids.slots.get_mut(&index).expect("checked");
    if max >= array.sems.len() {
        return Err(Errno::EFBIG);
    }
    if !array
        .perm
        .permits(&credentials, if alter { 0o222 } else { 0o444 })
    {
        return Err(Errno::EACCES);
    }
    let mut waiter = Waiter {
        ops,
        undo,
        pid,
        blocking: 0,
        alter,
        dupsop,
    };
    let adjust = undo.and_then(|list| adjustments(&mut sems.lists, list, id));
    match perform(&mut array.sems, &mut waiter, adjust)? {
        true => {
            let mut woken = Woken::new();
            if alter {
                smart_update(
                    &mut sems.lists,
                    array,
                    Some(&waiter.ops),
                    true,
                    now,
                    &mut woken,
                );
            } else {
                set_otime(array, Some(&waiter.ops), now);
            }
            unmerge(array);
            tell(kernel, woken);
            Ok(0)
        }
        false => {
            enqueue(array, tid, waiter);
            let thread = kernel.thread_mut(tid);
            thread.call.started = true;
            thread.call.deadline = deadline.map(|at| at.saturating_add(thread.timer_slack));
            kernel.park(tid, Channel::Nothing);
            Err(Errno::PARKED)
        }
    }
}

/// A waiting operation's call run again: how it was told it ended, or its time is up, a signal
/// came (neither restarts it), or it waits on.
fn waited<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
) -> Result<(), Errno> {
    if let Some(told) = kernel.ipc.sem.told.remove(&tid) {
        return told;
    }
    let Some(index) = kernel.ipc.sem.array_of(tid) else {
        return Err(Errno::EIDRM);
    };
    let call = kernel.threads[&tid].call;
    let error = if call
        .deadline
        .is_some_and(|deadline| deadline <= cx.monotonic())
    {
        Errno::EAGAIN
    } else if kernel.signal_pending(tid) {
        Errno::EINTR
    } else {
        kernel.park(tid, Channel::Nothing);
        return Err(Errno::PARKED);
    };
    let array = kernel.ipc.sem.ids.slots.get_mut(&index).expect("found");
    unlink(array, tid);
    unmerge(array);
    Err(error)
}

/// Whether a thread waits in a set's queue with its operation not yet done.
pub(crate) fn queued(kernel: &Kernel, tid: Tid) -> bool {
    let sems = &kernel.ipc.sem;
    !sems.told.contains_key(&tid) && sems.array_of(tid).is_some()
}

/// A thread that ends leaves the queue it waits in.
pub(crate) fn forget(kernel: &mut Kernel, tid: Tid) {
    let sems = &mut kernel.ipc.sem;
    sems.told.remove(&tid);
    if let Some(index) = sems.array_of(tid) {
        let array = sems.ids.slots.get_mut(&index).expect("found");
        unlink(array, tid);
        unmerge(array);
    }
}

/// A waiter taken off its kernel leaves the queue and is told EINTR, as a stop tells it.
pub(crate) fn leave(kernel: &mut Kernel, tid: Tid) {
    if !queued(kernel, tid) {
        return;
    }
    forget(kernel, tid);
    kernel.ipc.sem.told.insert(tid, Err(Errno::EINTR));
}

/// `copy_semundo`: a thread or process made with `CLONE_SYSVSEM` shares its maker's undo list.
pub(crate) fn copy_undo(kernel: &mut Kernel, parent: Tid, child: Tid, share: bool) {
    if share {
        let sems = &mut kernel.ipc.sem;
        let list = sems.undo_list(parent);
        sems.lists.get_mut(&list).expect("made").users += 1;
        sems.list_of.insert(child, list);
    }
}

/// A thread took another number (`de_thread`); its undo list goes with it.
pub(crate) fn renumber(kernel: &mut Kernel, from: Tid, to: Tid) {
    let sems = &mut kernel.ipc.sem;
    if let Some(list) = sems.list_of.remove(&from) {
        sems.list_of.insert(to, list);
    }
}

/// `exit_sem`: when the last thread using an undo list ends, each set's adjustments are added
/// (the values held between 0 and `SEMVMX`) and waiting operations looked at again.
pub(crate) fn exit_sem<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    pid: Pid,
    tid: Tid,
) {
    let sems = &mut kernel.ipc.sem;
    let Some(list) = sems.list_of.remove(&tid) else {
        return;
    };
    let users = &mut sems.lists.get_mut(&list).expect("listed").users;
    *users -= 1;
    if *users > 0 {
        return;
    }
    let list = sems.lists.remove(&list).expect("listed");
    let now = kernel.now(cx).seconds;
    let sems = &mut kernel.ipc.sem;
    let mut woken = Woken::new();
    for undo in list.undos {
        let Ok(index) = sems.ids.checked(undo.id) else {
            continue;
        };
        let array = sems.ids.slots.get_mut(&index).expect("checked");
        for (sem, adjust) in array.sems.iter_mut().zip(&undo.adjust) {
            if *adjust != 0 {
                sem.value = (sem.value + adjust).clamp(0, SEMVMX);
                sem.pid = pid;
            }
        }
        smart_update(&mut sems.lists, array, None, true, now, &mut woken);
        unmerge(array);
    }
    tell(kernel, woken);
}

impl Semaphores {
    /// The slot of the set a thread waits in.
    fn array_of(&self, tid: Tid) -> Option<u32> {
        self.ids
            .slots
            .iter()
            .find(|(_, array)| array.waiters.contains_key(&tid))
            .map(|(index, _)| *index)
    }

    /// `get_undo_list`: the thread's undo list, made when it has none.
    fn undo_list(&mut self, tid: Tid) -> u64 {
        if let Some(list) = self.list_of.get(&tid) {
            return *list;
        }
        let list = self.next_list;
        self.next_list += 1;
        self.lists.insert(
            list,
            UndoList {
                users: 1,
                undos: Vec::new(),
            },
        );
        self.list_of.insert(tid, list);
        list
    }

    /// `find_alloc_undo`: the process's adjustments for set `id`, moved to the front of its list,
    /// or new ones there when the set exists.
    fn find_alloc_undo(&mut self, tid: Tid, id: i32) -> Result<u64, Errno> {
        let list = self.undo_list(tid);
        let undos = &mut self.lists.get_mut(&list).expect("made").undos;
        if let Some(position) = undos.iter().position(|undo| undo.id == id) {
            let undo = undos.remove(position);
            undos.insert(0, undo);
            return Ok(list);
        }
        let index = self.ids.checked(id)?;
        let count = self.ids.slots[&index].sems.len();
        let undos = &mut self.lists.get_mut(&list).expect("made").undos;
        undos.insert(
            0,
            Undo {
                id,
                adjust: alloc::vec![0; count],
            },
        );
        Ok(list)
    }

    /// `freeary`: the set goes with its adjustments, and every waiting operation is told EIDRM.
    fn remove(&mut self, index: u32) -> Woken {
        let array = self.ids.slots.remove(&index).expect("checked");
        for list in self.lists.values_mut() {
            list.undos.retain(|undo| undo.id != array.perm.id);
        }
        self.used -= array.sems.len() as u64;
        let mut woken: Woken = Vec::new();
        let mut gone =
            |tids: &Vec<Tid>| woken.extend(tids.iter().map(|tid| (*tid, Err(Errno::EIDRM))));
        gone(&array.constant);
        gone(&array.alter);
        for sem in &array.sems {
            gone(&sem.constant);
            gone(&sem.alter);
        }
        woken
    }
}

/// The adjustments of set `id` in undo list `list`.
fn adjustments(lists: &mut BTreeMap<u64, UndoList>, list: u64, id: i32) -> Option<&mut Vec<i32>> {
    lists
        .get_mut(&list)?
        .undos
        .iter_mut()
        .find(|undo| undo.id == id)
        .map(|undo| &mut undo.adjust)
}

/// Tells the woken threads how their operations ended and wakes them.
fn tell(kernel: &mut Kernel, woken: Woken) {
    for (tid, result) in woken {
        kernel.ipc.sem.told.insert(tid, result);
        kernel.wake_thread(tid);
    }
}

/// `perform_atomic_semop` and its slow form for operations that may name a semaphore twice: all
/// the operations happen, or none. Ok(true) when they did, Ok(false) when the caller must wait
/// (`blocking` names the operation), EAGAIN when that operation must not wait, ERANGE past a limit.
fn perform(
    sems: &mut [Semaphore],
    waiter: &mut Waiter,
    mut adjust: Option<&mut Vec<i32>>,
) -> Result<bool, Errno> {
    let Waiter {
        ops,
        pid,
        blocking,
        dupsop,
        ..
    } = waiter;
    let undoes = |op: &sembuf| op.sem_flg & SEM_UNDO != 0;
    let would_block = |op: &sembuf| {
        if op.sem_flg & IPC_NOWAIT as i16 != 0 {
            Err(Errno::EAGAIN)
        } else {
            Ok(false)
        }
    };
    let in_range = |undo: i32| (-SEMAEM - 1..=SEMAEM).contains(&undo);
    if *dupsop {
        // Each operation is applied as it is checked, and taken back on failure.
        let mut failure = None;
        let mut done = 0;
        for (index, op) in ops.iter().enumerate() {
            let sem = &mut sems[usize::from(op.sem_num)];
            let result = sem.value + i32::from(op.sem_op);
            if (op.sem_op == 0 && sem.value != 0) || result < 0 {
                *blocking = index;
                failure = Some(would_block(op));
                break;
            }
            if result > SEMVMX {
                failure = Some(Err(Errno::ERANGE));
                break;
            }
            if undoes(op) {
                let adjust = adjust.as_deref_mut().expect("an undo");
                let undo = adjust[usize::from(op.sem_num)] - i32::from(op.sem_op);
                if !in_range(undo) {
                    failure = Some(Err(Errno::ERANGE));
                    break;
                }
                adjust[usize::from(op.sem_num)] = undo;
            }
            sem.value = result;
            done = index + 1;
        }
        let Some(failure) = failure else {
            for op in ops.iter() {
                sems[usize::from(op.sem_num)].pid = *pid;
            }
            return Ok(true);
        };
        for op in ops[..done].iter().rev() {
            sems[usize::from(op.sem_num)].value -= i32::from(op.sem_op);
            if undoes(op) {
                adjust.as_deref_mut().expect("an undo")[usize::from(op.sem_num)] +=
                    i32::from(op.sem_op);
            }
        }
        return failure;
    }
    // Checked whole first, then applied.
    for (index, op) in ops.iter().enumerate() {
        let value = sems[usize::from(op.sem_num)].value;
        let result = value + i32::from(op.sem_op);
        if (op.sem_op == 0 && value != 0) || result < 0 {
            *blocking = index;
            return would_block(op);
        }
        if result > SEMVMX {
            return Err(Errno::ERANGE);
        }
        if undoes(op) {
            let adjust = adjust.as_deref().expect("an undo");
            if !in_range(adjust[usize::from(op.sem_num)] - i32::from(op.sem_op)) {
                return Err(Errno::ERANGE);
            }
        }
    }
    for op in ops.iter() {
        if undoes(op) {
            adjust.as_deref_mut().expect("an undo")[usize::from(op.sem_num)] -=
                i32::from(op.sem_op);
        }
        let sem = &mut sems[usize::from(op.sem_num)];
        sem.value += i32::from(op.sem_op);
        sem.pid = *pid;
    }
    Ok(true)
}

/// Tries a waiting operation again.
fn attempt(
    lists: &mut BTreeMap<u64, UndoList>,
    array: &mut Array,
    tid: Tid,
) -> Result<bool, Errno> {
    let id = array.perm.id;
    let waiter = array.waiters.get_mut(&tid).expect("queued");
    let adjust = waiter.undo.and_then(|list| adjustments(lists, list, id));
    perform(&mut array.sems, waiter, adjust)
}

/// Puts a waiting operation in its queue, as `__do_semtimedop` does.
fn enqueue(array: &mut Array, tid: Tid, waiter: Waiter) {
    if waiter.ops.len() == 1 {
        let sem = &mut array.sems[usize::from(waiter.ops[0].sem_num)];
        if !waiter.alter {
            sem.constant.push(tid);
        } else if array.complex > 0 {
            array.alter.push(tid);
        } else {
            sem.alter.push(tid);
        }
    } else {
        if array.complex == 0 {
            merge(array);
        }
        if waiter.alter {
            array.alter.push(tid);
        } else {
            array.constant.push(tid);
        }
        array.complex += 1;
    }
    array.waiters.insert(tid, waiter);
}

/// `unlink_queue`.
fn unlink(array: &mut Array, tid: Tid) -> Waiter {
    let waiter = array.waiters.remove(&tid).expect("queued");
    let leave = |list: &mut Vec<Tid>| list.retain(|other| *other != tid);
    leave(&mut array.alter);
    leave(&mut array.constant);
    let sem = &mut array.sems[usize::from(waiter.ops[0].sem_num)];
    leave(&mut sem.alter);
    leave(&mut sem.constant);
    if waiter.ops.len() > 1 {
        array.complex -= 1;
    }
    waiter
}

/// `merge_queues`: each semaphore's waiting changes go to the front of the set's queue, the last
/// semaphore's first, so a waiting operation on several keeps the order.
fn merge(array: &mut Array) {
    let mut merged = Vec::new();
    for sem in array.sems.iter_mut().rev() {
        merged.append(&mut sem.alter);
    }
    merged.append(&mut array.alter);
    array.alter = merged;
}

/// `unmerge_queues`, as every release of the set's lock does: with no operation on several
/// semaphores waiting, the set's queue goes back to each operation's semaphore, in order.
fn unmerge(array: &mut Array) {
    if array.complex > 0 {
        return;
    }
    for tid in core::mem::take(&mut array.alter) {
        let num = usize::from(array.waiters[&tid].ops[0].sem_num);
        array.sems[num].alter.push(tid);
    }
}

/// `wake_const_ops`: waits for zero in one semaphore's queue, or the set's, that can end now.
fn wake_const(
    lists: &mut BTreeMap<u64, UndoList>,
    array: &mut Array,
    num: Option<usize>,
    woken: &mut Woken,
) -> bool {
    let pending = match num {
        Some(num) => array.sems[num].constant.clone(),
        None => array.constant.clone(),
    };
    let mut completed = false;
    for tid in pending {
        let outcome = attempt(lists, array, tid);
        if outcome == Ok(false) {
            continue;
        }
        unlink(array, tid);
        completed |= outcome.is_ok();
        woken.push((tid, outcome.map(|_| ())));
    }
    completed
}

/// `do_smart_wakeup_zero`: after the semaphores `ops` name (or all) changed, the waits for zero
/// that can end.
fn wakeup_zero(
    lists: &mut BTreeMap<u64, UndoList>,
    array: &mut Array,
    ops: Option<&[sembuf]>,
    woken: &mut Woken,
) -> bool {
    let nums: Vec<usize> = match ops {
        Some(ops) => ops.iter().map(|op| usize::from(op.sem_num)).collect(),
        None => (0..array.sems.len()).collect(),
    };
    let (mut completed, mut zero) = (false, false);
    for num in nums {
        if array.sems[num].value == 0 {
            zero = true;
            completed |= wake_const(lists, array, Some(num), woken);
        }
    }
    if zero {
        completed |= wake_const(lists, array, None, woken);
    }
    completed
}

/// `update_queue`: waiting changes in one semaphore's queue, or the set's, that can happen now,
/// in order, looking again from the start when one that happened may let earlier ones go.
fn update_queue(
    lists: &mut BTreeMap<u64, UndoList>,
    array: &mut Array,
    num: Option<usize>,
    woken: &mut Woken,
) -> bool {
    let mut completed = false;
    'again: loop {
        let pending = match num {
            Some(num) => array.sems[num].alter.clone(),
            None => array.alter.clone(),
        };
        for tid in pending {
            // A single decrement cannot go on at zero.
            if num.is_some_and(|num| array.sems[num].value == 0) {
                break 'again;
            }
            let outcome = attempt(lists, array, tid);
            if outcome == Ok(false) {
                continue;
            }
            let waiter = unlink(array, tid);
            let restart = outcome.is_ok() && {
                completed = true;
                wakeup_zero(lists, array, Some(&waiter.ops), woken);
                !array.alter.is_empty() || waiter.ops.len() > 1
            };
            woken.push((tid, outcome.map(|_| ())));
            if restart {
                continue 'again;
            }
        }
        break;
    }
    completed
}

/// `do_smart_update`: after the semaphores `ops` name (or all) changed, the waiting operations
/// that can happen now; the time of the last operation is set when `otime` or any did.
fn smart_update(
    lists: &mut BTreeMap<u64, UndoList>,
    array: &mut Array,
    ops: Option<&[sembuf]>,
    otime: bool,
    now: i64,
    woken: &mut Woken,
) {
    let mut otime = otime | wakeup_zero(lists, array, ops, woken);
    if !array.alter.is_empty() {
        otime |= update_queue(lists, array, None, woken);
    } else if let Some(ops) = ops {
        for op in ops.iter().filter(|op| op.sem_op > 0) {
            otime |= update_queue(lists, array, Some(usize::from(op.sem_num)), woken);
        }
    } else {
        for num in 0..array.sems.len() {
            otime |= update_queue(lists, array, Some(num), woken);
        }
    }
    if otime {
        set_otime(array, ops, now);
    }
}

/// `set_semotime`: the time is kept on the first semaphore `ops` names (or the set's first).
fn set_otime(array: &mut Array, ops: Option<&[sembuf]>, now: i64) {
    let num = ops.map_or(0, |ops| usize::from(ops[0].sem_num));
    array.sems[num].otime = now;
}

/// `count_semcnt`: how many waiting operations wait on semaphore `num` to grow, or to be zero.
fn count_waiting(array: &Array, num: usize, zero: bool) -> u64 {
    let sem = &array.sems[num];
    let simple = if zero {
        sem.constant.len()
    } else {
        sem.alter.len()
    };
    let waits_here = |tid: &&Tid| {
        let waiter = &array.waiters[*tid];
        let op = waiter.ops[waiter.blocking];
        usize::from(op.sem_num) == num && if zero { op.sem_op == 0 } else { op.sem_op < 0 }
    };
    let complex = array.alter.iter().filter(waits_here).count()
        + if zero {
            array.constant.iter().filter(waits_here).count()
        } else {
            0
        };
    (simple + complex) as u64
}

pub(super) fn semctl<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    [id, num, command, argument, ..]: Args,
) -> Result<u64, Errno> {
    let (id, num, command) = (id as i32, num as i32, command as i32);
    if id < 0 {
        return Err(Errno::EINVAL);
    }
    let credentials = kernel.credentials(tid).clone();
    let caller = kernel.threads[&tid].process;
    let now = kernel.now(cx).seconds;
    let sems = &mut kernel.ipc.sem;
    match command {
        IPC_INFO | SEM_INFO => {
            let limits = kernel.ipc.limits.sem;
            let mut info = seminfo {
                semmap: SEMMNS as i32,
                semmni: limits[SEMMNI_INDEX],
                semmns: limits[SEMMNS_INDEX],
                semmnu: SEMMNS as i32,
                semmsl: limits[SEMMSL_INDEX],
                semopm: limits[SEMOPM_INDEX],
                semume: SEMOPM as i32,
                semusz: SEMUSZ,
                semvmx: SEMVMX,
                semaem: SEMAEM,
            };
            if command == SEM_INFO {
                info.semusz = sems.ids.slots.len() as i32;
                info.semaem = sems.used as i32;
            }
            let max = sems.ids.max_index();
            guest(kernel, cx, tid).write(argument, info.as_bytes())?;
            Ok(max)
        }
        IPC_STAT | SEM_STAT | SEM_STAT_ANY => {
            let index = if command == IPC_STAT {
                sems.ids.checked(id)?
            } else {
                sems.ids.by_index(id)?
            };
            let array = &sems.ids.slots[&index];
            if command != SEM_STAT_ANY && !array.perm.permits(&credentials, 0o444) {
                return Err(Errno::EACCES);
            }
            let status = semid64_ds {
                sem_perm: array.perm.shown(),
                sem_otime: array.sems.iter().map(|sem| sem.otime).max().unwrap_or(0),
                sem_ctime: array.ctime,
                sem_nsems: array.sems.len() as u64,
                ..Default::default()
            };
            let answer = if command == IPC_STAT {
                0
            } else {
                array.perm.id as u64
            };
            guest(kernel, cx, tid).write(argument, status.as_bytes())?;
            Ok(answer)
        }
        GETALL | SETALL | GETVAL | GETPID | GETNCNT | GETZCNT => {
            // `semctl_main`.
            let index = sems.ids.checked(id)?;
            let array = &sems.ids.slots[&index];
            let access = if command == SETALL { 0o222 } else { 0o444 };
            if !array.perm.permits(&credentials, access) {
                return Err(Errno::EACCES);
            }
            let count = array.sems.len();
            match command {
                GETALL => {
                    let values: Vec<u8> = array
                        .sems
                        .iter()
                        .flat_map(|sem| (sem.value as u16).to_le_bytes())
                        .collect();
                    guest(kernel, cx, tid).write(argument, &values)?;
                    return Ok(0);
                }
                SETALL => {
                    let bytes = guest(kernel, cx, tid).read(argument, count as u64 * 2)?;
                    let values: Vec<u16> = bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| u16::from_le_bytes(*pair))
                        .collect();
                    if values.iter().any(|value| i32::from(*value) > SEMVMX) {
                        return Err(Errno::ERANGE);
                    }
                    let sems = &mut kernel.ipc.sem;
                    let array = sems.ids.slots.get_mut(&index).expect("checked");
                    for (sem, value) in array.sems.iter_mut().zip(values) {
                        sem.value = value.into();
                        sem.pid = caller;
                    }
                    clear_adjustments(&mut sems.lists, id, None);
                    array.ctime = now;
                    let mut woken = Woken::new();
                    smart_update(&mut sems.lists, array, None, false, now, &mut woken);
                    unmerge(array);
                    tell(kernel, woken);
                    return Ok(0);
                }
                _ => {}
            }
            if num < 0 || num as usize >= count {
                return Err(Errno::EINVAL);
            }
            let num = num as usize;
            let sem = &array.sems[num];
            Ok(match command {
                GETVAL => sem.value as u64,
                GETPID => sem.pid as u64,
                GETNCNT => count_waiting(array, num, false),
                _ => count_waiting(array, num, true),
            })
        }
        SETVAL => {
            // `semctl_setval`: the value is the argument's low 32 bits.
            let value = argument as i32;
            if !(0..=SEMVMX).contains(&value) {
                return Err(Errno::ERANGE);
            }
            let index = sems.ids.checked(id)?;
            let array = sems.ids.slots.get_mut(&index).expect("checked");
            if num < 0 || num as usize >= array.sems.len() {
                return Err(Errno::EINVAL);
            }
            if !array.perm.permits(&credentials, 0o222) {
                return Err(Errno::EACCES);
            }
            clear_adjustments(&mut sems.lists, id, Some(num as usize));
            let sem = &mut array.sems[num as usize];
            sem.value = value;
            sem.pid = caller;
            array.ctime = now;
            let mut woken = Woken::new();
            smart_update(&mut sems.lists, array, None, false, now, &mut woken);
            unmerge(array);
            tell(kernel, woken);
            Ok(0)
        }
        IPC_SET | IPC_RMID => {
            // `semctl_down`.
            let wanted = if command == IPC_SET {
                Some(guest(kernel, cx, tid).read_value::<semid64_ds>(argument)?)
            } else {
                None
            };
            let sems = &mut kernel.ipc.sem;
            let index = sems.ids.checked(id)?;
            let array = sems.ids.slots.get_mut(&index).expect("checked");
            if !array.perm.may_control(&credentials) {
                return Err(Errno::EPERM);
            }
            match wanted {
                Some(wanted) => {
                    // `ipc_update_perm`: ids that name no one are refused.
                    let perm = wanted.sem_perm;
                    if perm.uid == u32::MAX || perm.gid == u32::MAX {
                        return Err(Errno::EINVAL);
                    }
                    array.perm.uid = perm.uid;
                    array.perm.gid = perm.gid;
                    array.perm.mode = (array.perm.mode & !0o777) | (perm.mode & 0o777);
                    array.ctime = now;
                }
                None => {
                    let woken = sems.remove(index);
                    tell(kernel, woken);
                }
            }
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}

/// Zeroes the adjustments every undo list holds for set `id`, of one semaphore or all.
fn clear_adjustments(lists: &mut BTreeMap<u64, UndoList>, id: i32, num: Option<usize>) {
    for undo in lists
        .values_mut()
        .flat_map(|list| list.undos.iter_mut())
        .filter(|undo| undo.id == id)
    {
        match num {
            Some(num) => undo.adjust[num] = 0,
            None => undo.adjust.fill(0),
        }
    }
}

/// `/proc/sysvipc/sem` (`sysvipc_sem_proc_show`): every set of the namespace.
pub(super) fn listing(semaphores: &Semaphores) -> alloc::string::String {
    use core::fmt::Write;
    let mut out = alloc::string::String::from(
        "       key      semid perms      nsems   uid   gid  cuid  cgid      otime      ctime\n",
    );
    for array in semaphores.ids.slots.values() {
        let perm = &array.perm;
        let _ = writeln!(
            out,
            "{:10} {:10}  {:4o} {:10} {:5} {:5} {:5} {:5} {:10} {:10}",
            perm.key,
            perm.id,
            perm.mode,
            array.sems.len(),
            perm.uid,
            perm.gid,
            perm.cuid,
            perm.cgid,
            array.sems.iter().map(|sem| sem.otime).max().unwrap_or(0),
            array.ctime
        );
    }
    out
}
