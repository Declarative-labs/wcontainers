//! The program memory a system call will touch, known from its arguments before it runs, so that
//! the embedder's watches are met while the call can still be repeated. A copy outside what is
//! described here is counted (`Kernel::system_call`).

#![allow(non_upper_case_globals, reason = "Linux's names for its call numbers")]

use alloc::vec;
use alloc::vec::Vec;
use core::mem::{offset_of, size_of};

use wcontainers_abi::{PAGE_SIZE, SystemCallRequest};

use crate::linux::ctypes::{c_int, c_uint};
use crate::linux::general::*;
use crate::linux::ioctl::*;
use crate::linux::net::{mmsghdr, msghdr};
use crate::linux::prctl::*;
use crate::linux::ptrace::{
    SECCOMP_GET_ACTION_AVAIL, SECCOMP_GET_NOTIF_SIZES, SECCOMP_SET_MODE_FILTER,
    seccomp_notif_sizes, sock_filter, sock_fprog,
};
use crate::linux::system::{new_utsname, sysinfo};
use crate::linux_extra::{
    IPC_SET, IPC_STAT, MSG_STAT, MSG_STAT_ANY, SCHED_ATTR_SIZE_VER0, SEM_STAT, SEM_STAT_ANY,
    SHM_STAT, SHM_STAT_ANY, frame, mq_attr, msqid64_ds, sched_attr, sembuf, semid64_ds, shmid64_ds,
    tms,
};
use crate::memory::page_down;
use crate::system::{AccessKind, CallMemory};

/// What one thread's end touches: its lock list and the word that tells joiners it is gone.
#[derive(Clone, Copy)]
pub(crate) struct ThreadEnd {
    pub tid: i32,
    /// The address cleared at the end, if one is.
    pub clear_child_tid: Option<u64>,
    pub robust_list: u64,
}

/// Which threads a call ends.
#[derive(Clone, Copy)]
pub(crate) enum Ending {
    Caller,
    Process,
    /// Every thread of the process but the caller, and then the caller's release of the memory
    /// it leaves, as `execve` does.
    Exec,
}

/// The descriptor whose file a call reads, which may copy a process's memory.
pub(crate) fn read_descriptor(call: SystemCallRequest) -> Option<u64> {
    let [a0, a1, ..] = call.arguments;
    match u32::try_from(call.number.0).ok()? {
        __NR_read | __NR_pread64 | __NR_readv | __NR_preadv | __NR_preadv2 | __NR_splice => {
            Some(a0)
        }
        __NR_sendfile => Some(a1),
        _ => None,
    }
}

/// The memory a read of descriptor `fd` copies, met before the caller's own.
fn file_memory<S: crate::System>(memory: &mut CallMemory<'_, '_, '_, S>, fd: u64) -> bool {
    match memory.file_memory(fd) {
        Some((pid, start, length)) => {
            length == 0 || memory.touch_process(pid, start, length, AccessKind::Read)
        }
        None => true,
    }
}

/// Meets a range; a null address or nothing touches nothing.
fn meet<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
    length: u64,
    access: AccessKind,
) -> bool {
    address == 0 || length == 0 || memory.touch(address, length, access)
}

fn sized<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
    size: usize,
    access: AccessKind,
) -> bool {
    meet(memory, address, size as u64, access)
}

/// Meets a range whose bytes the walk then reads; none if the call must wait, empty if it will
/// fault on them itself.
fn fetch<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
    size: usize,
    access: AccessKind,
) -> Option<Vec<u8>> {
    if !sized(memory, address, size, access) {
        return None;
    }
    let mut bytes = vec![0; size];
    if address == 0 || !memory.read(address, &mut bytes) {
        bytes.clear();
    }
    Some(bytes)
}

fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + size_of::<u64>()].try_into().expect("a word"))
}

fn half(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(
        bytes[at..at + size_of::<u32>()]
            .try_into()
            .expect("a half word"),
    )
}

/// A string the call reads up to its zero byte, at most `limit` bytes, a page at a time.
fn string<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
    limit: u64,
) -> bool {
    let end = address.saturating_add(limit);
    let mut at = address;
    while address != 0 && at < end {
        let to = (page_down(at) + PAGE_SIZE).min(end);
        let Some(bytes) = fetch(memory, at, (to - at) as usize, AccessKind::Read) else {
            return false;
        };
        if bytes.is_empty() || bytes.contains(&0) {
            break;
        }
        at = to;
    }
    true
}

fn path<S: crate::System>(memory: &mut CallMemory<'_, '_, '_, S>, address: u64) -> bool {
    string(memory, address, PATH_MAX.into())
}

fn name<S: crate::System>(memory: &mut CallMemory<'_, '_, '_, S>, address: u64) -> bool {
    string(memory, address, u64::from(XATTR_NAME_MAX) + 1)
}

/// A null-ended array of string pointers, as `execve`'s arguments and environment.
fn strings<S: crate::System>(memory: &mut CallMemory<'_, '_, '_, S>, mut array: u64) -> bool {
    while array != 0 {
        let Some(bytes) = fetch(memory, array, size_of::<u64>(), AccessKind::Read) else {
            return false;
        };
        if bytes.is_empty() || word(&bytes, 0) == 0 {
            break;
        }
        if !string(memory, word(&bytes, 0), super::process::MAX_ARG_STRLEN) {
            return false;
        }
        array += size_of::<u64>() as u64;
    }
    true
}

/// A `struct iovec` array, read, then each segment touched as `access`.
fn vectors<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    array: u64,
    count: u64,
    access: AccessKind,
) -> bool {
    segments(memory, array, count, access, |base| base)
}

/// As [`vectors`], each segment's address first passed through `address`.
fn segments<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    array: u64,
    count: u64,
    access: AccessKind,
    address: fn(u64) -> u64,
) -> bool {
    let size = size_of::<iovec>();
    let count = count.min(UIO_MAXIOV.into()) as usize;
    let Some(bytes) = fetch(memory, array, count * size, AccessKind::Read) else {
        return false;
    };
    bytes.chunks_exact(size).all(|entry| {
        meet(
            memory,
            address(word(entry, offset_of!(iovec, iov_base))),
            word(entry, offset_of!(iovec, iov_len)),
            access,
        )
    })
}

/// A `struct msghdr`, read, then its name, control data and segments touched as `access`; a
/// received message's lengths and flags go back into it.
fn message<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    header: u64,
    access: AccessKind,
) -> bool {
    let Some(bytes) = fetch(memory, header, size_of::<msghdr>(), AccessKind::Read) else {
        return false;
    };
    let returned = [
        (offset_of!(msghdr, msg_namelen), size_of::<c_int>()),
        (offset_of!(msghdr, msg_controllen), size_of::<u64>()),
        (offset_of!(msghdr, msg_flags), size_of::<c_uint>()),
    ];
    bytes.is_empty()
        || (access == AccessKind::Read
            || returned
                .into_iter()
                .all(|(at, size)| sized(memory, header + at as u64, size, AccessKind::Write)))
            && meet(
                memory,
                word(&bytes, offset_of!(msghdr, msg_name)),
                half(&bytes, offset_of!(msghdr, msg_namelen)).into(),
                access,
            )
            && meet(
                memory,
                word(&bytes, offset_of!(msghdr, msg_control)),
                word(&bytes, offset_of!(msghdr, msg_controllen)),
                access,
            )
            && vectors(
                memory,
                word(&bytes, offset_of!(msghdr, msg_iov)),
                word(&bytes, offset_of!(msghdr, msg_iovlen)),
                access,
            )
}

/// A `struct mmsghdr` array: each message as `access`, and the length the call stores in it.
fn messages<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    array: u64,
    count: u64,
    access: AccessKind,
) -> bool {
    let size = size_of::<mmsghdr>() as u64;
    (0..count.min(UIO_MAXIOV.into())).all(|index| {
        let entry = array + index * size;
        message(memory, entry, access)
            && sized(
                memory,
                entry + offset_of!(mmsghdr, msg_len) as u64,
                size_of::<u32>(),
                AccessKind::Write,
            )
    })
}

/// A socket address the call writes, after the length beside it, which it reads and writes.
fn address_out<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
    length: u64,
) -> bool {
    if address == 0 {
        return true;
    }
    let Some(bytes) = fetch(memory, length, size_of::<u32>(), AccessKind::Write) else {
        return false;
    };
    bytes.is_empty() || meet(memory, address, half(&bytes, 0).into(), AccessKind::Write)
}

/// `pselect6`'s last argument: a pointer to the signal mask and its size, then the mask.
fn mask_and_size<S: crate::System>(memory: &mut CallMemory<'_, '_, '_, S>, address: u64) -> bool {
    let size = size_of::<[u64; 2]>();
    let Some(bytes) = fetch(memory, address, size, AccessKind::Read) else {
        return false;
    };
    bytes.is_empty()
        || meet(
            memory,
            word(&bytes, 0),
            word(&bytes, size_of::<u64>()),
            AccessKind::Read,
        )
}

/// The futex word at `address` of a size given by `flags` (`futex2`'s size bits).
fn futex2_word<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
    flags: u64,
    access: AccessKind,
) -> bool {
    meet(
        memory,
        address,
        1 << (flags & u64::from(FUTEX2_SIZE_MASK)),
        access,
    )
}

/// A `struct futex_waitv` array and the words it names.
fn futex_waiters<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    array: u64,
    count: u64,
) -> bool {
    let size = size_of::<futex_waitv>();
    let count = count.min(FUTEX_WAITV_MAX.into()) as usize;
    let Some(bytes) = fetch(memory, array, count * size, AccessKind::Read) else {
        return false;
    };
    bytes.chunks_exact(size).all(|entry| {
        futex2_word(
            memory,
            word(entry, offset_of!(futex_waitv, uaddr)),
            half(entry, offset_of!(futex_waitv, flags)).into(),
            AccessKind::Read,
        )
    })
}

/// `futex`'s operations: the words and timeouts each reads or changes.
fn futex<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    [word, op, _, timeout, second, _]: super::Args,
) -> bool {
    let (read, write) = (AccessKind::Read, AccessKind::Write);
    let four = size_of::<u32>();
    let timespec = size_of::<__kernel_timespec>();
    match (op as u32) & (FUTEX_CMD_MASK as u32) {
        FUTEX_WAIT | FUTEX_WAIT_BITSET | FUTEX_WAIT_REQUEUE_PI => {
            sized(memory, word, four, read) && sized(memory, timeout, timespec, read)
        }
        FUTEX_CMP_REQUEUE => sized(memory, word, four, read),
        FUTEX_WAKE_OP => sized(memory, second, four, write),
        FUTEX_CMP_REQUEUE_PI => {
            sized(memory, word, four, read) && sized(memory, second, four, write)
        }
        FUTEX_LOCK_PI | FUTEX_LOCK_PI2 => {
            sized(memory, word, four, write) && sized(memory, timeout, timespec, read)
        }
        FUTEX_TRYLOCK_PI | FUTEX_UNLOCK_PI => sized(memory, word, four, write),
        _ => true,
    }
}

/// `ioctl`'s argument: as the request's number encodes it, or as the terminal requests that
/// predate the encoding define it.
fn ioctl<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    request: u64,
    argument: u64,
) -> bool {
    let request = request as u32;
    let (read, write) = (AccessKind::Read, AccessKind::Write);
    let int = size_of::<i32>();
    match request {
        TCGETS | TIOCGLCKTRMIOS => sized(memory, argument, size_of::<termios>(), write),
        TCSETS | TCSETSW | TCSETSF | TIOCSLCKTRMIOS => {
            sized(memory, argument, size_of::<termios>(), read)
        }
        TCGETA => sized(memory, argument, size_of::<termio>(), write),
        TCSETA | TCSETAW | TCSETAF => sized(memory, argument, size_of::<termio>(), read),
        TIOCGWINSZ => sized(memory, argument, size_of::<winsize>(), write),
        TIOCSWINSZ => sized(memory, argument, size_of::<winsize>(), read),
        TIOCGPGRP | TIOCGSID | FIONREAD | TIOCOUTQ | TIOCMGET | TIOCGETD | TIOCGSOFTCAR
        | FIGETBSZ => sized(memory, argument, int, write),
        TIOCSPGRP | TIOCMSET | TIOCMBIS | TIOCMBIC | TIOCSETD | TIOCSSOFTCAR | FIONBIO
        | FIOASYNC | TIOCPKT => sized(memory, argument, int, read),
        TIOCSTI => sized(memory, argument, size_of::<u8>(), read),
        FIDEDUPERANGE => {
            let Some(bytes) = fetch(memory, argument, size_of::<file_dedupe_range>(), write) else {
                return false;
            };
            bytes.is_empty() || {
                let at = offset_of!(file_dedupe_range, dest_count);
                let count = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
                meet(
                    memory,
                    argument,
                    (size_of::<file_dedupe_range>()
                        + usize::from(count) * size_of::<file_dedupe_range_info>())
                        as u64,
                    write,
                )
            }
        }
        _ => {
            let direction = (request >> _IOC_DIRSHIFT) & _IOC_DIRMASK;
            let size = u64::from((request >> _IOC_SIZESHIFT) & _IOC_SIZEMASK);
            if direction & _IOC_READ != 0 {
                meet(memory, argument, size, write)
            } else if direction & _IOC_WRITE != 0 {
                meet(memory, argument, size, read)
            } else {
                true
            }
        }
    }
}

/// `fcntl`'s commands that take a structure.
fn fcntl<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    command: u64,
    argument: u64,
) -> bool {
    let (read, write) = (AccessKind::Read, AccessKind::Write);
    match command as u32 {
        F_GETLK | F_OFD_GETLK => sized(memory, argument, size_of::<flock>(), write),
        F_SETLK | F_SETLKW | F_OFD_SETLK | F_OFD_SETLKW => {
            sized(memory, argument, size_of::<flock>(), read)
        }
        F_GETOWN_EX => sized(memory, argument, size_of::<f_owner_ex>(), write),
        F_SETOWN_EX => sized(memory, argument, size_of::<f_owner_ex>(), read),
        F_GET_RW_HINT | F_GET_FILE_RW_HINT => sized(memory, argument, size_of::<u64>(), write),
        F_SET_RW_HINT | F_SET_FILE_RW_HINT => sized(memory, argument, size_of::<u64>(), read),
        _ => true,
    }
}

/// `prctl`'s options that take memory.
fn prctl<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    [option, second, third, ..]: super::Args,
) -> bool {
    let write = AccessKind::Write;
    let int = size_of::<i32>();
    match option as u32 {
        PR_SET_NAME => string(memory, second, crate::process::NAME_LENGTH as u64 + 1),
        PR_GET_NAME => meet(
            memory,
            second,
            crate::process::NAME_LENGTH as u64 + 1,
            write,
        ),
        PR_GET_CHILD_SUBREAPER | PR_GET_PDEATHSIG => sized(memory, second, int, write),
        PR_GET_AUXV => meet(memory, second, third, write),
        PR_SET_MM if third != 0 && second == PR_SET_MM_MAP_SIZE.into() => {
            sized(memory, third, int, write)
        }
        PR_SET_SECCOMP if second == SECCOMP_SET_MODE_FILTER.into() => filter(memory, third),
        _ => true,
    }
}

/// A seccomp filter: its `struct sock_fprog`, then its instructions.
fn filter<S: crate::System>(memory: &mut CallMemory<'_, '_, '_, S>, program: u64) -> bool {
    let Some(bytes) = fetch(memory, program, size_of::<sock_fprog>(), AccessKind::Read) else {
        return false;
    };
    bytes.is_empty() || {
        let at = offset_of!(sock_fprog, len);
        let length = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        meet(
            memory,
            word(&bytes, offset_of!(sock_fprog, filter)),
            u64::from(length) * size_of::<sock_filter>() as u64,
            AccessKind::Read,
        )
    }
}

/// `sched_setattr`'s structure as `sched_copy_attr` reads it: its size, then that many bytes; a
/// size it refuses (E2BIG) is answered by writing the size it knows.
fn sched_attributes<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
) -> bool {
    let size_word = size_of::<u32>();
    let known = size_of::<sched_attr>();
    let Some(bytes) = fetch(memory, address, size_word, AccessKind::Read) else {
        return false;
    };
    if bytes.is_empty() {
        return true;
    }
    let size = match half(&bytes, 0) {
        0 => SCHED_ATTR_SIZE_VER0,
        size => size,
    };
    let refused = if size < SCHED_ATTR_SIZE_VER0 || u64::from(size) > PAGE_SIZE {
        true
    } else {
        let Some(bytes) = fetch(memory, address, size as usize, AccessKind::Read) else {
            return false;
        };
        bytes
            .get(known..)
            .is_some_and(|past| past.iter().any(|byte| *byte != 0))
    };
    !refused || sized(memory, address, size_word, AccessKind::Write)
}

/// The capability calls' header: an unknown version is answered by writing the one Linux
/// prefers (`cap_validate_magic`).
fn capability_header<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    address: u64,
) -> bool {
    let size = size_of::<__user_cap_header_struct>();
    let Some(bytes) = fetch(memory, address, size, AccessKind::Read) else {
        return false;
    };
    let version = offset_of!(__user_cap_header_struct, version);
    bytes.is_empty()
        || [
            _LINUX_CAPABILITY_VERSION_1,
            _LINUX_CAPABILITY_VERSION_2,
            _LINUX_CAPABILITY_VERSION_3,
        ]
        .contains(&half(&bytes, version))
        || sized(memory, address, size, AccessKind::Write)
}

/// The IPC control calls' buffer: written by the commands that show an object, read by
/// `IPC_SET`.
fn ipc_control<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    command: u64,
    buffer: u64,
    shows: &[i32],
    size: usize,
) -> bool {
    let command = command as i32;
    if shows.contains(&command) {
        sized(memory, buffer, size, AccessKind::Write)
    } else if command == IPC_SET {
        sized(memory, buffer, size, AccessKind::Read)
    } else {
        true
    }
}

/// What each thread's end touches: the lock list's head, entries and the words of the locks it
/// holds, then the word cleared for its joiners.
pub(crate) fn thread_ends<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    ends: &[ThreadEnd],
) -> bool {
    ends.iter().all(|end| {
        robust_list(memory, end.tid, end.robust_list)
            && end
                .clear_child_tid
                .is_none_or(|address| sized(memory, address, size_of::<u32>(), AccessKind::Write))
    })
}

/// `exit_robust_list`'s walk: every entry is read, and a lock word written where the thread
/// holds it.
fn robust_list<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    tid: i32,
    head: u64,
) -> bool {
    let Some(bytes) = fetch(
        memory,
        head,
        size_of::<robust_list_head>(),
        AccessKind::Read,
    ) else {
        return false;
    };
    if bytes.is_empty() {
        return true;
    }
    let offset = word(&bytes, offset_of!(robust_list_head, futex_offset));
    let pending = word(&bytes, offset_of!(robust_list_head, list_op_pending)) & !1;
    let lock = |memory: &mut CallMemory<'_, '_, '_, S>, entry: u64| {
        let address = entry.wrapping_add(offset);
        let Some(value) = fetch(memory, address, size_of::<u32>(), AccessKind::Read) else {
            return false;
        };
        value.is_empty()
            || half(&value, 0) & FUTEX_TID_MASK != tid as u32
            || sized(memory, address, size_of::<u32>(), AccessKind::Write)
    };
    let mut entry = word(&bytes, offset_of!(robust_list_head, list)) & !1;
    for _ in 0..super::futex::ROBUST_LIST_LIMIT {
        if entry == head {
            break;
        }
        let Some(next) = fetch(memory, entry, size_of::<u64>(), AccessKind::Read) else {
            return false;
        };
        if entry != pending && !lock(memory, entry) {
            return false;
        }
        if next.is_empty() {
            return true;
        }
        entry = word(&next, 0) & !1;
    }
    pending == 0 || lock(memory, pending)
}

/// Meets the watches on what `call` will touch, as far as its arguments say; false if it must
/// wait.
pub(crate) fn touch<S: crate::System>(
    memory: &mut CallMemory<'_, '_, '_, S>,
    call: SystemCallRequest,
) -> bool {
    let arguments = call.arguments;
    let [a0, a1, a2, a3, a4, a5] = arguments;
    let Ok(number) = u32::try_from(call.number.0) else {
        return true;
    };
    let (read, write) = (AccessKind::Read, AccessKind::Write);
    let timespec = size_of::<__kernel_timespec>();
    let int = size_of::<i32>();
    let offset = size_of::<i64>();
    if let Some(fd) = read_descriptor(call)
        && !file_memory(memory, fd)
    {
        return false;
    }
    match number {
        __NR_read | __NR_pread64 | __NR_getdents64 => meet(memory, a1, a2, write),
        __NR_getrandom | __NR_getcwd => meet(memory, a0, a1, write),
        __NR_write | __NR_pwrite64 => meet(memory, a1, a2, read),
        __NR_readv | __NR_preadv | __NR_preadv2 => vectors(memory, a1, a2, write),
        __NR_writev | __NR_pwritev | __NR_pwritev2 => vectors(memory, a1, a2, read),
        __NR_sendfile => sized(memory, a2, offset, write),
        __NR_copy_file_range | __NR_splice => {
            sized(memory, a1, offset, write) && sized(memory, a3, offset, write)
        }
        __NR_vmsplice => match memory.writable(a0) {
            Some(true) => vectors(memory, a1, a2, read),
            Some(false) => vectors(memory, a1, a2, write),
            None => true,
        },
        __NR_fcntl => fcntl(memory, a1, a2),
        __NR_ioctl => ioctl(memory, a1, a2),
        __NR_ppoll => {
            meet(
                memory,
                a0,
                a1.saturating_mul(size_of::<pollfd>() as u64),
                write,
            ) && sized(memory, a2, timespec, write)
                && meet(memory, a3, a4, read)
        }
        __NR_pselect6 => {
            let set = a0.div_ceil(u64::BITS.into()) * size_of::<u64>() as u64;
            [a1, a2, a3]
                .into_iter()
                .all(|fds| meet(memory, fds, set, write))
                && sized(memory, a4, timespec, write)
                && mask_and_size(memory, a5)
        }
        __NR_cachestat => {
            sized(memory, a1, size_of::<cachestat_range>(), read)
                && sized(memory, a2, size_of::<cachestat>(), write)
        }
        __NR_name_to_handle_at => {
            let mount_id = if a4 as u32 & AT_HANDLE_MNT_ID_UNIQUE != 0 {
                size_of::<u64>()
            } else {
                int
            };
            path(memory, a1)
                && meet(
                    memory,
                    a2,
                    2 * int as u64 + u64::from(crate::linux_extra::MAX_HANDLE_SZ),
                    write,
                )
                && sized(memory, a3, mount_id, write)
        }
        __NR_pidfd_send_signal => sized(memory, a2, size_of::<siginfo>(), read),
        __NR_rt_sigqueueinfo => sized(memory, a2, size_of::<siginfo>(), read),
        __NR_rt_tgsigqueueinfo => sized(memory, a3, size_of::<siginfo>(), read),
        __NR_signalfd4 => meet(memory, a1, a2, read),
        __NR_rt_sigsuspend => meet(memory, a0, a1, read),
        __NR_inotify_add_watch => path(memory, a1),
        __NR_sendmmsg => messages(memory, a1, a2, read),
        __NR_recvmmsg => messages(memory, a1, a2, write) && sized(memory, a4, timespec, write),
        __NR_bind | __NR_connect => meet(memory, a1, a2, read),
        __NR_getsockname | __NR_getpeername | __NR_accept | __NR_accept4 => {
            address_out(memory, a1, a2)
        }
        __NR_recvfrom => meet(memory, a1, a2, write) && address_out(memory, a4, a5),
        __NR_sendto => meet(memory, a1, a2, read) && meet(memory, a4, a5, read),
        __NR_recvmsg => message(memory, a1, write),
        __NR_sendmsg => message(memory, a1, read),
        __NR_setsockopt => meet(memory, a3, a4, read),
        __NR_getsockopt => address_out(memory, a3, a4),
        __NR_epoll_ctl => sized(memory, a3, size_of::<epoll_event>(), read),
        __NR_epoll_pwait | __NR_epoll_pwait2 => {
            meet(
                memory,
                a1,
                a2.saturating_mul(size_of::<epoll_event>() as u64),
                write,
            ) && (number == __NR_epoll_pwait || sized(memory, a3, timespec, read))
                && meet(memory, a4, a5, read)
        }
        __NR_openat | __NR_mkdirat | __NR_mknodat | __NR_unlinkat | __NR_faccessat
        | __NR_faccessat2 | __NR_fchmodat | __NR_fchmodat2 | __NR_fchownat => path(memory, a1),
        __NR_openat2 => path(memory, a1) && meet(memory, a2, a3, read),
        __NR_symlinkat => path(memory, a0) && path(memory, a2),
        __NR_linkat | __NR_renameat | __NR_renameat2 => path(memory, a1) && path(memory, a3),
        __NR_readlinkat => path(memory, a1) && meet(memory, a2, a3, write),
        __NR_newfstatat => path(memory, a1) && sized(memory, a2, size_of::<stat>(), write),
        __NR_fstat => sized(memory, a1, size_of::<stat>(), write),
        __NR_statx => path(memory, a1) && sized(memory, a4, size_of::<statx>(), write),
        __NR_utimensat => path(memory, a1) && sized(memory, a2, 2 * timespec, read),
        __NR_truncate | __NR_chdir | __NR_chroot | __NR_memfd_create | __NR_mq_unlink => {
            path(memory, a0)
        }
        __NR_statfs => path(memory, a0) && sized(memory, a1, size_of::<statfs>(), write),
        __NR_fstatfs => sized(memory, a1, size_of::<statfs>(), write),
        __NR_setxattr | __NR_lsetxattr => {
            path(memory, a0) && name(memory, a1) && meet(memory, a2, a3, read)
        }
        __NR_fsetxattr => name(memory, a1) && meet(memory, a2, a3, read),
        __NR_getxattr | __NR_lgetxattr => {
            path(memory, a0) && name(memory, a1) && meet(memory, a2, a3, write)
        }
        __NR_fgetxattr => name(memory, a1) && meet(memory, a2, a3, write),
        __NR_listxattr | __NR_llistxattr => path(memory, a0) && meet(memory, a1, a2, write),
        __NR_flistxattr => meet(memory, a1, a2, write),
        __NR_removexattr | __NR_lremovexattr => path(memory, a0) && name(memory, a1),
        __NR_fremovexattr => name(memory, a1),
        __NR_mincore => meet(memory, a2, a1.div_ceil(PAGE_SIZE), write),
        // Pages put back to zero or to their file change as if written.
        __NR_madvise => match a2 as u32 {
            MADV_DONTNEED | MADV_DONTNEED_LOCKED | MADV_REMOVE => meet(memory, a0, a1, write),
            _ => true,
        },
        // A move reads the pages it takes along; with `MREMAP_DONTUNMAP` it also puts the old
        // ones back to zero or to their file.
        __NR_mremap => {
            let flags = a3 as u32;
            if flags & MREMAP_DONTUNMAP != 0 {
                meet(memory, a0, a1, write)
            } else if flags & MREMAP_MAYMOVE != 0 {
                meet(memory, a0, a1.min(a2), read)
            } else {
                true
            }
        }
        // The other side's segments are in the caller's memory too when the task is its own,
        // found with their tags stripped as `get_user_pages` finds them.
        __NR_process_vm_readv | __NR_process_vm_writev => {
            let (local, remote) = if number == __NR_process_vm_readv {
                (write, read)
            } else {
                (read, write)
            };
            vectors(memory, a1, a2, local)
                && if memory.own_task(a0) {
                    segments(memory, a3, a4, remote, crate::user::untagged)
                } else {
                    meet(
                        memory,
                        a3,
                        a4.min(UIO_MAXIOV.into()) * size_of::<iovec>() as u64,
                        read,
                    )
                }
        }
        __NR_execve | __NR_execveat => {
            let (file, arguments, environment) = if number == __NR_execve {
                (a0, a1, a2)
            } else {
                (a1, a2, a3)
            };
            let ends = memory.thread_ends(Ending::Exec);
            path(memory, file)
                && strings(memory, arguments)
                && strings(memory, environment)
                && thread_ends(memory, &ends)
        }
        __NR_exit => {
            let ends = memory.thread_ends(Ending::Caller);
            thread_ends(memory, &ends)
        }
        __NR_exit_group => {
            let ends = memory.thread_ends(Ending::Process);
            thread_ends(memory, &ends)
        }
        __NR_getresuid | __NR_getresgid => [a0, a1, a2]
            .into_iter()
            .all(|id| sized(memory, id, int, write)),
        __NR_getgroups | __NR_setgroups => {
            let access = if number == __NR_getgroups {
                write
            } else {
                read
            };
            meet(memory, a1, (a0 as i32).max(0) as u64 * int as u64, access)
        }
        __NR_capget => {
            capability_header(memory, a0)
                && sized(memory, a1, 2 * size_of::<__user_cap_data_struct>(), write)
        }
        __NR_capset => {
            capability_header(memory, a0)
                && sized(memory, a1, 2 * size_of::<__user_cap_data_struct>(), read)
        }
        __NR_get_robust_list => {
            sized(memory, a1, size_of::<u64>(), write) && sized(memory, a2, size_of::<u64>(), write)
        }
        __NR_rt_sigaction => {
            sized(memory, a1, size_of::<kernel_sigaction>(), read)
                && sized(memory, a2, size_of::<kernel_sigaction>(), write)
        }
        __NR_rt_sigprocmask => meet(memory, a1, a3, read) && meet(memory, a2, a3, write),
        __NR_rt_sigpending => meet(memory, a0, a1, write),
        __NR_sigaltstack => {
            sized(memory, a0, size_of::<sigaltstack>(), read)
                && sized(memory, a1, size_of::<sigaltstack>(), write)
        }
        __NR_rt_sigtimedwait => {
            meet(memory, a0, a3, read)
                && sized(memory, a1, size_of::<siginfo>(), write)
                && sized(memory, a2, timespec, read)
        }
        // The frame a handler's return reads back, at its stack pointer.
        __NR_rt_sigreturn => {
            let sp = memory.stack_pointer();
            meet(memory, sp, frame::SIZE, read)
        }
        // The parent's copy of the child's number or its descriptor for the child, and the
        // child's own copy, in memory the child shares or copies with its watches.
        // A vfork parent going on once its child let go of the memory stores nothing more.
        __NR_clone => {
            let flags = a0 as u32;
            memory.resuming()
                || (flags & (CLONE_PARENT_SETTID | CLONE_PIDFD) == 0
                    || sized(memory, a2, int, write))
                    && (flags & CLONE_CHILD_SETTID == 0 || sized(memory, a4, int, write))
        }
        __NR_futex => futex(memory, arguments),
        __NR_futex_waitv => futex_waiters(memory, a0, a1) && sized(memory, a3, timespec, read),
        __NR_futex_wait => futex2_word(memory, a0, a3, read) && sized(memory, a4, timespec, read),
        __NR_futex_requeue => futex_waiters(memory, a0, 2),
        __NR_wait4 => {
            sized(memory, a1, int, write) && sized(memory, a3, size_of::<rusage>(), write)
        }
        __NR_waitid => {
            sized(memory, a2, size_of::<siginfo>(), write)
                && sized(memory, a4, size_of::<rusage>(), write)
        }
        __NR_pipe2 => sized(memory, a0, 2 * int, write),
        __NR_socketpair => sized(memory, a3, 2 * int, write),
        __NR_prlimit64 => {
            sized(memory, a2, size_of::<rlimit64>(), read)
                && sized(memory, a3, size_of::<rlimit64>(), write)
        }
        __NR_getrlimit => sized(memory, a1, size_of::<rlimit>(), write),
        __NR_setrlimit => sized(memory, a1, size_of::<rlimit>(), read),
        __NR_getrusage => sized(memory, a1, size_of::<rusage>(), write),
        __NR_prctl => prctl(memory, arguments),
        __NR_seccomp => match a0 as u32 {
            SECCOMP_SET_MODE_FILTER => filter(memory, a2),
            SECCOMP_GET_ACTION_AVAIL => sized(memory, a2, size_of::<u32>(), read),
            SECCOMP_GET_NOTIF_SIZES => sized(memory, a2, size_of::<seccomp_notif_sizes>(), write),
            _ => true,
        },
        __NR_rseq => meet(memory, a0, a1, write),
        __NR_clock_gettime | __NR_clock_getres | __NR_sched_rr_get_interval => {
            sized(memory, a1, timespec, write)
        }
        __NR_gettimeofday => {
            sized(memory, a0, size_of::<timeval>(), write)
                && sized(memory, a1, size_of::<timezone>(), write)
        }
        __NR_nanosleep => sized(memory, a0, timespec, read) && sized(memory, a1, timespec, write),
        __NR_clock_nanosleep => {
            sized(memory, a2, timespec, read) && sized(memory, a3, timespec, write)
        }
        __NR_times => sized(memory, a0, size_of::<tms>(), write),
        __NR_getitimer => sized(memory, a1, size_of::<itimerval>(), write),
        __NR_setitimer => {
            sized(memory, a1, size_of::<itimerval>(), read)
                && sized(memory, a2, size_of::<itimerval>(), write)
        }
        __NR_timer_create => {
            sized(memory, a1, size_of::<sigevent>(), read) && sized(memory, a2, int, write)
        }
        __NR_timer_settime | __NR_timerfd_settime => {
            sized(memory, a2, size_of::<__kernel_itimerspec>(), read)
                && sized(memory, a3, size_of::<__kernel_itimerspec>(), write)
        }
        __NR_timer_gettime | __NR_timerfd_gettime => {
            sized(memory, a1, size_of::<__kernel_itimerspec>(), write)
        }
        __NR_uname => sized(memory, a0, size_of::<new_utsname>(), write),
        __NR_sysinfo => sized(memory, a0, size_of::<sysinfo>(), write),
        __NR_sethostname | __NR_setdomainname => meet(memory, a0, a1, read),
        __NR_sched_getaffinity => meet(memory, a2, a1, write),
        __NR_sched_setaffinity => meet(memory, a2, a1, read),
        __NR_sched_getparam => sized(memory, a1, int, write),
        __NR_sched_setparam => sized(memory, a1, int, read),
        __NR_sched_setscheduler => sized(memory, a2, int, read),
        __NR_getcpu => sized(memory, a0, int, write) && sized(memory, a1, int, write),
        __NR_sched_setattr => sched_attributes(memory, a1),
        __NR_sched_getattr => meet(memory, a1, a2.min(size_of::<sched_attr>() as u64), write),
        __NR_shmctl => ipc_control(
            memory,
            a1,
            a2,
            &[IPC_STAT, SHM_STAT, SHM_STAT_ANY],
            size_of::<shmid64_ds>(),
        ),
        __NR_msgctl => ipc_control(
            memory,
            a1,
            a2,
            &[IPC_STAT, MSG_STAT, MSG_STAT_ANY],
            size_of::<msqid64_ds>(),
        ),
        __NR_semctl => ipc_control(
            memory,
            a2,
            a3,
            &[IPC_STAT, SEM_STAT, SEM_STAT_ANY],
            size_of::<semid64_ds>(),
        ),
        __NR_msgsnd => meet(memory, a1, size_of::<i64>() as u64 + a2, read),
        __NR_msgrcv => meet(memory, a1, size_of::<i64>() as u64 + a2, write),
        __NR_semop | __NR_semtimedop => {
            meet(memory, a1, a2 * size_of::<sembuf>() as u64, read)
                && (number == __NR_semop || sized(memory, a3, timespec, read))
        }
        __NR_mq_open => path(memory, a0) && sized(memory, a3, size_of::<mq_attr>(), read),
        __NR_mq_timedsend => meet(memory, a1, a2, read) && sized(memory, a4, timespec, read),
        __NR_mq_timedreceive => {
            meet(memory, a1, a2, write)
                && sized(memory, a3, int, write)
                && sized(memory, a4, timespec, read)
        }
        __NR_mq_notify => sized(memory, a1, size_of::<sigevent>(), read),
        __NR_mq_getsetattr => {
            sized(memory, a1, size_of::<mq_attr>(), read)
                && sized(memory, a2, size_of::<mq_attr>(), write)
        }
        _ => true,
    }
}
