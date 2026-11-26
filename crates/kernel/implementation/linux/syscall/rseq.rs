//! Restartable sequences and memory barriers across a process's threads.
//!
//! Threads take turns on one CPU, so a barrier between them needs nothing done, and a thread's
//! restartable sequence is never interrupted by another thread on its CPU.

use super::{Args, guest};
use crate::linux::general::membarrier_cmd::*;
use crate::linux::general::{__NR_rseq, membarrier_cmd_flag};
use crate::linux_extra::*;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};
use wcontainers_executor::Executor;

/// The fields `rseq` registration writes: the CPU (twice), no critical section, no flags, NUMA node 0
/// and concurrency id 0 (the one CPU, and one thread so far).
fn registered_fields() -> [u8; RSEQ_WRITTEN_SIZE as usize] {
    [0; RSEQ_WRITTEN_SIZE as usize]
}

/// Linux's `sys_rseq`, as the reference's 7.0 kernel checks it (measured: an area of other than the
/// first size must be 64 byte aligned).
pub(super) fn rseq<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [area, length, flags, signature, ..]: Args,
) -> Result<u64, Errno> {
    let (length, flags, signature) = (length as u32, flags as u32, signature as u32);
    let registered = kernel.threads[&tid].rseq;
    if flags & RSEQ_FLAG_UNREGISTER != 0 {
        if flags & !RSEQ_FLAG_UNREGISTER != 0 {
            return Err(Errno::EINVAL);
        }
        let Some((at, their_length, their_signature)) = registered else {
            return Err(Errno::EINVAL);
        };
        if at != area || length != their_length {
            return Err(Errno::EINVAL);
        }
        if signature != their_signature {
            return Err(Errno::EPERM);
        }
        // `cpu_id_start`, `cpu_id`, then `node_id` and `mm_cid`.
        let mut guest = guest(kernel, cx, tid);
        guest.write(area, &0u32.to_le_bytes())?;
        guest.write(area + 4, &RSEQ_CPU_ID_UNINITIALIZED.to_le_bytes())?;
        guest.write(area + 20, &[0; 8])?;
        kernel.thread_mut(tid).rseq = None;
        return Ok(0);
    }
    // Time slice extensions are not built; a kernel without them refuses the flag.
    if flags == RSEQ_FLAG_SLICE_EXT_DEFAULT_ON {
        return Err(kernel
            .counters
            .refuse_flag(cx.host, __NR_rseq, flags.into(), Errno::EINVAL));
    }
    if flags != 0 {
        return Err(Errno::EINVAL);
    }
    if let Some((at, their_length, their_signature)) = registered {
        if at != area || length != their_length {
            return Err(Errno::EINVAL);
        }
        if signature != their_signature {
            return Err(Errno::EPERM);
        }
        return Err(Errno::EBUSY);
    }
    let alignment = if length == ORIG_RSEQ_SIZE { 32 } else { 64 };
    if length < ORIG_RSEQ_SIZE || !area.is_multiple_of(alignment) {
        return Err(Errno::EINVAL);
    }
    guest(kernel, cx, tid).write(area, &registered_fields())?;
    kernel.thread_mut(tid).rseq = Some((area, length, signature));
    Ok(0)
}

/// Linux's `MEMBARRIER_STATE_*`: each registration is marked when it begins and when it is ready.
const PRIVATE_EXPEDITED_READY: u32 = 1 << 0;
const PRIVATE_EXPEDITED: u32 = 1 << 1;
const GLOBAL_EXPEDITED_READY: u32 = 1 << 2;
const GLOBAL_EXPEDITED: u32 = 1 << 3;
const SYNC_CORE_READY: u32 = 1 << 4;
const SYNC_CORE: u32 = 1 << 5;
const RSEQ_READY: u32 = 1 << 6;
const RSEQ: u32 = 1 << 7;

/// Every command, as `MEMBARRIER_CMD_QUERY` reports them (the reference has no `nohz_full` CPUs).
const COMMANDS: u32 = MEMBARRIER_CMD_GLOBAL as u32
    | MEMBARRIER_CMD_GLOBAL_EXPEDITED as u32
    | MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED as u32
    | MEMBARRIER_CMD_PRIVATE_EXPEDITED as u32
    | MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as u32
    | MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE as u32
    | MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE as u32
    | MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ as u32
    | MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ as u32
    | MEMBARRIER_CMD_GET_REGISTRATIONS as u32;

/// Linux's `sys_membarrier`. The barriers themselves need nothing done (see above); the checks and
/// registrations are Linux's.
pub(super) fn membarrier<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    _: &mut Context<'_, S, C>,
    tid: Tid,
    [command, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (command, flags) = (command as i32 as u32, flags as u32);
    let rseq = MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ as u32;
    let cpu_flag = membarrier_cmd_flag::MEMBARRIER_CMD_FLAG_CPU as u32;
    if (command == rseq && flags != 0 && flags != cpu_flag) || (command != rseq && flags != 0) {
        return Err(Errno::EINVAL);
    }
    let state = &mut kernel.process_of(tid).map.borrow_mut().membarrier;
    // The private barriers need their registration to be ready.
    let ready = |state: u32, needed: u32| {
        if state & needed == 0 {
            Err(Errno::EPERM)
        } else {
            Ok(0)
        }
    };
    let register = |state: &mut u32, begun: u32, done: u32| {
        if *state & done != done {
            *state |= begun | done;
        }
        Ok(0)
    };
    match command {
        0 => Ok(COMMANDS.into()),
        _ if command == MEMBARRIER_CMD_GLOBAL as u32 => Ok(0),
        _ if command == MEMBARRIER_CMD_GLOBAL_EXPEDITED as u32 => Ok(0),
        _ if command == MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED as u32 => {
            register(state, GLOBAL_EXPEDITED, GLOBAL_EXPEDITED_READY)
        }
        _ if command == MEMBARRIER_CMD_PRIVATE_EXPEDITED as u32 => {
            ready(*state, PRIVATE_EXPEDITED_READY)
        }
        _ if command == MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as u32 => {
            register(state, PRIVATE_EXPEDITED, PRIVATE_EXPEDITED_READY)
        }
        _ if command == MEMBARRIER_CMD_PRIVATE_EXPEDITED_SYNC_CORE as u32 => {
            ready(*state, SYNC_CORE_READY)
        }
        // Linux marks the plain private registration begun too, but not ready.
        _ if command == MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE as u32 => {
            register(state, PRIVATE_EXPEDITED | SYNC_CORE, SYNC_CORE_READY)
        }
        _ if command == rseq => ready(*state, RSEQ_READY),
        _ if command == MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ as u32 => {
            register(state, PRIVATE_EXPEDITED | RSEQ, RSEQ_READY)
        }
        _ if command == MEMBARRIER_CMD_GET_REGISTRATIONS as u32 => {
            let pairs = [
                (
                    GLOBAL_EXPEDITED | GLOBAL_EXPEDITED_READY,
                    MEMBARRIER_CMD_REGISTER_GLOBAL_EXPEDITED,
                ),
                (
                    PRIVATE_EXPEDITED | PRIVATE_EXPEDITED_READY,
                    MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED,
                ),
                (
                    SYNC_CORE | SYNC_CORE_READY,
                    MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE,
                ),
                (
                    RSEQ | RSEQ_READY,
                    MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_RSEQ,
                ),
            ];
            Ok(pairs
                .iter()
                .filter(|(bits, _)| *state & bits != 0)
                .fold(0u64, |mask, (_, command)| mask | *command as u64))
        }
        _ => Err(Errno::EINVAL),
    }
}
