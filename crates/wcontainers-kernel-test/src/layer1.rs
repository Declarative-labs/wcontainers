//! Layer 1: random sequences of system calls, made in the reference by a small native runner and in our
//! kernel with a plain buffer as guest memory. Every return value and every byte written into the
//! sequence's memory is compared; the few values that must differ (inode numbers, times, process
//! numbers, `/proc`'s link count) are masked on both sides.

use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use linux_raw_sys::general as linux;
use wcontainers_abi::{Event, Exit, Next, Syscall};
use wcontainers_kernel::{Kernel, Start};

use crate::buffer::BufferCpu;
use crate::driver::TestHost;
use crate::image::{File, Kind};
use crate::{guest, reference};

/// Where each sequence's memory is, in both runners.
pub const ARENA: u64 = 0x7000_0000;
pub const ARENA_SIZE: u64 = 16 * 4096;
/// Where the memory calls play: mapped, unmapped and protected by the sequence itself.
pub const PLAY: u64 = 0x7100_0000;
pub const PLAY_PAGES: u64 = 16;

/// Offsets in the arena.
const PATHS: u64 = 0;
const IOVECS: u64 = 0x3000;
const POLLS: u64 = 0x3600;
const VALUES: u64 = 0x3800;
const UNREACHED_LIMITS: u64 = 0x3a00;
const CPU_MASKS: u64 = 0x3a40;
/// Sixteen zero bytes: an empty argument list.
const NULLS: u64 = 0x3a60;
/// Four descriptor sets for pselect6, 16 bytes each.
const FD_SETS: u64 = 0x3a80;
/// Three signal mask arguments for pselect6: an address and a size.
const MASK_PACKS: u64 = 0x3ac0;
/// Four `open_how` for openat2, 32 bytes each (the last 8 zero, or not).
const HOWS: u64 = 0x3b00;
/// Scheduling priorities for sched_setscheduler and sched_setparam.
const PRIORITIES: u64 = 0x3b80;
/// `sched_attr` structures for sched_setattr, 128 bytes apart.
const SCHED_ATTRS: u64 = 0x2000;
/// `stack_t` values for sigaltstack, 24 bytes apart.
const SIGNAL_STACKS: u64 = 0x2800;
/// Stack limits for RLIMIT_STACK, 16 bytes apart.
const STACK_LIMITS: u64 = 0x2900;
/// Extended attribute names, one after another.
const XATTR_NAMES: u64 = 0x3c00;
/// Attribute values with meaning: file capabilities in their two forms (the long one with a root of
/// 1000 and of 0), and an access control list.
const XATTR_VALUES: u64 = 0x3f00;

/// An argument each runner replaces by its caller's process number, so calls can name their own
/// process although the two runners' numbers differ. Its negation, the caller's own process group, is
/// not generated: our caller is process 1, and -1 means every process.
pub const SELF: u64 = 0x5e1f_5e1f_0000_0001;
const OUTPUT: u64 = 0x4000;

/// The sequence's work directory, made and entered by its first two calls and removed afterwards.
pub const WORK: &str = "/dev/shm/w";

/// The paths calls choose from, laid out one after another from the arena's start. Nothing reaches the
/// root file system, whose kind differs until the file backend exists.
const PATH_POOL: &[&str] = &[
    WORK,
    "a",
    "b",
    "c",
    "a/x",
    "a/y",
    "b/x",
    "l",
    "a/",
    ".",
    "..",
    "",
    "/dev/shm/",
    "a/../b",
    "nope/x",
    "a/x/",
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/shm/w/a",
    "/proc/self/fd/3",
    "/proc/self/cwd",
    "l/x",
    "c/",
    "/proc/self/",
    "/proc/self/fd",
    "/proc/sys",
];

/// Attribute values, 32 bytes apart from `XATTR_VALUES`.
const XATTR_VALUE_POOL: &[&[u8]] = &[
    &[
        0, 0, 0, 2, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    &[
        1, 0, 0, 2, 0x20, 0, 0, 0, 0x20, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
    ],
    &[
        1, 0, 0, 3, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xe8, 3, 0, 0,
    ],
    &[
        0, 0, 0, 3, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    &[
        0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff,
    ],
    &[
        2, 0, 0, 0, 1, 0, 6, 0, 0xff, 0xff, 0xff, 0xff, 4, 0, 4, 0, 0xff, 0xff, 0xff, 0xff, 0x20,
        0, 4, 0, 0xff, 0xff, 0xff, 0xff,
    ],
];

/// `sched_attr` values: the size, policy, flags, nice, priority, runtime, deadline and period, then the
/// clamps, and for one a byte past the structure.
fn sched_attrs() -> Vec<Vec<u8>> {
    /// Size, policy, flags, nice, priority, runtime, deadline, period, and a byte past the end.
    type Attributes = (u32, u32, u64, i32, u32, u64, u64, u64, bool);
    let attributes: &[Attributes] = &[
        (56, 0, 0, 5, 0, 0, 0, 0, false),
        (48, 3, 0, 19, 0, 3_000_000, 0, 0, false),
        (0, 0, 0, -3, 0, 0, 0, 0, false),
        (56, 1, 0, 0, 1, 0, 0, 0, false),
        (56, 6, 0, 0, 0, 1_000_000, 2_000_000, 3_000_000, false),
        (47, 0, 0, 0, 0, 0, 0, 0, false),
        (80, 0, 0, 0, 0, 0, 0, 0, true),
        (56, 5, 0x18, 0, 0, 0, 0, 0, false),
        (56, 3, 1, 25, 0, 0, 0, 0, false),
        (56, 3, 0x60, 0, 0, 0, 0, 0, false),
        (56, 7, 0, 0, 0, 0, 0, 0, false),
        (56, 3, 0x1000_0000, 0, 0, 0, 0, 0, false),
    ];
    attributes
        .iter()
        .map(
            |&(size, policy, flags, nice, priority, runtime, deadline, period, tail)| {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&size.to_le_bytes());
                bytes.extend_from_slice(&policy.to_le_bytes());
                bytes.extend_from_slice(&flags.to_le_bytes());
                bytes.extend_from_slice(&nice.to_le_bytes());
                bytes.extend_from_slice(&priority.to_le_bytes());
                bytes.extend_from_slice(&runtime.to_le_bytes());
                bytes.extend_from_slice(&deadline.to_le_bytes());
                bytes.extend_from_slice(&period.to_le_bytes());
                bytes.extend_from_slice(&[0; 8]);
                if tail {
                    bytes.extend_from_slice(&[0, 0, 0, 1]);
                }
                bytes
            },
        )
        .collect()
}

/// Extended attribute names calls choose from, each ending in a zero byte: every namespace, names
/// with nothing after the prefix, the longest name Linux takes and one longer.
fn xattr_names() -> Vec<Vec<u8>> {
    let mut names: Vec<Vec<u8>> = [
        "user.a",
        "user.b",
        "user.",
        "trusted.a",
        "security.a",
        "security.capability",
        "security.",
        "system.posix_acl_access",
        "system.posix_acl_default",
        "system.a",
        "other.a",
        "",
    ]
    .iter()
    .map(|name| name.as_bytes().to_vec())
    .collect();
    let mut longest = b"user.".to_vec();
    longest.resize(255, b'x');
    names.push(longest.clone());
    longest.push(b'x');
    names.push(longest);
    for name in &mut names {
        name.push(0);
    }
    names
}

fn xattr_name_offset(index: usize) -> u64 {
    xattr_names()[..index]
        .iter()
        .map(|name| name.len() as u64)
        .sum()
}

/// One call: its number and six raw arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub number: u32,
    pub arguments: [u64; 6],
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub arena: Vec<u8>,
    pub calls: Vec<Call>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallResult {
    pub value: i64,
    /// Changed ranges of the arena: offset and the bytes now there.
    pub changes: Vec<(u32, Vec<u8>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceResult {
    pub calls: Vec<CallResult>,
    /// The wait status of the process that made the calls.
    pub status: i32,
}

fn path_offsets() -> (Vec<u8>, Vec<u64>) {
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    for path in PATH_POOL {
        offsets.push(PATHS + bytes.len() as u64);
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
    }
    // A name longer than Linux allows, then a path longer than Linux allows.
    offsets.push(PATHS + bytes.len() as u64);
    bytes.extend(std::iter::repeat_n(b'n', 300));
    bytes.push(0);
    offsets.push(PATHS + bytes.len() as u64);
    for _ in 0..1100 {
        bytes.extend_from_slice(b"abc/");
    }
    bytes.push(0);
    (bytes, offsets)
}

/// A generated call before its arguments are decoded: which kind, and random words to pick them with.
pub type Raw = (u8, [u64; 6]);

/// Builds a sequence from generated parts: the arena's random values and the calls. The first two calls
/// always make and enter the work directory.
pub fn build(values: &[u64], raw: &[Raw]) -> Sequence {
    let (paths, path_at) = path_offsets();
    let mut arena = vec![0u8; OUTPUT as usize];
    arena[..paths.len()].copy_from_slice(&paths);
    // Sixteen vectors for readv and writev, each pointing into the output area or somewhere bad.
    for index in 0..16u64 {
        let word = values.get(index as usize).copied().unwrap_or(0);
        let base = match word % 5 {
            0 => 0,
            1 => PLAY + (word >> 8) % (PLAY_PAGES * 4096),
            _ => ARENA + OUTPUT + (word >> 8) % 0x8000,
        };
        let length = match (word >> 40) % 4 {
            0 => 0,
            1 => (word >> 44) % 16,
            2 => (word >> 44) % 4096,
            _ => (word >> 44) % 0x3000,
        };
        let at = (IOVECS + index * 16) as usize;
        arena[at..at + 8].copy_from_slice(&base.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&length.to_le_bytes());
    }
    // Eight entries for ppoll: a descriptor and the events asked for.
    for index in 0..8u64 {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(17);
        let fd = [3i32, 4, 5, -1, 100, 3, 6, 4][(word % 8) as usize];
        let events = (word >> 8) as u16;
        let at = (POLLS + index * 8) as usize;
        arena[at..at + 4].copy_from_slice(&fd.to_le_bytes());
        arena[at + 4..at + 6].copy_from_slice(&events.to_le_bytes());
    }
    // Values calls read: times, limits, masks, chosen to include the special ones. The last eight hold
    // the four sleeps `nanosleep` chooses from: short, none, and two invalid ones.
    for index in 0..56u64 {
        let word = values.get(16 + index as usize).copied().unwrap_or(0);
        let value = match word % 8 {
            0 => 0,
            1 => (word >> 8) % 1_000_000_000,
            2 => linux::UTIME_NOW as u64,
            3 => linux::UTIME_OMIT as u64,
            4 => u64::MAX,
            5 => (word >> 8) % 100_000,
            _ => word >> 8,
        };
        let at = (VALUES + index * 8) as usize;
        arena[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    let sleeps: [(i64, i64); 4] = [(0, 1000), (0, 0), (0, 1_000_000_000), (-1, 0)];
    for (index, (seconds, nanoseconds)) in sleeps.iter().enumerate() {
        let at = (VALUES + 56 * 8 + index as u64 * 16) as usize;
        arena[at..at + 8].copy_from_slice(&seconds.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&nanoseconds.to_le_bytes());
    }
    // Limits for RLIMIT_CPU and RLIMIT_FSIZE, all too large to be reached. Reaching either sends a
    // signal that ends the reference's caller but not ours, the container's first process, which Linux
    // keeps from signals it has no handler for; and a CPU limit is reached at a moment that depends on
    // the machine's speed.
    let infinity = linux::RLIM_INFINITY as u64;
    let unreached_limits: [(u64, u64); 4] = [
        (1 << 40, infinity),
        (1 << 40, 1 << 41),
        (1 << 41, 1 << 40),
        (infinity, infinity),
    ];
    for (index, (current, maximum)) in unreached_limits.iter().enumerate() {
        let at = (UNREACHED_LIMITS + index as u64 * 16) as usize;
        arena[at..at + 8].copy_from_slice(&current.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&maximum.to_le_bytes());
    }
    // CPU sets for sched_setaffinity, each with CPU 0: the guest is told it has one CPU.
    for (index, mask) in [1u64, 3, u64::MAX, 0x11].iter().enumerate() {
        let at = (CPU_MASKS + index as u64 * 8) as usize;
        arena[at..at + 8].copy_from_slice(&mask.to_le_bytes());
    }
    // Descriptor sets name only descriptors 3 to 7 (0 to 2 differ between the runners) and one far
    // past them.
    for index in 0..4u64 {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(29);
        let at = (FD_SETS + index * 16) as usize;
        arena[at] = (word as u8) & 0xf8;
        arena[at + 12] = if word >> 60 == 0 { 0x10 } else { 0 };
    }
    let packs: [(u64, u64); 3] = [(ARENA + VALUES, 8), (ARENA + VALUES, 16), (0, 8)];
    for (index, (address, size)) in packs.iter().enumerate() {
        let at = (MASK_PACKS + index as u64 * 16) as usize;
        arena[at..at + 8].copy_from_slice(&address.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&size.to_le_bytes());
    }
    for index in 0..4u64 {
        let word = values
            .get(4 + index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(41);
        let flags = open_flag_bits(word);
        let mode = pick(&[0u64, 0o644, 0o10000, 0o777], word >> 40);
        // Only `resolve` values whose meaning does not depend on what Linux happens to have cached, and
        // none of the restricted lookups (not built).
        let resolve = pick(&[0u64, 0, 0x18, 0x40], word >> 44);
        let at = (HOWS + index * 32) as usize;
        arena[at..at + 8].copy_from_slice(&flags.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&mode.to_le_bytes());
        arena[at + 16..at + 24].copy_from_slice(&resolve.to_le_bytes());
        arena[at + 24] = u8::from(word >> 50 == 0);
    }
    for (index, priority) in [0i32, 1, 50, 99, 100, -1].iter().enumerate() {
        let at = (PRIORITIES + index as u64 * 4) as usize;
        arena[at..at + 4].copy_from_slice(&priority.to_le_bytes());
    }
    for (index, name) in xattr_names().iter().enumerate() {
        let at = (XATTR_NAMES + xattr_name_offset(index)) as usize;
        arena[at..at + name.len()].copy_from_slice(name);
    }
    for (index, attributes) in sched_attrs().iter().enumerate() {
        let at = (SCHED_ATTRS + index as u64 * 128) as usize;
        arena[at..at + attributes.len()].copy_from_slice(attributes);
    }
    // Alternate signal stacks in the play area, never holding either caller's stack pointer (which
    // differ): enough, too small, disabled, given up in handlers, already on it, and unknown flags.
    let stacks: [(u64, i32, u64); 6] = [
        (PLAY, 0, 8192),
        (PLAY, 0, 1024),
        (0, linux::SS_DISABLE as i32, 0),
        (PLAY, linux::SS_AUTODISARM as i32, 65536),
        (PLAY + 4096, linux::SS_ONSTACK as i32, 1 << 20),
        (PLAY, 3, 8192),
    ];
    for (index, (base, flags, size)) in stacks.iter().enumerate() {
        let at = (SIGNAL_STACKS + index as u64 * 24) as usize;
        arena[at..at + 8].copy_from_slice(&base.to_le_bytes());
        arena[at + 8..at + 12].copy_from_slice(&flags.to_le_bytes());
        arena[at + 16..at + 24].copy_from_slice(&size.to_le_bytes());
    }
    // Stack limits no larger than 1 GiB: in our 4 GiB address space nothing lies between the stack and
    // the play area, so a larger one would let the stack grow down to it, which the reference's layout
    // never allows.
    let stack_limits: [(u64, u64); 4] = [
        (8 << 20, infinity),
        (1 << 20, 1 << 30),
        (1 << 30, 1 << 30),
        (0, infinity),
    ];
    for (index, (current, maximum)) in stack_limits.iter().enumerate() {
        let at = (STACK_LIMITS + index as u64 * 16) as usize;
        arena[at..at + 8].copy_from_slice(&current.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&maximum.to_le_bytes());
    }
    for (index, value) in XATTR_VALUE_POOL.iter().enumerate() {
        let at = (XATTR_VALUES + index as u64 * 32) as usize;
        arena[at..at + value.len()].copy_from_slice(value);
    }
    let mut calls = vec![
        Call {
            number: linux::__NR_mkdirat,
            arguments: [linux::AT_FDCWD as u64, ARENA + path_at[0], 0o777, 0, 0, 0],
        },
        Call {
            number: linux::__NR_chdir,
            arguments: [ARENA + path_at[0], 0, 0, 0, 0, 0],
        },
    ];
    calls.extend(raw.iter().map(|raw| decode(raw, &path_at)));
    Sequence { arena, calls }
}

/// Flags for an open, each bit present one time in eight.
fn open_flag_bits(word: u64) -> u64 {
    let bits = [
        linux::O_WRONLY,
        linux::O_RDWR,
        linux::O_CREAT,
        linux::O_EXCL,
        linux::O_TRUNC,
        linux::O_APPEND,
        linux::O_DIRECTORY,
        linux::O_NOFOLLOW,
        linux::O_CLOEXEC,
        linux::O_PATH,
        linux::O_NONBLOCK,
        linux::O_CREAT,
        linux::FASYNC,
    ];
    let mut flags = 0;
    for (index, bit) in bits.iter().enumerate() {
        if (word >> (index * 3)) & 7 == 0 {
            flags |= bit;
        }
    }
    u64::from(flags)
}

fn pick<T: Copy>(pool: &[T], word: u64) -> T {
    pool[(word % pool.len() as u64) as usize]
}

/// The calls layer 1 makes, by number.
pub const CALLS: &[u32] = &[
    linux::__NR_openat,
    linux::__NR_close,
    linux::__NR_read,
    linux::__NR_write,
    linux::__NR_pread64,
    linux::__NR_pwrite64,
    linux::__NR_readv,
    linux::__NR_writev,
    linux::__NR_lseek,
    linux::__NR_getdents64,
    linux::__NR_dup,
    linux::__NR_dup3,
    linux::__NR_fcntl,
    linux::__NR_ioctl,
    linux::__NR_fsync,
    linux::__NR_mkdirat,
    linux::__NR_mknodat,
    linux::__NR_symlinkat,
    linux::__NR_linkat,
    linux::__NR_unlinkat,
    linux::__NR_renameat2,
    linux::__NR_readlinkat,
    linux::__NR_newfstatat,
    linux::__NR_fstat,
    linux::__NR_statx,
    linux::__NR_faccessat2,
    linux::__NR_fchmod,
    linux::__NR_fchmodat,
    linux::__NR_fchownat,
    linux::__NR_utimensat,
    linux::__NR_truncate,
    linux::__NR_ftruncate,
    linux::__NR_fallocate,
    linux::__NR_getcwd,
    linux::__NR_chdir,
    linux::__NR_fchdir,
    linux::__NR_umask,
    linux::__NR_fstatfs,
    linux::__NR_sendfile,
    linux::__NR_mmap,
    linux::__NR_munmap,
    linux::__NR_mprotect,
    linux::__NR_madvise,
    linux::__NR_uname,
    linux::__NR_getuid,
    linux::__NR_getgroups,
    linux::__NR_setresuid,
    linux::__NR_setresgid,
    linux::__NR_prlimit64,
    linux::__NR_clock_gettime,
    linux::__NR_clock_getres,
    linux::__NR_getrandom,
    linux::__NR_rt_sigprocmask,
    linux::__NR_rt_sigaction,
    linux::__NR_sigaltstack,
    linux::__NR_prctl,
    linux::__NR_sched_getaffinity,
    linux::__NR_nanosleep,
    linux::__NR_capget,
    linux::__NR_getpid,
    linux::__NR_getppid,
    linux::__NR_gettid,
    linux::__NR_getpgid,
    linux::__NR_getsid,
    linux::__NR_getresuid,
    linux::__NR_getresgid,
    linux::__NR_setuid,
    linux::__NR_setgid,
    linux::__NR_setreuid,
    linux::__NR_setregid,
    linux::__NR_setfsuid,
    linux::__NR_setfsgid,
    linux::__NR_setgroups,
    linux::__NR_getrlimit,
    linux::__NR_setrlimit,
    linux::__NR_times,
    linux::__NR_gettimeofday,
    linux::__NR_clock_nanosleep,
    linux::__NR_sysinfo,
    linux::__NR_getrusage,
    linux::__NR_getcpu,
    linux::__NR_statfs,
    linux::__NR_faccessat,
    linux::__NR_fchmodat2,
    linux::__NR_fchown,
    linux::__NR_renameat,
    linux::__NR_preadv,
    linux::__NR_pwritev,
    linux::__NR_fdatasync,
    linux::__NR_ppoll,
    linux::__NR_set_robust_list,
    linux::__NR_rt_sigpending,
    linux::__NR_personality,
    linux::__NR_sched_yield,
    linux::__NR_getpriority,
    linux::__NR_kill,
    linux::__NR_tkill,
    linux::__NR_tgkill,
    linux::__NR_setpgid,
    linux::__NR_setsid,
    linux::__NR_chroot,
    linux::__NR_sethostname,
    linux::__NR_setdomainname,
    linux::__NR_sync,
    linux::__NR_syncfs,
    linux::__NR_get_robust_list,
    linux::__NR_set_tid_address,
    linux::__NR_sched_getparam,
    linux::__NR_sched_getscheduler,
    linux::__NR_sched_setaffinity,
    linux::__NR_exit,
    linux::__NR_exit_group,
    linux::__NR_execve,
    linux::__NR_msync,
    linux::__NR_mremap,
    linux::__NR_capset,
    linux::__NR_close_range,
    linux::__NR_preadv2,
    linux::__NR_pwritev2,
    linux::__NR_copy_file_range,
    linux::__NR_fadvise64,
    linux::__NR_readahead,
    linux::__NR_sync_file_range,
    linux::__NR_pselect6,
    linux::__NR_flock,
    linux::__NR_openat2,
    linux::__NR_setpriority,
    linux::__NR_sched_setscheduler,
    linux::__NR_sched_setparam,
    linux::__NR_sched_get_priority_max,
    linux::__NR_sched_get_priority_min,
    linux::__NR_mlock,
    linux::__NR_mlock2,
    linux::__NR_munlock,
    linux::__NR_mlockall,
    linux::__NR_munlockall,
    linux::__NR_setxattr,
    linux::__NR_lsetxattr,
    linux::__NR_fsetxattr,
    linux::__NR_getxattr,
    linux::__NR_lgetxattr,
    linux::__NR_fgetxattr,
    linux::__NR_listxattr,
    linux::__NR_llistxattr,
    linux::__NR_flistxattr,
    linux::__NR_removexattr,
    linux::__NR_lremovexattr,
    linux::__NR_fremovexattr,
    linux::__NR_memfd_create,
    linux::__NR_sched_setattr,
    linux::__NR_sched_getattr,
    linux::__NR_sched_rr_get_interval,
    linux::__NR_mincore,
    linux::__NR_ioprio_get,
    linux::__NR_ioprio_set,
];

/// Turns a generated call into real arguments.
fn decode((kind, words): &Raw, path_at: &[u64]) -> Call {
    let number = CALLS[*kind as usize % CALLS.len()];
    let [a, b, c, d, e, f] = *words;
    let path = |word: u64| ARENA + pick(path_at, word);
    let fd = |word: u64| pick(&[3u64, 3, 4, 4, 5, 6, u64::MAX, 100], word);
    let dirfd = |word: u64| {
        pick(
            &[linux::AT_FDCWD as u64, linux::AT_FDCWD as u64, 3, 4, 5, 100],
            word,
        )
    };
    let buffer = |word: u64| match word % 8 {
        0 => 0,
        1 => PLAY + (word >> 8) % (PLAY_PAGES * 4096),
        2 => u64::MAX - 4095,
        _ => ARENA + OUTPUT + (word >> 8) % 0x8000,
    };
    let count = |word: u64| {
        pick(
            &[0u64, 1, 7, 64, 300, 4096, 5000, 0x9000, u64::MAX, 1 << 40],
            word,
        )
    };
    let offset = |word: u64| {
        pick(
            &[
                0u64,
                1,
                3,
                100,
                4096,
                5000,
                1 << 20,
                u64::MAX,
                (1 << 63) - 1,
            ],
            word,
        )
    };
    let mode = |word: u64| word % 0o10000;
    let open_flags = open_flag_bits;
    let at_flags = |word: u64| {
        u64::from(pick(
            &[
                0,
                0,
                linux::AT_SYMLINK_NOFOLLOW,
                linux::AT_EMPTY_PATH,
                linux::AT_REMOVEDIR,
                linux::AT_SYMLINK_FOLLOW,
                0x8000_0000,
            ],
            word,
        ))
    };
    let value = |word: u64| ARENA + VALUES + (word % 52) * 8;
    // A new limit for a resource: CPU and file size limits only from the ones never reached.
    let new_limit = |resource: u64, word: u64| {
        if resource == u64::from(linux::RLIMIT_CPU) || resource == u64::from(linux::RLIMIT_FSIZE) {
            ARENA + UNREACHED_LIMITS + (word % 4) * 16
        } else if resource == u64::from(linux::RLIMIT_STACK) {
            ARENA + STACK_LIMITS + (word % 4) * 16
        } else {
            value(word)
        }
    };
    let play =
        |word: u64| PLAY + ((word % (PLAY_PAGES + 2)) * 4096) + if word >> 60 == 0 { 1 } else { 0 };
    // Lengths stay below the program's own mappings, which lie higher here (in a 4 GiB address
    // space) than in the reference.
    let play_length = |word: u64| {
        pick(
            &[0u64, 1, 4096, 8192, 3 * 4096, PLAY_PAGES * 4096, 1 << 28],
            word,
        )
    };
    // Signals whose default action is the same for the container's first process (our caller) and for
    // any other (the reference's caller): nothing, or a refusal of the number.
    let signal = |word: u64| pick(&[0u64, 17, 18, 23, 28, 65], word);
    let prot = |word: u64| (word % 8) | if word >> 62 == 3 { 0x100 } else { 0 };
    let arguments = match number {
        linux::__NR_openat => [dirfd(a), path(b), open_flags(c), mode(d), 0, 0],
        linux::__NR_close | linux::__NR_dup | linux::__NR_fsync | linux::__NR_fchdir => {
            [fd(a), 0, 0, 0, 0, 0]
        }
        linux::__NR_read | linux::__NR_write => [fd(a), buffer(b), count(c), 0, 0, 0],
        linux::__NR_pread64 | linux::__NR_pwrite64 => [fd(a), buffer(b), count(c), offset(d), 0, 0],
        linux::__NR_readv | linux::__NR_writev => [
            fd(a),
            ARENA + IOVECS + (b % 16) * 16,
            pick(&[0, 1, 2, 3, 16, 1025], c),
            0,
            0,
            0,
        ],
        linux::__NR_lseek => [fd(a), offset(b), pick(&[0, 1, 2, 3, 4, 5], c), 0, 0, 0],
        linux::__NR_getdents64 => [
            fd(a),
            buffer(b),
            pick(&[0, 10, 24, 64, 4096, 0x8000], c),
            0,
            0,
            0,
        ],
        linux::__NR_dup3 => [
            fd(a),
            fd(b),
            pick(&[0, linux::O_CLOEXEC as u64, 1], c),
            0,
            0,
            0,
        ],
        linux::__NR_fcntl => {
            let command = pick(
                &[
                    linux::F_DUPFD,
                    linux::F_GETFD,
                    linux::F_SETFD,
                    linux::F_GETFL,
                    linux::F_SETFL,
                    linux::F_DUPFD_CLOEXEC,
                    linux::F_ADD_SEALS,
                    linux::F_GET_SEALS,
                    1000,
                ],
                b,
            );
            [
                fd(a),
                command.into(),
                pick(
                    &[0, 1, 5, 100, 0x800, 0x400 | 0x800, u64::MAX, 8, 0x20, 0x16],
                    c,
                ),
                0,
                0,
                0,
            ]
        }
        linux::__NR_ioctl => {
            let request = pick(&[0x5451u64, 0x5450, 0x5401, 0x5413, 0x5421, 0x1234], b);
            [fd(a), request, ARENA + OUTPUT + (c % 64) * 8, 0, 0, 0]
        }
        linux::__NR_mkdirat => [dirfd(a), path(b), mode(c), 0, 0, 0],
        linux::__NR_mknodat => {
            // No named pipes: opening one waits for the other end, and pipes come with K2.
            let kind = pick(&[0u64, 0o100000, 0o020000, 0o040000, 0o140000, 0o170000], c);
            [
                dirfd(a),
                path(b),
                kind | mode(d) & 0o777,
                pick(&[0, 0x103], e),
                0,
                0,
            ]
        }
        linux::__NR_symlinkat => [path(a), dirfd(b), path(c), 0, 0, 0],
        linux::__NR_linkat => [dirfd(a), path(b), dirfd(c), path(d), at_flags(e), 0],
        linux::__NR_unlinkat => [dirfd(a), path(b), at_flags(c), 0, 0, 0],
        linux::__NR_renameat2 => [
            dirfd(a),
            path(b),
            dirfd(c),
            path(d),
            pick(&[0, 1, 2, 3, 4], e),
            0,
        ],
        linux::__NR_readlinkat => [
            dirfd(a),
            path(b),
            buffer(c),
            pick(&[0, 1, 5, 4096, u64::MAX], d),
            0,
            0,
        ],
        linux::__NR_newfstatat => [dirfd(a), path(b), buffer(c), at_flags(d), 0, 0],
        linux::__NR_fstat => [fd(a), buffer(b), 0, 0, 0, 0],
        linux::__NR_statx => [
            dirfd(a),
            path(b),
            at_flags(c),
            pick(&[0x7ff, 0xfff, 0x8000_0000, 0], d),
            buffer(e),
            0,
        ],
        linux::__NR_faccessat2 => [
            dirfd(a),
            path(b),
            pick(&[0, 1, 2, 4, 7, 8], c),
            pick(&[0, 0x200, 0x100, 0x1000, 1], d),
            0,
            0,
        ],
        linux::__NR_fchmod => [fd(a), mode(b), 0, 0, 0, 0],
        linux::__NR_fchmodat => [dirfd(a), path(b), mode(c), 0, 0, 0],
        linux::__NR_fchownat => {
            let id = |word: u64| pick(&[0u64, 1, 1000, u32::MAX as u64], word);
            [dirfd(a), path(b), id(c), id(d), at_flags(e), 0]
        }
        linux::__NR_utimensat => [
            dirfd(a),
            if b % 6 == 0 { 0 } else { path(b) },
            if c % 4 == 0 { 0 } else { value(c) },
            at_flags(d),
            0,
            0,
        ],
        linux::__NR_truncate => [path(a), offset(b) % (1 << 21), 0, 0, 0, 0],
        linux::__NR_ftruncate => [
            fd(a),
            pick(&[0u64, 1, 5000, 1 << 20, u64::MAX], b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_fallocate => [
            fd(a),
            pick(&[0, 0, 1, 3], b),
            offset(c) % (1 << 20),
            pick(&[0, 1, 4096, 10000], d),
            0,
            0,
        ],
        linux::__NR_getcwd => [buffer(a), pick(&[0, 1, 5, 11, 12, 4096], b), 0, 0, 0, 0],
        linux::__NR_chdir => [path(a), 0, 0, 0, 0, 0],
        linux::__NR_umask => [a % 0o1000, 0, 0, 0, 0, 0],
        linux::__NR_fstatfs => [fd(a), buffer(b), 0, 0, 0, 0],
        linux::__NR_sendfile => [
            fd(a),
            fd(b),
            if c % 3 == 0 { value(c) } else { 0 },
            count(d),
            0,
            0,
        ],
        linux::__NR_mmap => {
            let flags = pick(
                &[
                    0x22u64 | 0x10,
                    0x21 | 0x10,
                    0x02 | 0x10,
                    0x22 | 0x100000,
                    0x22,
                    0x23 | 0x10,
                ],
                c,
            );
            [
                play(a),
                play_length(b),
                prot(d),
                flags,
                fd(e),
                pick(&[0, 4096, 1], f),
            ]
        }
        linux::__NR_munmap => [play(a), play_length(b), 0, 0, 0, 0],
        linux::__NR_mprotect => [play(a), play_length(b), prot(c), 0, 0, 0],
        linux::__NR_madvise => [
            play(a),
            play_length(b),
            pick(
                &[
                    0, 1, 2, 3, 4, 8, 9, 100, 10, 11, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                ],
                c,
            ),
            0,
            0,
            0,
        ],
        linux::__NR_uname => [buffer(a), 0, 0, 0, 0, 0],
        linux::__NR_getuid => [0; 6],
        linux::__NR_getgroups => [pick(&[0, 1, 32, u64::MAX], a), buffer(b), 0, 0, 0, 0],
        linux::__NR_setresuid | linux::__NR_setresgid => {
            let id = |word: u64| pick(&[0u64, 1000, u32::MAX as u64], word);
            [id(a), id(b), id(c), 0, 0, 0]
        }
        // No other process numbers: the reference's caller is a child, not process 1.
        linux::__NR_prlimit64 => [
            pick(&[0, 99999, SELF], a),
            b % 17,
            if c % 3 == 0 { new_limit(b % 17, c) } else { 0 },
            if d % 2 == 0 { buffer(d) } else { 0 },
            0,
            0,
        ],
        linux::__NR_clock_gettime | linux::__NR_clock_getres => [
            pick(&[0, 1, 2, 3, 4, 5, 6, 7, 11, 99], a),
            buffer(b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_getrandom => [
            buffer(a),
            pick(&[0, 1, 16, 300], b),
            pick(&[0, 1, 2, 4, 8], c),
            0,
            0,
            0,
        ],
        linux::__NR_rt_sigprocmask => [
            pick(&[0, 1, 2, 3], a),
            if b % 3 == 0 { 0 } else { value(b) },
            if c % 2 == 0 { buffer(c) } else { 0 },
            pick(&[8, 8, 4, 16], d),
            0,
            0,
        ],
        linux::__NR_rt_sigaction => [
            // None of the signals `kill` sends: a handler cannot run here yet.
            pick(&[1, 2, 9, 10, 15, 14, 19, 64, 65, 0], a),
            if b % 3 == 0 { 0 } else { value(b) },
            if c % 2 == 0 { buffer(c) } else { 0 },
            pick(&[8, 8, 4], d),
            0,
            0,
        ],
        linux::__NR_sigaltstack => [
            match a % 4 {
                0 => 0,
                1 => buffer(a >> 2),
                _ => ARENA + SIGNAL_STACKS + ((a >> 2) % 6) * 24,
            },
            if b % 2 == 0 { buffer(b) } else { 0 },
            0,
            0,
            0,
            0,
        ],
        linux::__NR_prctl => [
            pick(&[15u64, 16, 3, 4, 7, 8, 24, 38, 39, 9999], a),
            buffer(b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_sched_getaffinity => [
            pick(&[0, SELF, 99999], c),
            pick(&[0, 4, 8, 128], a),
            buffer(b),
            0,
            0,
            0,
        ],
        linux::__NR_nanosleep => [ARENA + VALUES + 56 * 8 + (a % 4) * 16, 0, 0, 0, 0, 0],
        linux::__NR_capget => [value(a), if b % 2 == 0 { buffer(b) } else { 0 }, 0, 0, 0, 0],
        linux::__NR_getpid | linux::__NR_getppid | linux::__NR_gettid | linux::__NR_sched_yield => {
            [0; 6]
        }
        linux::__NR_getpgid | linux::__NR_getsid => {
            [pick(&[0, 99999, u64::MAX, SELF], a), 0, 0, 0, 0, 0]
        }
        linux::__NR_getresuid | linux::__NR_getresgid => [buffer(a), buffer(b), buffer(c), 0, 0, 0],
        linux::__NR_setuid | linux::__NR_setgid | linux::__NR_setfsuid | linux::__NR_setfsgid => {
            [pick(&[0u64, 1000, 1, u32::MAX as u64], a), 0, 0, 0, 0, 0]
        }
        linux::__NR_setreuid | linux::__NR_setregid => {
            let id = |word: u64| pick(&[0u64, 1000, u32::MAX as u64], word);
            [id(a), id(b), 0, 0, 0, 0]
        }
        linux::__NR_setgroups => [pick(&[0, 1, 2, 3, 65537], a), value(b), 0, 0, 0, 0],
        linux::__NR_getrlimit => [b % 17, buffer(a), 0, 0, 0, 0],
        linux::__NR_setrlimit => [
            b % 17,
            if a % 4 == 0 { 0 } else { new_limit(b % 17, a) },
            0,
            0,
            0,
            0,
        ],
        linux::__NR_times => [if a % 3 == 0 { 0 } else { buffer(a) }, 0, 0, 0, 0, 0],
        linux::__NR_gettimeofday => [
            if a % 3 == 0 { 0 } else { buffer(a) },
            if b % 3 == 0 { 0 } else { buffer(b) },
            0,
            0,
            0,
            0,
        ],
        // A relative sleep on the process's CPU clock never ends (it does not move while sleeping), so
        // that clock is only slept on until a time already past.
        linux::__NR_clock_nanosleep => [
            pick(&[0, 1, 2, 3, 7, 99], a),
            if a % 6 == 2 { 1 } else { pick(&[0, 1], b) },
            ARENA + VALUES + 56 * 8 + (c % 4) * 16,
            0,
            0,
            0,
        ],
        linux::__NR_sysinfo | linux::__NR_getcpu => [
            buffer(a),
            if b % 2 == 0 { 0 } else { buffer(b) },
            0,
            0,
            0,
            0,
        ],
        linux::__NR_getrusage => [
            pick(&[0, u64::MAX, u64::MAX - 1, 1, 5], a),
            buffer(b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_statfs => [path(a), buffer(b), 0, 0, 0, 0],
        linux::__NR_faccessat => [dirfd(a), path(b), pick(&[0, 1, 2, 4, 7, 8], c), 0, 0, 0],
        linux::__NR_fchmodat2 => [dirfd(a), path(b), mode(c), at_flags(d), 0, 0],
        linux::__NR_fchown => {
            let id = |word: u64| pick(&[0u64, 1, 1000, u32::MAX as u64], word);
            [fd(a), id(b), id(c), 0, 0, 0]
        }
        linux::__NR_renameat => [dirfd(a), path(b), dirfd(c), path(d), 0, 0],
        linux::__NR_preadv | linux::__NR_pwritev => [
            fd(a),
            ARENA + IOVECS + (b % 16) * 16,
            pick(&[0, 1, 2, 3, 16, 1025], c),
            offset(d),
            0,
            0,
        ],
        linux::__NR_fdatasync => [fd(a), 0, 0, 0, 0, 0],
        linux::__NR_ppoll => [
            ARENA + POLLS + (a % 8) * 8,
            pick(&[0, 1, 2, 8, 1 << 21], b),
            // Always a timeout: with none and nothing ready both sides would wait for ever.
            ARENA + VALUES + 56 * 8 + (c % 4) * 16,
            if d % 2 == 0 { 0 } else { value(d) },
            pick(&[8, 8, 4], e),
            0,
        ],
        linux::__NR_set_robust_list => [value(a), pick(&[24, 0, 16], b), 0, 0, 0, 0],
        linux::__NR_rt_sigpending => [buffer(a), pick(&[8, 4, 16, 0], b), 0, 0, 0, 0],
        linux::__NR_personality => [pick(&[0xffff_ffffu64, 0, 8, 0x0040_0000], a), 0, 0, 0, 0, 0],
        // Not by user: that also counts the reference's runner, which has no counterpart here.
        linux::__NR_getpriority => [pick(&[0, 1, 3], a), pick(&[0, 99999], b), 0, 0, 0, 0],
        linux::__NR_kill => [
            pick(&[SELF, SELF, 0, u64::MAX, 99999, -99999i64 as u64], a),
            signal(b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_tkill => [
            pick(&[SELF, SELF, 0, 99999, u64::MAX], a),
            signal(b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_tgkill => [
            pick(&[SELF, SELF, 0, 99999], a),
            pick(&[SELF, SELF, 99999, u64::MAX], b),
            signal(c),
            0,
            0,
            0,
        ],
        linux::__NR_setpgid => {
            let pid = |word: u64| pick(&[0, SELF, 99999, u64::MAX], word);
            [pid(a), pid(b), 0, 0, 0, 0]
        }
        linux::__NR_setsid | linux::__NR_sync => [0; 6],
        linux::__NR_chroot => [path(a), 0, 0, 0, 0, 0],
        linux::__NR_sethostname | linux::__NR_setdomainname => {
            [buffer(a), pick(&[0, 5, 64, 65, u64::MAX], b), 0, 0, 0, 0]
        }
        linux::__NR_syncfs => [fd(a), 0, 0, 0, 0, 0],
        linux::__NR_get_robust_list => [pick(&[0, SELF, 99999], a), buffer(b), buffer(c), 0, 0, 0],
        linux::__NR_set_tid_address => [buffer(a), 0, 0, 0, 0, 0],
        linux::__NR_sched_getparam => [pick(&[0, SELF, 99999, u64::MAX], a), buffer(b), 0, 0, 0, 0],
        linux::__NR_sched_getscheduler => [pick(&[0, SELF, 99999, u64::MAX], a), 0, 0, 0, 0, 0],
        linux::__NR_sched_setaffinity => [
            pick(&[0, SELF, 99999], a),
            pick(&[0, 4, 8, 128], b),
            ARENA + CPU_MASKS + (c % 4) * 8,
            0,
            0,
            0,
        ],
        linux::__NR_exit | linux::__NR_exit_group => {
            [pick(&[0, 1, 255, 256, 0x1_0000_0001], a), 0, 0, 0, 0, 0]
        }
        // Every path here fails: /dev/shm is mounted without execution and the rest are not programs.
        linux::__NR_execve => {
            let list = |word: u64| pick(&[0, ARENA + NULLS, u64::MAX - 4095], word);
            [path(a), list(b), list(c), 0, 0, 0]
        }
        linux::__NR_msync => [
            play(a),
            play_length(b),
            pick(&[0, 1, 2, 4, 3, 5, 6, 8], c),
            0,
            0,
            0,
        ],
        linux::__NR_mremap => [
            play(a),
            play_length(b),
            play_length(c),
            pick(&[0, 1, 3, 5, 2, 8], d),
            play(e),
            0,
        ],
        linux::__NR_capset => [value(a), value(b), 0, 0, 0, 0],
        // Never descriptors 0 to 2: the reference's caller reports through them.
        linux::__NR_close_range => [
            pick(&[3u64, 4, 5, 6, 100, u32::MAX as u64], a),
            pick(&[3u64, 5, 6, 100, u32::MAX as u64, 2], b),
            pick(&[0, 4, 2, 6, 1, 8], c),
            0,
            0,
            0,
        ],
        linux::__NR_preadv2 | linux::__NR_pwritev2 => [
            fd(a),
            ARENA + IOVECS + (b % 16) * 16,
            pick(&[0, 1, 2, 3, 16, 1025], c),
            if d % 3 == 0 { u64::MAX } else { offset(d) },
            0,
            pick(&[0, 0, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100], f),
        ],
        linux::__NR_copy_file_range => [
            fd(a),
            if b % 3 == 0 { value(b) } else { 0 },
            fd(c),
            if d % 3 == 0 { value(d) } else { 0 },
            count(e),
            pick(&[0, 0, 0, 1], f),
        ],
        linux::__NR_fadvise64 => [
            fd(a),
            offset(b),
            pick(&[0, 1, 4096, u64::MAX], c),
            pick(&[0, 1, 2, 3, 4, 5, 6, 100], d),
            0,
            0,
        ],
        linux::__NR_readahead => [fd(a), offset(b), count(c), 0, 0, 0],
        linux::__NR_sync_file_range => [
            fd(a),
            offset(b),
            pick(&[0, 1, 4096, u64::MAX], c),
            pick(&[0, 1, 2, 4, 7, 8], d),
            0,
            0,
        ],
        linux::__NR_pselect6 => {
            let set = |word: u64| {
                if word.is_multiple_of(4) {
                    0
                } else {
                    ARENA + FD_SETS + (word % 4) * 16
                }
            };
            [
                pick(&[0, 1, 4, 7, 8, 64, 101, 1025, u64::MAX], a),
                set(b),
                set(c),
                set(d),
                // Always a timeout: with none and nothing ready both sides would wait for ever.
                ARENA + VALUES + 56 * 8 + (e % 4) * 16,
                if f % 4 == 0 {
                    0
                } else {
                    ARENA + MASK_PACKS + (f % 3) * 16
                },
            ]
        }
        // No lock that waits: it would wait for ever, as only this process holds locks.
        linux::__NR_flock => [fd(a), pick(&[5u64, 6, 8, 12, 32, 0, 7, 9], b), 0, 0, 0, 0],
        linux::__NR_openat2 => [
            dirfd(a),
            path(b),
            ARENA + HOWS + (c % 4) * 32,
            pick(&[24, 24, 32, 16, 8192], d),
            0,
            0,
        ],
        // Not by user: that reaches the reference's runner too, which has no counterpart here, and
        // a nice value raised cannot be lowered again without power.
        linux::__NR_setpriority => [
            pick(&[0, 1, 3], a),
            pick(&[0, SELF, 99999], b),
            pick(&[-21i64 as u64, u64::MAX, 0, 5, 19, 20, 100], c),
            0,
            0,
            0,
        ],
        linux::__NR_sched_setscheduler => [
            pick(&[0, SELF, 99999, u64::MAX], a),
            pick(
                &[0, 1, 2, 3, 5, 6, 7, 0x4000_0000, 0x4000_0003, u64::MAX, 100],
                b,
            ),
            if c % 5 == 0 {
                0
            } else {
                ARENA + PRIORITIES + (c % 6) * 4
            },
            0,
            0,
            0,
        ],
        linux::__NR_sched_setparam => [
            pick(&[0, SELF, 99999, u64::MAX], a),
            if b % 5 == 0 {
                0
            } else {
                ARENA + PRIORITIES + (b % 6) * 4
            },
            0,
            0,
            0,
            0,
        ],
        linux::__NR_sched_get_priority_max | linux::__NR_sched_get_priority_min => {
            [pick(&[0, 1, 2, 3, 4, 5, 6, 7, u64::MAX], a), 0, 0, 0, 0, 0]
        }
        linux::__NR_mlock | linux::__NR_munlock => [play(a), play_length(b), 0, 0, 0, 0],
        linux::__NR_mlock2 => [play(a), play_length(b), pick(&[0, 1, 2], c), 0, 0, 0],
        linux::__NR_mlockall => [pick(&[0, 1, 2, 3, 4, 5, 6, 7, 8], a), 0, 0, 0, 0, 0],
        linux::__NR_munlockall => [0; 6],
        linux::__NR_sched_setattr => {
            let attributes = if b % 4 == 0 {
                buffer(b >> 2)
            } else {
                ARENA + SCHED_ATTRS + ((b >> 2) % sched_attrs().len() as u64) * 128
            };
            [
                pick(&[0, SELF, 99999, u64::MAX], a),
                attributes,
                pick(&[0, 0, 0, 1], c),
                0,
                0,
                0,
            ]
        }
        linux::__NR_sched_getattr => [
            pick(&[0, SELF, 99999, u64::MAX], a),
            buffer(b),
            pick(&[0, 47, 48, 56, 64, 120, 4096, 4097], c),
            pick(&[0, 0, 0, 1], d),
            0,
            0,
        ],
        linux::__NR_sched_rr_get_interval => {
            [pick(&[0, SELF, 99999, u64::MAX], a), buffer(b), 0, 0, 0, 0]
        }
        linux::__NR_mincore => [play(a), play_length(b), buffer(c), 0, 0, 0],
        // Not by user: that names the reference's runner too.
        linux::__NR_ioprio_get | linux::__NR_ioprio_set => [
            pick(&[0, 1, 1, 2, 4], a),
            pick(&[0, SELF, 99999], b),
            pick(
                &[
                    0u64,
                    1,
                    1 << 13,
                    2 << 13 | 5,
                    2 << 13 | 7,
                    3 << 13,
                    7 << 13,
                    2 << 13 | 8,
                    1 << 16 | 2 << 13,
                    u64::MAX,
                ],
                c,
            ),
            0,
            0,
            0,
        ],
        // Not huge page files (hugetlbfs is not built).
        linux::__NR_memfd_create => {
            let names = xattr_names().len() as u64;
            let name = match a % 4 {
                0 => 8,
                1 => ARENA + XATTR_NAMES + xattr_name_offset((a >> 8) as usize % names as usize),
                _ => path(a >> 8),
            };
            let flags = pick(
                &[0u64, 1, 2, 3, 8, 0x10, 0x18, 0x100, 0x0a, 0x12, 21 << 26],
                b,
            );
            [name, flags, 0, 0, 0, 0]
        }
        linux::__NR_setxattr
        | linux::__NR_lsetxattr
        | linux::__NR_fsetxattr
        | linux::__NR_getxattr
        | linux::__NR_lgetxattr
        | linux::__NR_fgetxattr
        | linux::__NR_listxattr
        | linux::__NR_llistxattr
        | linux::__NR_flistxattr
        | linux::__NR_removexattr
        | linux::__NR_lremovexattr
        | linux::__NR_fremovexattr => {
            let by_fd = matches!(
                number,
                linux::__NR_fsetxattr
                    | linux::__NR_fgetxattr
                    | linux::__NR_flistxattr
                    | linux::__NR_fremovexattr
            );
            let target = if by_fd { fd(a) } else { path(a) };
            let names = xattr_names().len();
            let name = ARENA + XATTR_NAMES + xattr_name_offset((b % names as u64) as usize);
            let name = if b >> 60 == 0 { 8 } else { name };
            let size = pick(&[0u64, 1, 5, 20, 24, 28, 64, 65536, 65537], d);
            let value = if c % 3 == 0 {
                ARENA + XATTR_VALUES + ((c >> 8) % XATTR_VALUE_POOL.len() as u64) * 32
            } else {
                buffer(c)
            };
            match number {
                linux::__NR_setxattr | linux::__NR_lsetxattr | linux::__NR_fsetxattr => {
                    let size = if (ARENA + XATTR_VALUES..ARENA + OUTPUT).contains(&value)
                        && c >> 60 != 0
                    {
                        XATTR_VALUE_POOL[((value - ARENA - XATTR_VALUES) / 32) as usize].len()
                            as u64
                    } else {
                        size
                    };
                    [target, name, value, size, pick(&[0, 0, 1, 2, 3, 4], e), 0]
                }
                linux::__NR_getxattr | linux::__NR_lgetxattr | linux::__NR_fgetxattr => {
                    [target, name, buffer(c), size, 0, 0]
                }
                linux::__NR_listxattr | linux::__NR_llistxattr | linux::__NR_flistxattr => {
                    [target, buffer(c), size, 0, 0, 0]
                }
                _ => [target, name, 0, 0, 0, 0],
            }
        }
        _ => unreachable!("every call in CALLS is decoded"),
    };
    Call { number, arguments }
}

fn sequence_bytes(sequence: &Sequence) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(sequence.arena.len() as u32).to_le_bytes());
    out.extend_from_slice(&sequence.arena);
    out.extend_from_slice(&(sequence.calls.len() as u32).to_le_bytes());
    for call in &sequence.calls {
        out.extend_from_slice(&u64::from(call.number).to_le_bytes());
        for argument in call.arguments {
            out.extend_from_slice(&argument.to_le_bytes());
        }
    }
    out
}

/// The runner in a long-lived reference container, taking one sequence at a time.
pub struct Reference {
    name: String,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Reference {
    pub fn start(runner: &Path) -> Self {
        let name = format!("wc-kernel-layer1-{}", std::process::id());
        let directory = runner
            .parent()
            .expect("runner has a directory")
            .canonicalize()
            .expect("directory exists");
        let mut child = Command::new("docker")
            .args([
                "run",
                "-i",
                "--rm",
                "--platform",
                "linux/arm64",
                "--name",
                &name,
            ])
            .args(["--hostname", reference::HOSTNAME])
            .arg("--volume")
            .arg(format!("{}:/wc:ro", directory.display()))
            .arg(reference::IMAGE)
            .arg(Path::new("/wc").join(runner.file_name().expect("runner has a name")))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("running docker");
        let input = child.stdin.take().expect("stdin");
        let output = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            name,
            child,
            input,
            output,
        }
    }

    pub fn run(&mut self, sequence: &Sequence) -> SequenceResult {
        self.input
            .write_all(&sequence_bytes(sequence))
            .expect("sending a sequence");
        self.input.flush().expect("sending a sequence");
        let mut word = [0u8; 4];
        self.output.read_exact(&mut word).expect("reading a result");
        let mut reported = vec![0; u32::from_le_bytes(word) as usize];
        self.output
            .read_exact(&mut reported)
            .expect("reading a result");
        self.output
            .read_exact(&mut word)
            .expect("reading the status");
        SequenceResult {
            calls: parse_reports(&reported),
            status: i32::from_le_bytes(word),
        }
    }
}

/// The calls a child reported, without a last record cut short by the child's death.
fn parse_reports(mut bytes: &[u8]) -> Vec<CallResult> {
    fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Option<&'a [u8]> {
        let (taken, rest) = bytes.split_at_checked(length)?;
        *bytes = rest;
        Some(taken)
    }
    fn word(bytes: &mut &[u8]) -> Option<u32> {
        Some(u32::from_le_bytes(take(bytes, 4)?.try_into().ok()?))
    }
    let mut calls = Vec::new();
    let mut record = || -> Option<CallResult> {
        let value = i64::from_le_bytes(take(&mut bytes, 8)?.try_into().ok()?);
        let mut changes = Vec::new();
        for _ in 0..word(&mut bytes)? {
            let offset = word(&mut bytes)?;
            let length = word(&mut bytes)?;
            changes.push((offset, take(&mut bytes, length as usize)?.to_vec()));
        }
        Some(CallResult { value, changes })
    };
    while let Some(call) = record() {
        calls.push(call);
    }
    calls
}

impl Drop for Reference {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .output();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The runner program, built once per test target directory.
pub fn runner(out: &Path) -> std::path::PathBuf {
    guest::build("layer1", &[], out)
}

/// The changed ranges between two snapshots, as the runner reports them.
fn changes(before: &[u8], after: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < after.len() {
        if after[at] == before[at] {
            at += 1;
            continue;
        }
        let start = at;
        while at < after.len() && after[at] != before[at] {
            at += 1;
        }
        out.push((start as u32, after[start..at].to_vec()));
    }
    out
}

/// Runs a sequence on our kernel: a fresh container with the runner as its program, whose process makes
/// the calls with no instructions run.
pub fn run_ours(sequence: &Sequence, runner: &[u8]) -> SequenceResult {
    let mut cpu = BufferCpu::new();
    let mut host = TestHost::new(0);
    let mut kernel = Kernel::new();
    let files = [
        File {
            path: "wc".into(),
            mode: 0o755,
            uid: 0,
            gid: 0,
            mtime: 0,
            kind: Kind::Directory,
        },
        File {
            path: "wc/layer1".into(),
            mode: 0o755,
            uid: 0,
            gid: 0,
            mtime: 0,
            kind: Kind::Regular(runner.to_vec()),
        },
        File {
            path: "dev".into(),
            mode: 0o755,
            uid: 0,
            gid: 0,
            mtime: 0,
            kind: Kind::Directory,
        },
        File {
            path: "proc".into(),
            mode: 0o555,
            uid: 0,
            gid: 0,
            mtime: 0,
            kind: Kind::Directory,
        },
    ];
    crate::image::add_files(&mut kernel, &files);
    let environment: Vec<&[u8]> = reference::ENVIRONMENT
        .iter()
        .map(|variable| variable.as_bytes())
        .collect();
    let start = Start {
        path: b"/wc/layer1",
        arguments: &[b"/wc/layer1"],
        environment: &environment,
        working_directory: b"/",
        hostname: reference::HOSTNAME.as_bytes(),
        groups: &[0, 0, 1, 2, 3, 4, 6, 10, 11, 20, 26, 27],
    };
    kernel
        .start(&mut cpu, &mut host, &start)
        .expect("starting the runner");
    let memory = wcontainers_abi::MemoryId(1);
    match kernel.next() {
        Next::Run { .. } => {}
        other => panic!("the runner did not start: {other:?}"),
    }
    // Makes one call, as if the running thread had reached it, and waits for its value; the process's
    // wait status if it ended.
    let call = |kernel: &mut Kernel,
                cpu: &mut BufferCpu,
                host: &mut TestHost,
                call: Call|
     -> Result<i64, i32> {
        kernel.exit(
            cpu,
            host,
            Exit::Syscall(Syscall {
                number: call.number.into(),
                arguments: call.arguments,
            }),
        );
        loop {
            match kernel.next() {
                Next::Run {
                    resume: wcontainers_abi::Resume::Return(value),
                    ..
                } => return Ok(value as i64),
                Next::Run { resume, .. } => panic!("a call ended with {resume:?}"),
                Next::Finished { status } => return Err(status),
                // Waiting for ever: the runner kills such a sequence at its deadline.
                Next::Idle {
                    until_nanoseconds: None,
                } => return Err(linux::SIGKILL as i32),
                Next::Idle {
                    until_nanoseconds: Some(until),
                } => {
                    host.skip_to(until);
                    kernel.event(cpu, host, Event::Timer);
                }
            }
        }
    };
    let setup = Call {
        number: linux::__NR_mmap,
        arguments: [ARENA, ARENA_SIZE, 3, 0x32, u64::MAX, 0],
    };
    assert_eq!(
        call(&mut kernel, &mut cpu, &mut host, setup),
        Ok(ARENA as i64)
    );
    cpu.write(memory, ARENA, &sequence.arena)
        .expect("writing the arena");
    let mut before = vec![0u8; ARENA_SIZE as usize];
    use wcontainers_abi::Cpu;
    cpu.read(memory, ARENA, &mut before)
        .expect("reading the arena");
    let own = Call {
        number: linux::__NR_getpid,
        arguments: [0; 6],
    };
    let pid = call(&mut kernel, &mut cpu, &mut host, own).expect("getpid") as u64;
    let mut results = Vec::new();
    for each in &sequence.calls {
        let mut each = *each;
        for argument in &mut each.arguments {
            if *argument == SELF {
                *argument = pid;
            }
        }
        match call(&mut kernel, &mut cpu, &mut host, each) {
            Ok(value) => {
                let mut after = vec![0u8; ARENA_SIZE as usize];
                cpu.read(memory, ARENA, &mut after)
                    .expect("reading the arena");
                results.push(CallResult {
                    value,
                    changes: changes(&before, &after),
                });
                before = after;
            }
            Err(status) => {
                return SequenceResult {
                    calls: results,
                    status,
                };
            }
        }
    }
    SequenceResult {
        calls: results,
        status: 0,
    }
}

/// What is compared for one call: its value and the whole arena after it, with the bytes that must
/// differ between two machines cleared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub value: i64,
    pub arena: Vec<u8>,
}

/// The ranges of a call's output that hold values no two machines share: inode and device numbers,
/// times, addresses chosen by the kernel, random bytes and the kernel's own version.
fn volatile(call: &Call, value: i64, arena: &[u8]) -> Vec<(u64, u64)> {
    let [a, b, c, d, e, _] = call.arguments;
    let mut out = Vec::new();
    // The time left, which ppoll writes back even when it fails.
    if call.number == linux::__NR_ppoll && c != 0 {
        out.push((c, 16));
    }
    if call.number == linux::__NR_pselect6 && e != 0 {
        out.push((e, 16));
    }
    // The CPU number, written before a fault on the node number.
    if call.number == linux::__NR_getcpu {
        out.push((a, 4));
    }
    // The time, written before a fault on the time zone.
    if call.number == linux::__NR_gettimeofday {
        out.push((a, 16));
    }
    // Which pages are in memory: every mapped one here, the touched ones in the reference (a declared
    // difference), written up to a gap that fails the call.
    if call.number == linux::__NR_mincore {
        out.push((c, b.div_ceil(4096).min(1 << 20)));
    }
    if value < 0 {
        return out;
    }
    if call.number == linux::__NR_getcwd && under_proc(a, arena) {
        out.push((a, value as u64));
    }
    // The link count of `/proc` itself counts every process on the machine (Linux's
    // `proc_root_getattr`), and the reference shares its machine. `/proc` is inode 1 of a file system
    // with 1024 byte blocks.
    let read = |address: u64, length: usize| {
        address
            .checked_sub(ARENA)
            .and_then(|offset| arena.get(offset as usize..offset as usize + length))
            .map(|bytes| {
                let mut word = [0u8; 8];
                word[..length].copy_from_slice(bytes);
                u64::from_le_bytes(word)
            })
    };
    let proc_root =
        |ino: u64, blksize: u64| read(ino, 8) == Some(1) && read(blksize, 4) == Some(1024);
    match call.number {
        linux::__NR_newfstatat if proc_root(c + 8, c + 56) => out.push((c + 20, 4)),
        linux::__NR_fstat if proc_root(b + 8, b + 56) => out.push((b + 20, 4)),
        linux::__NR_statx if proc_root(e + 32, e + 4) => out.push((e + 16, 4)),
        _ => {}
    }
    match call.number {
        linux::__NR_newfstatat => out.extend([(c, 16), (c + 72, 48)]),
        linux::__NR_fstat => out.extend([(b, 16), (b + 72, 48)]),
        linux::__NR_statx => out.extend([(e + 32, 8), (e + 64, 64), (e + 136, 16)]),
        linux::__NR_fstatfs => out.push((b + 16, 48)),
        linux::__NR_clock_gettime => out.push((b, 16)),
        linux::__NR_times => out.push((a, 32)),
        linux::__NR_sysinfo => out.push((a, 112)),
        linux::__NR_getrusage => out.push((b, 144)),
        linux::__NR_statfs => out.push((b + 16, 48)),
        linux::__NR_getrandom => out.push((a, value as u64)),
        // The default time slice depends on the machine's CPU count, one here.
        linux::__NR_sched_rr_get_interval => out.push((b, 16)),
        linux::__NR_sched_getattr if c >= 32 => out.push((b + 24, 8)),
        linux::__NR_uname => out.push((a + 130, 130)),
        // The guest is told it has one CPU, a declared difference.
        linux::__NR_sched_getaffinity => out.push((c, value as u64)),
        linux::__NR_getdents64 => {
            let mut at = 0u64;
            while at + 19 <= value as u64 {
                out.push((b + at, 8));
                let offset = (b + at + 16).wrapping_sub(ARENA) as usize;
                let length = arena
                    .get(offset..offset + 2)
                    .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
                if length == 0 {
                    break;
                }
                at += u64::from(length);
            }
        }
        _ => {}
    }
    let _ = d;
    out
}

/// Whether the path written at `address` in the arena lies in `/proc`.
fn under_proc(address: u64, arena: &[u8]) -> bool {
    address
        .checked_sub(ARENA)
        .and_then(|offset| arena.get(offset as usize..offset as usize + 6))
        == Some(b"/proc/".as_slice())
}

/// Whether a call's value is an address the kernel chose.
/// Process numbers differ too: the reference's caller is a child of its runner.
fn value_is_volatile(call: &Call, arena: &[u8]) -> bool {
    match call.number {
        // A working directory inside `/proc/<pid>` names the caller's process number.
        linux::__NR_getcwd => under_proc(call.arguments[0], arena),
        linux::__NR_mmap => call.arguments[3] & 0x10_0010 == 0,
        linux::__NR_getpid
        | linux::__NR_getppid
        | linux::__NR_gettid
        | linux::__NR_getpgid
        | linux::__NR_getsid
        | linux::__NR_times
        | linux::__NR_setsid
        | linux::__NR_set_tid_address => true,
        // Moved by the kernel to a place of its choosing.
        linux::__NR_mremap => call.arguments[3] & 3 == 1,
        _ => false,
    }
}

/// Everything compared for a sequence, call by call, and the final status.
pub fn seen(sequence: &Sequence, result: &SequenceResult) -> (Vec<Seen>, i32) {
    let mut arena = sequence.arena.clone();
    arena.resize(ARENA_SIZE as usize, 0);
    let mut mask = vec![false; ARENA_SIZE as usize];
    let mut out = Vec::new();
    for (call, each) in sequence.calls.iter().zip(&result.calls) {
        for (offset, bytes) in &each.changes {
            arena[*offset as usize..*offset as usize + bytes.len()].copy_from_slice(bytes);
        }
        for (start, length) in volatile(call, each.value, &arena) {
            let Some(from) = start.checked_sub(ARENA) else {
                continue;
            };
            for index in from..(from + length).min(ARENA_SIZE) {
                mask[index as usize] = true;
            }
        }
        let mut shown = arena.clone();
        for (byte, masked) in shown.iter_mut().zip(&mask) {
            if *masked {
                *byte = 0;
            }
        }
        let value = if value_is_volatile(call, &arena) && each.value >= 0 {
            0
        } else {
            each.value
        };
        out.push(Seen {
            value,
            arena: shown,
        });
    }
    (out, result.status)
}

/// A readable account of where two runs of a sequence part.
pub fn difference(
    sequence: &Sequence,
    ours: &SequenceResult,
    theirs: &SequenceResult,
) -> Option<String> {
    let (our_seen, our_status) = seen(sequence, ours);
    let (their_seen, their_status) = seen(sequence, theirs);
    let mut text = String::new();
    for (index, call) in sequence.calls.iter().enumerate() {
        let name = wcontainers_kernel::system_call_name(call.number.into()).unwrap_or("?");
        let line = format!("{index}: {name}({:x?})", call.arguments);
        match (our_seen.get(index), their_seen.get(index)) {
            (Some(our), Some(their)) if our == their => {
                text.push_str(&format!("{line} = {}\n", our.value))
            }
            (Some(our), Some(their)) => {
                text.push_str(&format!(
                    "{line}: ours {} theirs {}\n",
                    our.value, their.value
                ));
                for (offset, (mine, other)) in our.arena.iter().zip(&their.arena).enumerate() {
                    if mine != other {
                        text.push_str(&format!(
                            "  arena+{offset:#x}: ours {mine:#04x} theirs {other:#04x}\n"
                        ));
                    }
                }
                return Some(text);
            }
            (None, None) => break,
            (our, their) => {
                text.push_str(&format!(
                    "{line}: ours {:?} theirs {:?} (one side ended; status ours {our_status:#x} theirs {their_status:#x})\n",
                    our.map(|seen| seen.value),
                    their.map(|seen| seen.value)
                ));
                return Some(text);
            }
        }
    }
    (our_status != their_status)
        .then(|| format!("{text}status: ours {our_status:#x} theirs {their_status:#x}"))
}
