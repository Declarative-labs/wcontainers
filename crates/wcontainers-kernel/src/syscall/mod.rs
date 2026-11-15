//! The one dispatch table: an entry for every ARM64 Linux system call, by number.
//!
//! Each implemented call is one typed Rust function. A call Linux has that is not built yet returns
//! `ENOSYS`, is counted and warns. So does a number Linux does not have.

mod files;
mod io;
mod list;
mod memfd;
mod memory;
mod process;
mod system;
mod time;
mod xattr;

use wcontainers_abi::Syscall;

pub(crate) use xattr::CAPABILITY;

use crate::counters::reason;
use crate::process::Tid;
use crate::user::Guest;
use crate::{Context, Errno, Kernel};

/// A call's six raw arguments.
pub(crate) type Args = [u64; 6];

/// How a system call ended for its thread.
pub(crate) enum Outcome {
    /// Put this in x0 and carry on.
    Return(u64),
    /// The thread's registers were replaced (a new program); it starts from them.
    Restart,
    /// The thread is waiting; it runs again when what it waits for happens.
    Parked,
    /// The thread no longer exists.
    Gone,
}

impl From<Result<u64, Errno>> for Outcome {
    fn from(result: Result<u64, Errno>) -> Self {
        Outcome::Return(match result {
            Ok(value) => value,
            Err(errno) => errno.to_return(),
        })
    }
}

/// A call that ends with a value or an error.
type Simple = fn(&mut Kernel, &mut Context<'_>, Tid, Args) -> Result<u64, Errno>;
/// A call that may also end its thread, park it or replace its registers.
type Full = fn(&mut Kernel, &mut Context<'_>, Tid, Args) -> Outcome;

#[derive(Clone, Copy)]
enum Handler {
    Simple(Simple),
    Full(Full),
    NotImplemented,
    Unknown,
}

#[derive(Clone, Copy)]
struct Entry {
    /// Linux's name for the call; empty where Linux has no call with this number.
    name: &'static str,
    handler: Handler,
}

/// One more than the highest call number Linux has.
const LENGTH: usize = {
    let mut highest = 0;
    let mut index = 0;
    while index < list::LIST.len() {
        if list::LIST[index].0 > highest {
            highest = list::LIST[index].0;
        }
        index += 1;
    }
    highest as usize + 1
};

static TABLE: [Entry; LENGTH] = {
    use crate::linux::general::*;
    use Handler::{Full as F, Simple as S};

    let mut table = [Entry {
        name: "",
        handler: Handler::Unknown,
    }; LENGTH];
    let mut index = 0;
    while index < list::LIST.len() {
        let (number, name) = list::LIST[index];
        table[number as usize] = Entry {
            name,
            handler: Handler::NotImplemented,
        };
        index += 1;
    }

    let implemented: &[(u32, Handler)] = &[
        // Files and descriptors.
        (__NR_read, S(io::read)),
        (__NR_write, S(io::write)),
        (__NR_pread64, S(io::pread64)),
        (__NR_pwrite64, S(io::pwrite64)),
        (__NR_readv, S(io::readv)),
        (__NR_writev, S(io::writev)),
        (__NR_preadv, S(io::preadv)),
        (__NR_pwritev, S(io::pwritev)),
        (__NR_lseek, S(io::lseek)),
        (__NR_sendfile, S(io::sendfile)),
        (__NR_getdents64, S(io::getdents64)),
        (__NR_close, S(io::close)),
        (__NR_close_range, S(io::close_range)),
        (__NR_preadv2, S(io::preadv2)),
        (__NR_pwritev2, S(io::pwritev2)),
        (__NR_copy_file_range, S(io::copy_file_range)),
        (__NR_fadvise64, S(io::fadvise64)),
        (__NR_readahead, S(io::readahead)),
        (__NR_sync_file_range, S(io::sync_file_range)),
        (__NR_dup, S(io::dup)),
        (__NR_dup3, S(io::dup3)),
        (__NR_fcntl, S(io::fcntl)),
        (__NR_ioctl, S(io::ioctl)),
        (__NR_ppoll, F(io::ppoll)),
        (__NR_pselect6, F(io::pselect6)),
        (__NR_fsync, S(io::fsync)),
        (__NR_fdatasync, S(io::fsync)),
        (__NR_syncfs, S(io::syncfs)),
        (__NR_sync, S(io::sync)),
        (__NR_openat, S(files::openat)),
        (__NR_openat2, S(files::openat2)),
        (__NR_flock, F(files::flock)),
        (__NR_mkdirat, S(files::mkdirat)),
        (__NR_mknodat, S(files::mknodat)),
        (__NR_symlinkat, S(files::symlinkat)),
        (__NR_linkat, S(files::linkat)),
        (__NR_unlinkat, S(files::unlinkat)),
        (__NR_renameat, S(files::renameat)),
        (__NR_renameat2, S(files::renameat2)),
        (__NR_readlinkat, S(files::readlinkat)),
        (__NR_newfstatat, S(files::newfstatat)),
        (__NR_fstat, S(files::fstat)),
        (__NR_statx, S(files::statx)),
        (__NR_faccessat, S(files::faccessat)),
        (__NR_faccessat2, S(files::faccessat2)),
        (__NR_fchmod, S(files::fchmod)),
        (__NR_fchmodat, S(files::fchmodat)),
        (__NR_fchmodat2, S(files::fchmodat2)),
        (__NR_fchown, S(files::fchown)),
        (__NR_fchownat, S(files::fchownat)),
        (__NR_utimensat, S(files::utimensat)),
        (__NR_truncate, S(files::truncate)),
        (__NR_ftruncate, S(files::ftruncate)),
        (__NR_fallocate, S(files::fallocate)),
        (__NR_getcwd, S(files::getcwd)),
        (__NR_chdir, S(files::chdir)),
        (__NR_fchdir, S(files::fchdir)),
        (__NR_chroot, S(files::chroot)),
        (__NR_umask, S(files::umask)),
        (__NR_statfs, S(files::statfs)),
        (__NR_fstatfs, S(files::fstatfs)),
        (__NR_memfd_create, S(memfd::memfd_create)),
        (__NR_setxattr, S(xattr::setxattr)),
        (__NR_lsetxattr, S(xattr::lsetxattr)),
        (__NR_fsetxattr, S(xattr::fsetxattr)),
        (__NR_getxattr, S(xattr::getxattr)),
        (__NR_lgetxattr, S(xattr::lgetxattr)),
        (__NR_fgetxattr, S(xattr::fgetxattr)),
        (__NR_listxattr, S(xattr::listxattr)),
        (__NR_llistxattr, S(xattr::llistxattr)),
        (__NR_flistxattr, S(xattr::flistxattr)),
        (__NR_removexattr, S(xattr::removexattr)),
        (__NR_lremovexattr, S(xattr::lremovexattr)),
        (__NR_fremovexattr, S(xattr::fremovexattr)),
        // Memory.
        (__NR_brk, S(memory::brk)),
        (__NR_mmap, S(memory::mmap)),
        (__NR_munmap, S(memory::munmap)),
        (__NR_mprotect, S(memory::mprotect)),
        (__NR_madvise, S(memory::madvise)),
        (__NR_mremap, S(memory::mremap)),
        (__NR_msync, S(memory::msync)),
        (__NR_mlock, S(memory::mlock)),
        (__NR_mlock2, S(memory::mlock2)),
        (__NR_munlock, S(memory::munlock)),
        (__NR_mlockall, S(memory::mlockall)),
        (__NR_munlockall, S(memory::munlockall)),
        (__NR_mincore, S(memory::mincore)),
        // The process and its thread.
        (__NR_exit, F(process::exit)),
        (__NR_exit_group, F(process::exit_group)),
        (__NR_execve, F(process::execve)),
        (__NR_getpid, S(process::getpid)),
        (__NR_getppid, S(process::getppid)),
        (__NR_gettid, S(process::gettid)),
        (__NR_getpgid, S(process::getpgid)),
        (__NR_setpgid, S(process::setpgid)),
        (__NR_getsid, S(process::getsid)),
        (__NR_setsid, S(process::setsid)),
        (__NR_getuid, S(process::getuid)),
        (__NR_geteuid, S(process::geteuid)),
        (__NR_getgid, S(process::getgid)),
        (__NR_getegid, S(process::getegid)),
        (__NR_getresuid, S(process::getresuid)),
        (__NR_getresgid, S(process::getresgid)),
        (__NR_setuid, S(process::setuid)),
        (__NR_setgid, S(process::setgid)),
        (__NR_setreuid, S(process::setreuid)),
        (__NR_setregid, S(process::setregid)),
        (__NR_setresuid, S(process::setresuid)),
        (__NR_setresgid, S(process::setresgid)),
        (__NR_setfsuid, S(process::setfsuid)),
        (__NR_setfsgid, S(process::setfsgid)),
        (__NR_getgroups, S(process::getgroups)),
        (__NR_setgroups, S(process::setgroups)),
        (__NR_capget, S(process::capget)),
        (__NR_capset, S(process::capset)),
        (__NR_set_tid_address, S(process::set_tid_address)),
        (__NR_set_robust_list, S(process::set_robust_list)),
        (__NR_get_robust_list, S(process::get_robust_list)),
        (__NR_rt_sigaction, S(process::rt_sigaction)),
        (__NR_rt_sigprocmask, S(process::rt_sigprocmask)),
        (__NR_rt_sigpending, S(process::rt_sigpending)),
        (__NR_sigaltstack, S(process::sigaltstack)),
        (__NR_kill, S(process::kill)),
        (__NR_tkill, S(process::tkill)),
        (__NR_tgkill, S(process::tgkill)),
        (__NR_prlimit64, S(process::prlimit64)),
        (__NR_getrlimit, S(process::getrlimit)),
        (__NR_setrlimit, S(process::setrlimit)),
        (__NR_getrusage, S(process::getrusage)),
        (__NR_prctl, S(process::prctl)),
        (__NR_personality, S(process::personality)),
        // Time.
        (__NR_clock_gettime, S(time::clock_gettime)),
        (__NR_clock_getres, S(time::clock_getres)),
        (__NR_gettimeofday, S(time::gettimeofday)),
        (__NR_nanosleep, F(time::nanosleep)),
        (__NR_clock_nanosleep, F(time::clock_nanosleep)),
        (__NR_times, S(time::times)),
        // The machine.
        (__NR_uname, S(system::uname)),
        (__NR_sethostname, S(system::sethostname)),
        (__NR_setdomainname, S(system::setdomainname)),
        (__NR_sysinfo, S(system::sysinfo)),
        (__NR_getrandom, S(system::getrandom)),
        (__NR_sched_yield, S(system::sched_yield)),
        (__NR_sched_getaffinity, S(system::sched_getaffinity)),
        (__NR_sched_setaffinity, S(system::sched_setaffinity)),
        (__NR_sched_getscheduler, S(system::sched_getscheduler)),
        (__NR_sched_getparam, S(system::sched_getparam)),
        (__NR_getpriority, S(system::getpriority)),
        (__NR_setpriority, S(system::setpriority)),
        (__NR_sched_setscheduler, S(system::sched_setscheduler)),
        (__NR_sched_setparam, S(system::sched_setparam)),
        (
            __NR_sched_get_priority_max,
            S(system::sched_get_priority_max),
        ),
        (
            __NR_sched_get_priority_min,
            S(system::sched_get_priority_min),
        ),
        (__NR_getcpu, S(system::getcpu)),
        (__NR_sched_setattr, S(system::sched_setattr)),
        (__NR_sched_getattr, S(system::sched_getattr)),
        (__NR_sched_rr_get_interval, S(system::sched_rr_get_interval)),
    ];
    let mut index = 0;
    while index < implemented.len() {
        let (number, handler) = implemented[index];
        assert!(!table[number as usize].name.is_empty());
        table[number as usize].handler = handler;
        index += 1;
    }
    table
};

pub(crate) fn dispatch(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    call: Syscall,
) -> Outcome {
    // ARM64 Linux takes the call number from the low 32 bits of x8 only.
    let number = call.number as u32;
    let handler = TABLE
        .get(number as usize)
        .map_or(Handler::Unknown, |entry| entry.handler);
    let known = !matches!(handler, Handler::Unknown);
    // Docker's profile answers first; what it answers is exact, but a number Linux does not have is
    // still counted.
    if let Some(errno) = crate::seccomp::docker_default(number, &call.arguments, known) {
        if !known {
            kernel
                .counters
                .count(cx.host, reason::UNKNOWN_SYSCALL, number.into());
        }
        return Err(errno).into();
    }
    match handler {
        Handler::Simple(handler) => handler(kernel, cx, tid, call.arguments).into(),
        Handler::Full(handler) => handler(kernel, cx, tid, call.arguments),
        Handler::NotImplemented => {
            kernel
                .counters
                .count(cx.host, reason::NOT_IMPLEMENTED, number.into());
            Err(Errno::ENOSYS).into()
        }
        Handler::Unknown => {
            kernel
                .counters
                .count(cx.host, reason::UNKNOWN_SYSCALL, number.into());
            Err(Errno::ENOSYS).into()
        }
    }
}

/// The calling thread's process memory, for copying to and from it.
pub(crate) fn guest<'a>(kernel: &'a mut Kernel, cx: &'a mut Context<'_>, tid: Tid) -> Guest<'a> {
    let process = kernel.process_of_mut(tid);
    Guest {
        cpu: cx.cpu,
        memory: process.memory,
        map: &mut process.map,
        limits: &process.limits,
    }
}

pub(crate) fn all() -> impl Iterator<Item = crate::SystemCall> {
    TABLE
        .iter()
        .enumerate()
        .filter(|(_, entry)| !entry.name.is_empty())
        .map(|(number, entry)| crate::SystemCall {
            number: number as u64,
            name: entry.name,
            implemented: matches!(entry.handler, Handler::Simple(_) | Handler::Full(_)),
        })
}

pub(crate) fn name(number: u64) -> Option<&'static str> {
    TABLE
        .get(usize::try_from(number).ok()?)
        .map(|entry| entry.name)
        .filter(|name| !name.is_empty())
}
