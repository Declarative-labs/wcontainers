//! The program memory a system call will touch, known from its arguments before it runs, so that
//! the embedder's watches are met while the call can still be repeated. A call not described here
//! meets them at its copies instead (`Kernel::system_call`).

#![allow(non_upper_case_globals, reason = "Linux's names for its call numbers")]

use core::mem::{offset_of, size_of};

use wcontainers_abi::Syscall;

use crate::linux::general::*;
use crate::linux::net::msghdr;
use crate::linux::system::{new_utsname, sysinfo};
use crate::linux_extra::frame;
use crate::system::Access;

/// The calling thread's memory as the walk sees it.
pub(crate) trait Memory {
    /// Meets the watches on a range; false if the call must wait.
    fn touch(&mut self, address: u64, length: u64, access: Access) -> bool;
    /// Bytes the call reads to learn what else it touches, once touched; false if unreadable.
    fn read(&mut self, address: u64, into: &mut [u8]) -> bool;
    fn stack_pointer(&mut self) -> u64;
}

/// Meets a range; a null address or nothing touches nothing.
fn meet(memory: &mut dyn Memory, address: u64, length: u64, access: Access) -> bool {
    address == 0 || length == 0 || memory.touch(address, length, access)
}

fn sized(memory: &mut dyn Memory, address: u64, size: usize, access: Access) -> bool {
    meet(memory, address, size as u64, access)
}

fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + size_of::<u64>()].try_into().expect("a word"))
}

/// A `struct iovec` array, read, then each segment touched as `access`.
fn vectors(memory: &mut dyn Memory, array: u64, count: u64, access: Access) -> bool {
    let size = size_of::<iovec>() as u64;
    let count = count.min(UIO_MAXIOV.into());
    if !meet(memory, array, count * size, Access::Read) {
        return false;
    }
    for index in 0..count {
        let mut entry = [0; size_of::<iovec>()];
        if !memory.read(array + index * size, &mut entry) {
            return true;
        }
        let (base, length) = (
            word(&entry, offset_of!(iovec, iov_base)),
            word(&entry, offset_of!(iovec, iov_len)),
        );
        if !meet(memory, base, length, access) {
            return false;
        }
    }
    true
}

/// A `struct msghdr`, read, then its name, control data and segments touched as `access`.
fn message(memory: &mut dyn Memory, header: u64, access: Access) -> bool {
    if !sized(memory, header, size_of::<msghdr>(), Access::Read) {
        return false;
    }
    let mut bytes = [0; size_of::<msghdr>()];
    if header == 0 || !memory.read(header, &mut bytes) {
        return true;
    }
    let name_length = u32::from_le_bytes(
        bytes[offset_of!(msghdr, msg_namelen)..][..size_of::<u32>()]
            .try_into()
            .expect("a length"),
    );
    meet(
        memory,
        word(&bytes, offset_of!(msghdr, msg_name)),
        name_length.into(),
        access,
    ) && meet(
        memory,
        word(&bytes, offset_of!(msghdr, msg_control)),
        word(&bytes, offset_of!(msghdr, msg_controllen)),
        access,
    ) && vectors(
        memory,
        word(&bytes, offset_of!(msghdr, msg_iov)),
        word(&bytes, offset_of!(msghdr, msg_iovlen)),
        access,
    )
}

/// A socket address the call writes, after the length beside it, which it reads and writes.
fn address_out(memory: &mut dyn Memory, address: u64, length: u64) -> bool {
    if address == 0 {
        return true;
    }
    if !sized(memory, length, size_of::<u32>(), Access::Write) {
        return false;
    }
    let mut bytes = [0; size_of::<u32>()];
    !memory.read(length, &mut bytes)
        || meet(
            memory,
            address,
            u32::from_le_bytes(bytes).into(),
            Access::Write,
        )
}

/// Meets the watches on what `call` will touch, as far as its arguments say; false if it must
/// wait.
pub(crate) fn touch(memory: &mut dyn Memory, call: Syscall) -> bool {
    let [a0, a1, a2, a3, a4, a5] = call.arguments;
    let Ok(number) = u32::try_from(call.number) else {
        return true;
    };
    let (read, write) = (Access::Read, Access::Write);
    match number {
        __NR_read | __NR_pread64 | __NR_getdents64 => meet(memory, a1, a2, write),
        __NR_getrandom | __NR_getcwd => meet(memory, a0, a1, write),
        __NR_write | __NR_pwrite64 => meet(memory, a1, a2, read),
        __NR_readv | __NR_preadv | __NR_preadv2 => vectors(memory, a1, a2, write),
        __NR_writev | __NR_pwritev | __NR_pwritev2 => vectors(memory, a1, a2, read),
        __NR_recvfrom => meet(memory, a1, a2, write) && address_out(memory, a4, a5),
        __NR_sendto => meet(memory, a1, a2, read) && meet(memory, a4, a5, read),
        __NR_recvmsg => message(memory, a1, write),
        __NR_sendmsg => message(memory, a1, read),
        __NR_accept | __NR_accept4 => address_out(memory, a1, a2),
        __NR_readlinkat => meet(memory, a2, a3, write),
        __NR_wait4 => {
            sized(memory, a1, size_of::<i32>(), write)
                && sized(memory, a3, size_of::<rusage>(), write)
        }
        __NR_waitid => {
            sized(memory, a2, size_of::<siginfo>(), write)
                && sized(memory, a4, size_of::<rusage>(), write)
        }
        __NR_pipe2 => sized(memory, a0, 2 * size_of::<i32>(), write),
        // The parent's copy of the child's number, or its descriptor for the child.
        __NR_clone if a0 & u64::from(CLONE_PARENT_SETTID | CLONE_PIDFD) != 0 => {
            sized(memory, a2, size_of::<i32>(), write)
        }
        __NR_socketpair => sized(memory, a3, 2 * size_of::<i32>(), write),
        __NR_clock_gettime | __NR_clock_getres => {
            sized(memory, a1, size_of::<__kernel_timespec>(), write)
        }
        __NR_gettimeofday => sized(memory, a0, size_of::<timeval>(), write),
        __NR_nanosleep => {
            sized(memory, a0, size_of::<__kernel_timespec>(), read)
                && sized(memory, a1, size_of::<__kernel_timespec>(), write)
        }
        __NR_clock_nanosleep => {
            sized(memory, a2, size_of::<__kernel_timespec>(), read)
                && sized(memory, a3, size_of::<__kernel_timespec>(), write)
        }
        __NR_fstat => sized(memory, a1, size_of::<stat>(), write),
        __NR_newfstatat => sized(memory, a2, size_of::<stat>(), write),
        __NR_statx => sized(memory, a4, size_of::<statx>(), write),
        __NR_uname => sized(memory, a0, size_of::<new_utsname>(), write),
        __NR_sysinfo => sized(memory, a0, size_of::<sysinfo>(), write),
        __NR_getrusage => sized(memory, a1, size_of::<rusage>(), write),
        __NR_prlimit64 => {
            sized(memory, a2, size_of::<rlimit64>(), read)
                && sized(memory, a3, size_of::<rlimit64>(), write)
        }
        __NR_rt_sigaction => {
            sized(memory, a1, size_of::<kernel_sigaction>(), read)
                && sized(memory, a2, size_of::<kernel_sigaction>(), write)
        }
        __NR_rt_sigprocmask => meet(memory, a1, a3, read) && meet(memory, a2, a3, write),
        __NR_epoll_pwait | __NR_epoll_pwait2 => meet(
            memory,
            a1,
            a2.saturating_mul(size_of::<epoll_event>() as u64),
            write,
        ),
        __NR_ppoll => meet(
            memory,
            a0,
            a1.saturating_mul(size_of::<pollfd>() as u64),
            write,
        ),
        // The frame a handler's return reads back, at its stack pointer.
        __NR_rt_sigreturn => {
            let sp = memory.stack_pointer();
            meet(memory, sp, frame::SIZE, read)
        }
        _ => true,
    }
}
