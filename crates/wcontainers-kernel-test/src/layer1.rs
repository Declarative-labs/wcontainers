//! Layer 1: random sequences of system calls, made in the reference by a small native runner and in our
//! kernel with a plain buffer as guest memory. Every return value and the arena after every call are
//! compared, and the play area (which pages can be read, and their bytes) after the last call: reading
//! it brings pages in, which would change what later calls see. The few values that must differ
//! (inode numbers, times, process numbers, `/proc`'s link count) are masked on both sides.

use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use linux_raw_sys::general as linux;
use linux_raw_sys::ioctl;
use linux_raw_sys::net;
use linux_raw_sys::ptrace as seccomp;
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
/// Socket addresses: a local abstract name, IPv4 and IPv6 loopback at a port nothing listens on
/// (no sequence binds it, so nothing a sequence leaves behind in the reference's network can
/// change a later one), netlink's kernel, and the IPv4 any address with port 0.
const SOCKET_ADDRESSES: u64 = 0x3680;
const SOCKET_ADDRESS_AT: [(u64, u64); 5] = [(0, 6), (0x10, 16), (0x20, 28), (0x40, 12), (0x50, 16)];
/// Where recvmsg puts control messages: the receive headers all point here, and both runners zero
/// it after each recvmsg (a sender's process number and times land there).
const RECV_CONTROL: u64 = 0x1500;
const RECV_CONTROL_SIZE: u64 = 128;
/// Control messages sendmsg's headers carry, one after another: descriptors (one, two, one not
/// open), credentials naming process 0, a header alone, and an IP_TTL for a datagram.
const SEND_CONTROLS: u64 = 0x1580;
/// Where the starting socketpair puts its two descriptors.
const SOCKET_PAIR: u64 = 0x1ac0;
/// `struct msghdr`s, 56 bytes each: the first `SEND_HEADERS` for sendmsg, the rest for recvmsg.
const MSGHDRS: u64 = 0x1640;
const MSGHDR_COUNT: u64 = 8;
const SEND_HEADERS: u64 = 5;
/// `struct epoll_event`s, 16 bytes apart on ARM64.
const EPOLL_EVENTS: u64 = 0x3700;
const EPOLL_EVENT_MASKS: [u32; 4] = [
    linux::EPOLLIN | linux::EPOLLOUT,
    linux::EPOLLIN | linux::EPOLLET,
    linux::EPOLLOUT | linux::EPOLLONESHOT,
    linux::EPOLLIN | linux::EPOLLEXCLUSIVE,
];
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
/// Where getrandom writes: 96 bytes no call reads, since the two runners' random bytes differ and
/// would steer later calls apart.
const RANDOM: u64 = 0x3ba0;
/// Where uname writes, past the paths: no call reads it, since the runners' release and version
/// differ (a declared difference) and would steer later calls apart.
const UNAME: u64 = 0x1800;
/// Where name_to_handle_at writes the mount number, which differs between the machines; no call reads
/// it.
const MOUNT_NUMBER: u64 = 0x1a00;
/// `termios2` values for the terminal requests, 48 bytes apart: Linux's default for a terminal, raw
/// input taking a byte at a time, and one made from generated words.
const TERMIOS: u64 = 0x1b00;
const TERMIOS_COUNT: u64 = 3;
/// `struct futex_waitv` entries for the futex2 calls, 24 bytes apart: words of `VALUES`, one shared,
/// one 64 bit (refused) and one misaligned.
const FUTEX_VECTORS: u64 = 0x1990;
/// Timer values, 32 bytes apart, each read as an `itimerspec` and as an `itimerval`: none, a
/// one-shot and a periodic one that never come within a sequence on any clock (2^33 seconds, which
/// is also an absolute time far ahead), and an invalid one.
const ITIMERS: u64 = 0x1a40;
/// `struct flock` requests for fcntl's record locks, 32 bytes apart.
const FLOCKS: u64 = 0x1c00;
const FLOCK_COUNT: u64 = 8;
/// `struct sembuf` operations for semop, 6 bytes apart: the first `SEMBUFS_NOWAIT` never wait
/// (IPC_NOWAIT), the rest may (only semtimedop with a timeout reaches them). No call writes here.
const SEMBUFS: u64 = 0x1d00;
const SEMBUF_COUNT: u64 = 16;
const SEMBUFS_NOWAIT: u64 = 12;
/// Line disciplines for TIOCSETD, 4 bytes apart: `n_tty`, ones neither machine has registered,
/// `N_MOUSE` (registered on the reference; its open needs a power a container lacks) and numbers out
/// of range. Not those the reference has registered and this kernel does not build. No call writes
/// here.
const DISCIPLINES: u64 = 0x1d60;
const DISCIPLINE_VALUES: [i32; 8] = [0, 0, 1, 2, 4, -1, 31, 256];
/// Four `struct mq_attr` for mq_open and mq_getsetattr, 64 bytes apart.
const MQ_ATTRS: u64 = 0x1d80;
/// Four `struct sigevent` for mq_notify, 64 bytes apart, and the cookie a thread notification reads.
const SIGEVENTS: u64 = 0x1e80;
/// `sched_attr` structures for sched_setattr, 128 bytes apart.
const SCHED_ATTRS: u64 = 0x2000;
/// `file_handle` headers for name_to_handle_at, 128 bytes apart: room for 128 bytes, the 12 tmpfs
/// needs, none, and more than Linux allows.
const HANDLES: u64 = 0x2600;
const HANDLE_ROOMS: [u32; 4] = [128, 12, 0, 129];
/// `stack_t` values for sigaltstack, 24 bytes apart.
const SIGNAL_STACKS: u64 = 0x2800;
/// Stack limits for RLIMIT_STACK, 16 bytes apart.
const STACK_LIMITS: u64 = 0x2900;
/// Seccomp filter programs, `FILTER_SPACING` bytes apart.
const FILTERS: u64 = 0x2a00;
const FILTER_COUNT: u64 = 4;
const FILTER_SPACING: u64 = 0x180;
/// `sock_fprog` values for seccomp, 16 bytes apart: one for each filter, then an empty one, one too
/// long, one with no program and one whose program cannot be read.
const FPROGS: u64 = 0x3100;
const FPROG_COUNT: u64 = 8;
/// Actions for SECCOMP_GET_ACTION_AVAIL, 4 bytes apart.
const ACTIONS: u64 = 0x3180;
/// How many random words make the filters: for each, a length and up to twelve instructions.
const FILTER_WORDS: usize = 13;
/// Where the filters' words start in the generated values.
const FILTER_VALUES: usize = 72;
/// Areas for rseq: 64 byte aligned, 32 byte aligned and 16 byte aligned ones.
const RSEQS: u64 = 0x3200;
/// The offsets of the rseq areas that can be registered (aligned to 32 bytes at least).
const RSEQ_AREAS: [u64; 3] = [0, 0x20, 0x40];
/// Arguments for ioctl's requests, 64 bytes apart: inode flags (to keep, an immutable one, unknown
/// ones, none), extended flags (to keep, an extent size tmpfs refuses), a clone range from
/// descriptor 3, a deduplication range for descriptor 3, and preallocation ranges from each origin
/// and from an unknown one.
const IOCTL_ARGS: u64 = 0x3300;
const IOCTL_ARG_COUNT: u64 = 12;

/// Extended attribute names, one after another.
const XATTR_NAMES: u64 = 0x3c00;
/// Attribute values with meaning: file capabilities in their two forms (the long one with a root of
/// 1000 and of 0), and an access control list.
const XATTR_VALUES: u64 = 0x3f00;

/// An argument each runner replaces by its caller's process number, so calls can name their own
/// process although the two runners' numbers differ. Its negation, the caller's own process group, is
/// not generated: our caller is process 1, and -1 means every process.
pub const SELF: u64 = 0x5e1f_5e1f_0000_0001;
/// An argument each runner replaces by the identifier its sequence's last successful shmget gave, or
/// by `NO_SEGMENT` before one: identifiers differ between the runners (the reference's slots and
/// sequence numbers carry over from earlier sequences).
pub const SEGMENT: u64 = 0x5e1f_5e1f_0000_0002;
/// An identifier whose slot neither runner reaches.
pub const NO_SEGMENT: u64 = 4000;
/// As `SEGMENT`, for the last identifier msgget gave (`NO_SEGMENT` before one).
pub const QUEUE: u64 = 0x5e1f_5e1f_0000_0003;
/// As `SEGMENT`, for the last identifier semget gave (`NO_SEGMENT` before one).
pub const SEMAPHORES: u64 = 0x5e1f_5e1f_0000_0004;
const OUTPUT: u64 = 0x4000;

/// The sequence's work directory, made and entered by its first two calls and removed afterwards.
pub const WORK: &str = "/dev/shm/w";

/// The name in the work directory a named pipe is made at. Opens of it never wait: they are
/// nonblocking unless for reading and writing (a reopen through `/proc/self/fd` may still wait, which
/// both runners end at the reference's deadline).
const FIFO: &str = "p";

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
    // The random devices: what a read of them puts anywhere is zeroed by both runners (see
    // `CallResult::random`), so every call can be made on them.
    "/dev/random",
    "/dev/urandom",
    "/dev/shm/w/a",
    "/proc/self/fd/3",
    "/proc/self/cwd",
    "l/x",
    "c/",
    "/proc/self/",
    "/proc/self/fd",
    "/proc/sys",
    // `/proc` files whose bytes are the same in both runners: a seq_file, one read from memory and a
    // sysctl, each with its own way of seeking, reading and writing.
    "/proc/self/comm",
    "/proc/self/cmdline",
    "/proc/sys/kernel/ostype",
    "/proc/self/cgroup",
    "/proc/self/limits",
    // devpts's root, which statx and the others answer by its own rules.
    "/dev/pts",
    // The mqueue file system and a queue in it, which mq_open names "a".
    "/dev/mqueue",
    "/dev/mqueue/a",
    // Pseudo-terminals: the master, the first two slaves, and the controlling terminal, which no
    // sequence has (every open is O_NOCTTY).
    "/dev/ptmx",
    "/dev/pts/0",
    "/dev/pts/1",
    "/dev/tty",
];

/// The paths whose opens make or reach a pseudo-terminal.
const TERMINAL_PATHS: &[&str] = &["/dev/ptmx", "/dev/pts/0", "/dev/pts/1"];

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
    /// Whether its work directory is on the file backend, mounted at [`BACKEND_ROOT`].
    pub backend: bool,
    /// Whether that backend opens files for the page cache rather than for direct reads and
    /// writes.
    pub cached: bool,
}

/// Where the file backend is mounted for sequences on it: a name as long as `/dev/shm`, which it
/// takes the place of in the path pool, so every path keeps its offset.
pub const BACKEND_ROOT: &str = "/backend";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallResult {
    pub value: i64,
    /// Changed ranges of the arena: offset and the bytes now there.
    pub changes: Vec<(u32, Vec<u8>)>,
    /// The call's result depends on what the two runners cannot share: a directory that outlives
    /// the sequence (`/dev/shm` or `/dev`, whose offsets in the reference count every entry earlier
    /// sequences made), or a working directory in `/proc` (whose path names the caller's process
    /// number, and so differs in length). Only the reference reports it.
    pub unshared: bool,
    /// The call put a random device's bytes in memory or a file, which the reference's runner then
    /// zeroed (see `clear_random` in the runner); ours does the same when the reference says so.
    pub random: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceResult {
    pub calls: Vec<CallResult>,
    /// The wait status of the process that made the calls.
    pub status: i32,
    /// The play area after the last call, read by process_vm_readv: for each page whether it could
    /// be read, then its bytes (zeros when not). None when the process did not reach the end.
    pub play: Option<Vec<u8>>,
}

/// How many calls of a sequence the reference answered with success on its named pipe: opens of it,
/// and its making, to show they are reached.
pub fn named_pipe_calls(sequence: &Sequence, theirs: &SequenceResult) -> u64 {
    let fifo = ARENA + *path_offsets().1.last().expect("paths");
    sequence
        .calls
        .iter()
        .zip(&theirs.calls)
        .filter(|(call, result)| {
            result.value >= 0
                && match call.number {
                    linux::__NR_openat => call.arguments[1] == fifo,
                    linux::__NR_mknodat => call.arguments[1] == fifo,
                    _ => false,
                }
        })
        .count() as u64
}

/// The size of a play area record: a byte and the bytes of each page.
const PLAY_RECORD: usize = PLAY_PAGES as usize * 4097;

/// The control messages sendmsg's headers choose from: where each starts in `SEND_CONTROLS` and
/// the length a header gives it.
const SEND_CONTROL_AT: [(u64, u64); 6] =
    [(0, 24), (24, 24), (48, 24), (72, 32), (104, 16), (120, 24)];

/// Lays out sendmsg's control messages and the message headers, each chosen from a value: a
/// destination or none, one to three of the vectors, and a control message, none or one too long
/// for Linux; the receive headers name no sender (an automatic name is random) and give room for
/// control messages from none to all of `RECV_CONTROL`.
fn message_areas(arena: &mut [u8], values: &[u64]) {
    let mut put = |at: u64, bytes: &[u8]| {
        let at = at as usize;
        arena[at..at + bytes.len()].copy_from_slice(bytes);
    };
    let header = |length: u64, level: u32, kind: u32| {
        let mut bytes = length.to_le_bytes().to_vec();
        bytes.extend_from_slice(&level.to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes
    };
    let ints = |words: &[i32]| -> Vec<u8> { words.iter().flat_map(|w| w.to_le_bytes()).collect() };
    let rights = linux_raw_sys::net::SCM_RIGHTS;
    let solsocket = net::SOL_SOCKET;
    put(
        SEND_CONTROLS,
        &[header(20, solsocket, rights), ints(&[3])].concat(),
    );
    put(
        SEND_CONTROLS + 24,
        &[header(24, solsocket, rights), ints(&[3, 4])].concat(),
    );
    put(
        SEND_CONTROLS + 48,
        &[header(20, solsocket, rights), ints(&[100])].concat(),
    );
    let credentials = linux_raw_sys::net::SCM_CREDENTIALS;
    put(
        SEND_CONTROLS + 72,
        &[header(28, solsocket, credentials), ints(&[0, 0, 0])].concat(),
    );
    put(SEND_CONTROLS + 104, &header(8, solsocket, rights));
    let ttl = header(20, net::IPPROTO_IP as u32, net::IP_TTL);
    put(SEND_CONTROLS + 120, &[ttl, ints(&[7])].concat());
    for index in 0..MSGHDR_COUNT {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(53);
        let vector = ARENA + IOVECS + ((word >> 8) % 16) * 16;
        let (name, length, count, control, room) = if index < SEND_HEADERS {
            let (name, length) = match word % 6 {
                0 | 1 => (0, 0),
                choice => {
                    let (at, length) = SOCKET_ADDRESS_AT[[0, 1, 2, 4][choice as usize - 2]];
                    (ARENA + SOCKET_ADDRESSES + at, length)
                }
            };
            let (control, room) = match (word >> 16) % 9 {
                0 | 1 => (0, 0),
                2 => (ARENA + SEND_CONTROLS, 0x8000_0000),
                choice => {
                    let (at, room) = SEND_CONTROL_AT[choice as usize - 3];
                    (ARENA + SEND_CONTROLS + at, room)
                }
            };
            (
                name,
                length,
                pick(&[1u64, 1, 2, 0, 3], word >> 24),
                control,
                room,
            )
        } else {
            let room = pick(&[0u64, 16, 20, 24, 48, RECV_CONTROL_SIZE], word >> 16);
            (
                0,
                0,
                pick(&[1u64, 1, 2, 3], word >> 24),
                ARENA + RECV_CONTROL,
                room,
            )
        };
        let at = MSGHDRS + index * 56;
        put(at, &name.to_le_bytes());
        put(at + 8, &(length as u32).to_le_bytes());
        put(at + 16, &vector.to_le_bytes());
        put(at + 24, &count.to_le_bytes());
        put(at + 32, &control.to_le_bytes());
        put(at + 40, &room.to_le_bytes());
        put(at + 48, &0u32.to_le_bytes());
    }
}

/// How many sendmsg calls with control messages and how many recvmsg calls the reference ran
/// well, to show they are reached.
pub fn message_calls(sequence: &Sequence, theirs: &SequenceResult) -> (u64, u64) {
    let (mut sends, mut receives) = (0, 0);
    for (call, each) in sequence.calls.iter().zip(&theirs.calls) {
        if each.value < 0 {
            continue;
        }
        match call.number {
            linux::__NR_sendmsg => {
                let at = (call.arguments[1] - ARENA + 32) as usize;
                if sequence.arena[at..at + 8] != [0; 8] {
                    sends += 1;
                }
            }
            linux::__NR_recvmsg => receives += 1,
            _ => {}
        }
    }
    (sends, receives)
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
    // Last, the one name a named pipe is made at, which only mknodat and openat choose.
    offsets.push(PATHS + bytes.len() as u64);
    bytes.extend_from_slice(FIFO.as_bytes());
    bytes.push(0);
    assert!(
        PATHS + (bytes.len() as u64) <= RECV_CONTROL,
        "the paths reach the message areas"
    );
    (bytes, offsets)
}

/// A generated call before its arguments are decoded: which kind (wide enough to reach every entry of
/// a call list longer than 256), and random words to pick its arguments with.
pub type Raw = (u16, [u64; 6]);

/// Builds a sequence from generated parts: the arena's random values and the calls. The first two calls
/// always make and enter the work directory.
pub fn build(values: &[u64], raw: &[Raw]) -> Sequence {
    build_on(values, raw, false)
}

/// As [`build`], for a work directory on the file backend: the calls are those of
/// [`BACKEND_CALLS`] and the paths under `/dev/shm` are under [`BACKEND_ROOT`].
pub fn build_on_backend(values: &[u64], raw: &[Raw], cached: bool) -> Sequence {
    Sequence {
        cached,
        ..build_on(values, raw, true)
    }
}

fn build_on(values: &[u64], raw: &[Raw], backend: bool) -> Sequence {
    let (mut paths, path_at) = path_offsets();
    if backend {
        let (from, to) = (b"/dev/shm".as_slice(), BACKEND_ROOT.as_bytes());
        let mut at = 0;
        while let Some(found) = paths[at..]
            .windows(from.len())
            .position(|window| window == from)
        {
            paths[at + found..at + found + to.len()].copy_from_slice(to);
            at += found + to.len();
        }
    }
    let calls_made = if backend { BACKEND_CALLS } else { CALLS };
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
    // Semaphore operations: numbers in and past small sets, changes up to the limits, undo often.
    for index in 0..SEMBUF_COUNT {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(37);
        let num: u16 = [0, 0, 1, 2, 3, 7, 65535, 1][(word % 8) as usize];
        let op: i16 =
            [0, 0, 1, -1, 2, -2, 5, -5, 32767, -32768, 1, -1][((word >> 4) % 12) as usize];
        let undo: i16 = [0, 0, 0x1000, 0x1000, 0x2000, -0x8000][((word >> 8) % 6) as usize];
        let nowait = if index < SEMBUFS_NOWAIT { 0o4000 } else { 0 };
        let at = (SEMBUFS + index * 6) as usize;
        arena[at..at + 2].copy_from_slice(&num.to_le_bytes());
        arena[at + 2..at + 4].copy_from_slice(&op.to_le_bytes());
        arena[at + 4..at + 6].copy_from_slice(&(undo | nowait).to_le_bytes());
    }
    for (index, discipline) in DISCIPLINE_VALUES.iter().enumerate() {
        let at = DISCIPLINES as usize + index * 4;
        arena[at..at + 4].copy_from_slice(&discipline.to_le_bytes());
    }
    // Queue attributes: flags (O_NONBLOCK or not, or another bit), limits within and past the
    // namespace's.
    for index in 0..4u64 {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(41);
        let flags: i64 = [0, 0o4000, 0o4000, 1][(word % 4) as usize];
        let most: i64 = [10, 4, 1, 0, 11, -1][((word >> 2) % 6) as usize];
        let size: i64 = [8192, 16, 1, 0, 8193, 100][((word >> 5) % 6) as usize];
        let at = (MQ_ATTRS + index * 64) as usize;
        arena[at..at + 64].fill(0);
        for (field, value) in [flags, most, size].iter().enumerate() {
            arena[at + field * 8..at + field * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    // Notification requests: a value or cookie address, a signal whose default action is nothing
    // in both runners (or none, or past the last), and how to tell.
    for index in 0..4u64 {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(47);
        let value = [ARENA + SIGEVENTS + 256, 8, 0x1234][(word % 3) as usize];
        let signal: u32 = [0, 17, 18, 23, 28, 65][((word >> 2) % 6) as usize];
        let how: u32 = [0, 0, 1, 2, 7][((word >> 5) % 5) as usize];
        let at = (SIGEVENTS + index * 64) as usize;
        arena[at..at + 64].fill(0);
        arena[at..at + 8].copy_from_slice(&value.to_le_bytes());
        arena[at + 8..at + 12].copy_from_slice(&signal.to_le_bytes());
        arena[at + 12..at + 16].copy_from_slice(&how.to_le_bytes());
    }
    // Futex vectors and timer values.
    let vectors: [(u64, u64, u32); 4] = [
        (
            0,
            ARENA + VALUES,
            linux::FUTEX2_SIZE_U32 | linux::FUTEX2_PRIVATE,
        ),
        (1, ARENA + VALUES + 8, linux::FUTEX2_SIZE_U32),
        (0, ARENA + VALUES + 16, linux::FUTEX2_SIZE_U64),
        (
            0,
            ARENA + VALUES + 2,
            linux::FUTEX2_SIZE_U32 | linux::FUTEX2_PRIVATE,
        ),
    ];
    for (index, (value, address, flags)) in vectors.iter().enumerate() {
        let at = (FUTEX_VECTORS + index as u64 * 24) as usize;
        arena[at..at + 8].copy_from_slice(&value.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&address.to_le_bytes());
        arena[at + 16..at + 20].copy_from_slice(&flags.to_le_bytes());
        arena[at + 20..at + 24].fill(0);
    }
    let far: i64 = 1 << 33;
    let timers: [[i64; 4]; 4] = [
        [0, 0, 0, 0],
        [0, 0, far, 0],
        [1000, 0, far, 0],
        [0, 0, 0, 1_000_000_000],
    ];
    for (index, words) in timers.iter().enumerate() {
        let at = (ITIMERS + index as u64 * 32) as usize;
        for (offset, word) in words.iter().enumerate() {
            arena[at + offset * 8..at + offset * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
    }
    // Record lock requests: each kind, origin, start and length, and sometimes a process number
    // (which open file locks refuse).
    for index in 0..FLOCK_COUNT {
        let word = values
            .get(index as usize)
            .copied()
            .unwrap_or(0)
            .rotate_left(29);
        let kind: i16 = [0, 1, 2, 0, 1, 7][(word % 6) as usize];
        let whence: i16 = [0, 0, 0, 1, 2, 3][((word >> 4) % 6) as usize];
        let start: i64 = [0, 0, 1, 5, 100, -1, i64::MAX, 4096][((word >> 8) % 8) as usize];
        let length: i64 = [0, 1, 1, 10, -3, 4096, i64::MAX, -1][((word >> 12) % 8) as usize];
        let pid: i32 = if (word >> 16) % 8 == 0 { 5 } else { 0 };
        let at = (FLOCKS + index * 32) as usize;
        arena[at..at + 2].copy_from_slice(&kind.to_le_bytes());
        arena[at + 2..at + 4].copy_from_slice(&whence.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&start.to_le_bytes());
        arena[at + 16..at + 24].copy_from_slice(&length.to_le_bytes());
        arena[at + 24..at + 28].copy_from_slice(&pid.to_le_bytes());
    }
    // Socket addresses and epoll events.
    let mut put = |at: u64, bytes: &[u8]| {
        let at = (SOCKET_ADDRESSES + at) as usize;
        arena[at..at + bytes.len()].copy_from_slice(bytes);
    };
    put(0, &[1, 0, 0, b'l', b'1', b'x']);
    put(0x10, &[2, 0, 0x13, 0xeb, 127, 0, 0, 1]);
    put(0x20, &[10, 0, 0x13, 0xeb]);
    put(0x20 + 23, &[1]);
    put(0x40, &[16, 0]);
    put(0x50, &[2, 0]);
    message_areas(&mut arena, values);
    for (index, mask) in EPOLL_EVENT_MASKS.iter().enumerate() {
        let at = (EPOLL_EVENTS + index as u64 * 16) as usize;
        arena[at..at + 4].copy_from_slice(&mask.to_le_bytes());
        arena[at + 8..at + 16].copy_from_slice(&(index as u64 + 1).to_le_bytes());
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
    for (index, room) in HANDLE_ROOMS.iter().enumerate() {
        let at = (HANDLES + index as u64 * 128) as usize;
        arena[at..at + 4].copy_from_slice(&room.to_le_bytes());
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
    for (index, words) in ioctl_arguments().iter().enumerate() {
        let at = (IOCTL_ARGS + index as u64 * 64) as usize;
        for (offset, word) in words.iter().enumerate() {
            arena[at + offset * 8..at + offset * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
    }
    let generated: Vec<u64> = (0..6)
        .map(|index| values.get(40 + index).copied().unwrap_or(0))
        .collect();
    for (index, termios) in [
        terminal_settings(None),
        terminal_settings(Some(&[0; 6])),
        terminal_settings(Some(&generated)),
    ]
    .iter()
    .enumerate()
    {
        let at = (TERMIOS + index as u64 * 48) as usize;
        arena[at..at + termios.len()].copy_from_slice(termios);
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
    let choice = values
        .get(FILTER_VALUES + FILTER_WORDS * FILTER_COUNT as usize)
        .copied()
        .unwrap_or(0);
    // A quarter of the sequences start with the named pipe made.
    if (choice >> 1) % 4 == 0 {
        calls.push(Call {
            number: linux::__NR_mknodat,
            arguments: [
                linux::AT_FDCWD as u64,
                ARENA + path_at.last().expect("paths"),
                u64::from(linux::S_IFIFO | 0o600),
                0,
                0,
                0,
            ],
        });
    }
    // A quarter start with a pseudo-terminal: the master opened, its slave unlocked and opened, on
    // the descriptors calls choose most.
    if (choice >> 3) % 4 == 0 {
        let flags = u64::from(linux::O_RDWR | linux::O_NOCTTY | linux::O_NONBLOCK);
        let open = |path: &str| Call {
            number: linux::__NR_openat,
            arguments: [
                linux::AT_FDCWD as u64,
                ARENA + PATHS + path_offset(path),
                flags,
                0,
                0,
                0,
            ],
        };
        calls.push(open("/dev/ptmx"));
        calls.push(Call {
            number: linux::__NR_ioctl,
            arguments: [
                3,
                u64::from(ioctl::TIOCSPTLCK),
                ARENA + IOCTL_ARGS + 3 * 64,
                0,
                0,
                0,
            ],
        });
        calls.push(open("/dev/pts/0"));
    }
    // A quarter start with a pair of local sockets of one kind, and an eighth with a UDP socket at
    // the loopback port nothing else binds (which its datagrams to that address reach), on the
    // descriptors calls choose most.
    let mut next_fd = 3 + if (choice >> 3) % 4 == 0 { 2 } else { 0 };
    let mut receivers = Vec::new();
    if !backend && (choice >> 5) % 4 == 0 {
        let kind = pick(
            &[net::SOCK_STREAM, net::SOCK_DGRAM, net::SOCK_SEQPACKET],
            choice >> 7,
        );
        calls.push(Call {
            number: linux::__NR_socketpair,
            arguments: [
                u64::from(net::AF_UNIX),
                u64::from(kind | linux::O_NONBLOCK),
                0,
                ARENA + SOCKET_PAIR,
                0,
                0,
            ],
        });
        // A message to the other end, which a receive of it ends the sequence with.
        calls.push(Call {
            number: linux::__NR_sendmsg,
            arguments: [
                next_fd,
                ARENA + MSGHDRS + ((choice >> 12) % SEND_HEADERS) * 56,
                u64::from(net::MSG_DONTWAIT | net::MSG_NOSIGNAL),
                0,
                0,
                0,
            ],
        });
        receivers.push(next_fd + 1);
        next_fd += 2;
    }
    if !backend && (choice >> 9) % 8 == 0 {
        let flags = net::SOCK_DGRAM | linux::O_NONBLOCK;
        calls.push(Call {
            number: linux::__NR_socket,
            arguments: [u64::from(net::AF_INET), u64::from(flags), 0, 0, 0, 0],
        });
        let (at, length) = SOCKET_ADDRESS_AT[1];
        let address = ARENA + SOCKET_ADDRESSES + at;
        calls.push(Call {
            number: linux::__NR_bind,
            arguments: [next_fd, address, length, 0, 0, 0],
        });
        // A datagram to itself.
        let flags = u64::from(net::MSG_DONTWAIT);
        calls.push(Call {
            number: linux::__NR_sendto,
            arguments: [next_fd, ARENA + OUTPUT, 7, flags, address, length],
        });
        receivers.push(next_fd);
    }
    // Half the sequences may install seccomp filters, which needs no_new_privs (Docker's root lacks
    // CAP_SYS_ADMIN).
    if choice % 2 == 1 {
        calls.push(Call {
            number: linux::__NR_prctl,
            arguments: [
                u64::from(linux_raw_sys::prctl::PR_SET_NO_NEW_PRIVS),
                1,
                0,
                0,
                0,
                0,
            ],
        });
    }
    calls.extend(raw.iter().map(|raw| decode(raw, &path_at, calls_made)));
    for (index, fd) in receivers.into_iter().enumerate() {
        let word = choice >> (15 + index * 5);
        let header = SEND_HEADERS + word % (MSGHDR_COUNT - SEND_HEADERS);
        let flags =
            net::MSG_DONTWAIT | pick(&[0, net::MSG_PEEK, net::MSG_CMSG_CLOEXEC, 0], word >> 2);
        calls.push(Call {
            number: linux::__NR_recvmsg,
            arguments: [fd, ARENA + MSGHDRS + header * 56, u64::from(flags), 0, 0, 0],
        });
    }
    let terminal = opens_terminal(&calls);
    if terminal {
        keep_terminals_settled(&mut arena, &mut calls);
    }
    // A child that has ended is waited for by both runners after the call that made it, so waits
    // need not wait: a wait for a child that never stops or continues would wait for ever.
    let forks = calls.iter().any(|call| call.number == linux::__NR_clone);
    // Children have small process numbers here and others there, so a process 3 waited for is
    // one past every process number instead.
    if forks {
        for call in &mut calls {
            match call.number {
                linux::__NR_wait4 => call.arguments[2] |= u64::from(linux::WNOHANG),
                linux::__NR_waitid => {
                    call.arguments[3] |= u64::from(linux::WNOHANG);
                    if call.arguments[0] == u64::from(linux::P_PID) && call.arguments[1] == 3 {
                        call.arguments[1] = 4_000_000;
                    }
                }
                _ => {}
            }
        }
    }
    // Each filter first lets through every call the runners make themselves between and after the
    // sequence's calls (to report, to ask what a descriptor is, to zero bytes, to read the play area),
    // whose arguments (process numbers, the runners' own buffers) differ between them, and every call
    // of the sequence that names a process.
    let mut passed = vec![
        linux::__NR_write,
        linux::__NR_exit_group,
        linux::__NR_name_to_handle_at,
        linux::__NR_readlinkat,
        linux::__NR_fstat,
        linux::__NR_lseek,
        linux::__NR_pwrite64,
        linux::__NR_process_vm_readv,
        linux::__NR_process_vm_writev,
    ];
    // With a terminal, the reference's runner also polls every descriptor after each call.
    if terminal {
        passed.push(linux::__NR_ppoll);
    }
    // And after a child is made, waits for it to end, and tells it from its parent by getpid.
    if forks {
        passed.extend([linux::__NR_waitid, linux::__NR_getpid]);
    }
    for call in &calls {
        if (call.arguments.contains(&SELF)
            || call.arguments.contains(&SEGMENT)
            || call.arguments.contains(&QUEUE)
            || call.arguments.contains(&SEMAPHORES))
            && !passed.contains(&call.number)
        {
            passed.push(call.number);
        }
    }
    for index in 0..FILTER_COUNT {
        let start = FILTER_VALUES + FILTER_WORDS * index as usize;
        let words: Vec<u64> = (start..start + FILTER_WORDS)
            .map(|at| values.get(at).copied().unwrap_or(0))
            .collect();
        let program = filter_program(&words, &passed);
        let at = (FILTERS + index * FILTER_SPACING) as usize;
        arena[at..at + program.len()].copy_from_slice(&program);
        let fprog = (FPROGS + index * 16) as usize;
        arena[fprog..fprog + 2].copy_from_slice(&((program.len() / 8) as u16).to_le_bytes());
        arena[fprog + 8..fprog + 16]
            .copy_from_slice(&(ARENA + FILTERS + index * FILTER_SPACING).to_le_bytes());
    }
    let odd: [(u16, u64); 4] = [
        (0, ARENA + FILTERS),
        (4097, ARENA + FILTERS),
        (1, 0),
        (2, u64::MAX - 4095),
    ];
    for (index, (length, address)) in odd.iter().enumerate() {
        let fprog = (FPROGS + (FILTER_COUNT + index as u64) * 16) as usize;
        arena[fprog..fprog + 2].copy_from_slice(&length.to_le_bytes());
        arena[fprog + 8..fprog + 16].copy_from_slice(&address.to_le_bytes());
    }
    for (index, action) in SECCOMP_ACTIONS.iter().enumerate() {
        let at = (ACTIONS + index as u64 * 4) as usize;
        arena[at..at + 4].copy_from_slice(&action.to_le_bytes());
    }
    Sequence {
        arena,
        calls,
        backend,
        cached: false,
    }
}

/// Whether a call opens a pseudo-terminal's path.
fn opens_terminal_path(call: &Call) -> bool {
    matches!(call.number, linux::__NR_openat | linux::__NR_openat2)
        && TERMINAL_PATHS
            .iter()
            .any(|path| call.arguments[1] == ARENA + PATHS + path_offset(path))
}

/// Whether a sequence opens a pseudo-terminal.
fn opens_terminal(calls: &[Call]) -> bool {
    calls.iter().any(opens_terminal_path)
}

/// How many children a sequence's calls made in the reference.
pub fn children_made(sequence: &Sequence, theirs: &SequenceResult) -> u64 {
    sequence
        .calls
        .iter()
        .zip(&theirs.calls)
        .filter(|(call, result)| call.number == linux::__NR_clone && result.value > 0)
        .count() as u64
}

/// How many of a sequence's calls opened a pseudo-terminal in the reference.
pub fn terminal_opens(sequence: &Sequence, theirs: &SequenceResult) -> u64 {
    sequence
        .calls
        .iter()
        .zip(&theirs.calls)
        .filter(|(call, result)| result.value >= 0 && opens_terminal_path(call))
        .count() as u64
}

/// Linux moves what one side of a pseudo-terminal writes into the other's line discipline later, in
/// a work queue, while here it is moved at once. The reference's runner polls every descriptor after
/// each call, which runs that work (`n_tty_poll`), so both agree between calls; within a call they
/// agree as long as nothing fills: a sequence with a terminal writes at most 256 bytes a call
/// (fifteen calls stay below the line discipline's 4095), and every descriptor stays nonblocking,
/// since a wait for input that never comes would cost the reference's deadline.
fn keep_terminals_settled(arena: &mut [u8], calls: &mut [Call]) {
    const MOST: u64 = 256;
    for index in 0..16u64 {
        let at = (IOVECS + index * 16 + 8) as usize;
        let length = u64::from_le_bytes(arena[at..at + 8].try_into().expect("8 bytes"));
        arena[at..at + 8].copy_from_slice(&length.min(MOST).to_le_bytes());
    }
    for index in 0..4u64 {
        let at = (HOWS + index * 32) as usize;
        let flags = u64::from_le_bytes(arena[at..at + 8].try_into().expect("8 bytes"));
        arena[at..at + 8].copy_from_slice(&(flags | u64::from(linux::O_NONBLOCK)).to_le_bytes());
    }
    let nonblocking = u64::from(linux::O_NONBLOCK);
    for call in calls {
        let arguments = &mut call.arguments;
        match call.number {
            linux::__NR_openat => arguments[2] |= nonblocking,
            linux::__NR_write | linux::__NR_pwrite64 | linux::__NR_sendfile => {
                let count = if call.number == linux::__NR_sendfile {
                    3
                } else {
                    2
                };
                arguments[count] = arguments[count].min(MOST);
            }
            linux::__NR_splice => arguments[4] = arguments[4].min(MOST),
            linux::__NR_writev | linux::__NR_pwritev | linux::__NR_pwritev2
                if arguments[2] <= 16 =>
            {
                arguments[2] = arguments[2].min(1);
            }
            linux::__NR_fcntl if arguments[1] == u64::from(linux::F_SETFL) => {
                arguments[2] |= nonblocking;
            }
            linux::__NR_ioctl if arguments[1] == u64::from(ioctl::FIONBIO) => {
                arguments[2] = ARENA + IOCTL_ARGS;
            }
            _ => {}
        }
    }
}

/// A `termios2`: Linux's default for a terminal (`tty_std_termios`) with `None`, raw input taking a
/// byte at a time with all-zero words, else flags, control characters and speeds from the words,
/// each flag word kept to the bits Linux defines.
fn terminal_settings(words: Option<&[u64]>) -> Vec<u8> {
    const INIT_C_CC: [u8; 19] = [
        3, 28, 127, 21, 4, 0, 1, 0, 17, 19, 26, 0, 18, 15, 23, 22, 0, 0, 0,
    ];
    let cflag = linux::B38400 | linux::CS8 | linux::CREAD;
    let (iflag, oflag, cflag, lflag, cc, speed) = match words {
        None => (
            linux::ICRNL | linux::IXON,
            linux::OPOST | linux::ONLCR,
            cflag | linux::HUPCL,
            linux::ISIG
                | linux::ICANON
                | linux::ECHO
                | linux::ECHOE
                | linux::ECHOK
                | linux::ECHOCTL
                | linux::ECHOKE
                | linux::IEXTEN,
            INIT_C_CC,
            38400,
        ),
        Some(words) if words.iter().all(|word| *word == 0) => {
            let mut cc = INIT_C_CC;
            cc[linux::VMIN as usize] = 1;
            cc[linux::VTIME as usize] = 0;
            (0, 0, cflag, 0, cc, 38400)
        }
        Some(words) => {
            let mut cc = [0; 19];
            for (index, byte) in cc.iter_mut().enumerate() {
                *byte = (words[3 + index / 8] >> (index % 8 * 8)) as u8;
            }
            (
                words[0] as u32 & 0o37777,
                (words[0] >> 32) as u32 & 0o377777,
                words[1] as u32 & 0o10037777777,
                (words[1] >> 32) as u32 & 0o377777,
                cc,
                words[2] as u32 % 5_000_000,
            )
        }
    };
    let mut bytes = Vec::new();
    for word in [iflag, oflag, cflag, lflag] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.push(0);
    bytes.extend_from_slice(&cc);
    bytes.extend_from_slice(&speed.to_le_bytes());
    bytes.extend_from_slice(&speed.to_le_bytes());
    bytes
}

/// Filter answers: every action, an ERRNO past Linux's largest, data on actions that take none, and
/// values that are no action.
const SECCOMP_ACTIONS: [u32; 12] = [
    seccomp::SECCOMP_RET_ALLOW,
    seccomp::SECCOMP_RET_ERRNO | 13,
    seccomp::SECCOMP_RET_ERRNO | 5000,
    seccomp::SECCOMP_RET_LOG,
    seccomp::SECCOMP_RET_TRACE | 1,
    seccomp::SECCOMP_RET_USER_NOTIF,
    seccomp::SECCOMP_RET_TRAP,
    seccomp::SECCOMP_RET_KILL_PROCESS,
    seccomp::SECCOMP_RET_KILL_THREAD | 7,
    seccomp::SECCOMP_RET_ERRNO,
    0x0001_0000,
    0x7fff_0001,
];

/// A seccomp filter as bytes: it loads the call number and lets `passed` through, then runs up to
/// eight instructions chosen by `words` (some of them ones Linux refuses, so that its checks are
/// tested), then lets everything else through. It never loads the instruction pointer, which differs
/// between the runners.
fn filter_program(words: &[u64], passed: &[u32]) -> Vec<u8> {
    use seccomp::*;
    let constants = |word: u64| -> u32 {
        pick(
            &[
                0,
                1,
                2,
                3,
                31,
                32,
                0xfff,
                u32::MAX,
                0x8000_0000,
                AUDIT_ARCH_AARCH64,
                linux::__NR_openat,
                linux::__NR_close,
                linux::__NR_getpid,
                ARENA as u32,
                (word >> 32) as u32,
            ],
            word,
        )
    };
    let jumps = |word: u64| pick(&[0u8, 0, 1, 2, 3], word);
    let body_length = 1 + (words[0] % 8) as usize;
    let mut body: Vec<(u16, u8, u8, u32)> = Vec::new();
    // Scratch words stored so far, so that most loads from scratch memory are valid.
    let mut stored: Vec<u32> = Vec::new();
    for (index, &word) in words[1..=body_length].iter().enumerate() {
        // One instruction in sixteen is one Linux refuses.
        let broken = (word >> 60) == 0;
        let w = word >> 4;
        let x_or_k = if w & 1 == 0 { BPF_K } else { BPF_X };
        // How far a jump from here may go and still land in the program.
        let reach = (body_length - 1 - index) as u32;
        let mut instruction = match word % 16 {
            0..=2 => (
                BPF_LD | BPF_W | BPF_ABS,
                0,
                0,
                if broken {
                    pick(&[2u32, 64, 0xffff_f000, 62], w >> 1)
                } else {
                    pick(&[0u32, 4, 16, 20, 24, 28, 32, 40, 56, 60], w >> 1)
                },
            ),
            3 => (BPF_LD | BPF_IMM, 0, 0, constants(w)),
            4 => (BPF_LDX | BPF_IMM, 0, 0, constants(w)),
            5 if !stored.is_empty() || broken => (
                pick(&[BPF_LD | BPF_MEM, BPF_LDX | BPF_MEM], w),
                0,
                0,
                if broken {
                    pick(&[16u32, 3], w >> 1)
                } else {
                    pick(&stored, w >> 1)
                },
            ),
            5 | 6 => {
                let slot = if broken {
                    16
                } else {
                    pick(&[0u32, 1, 15], w >> 1)
                };
                stored.push(slot);
                (pick(&[BPF_ST, BPF_STX], w), 0, 0, slot)
            }
            7 => (
                pick(
                    &[BPF_MISC | BPF_TAX, BPF_MISC | BPF_TXA, BPF_ALU | BPF_NEG],
                    w,
                ),
                0,
                0,
                0,
            ),
            8 | 9 => {
                let operation = if broken {
                    pick(&[BPF_MOD, BPF_DIV, BPF_LSH], w >> 1)
                } else {
                    pick(
                        &[
                            BPF_ADD, BPF_SUB, BPF_MUL, BPF_DIV, BPF_OR, BPF_AND, BPF_XOR, BPF_LSH,
                            BPF_RSH,
                        ],
                        w >> 1,
                    )
                };
                let k = if broken && operation != BPF_MOD {
                    pick(&[0u32, 32, 33], w >> 5)
                } else {
                    constants(w >> 5)
                };
                (BPF_ALU | x_or_k | operation, 0, 0, k)
            }
            10 => (BPF_JMP | BPF_JA, 0, 0, pick(&[0u32, 1, 2, 3], w)),
            11 | 12 => (
                BPF_JMP | x_or_k | pick(&[BPF_JEQ, BPF_JGT, BPF_JGE, BPF_JSET], w >> 1),
                jumps(w >> 3),
                jumps(w >> 7),
                constants(w >> 11),
            ),
            13 | 15 if !broken => (BPF_RET | BPF_K, 0, 0, pick(&SECCOMP_ACTIONS, w)),
            14 => (BPF_RET | BPF_A, 0, 0, 0),
            _ => (
                (w & 0xffff) as u32,
                (w >> 16) as u8,
                (w >> 24) as u8,
                (w >> 32) as u32,
            ),
        };
        // Jumps past the end, unless this one is to be refused.
        if instruction.0 & 7 == BPF_JMP {
            if broken {
                instruction.1 = instruction.1.saturating_add(reach as u8 + 1);
                instruction.3 = instruction.3.wrapping_add(reach + 1);
            } else {
                instruction.1 = instruction.1.min(reach as u8);
                instruction.2 = instruction.2.min(reach as u8);
                if instruction.0 == BPF_JMP | BPF_JA {
                    instruction.3 = instruction.3.min(reach);
                }
            }
        }
        let (code, jt, jf, k) = instruction;
        body.push((code as u16, jt, jf, k));
    }
    let total = 1 + passed.len() + body.len() + 1;
    let mut program = vec![((BPF_LD | BPF_W | BPF_ABS) as u16, 0u8, 0u8, 0u32)];
    for (index, number) in passed.iter().enumerate() {
        let to_end = (total - 1) - (1 + index) - 1;
        program.push(((BPF_JMP | BPF_JEQ | BPF_K) as u16, to_end as u8, 0, *number));
    }
    program.extend(body);
    program.push(((BPF_RET | BPF_K) as u16, 0, 0, SECCOMP_RET_ALLOW));
    let mut bytes = Vec::new();
    for (code, jt, jf, k) in program {
        bytes.extend_from_slice(&code.to_le_bytes());
        bytes.push(jt);
        bytes.push(jf);
        bytes.extend_from_slice(&k.to_le_bytes());
    }
    bytes
}

/// Flags for an open, each bit present one time in eight, and always O_NOCTTY: both callers lead
/// their sessions, and a controlling terminal would bring job control signals that end the
/// reference's caller and not ours (the container's first process).
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
        linux::O_DIRECT,
        linux::O_NOATIME,
    ];
    let mut flags = linux::O_NOCTTY;
    for (index, bit) in bits.iter().enumerate() {
        if (word >> (index * 3)) & 7 == 0 {
            flags |= bit;
        }
    }
    u64::from(flags)
}

/// A CPU clock id as Linux's `CPUCLOCK_*` encode it: a process or thread number (0 for the caller's
/// own), and the clock's low three bits (which time, and whether a thread's).
const fn cpu_clock(pid: i64, bits: i64) -> u64 {
    ((!pid << 3) | bits) as u64
}

/// Clock ids: every fixed one Linux has and some it has not, and CPU clocks of the caller's process
/// and thread for each time, a clock device on descriptor 0, and a process that does not exist.
/// Process 1 is left out: it is our caller, but not the reference's.
const CLOCKS: &[u64] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    99,
    cpu_clock(0, 0),
    cpu_clock(0, 1),
    cpu_clock(0, 2),
    cpu_clock(0, 3),
    cpu_clock(0, 4),
    cpu_clock(0, 5),
    cpu_clock(0, 6),
    cpu_clock(0, 7),
    cpu_clock(99999, 2),
    cpu_clock(99999, 6),
];

/// The process CPU clocks slept on: a relative sleep on one never ends (it does not move while the
/// caller sleeps), so they are only slept on until a time already past. The profiling and user time
/// clocks are left out of sleeps: Linux samples them at ticks, so a young process's may still be 0.
const PROCESS_CPU_CLOCKS: &[u64] = &[2, cpu_clock(0, 2)];

const SLEEP_CLOCKS: &[u64] = &[
    0,
    1,
    2,
    3,
    7,
    8,
    9,
    99,
    cpu_clock(0, 2),
    cpu_clock(0, 3),
    cpu_clock(0, 6),
    cpu_clock(0, 7),
    cpu_clock(99999, 2),
];

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
    linux::__NR_geteuid,
    linux::__NR_getgid,
    linux::__NR_getegid,
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
    linux::__NR_pipe2,
    linux::__NR_wait4,
    linux::__NR_waitid,
    linux::__NR_clone,
    linux::__NR_rt_sigqueueinfo,
    linux::__NR_rt_tgsigqueueinfo,
    linux::__NR_rt_sigtimedwait,
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
    linux::__NR_execveat,
    linux::__NR_pkey_mprotect,
    linux::__NR_pkey_alloc,
    linux::__NR_pkey_free,
    linux::__NR_map_shadow_stack,
    linux::__NR_rseq,
    linux::__NR_membarrier,
    linux::__NR_acct,
    linux::__NR_vhangup,
    linux::__NR_swapoff,
    linux::__NR_init_module,
    linux::__NR_finit_module,
    linux::__NR_reboot,
    linux::__NR_syslog,
    linux::__NR_fsopen,
    linux::__NR_fspick,
    linux::__NR_fsmount,
    linux::__NR_move_mount,
    linux::__NR_process_vm_readv,
    linux::__NR_process_vm_writev,
    linux::__NR_cachestat,
    linux::__NR_name_to_handle_at,
    linux::__NR_memfd_secret,
    // Three times, since most of its sequences need several filters to show anything.
    linux::__NR_seccomp,
    linux::__NR_seccomp,
    linux::__NR_seccomp,
    // Three more times, since the play area is mapped only by it and most of its calls fail.
    linux::__NR_mmap,
    linux::__NR_mmap,
    linux::__NR_mmap,
    // Three more times, since most calls need an open file.
    linux::__NR_openat,
    linux::__NR_openat,
    linux::__NR_openat,
    // Two more times, so that waits often find children.
    linux::__NR_clone,
    linux::__NR_clone,
    linux::__NR_splice,
    linux::__NR_splice,
    linux::__NR_tee,
    linux::__NR_vmsplice,
    linux::__NR_shmget,
    linux::__NR_shmget,
    linux::__NR_shmat,
    linux::__NR_shmat,
    linux::__NR_shmdt,
    linux::__NR_shmctl,
    linux::__NR_shmctl,
    linux::__NR_pidfd_open,
    linux::__NR_pidfd_open,
    linux::__NR_pidfd_send_signal,
    linux::__NR_process_mrelease,
    linux::__NR_signalfd4,
    linux::__NR_signalfd4,
    linux::__NR_msgget,
    linux::__NR_msgget,
    linux::__NR_msgsnd,
    linux::__NR_msgsnd,
    linux::__NR_msgrcv,
    linux::__NR_msgrcv,
    linux::__NR_msgctl,
    linux::__NR_semget,
    linux::__NR_semget,
    linux::__NR_semop,
    linux::__NR_semop,
    linux::__NR_semtimedop,
    linux::__NR_semctl,
    linux::__NR_semctl,
    linux::__NR_mq_open,
    linux::__NR_mq_open,
    linux::__NR_mq_unlink,
    linux::__NR_mq_timedsend,
    linux::__NR_mq_timedsend,
    linux::__NR_mq_timedreceive,
    linux::__NR_mq_timedreceive,
    linux::__NR_mq_notify,
    linux::__NR_mq_getsetattr,
    linux::__NR_futex,
    linux::__NR_futex,
    linux::__NR_futex_waitv,
    linux::__NR_futex_wake,
    linux::__NR_futex_wait,
    linux::__NR_futex_requeue,
    linux::__NR_getitimer,
    linux::__NR_setitimer,
    linux::__NR_timer_create,
    linux::__NR_timer_create,
    linux::__NR_timer_settime,
    linux::__NR_timer_gettime,
    linux::__NR_timer_getoverrun,
    linux::__NR_timer_delete,
    linux::__NR_timerfd_create,
    linux::__NR_timerfd_create,
    linux::__NR_timerfd_settime,
    linux::__NR_timerfd_gettime,
    linux::__NR_socket,
    linux::__NR_socket,
    linux::__NR_socketpair,
    linux::__NR_bind,
    linux::__NR_listen,
    linux::__NR_connect,
    linux::__NR_sendto,
    linux::__NR_sendto,
    linux::__NR_recvfrom,
    linux::__NR_recvfrom,
    linux::__NR_shutdown,
    linux::__NR_setsockopt,
    linux::__NR_setsockopt,
    linux::__NR_getsockopt,
    linux::__NR_accept4,
    linux::__NR_sendmsg,
    linux::__NR_sendmsg,
    linux::__NR_recvmsg,
    linux::__NR_recvmsg,
    linux::__NR_eventfd2,
    linux::__NR_epoll_create1,
    linux::__NR_epoll_ctl,
    linux::__NR_epoll_ctl,
    linux::__NR_epoll_pwait,
    linux::__NR_inotify_init1,
    linux::__NR_inotify_add_watch,
    linux::__NR_inotify_add_watch,
    linux::__NR_inotify_rm_watch,
];

/// The calls layer 1 makes on the backend: those of [`CALLS`] on files, memory and the identity
/// file calls are checked against, and none that Docker's seccomp profile lets through only with
/// CAP_SYS_ADMIN, which the reference's container has to mount its FUSE file system.
pub const BACKEND_CALLS: &[u32] = &[
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
    linux::__NR_msync,
    linux::__NR_mremap,
    linux::__NR_setresuid,
    linux::__NR_setresgid,
    linux::__NR_setfsuid,
    linux::__NR_setfsgid,
    linux::__NR_setgroups,
    linux::__NR_statfs,
    linux::__NR_faccessat,
    linux::__NR_fchmodat2,
    linux::__NR_fchown,
    linux::__NR_renameat,
    linux::__NR_preadv,
    linux::__NR_pwritev,
    linux::__NR_fdatasync,
    linux::__NR_syncfs,
    linux::__NR_close_range,
    linux::__NR_preadv2,
    linux::__NR_pwritev2,
    linux::__NR_copy_file_range,
    linux::__NR_fadvise64,
    linux::__NR_readahead,
    linux::__NR_sync_file_range,
    linux::__NR_flock,
    linux::__NR_openat2,
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
    linux::__NR_mincore,
    linux::__NR_pipe2,
    linux::__NR_splice,
    linux::__NR_tee,
    linux::__NR_cachestat,
];

/// System V IPC's flag to return at once rather than wait (`linux/ipc.h`).
mod linux_extra_ipc {
    pub const IPC_NOWAIT: u32 = 0o4000;
}

/// Turns a generated call into real arguments.
fn decode((kind, words): &Raw, path_at: &[u64], calls: &[u32]) -> Call {
    let number = calls[*kind as usize % calls.len()];
    let [a, b, c, d, e, f] = *words;
    let (fifo, path_at) = path_at.split_last().expect("paths");
    let fifo = ARENA + fifo;
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
    // Mostly the generated filters, else the odd ones and bad addresses.
    let fprog = |word: u64| match word % 16 {
        0..=9 => ARENA + FPROGS + (word >> 4) % FILTER_COUNT * 16,
        10..=13 => {
            ARENA + FPROGS + (FILTER_COUNT + (word >> 4) % (FPROG_COUNT - FILTER_COUNT)) * 16
        }
        14 => 0,
        _ => buffer(word >> 4),
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
    // The stat calls take a null path with AT_EMPTY_PATH as an empty one, which with a descriptor
    // ignores the other flags.
    let stat_path = |word: u64| {
        if word.is_multiple_of(8) {
            0
        } else {
            path(word)
        }
    };
    let stat_flags =
        |word: u64| at_flags(word) | u64::from(pick(&[0, 0, 0, linux::AT_EMPTY_PATH], word >> 8));
    let value = |word: u64| ARENA + VALUES + (word % 52) * 8;
    // A new limit for a resource: CPU and file size limits only from the ones never reached.
    let new_limit = |resource: u64, word: u64| {
        // A process limit counts every process of the user on the whole machine, which the reference
        // shares with other runs.
        if resource == u64::from(linux::RLIMIT_CPU)
            || resource == u64::from(linux::RLIMIT_FSIZE)
            || resource == u64::from(linux::RLIMIT_NPROC)
        {
            ARENA + UNREACHED_LIMITS + (word % 4) * 16
        } else if resource == u64::from(linux::RLIMIT_STACK) {
            ARENA + STACK_LIMITS + (word % 4) * 16
        } else if resource == u64::from(linux::RLIMIT_CORE) {
            // No core size limit above zero: core dumps are not written (counted).
            ARENA + NULLS
        } else {
            value(word)
        }
    };
    // Some carry a top byte tag, which arm64 Linux's memory calls strip and the others do not.
    let play = |word: u64| {
        let tag = pick(&[0u64, 0, 0, 0, 0, 0, 0x5a << 56, 0xff << 56], word >> 56);
        (PLAY + ((word % (PLAY_PAGES + 2)) * 4096) + if word >> 60 == 0 { 1 } else { 0 }) | tag
    };
    // Lengths that reach memory stay below the program's own mappings, which lie higher here (in a
    // 4 GiB address space) than in the reference. The last three reach none: rounded up to a page
    // they wrap to 0 or overflow the address.
    let play_length = |word: u64| {
        pick(
            &[
                0u64,
                1,
                4096,
                8192,
                3 * 4096,
                PLAY_PAGES * 4096,
                1 << 28,
                u64::MAX,
                u64::MAX - 4094,
                u64::MAX - 8191,
            ],
            word,
        )
    };
    // Signals whose default action is the same for the container's first process (our caller) and for
    // any other (the reference's caller): nothing, or a refusal of the number.
    let signal = |word: u64| pick(&[0u64, 17, 18, 23, 28, 65], word);
    let prot = |word: u64| (word % 8) | if word >> 62 == 3 { 0x100 } else { 0 };
    let splice_flags = |word: u64| {
        u64::from(linux::SPLICE_F_NONBLOCK) | pick(&[0u64, 0, 1, 4, 8, 0x10, 0x8000_0000], word)
    };
    let segment = |word: u64| {
        pick(
            &[SEGMENT, SEGMENT, SEGMENT, SEGMENT, u64::MAX, NO_SEGMENT],
            word,
        )
    };
    let queue = |word: u64| pick(&[QUEUE, QUEUE, QUEUE, QUEUE, u64::MAX, NO_SEGMENT], word);
    let semaphores = |word: u64| {
        pick(
            &[
                SEMAPHORES,
                SEMAPHORES,
                SEMAPHORES,
                SEMAPHORES,
                u64::MAX,
                NO_SEGMENT,
            ],
            word,
        )
    };
    // A run of semop operations within the first `limit` entries, or none or too many, or a bad
    // address.
    let sembufs = |word: u64, limit: u64| {
        let start = (word >> 8) % limit;
        let count = match word % 8 {
            0 => 0,
            1 => 501,
            2 => (1 << 32) | 1,
            _ => 1 + (word >> 16) % (limit - start),
        };
        let address = match (word >> 4) % 16 {
            0 => 0,
            1 => u64::MAX - 4095,
            _ => ARENA + SEMBUFS + start * 6,
        };
        (address, count)
    };
    let arguments = match number {
        // One open in eight names a random device, so that later reads reach their bytes.
        linux::__NR_openat if b % 8 == 0 => {
            let device = pick(&["/dev/random", "/dev/urandom"], b >> 3);
            [
                dirfd(a),
                ARENA + PATHS + path_offset(device),
                open_flags(c),
                mode(d),
                0,
                0,
            ]
        }
        linux::__NR_openat if e % 4 == 0 => {
            let flags = open_flags(c);
            let flags = if flags & u64::from(linux::O_ACCMODE) == u64::from(linux::O_RDWR) {
                flags
            } else {
                flags | u64::from(linux::O_NONBLOCK)
            };
            [linux::AT_FDCWD as u64, fifo, flags, mode(d), 0, 0]
        }
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
        // The caller's own memory: both sides from the vectors readv and writev use.
        linux::__NR_process_vm_readv | linux::__NR_process_vm_writev => {
            // Segment lists the comparison can read: in the arena, or at a bad address.
            let vectors = |word: u64| match word % 8 {
                0 => 0,
                1 => u64::MAX - 4095,
                2 => ARENA + OUTPUT + (word >> 3) % 0x8000,
                _ => ARENA + IOVECS + ((word >> 3) % 16) * 16,
            };
            let counts = |word: u64| pick(&[0, 1, 2, 3, 16, 1025], word);
            [
                pick(&[SELF, SELF, SELF, 0, 99999], a),
                vectors(b),
                counts(c),
                vectors(d),
                counts(e),
                pick(&[0, 0, 0, 1], f),
            ]
        }
        linux::__NR_memfd_secret => [pick(&[0, 0, 0x80000, 1, 0x80001], a), 0, 0, 0, 0, 0],
        // Process descriptors: of the caller (others' numbers differ between the runners; a child's
        // comes from clone's CLONE_PIDFD), with every flag, and signals sent through them, the
        // caller's own numbers and other files.
        linux::__NR_pidfd_open => [
            pick(&[SELF, SELF, SELF, 0, 99999, u64::MAX], a),
            pick(
                &[
                    0,
                    0,
                    u64::from(linux::O_NONBLOCK),
                    u64::from(linux::O_EXCL),
                    u64::from(linux::O_CLOEXEC),
                    3,
                ],
                b,
            ),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_pidfd_send_signal => [
            match a % 4 {
                0 => linux::PIDFD_SELF_THREAD as u64,
                1 => linux::PIDFD_SELF_THREAD_GROUP as u64,
                _ => fd(a >> 2),
            },
            signal(b),
            match c % 2 {
                0 => 0,
                _ => buffer(c >> 1),
            },
            pick(&[0, 0, 1, 2, 4, 3, 8], d),
            0,
            0,
        ],
        linux::__NR_process_mrelease => [fd(a), pick(&[0, 0, 1], b), 0, 0, 0, 0],
        // Signal descriptors, always nonblocking so that a read never waits, made or changed.
        linux::__NR_signalfd4 => [
            pick(&[u64::MAX, u64::MAX, 3, 4, 100], a),
            buffer(b),
            pick(&[8, 8, 8, 4, 16], c),
            u64::from(linux::O_NONBLOCK)
                | pick(&[0, 0, u64::from(linux::O_CLOEXEC), 1, 0x8000_0000], d),
            0,
            0,
        ],
        linux::__NR_name_to_handle_at => [
            dirfd(a),
            path(b),
            match c % 8 {
                0 => 0,
                1 => u64::MAX - 4095,
                _ => ARENA + HANDLES + (c >> 3) % HANDLE_ROOMS.len() as u64 * 128,
            },
            match d % 8 {
                0 => 0,
                1 => u64::MAX - 4095,
                _ => ARENA + MOUNT_NUMBER,
            },
            pick(
                &[
                    0, 0, 0x400, 0x1000, 0x200, 1, 2, 0x202, 0x1002, 4, 0x401, 0x1201,
                ],
                e,
            ),
            0,
        ],
        // The range from the generated values, or a bad address.
        linux::__NR_cachestat => [
            fd(a),
            if b % 8 == 0 {
                buffer(b >> 3)
            } else {
                value(b >> 3)
            },
            buffer(c),
            pick(&[0, 0, 0, 1], d),
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
                    linux::F_GETOWN,
                    linux::F_SETOWN,
                    linux::F_GETOWN_EX,
                    linux::F_GETSIG,
                    linux::F_SETSIG,
                    linux::F_GETLEASE,
                    linux::F_GETPIPE_SZ,
                    linux::F_SETPIPE_SZ,
                    linux::F_GET_RW_HINT,
                    linux::F_SET_RW_HINT,
                    linux::F_DUPFD_QUERY,
                    linux::F_CREATED_QUERY,
                    // Record locks; not the forms that wait, whose waits here last for ever.
                    linux::F_GETLK,
                    linux::F_SETLK,
                    linux::F_SETLK,
                    linux::F_OFD_GETLK,
                    linux::F_OFD_SETLK,
                    linux::F_OFD_SETLK,
                ],
                b,
            );
            let argument = match command {
                // Owners only of numbers in use in both runners or in neither: process and group 1,
                // none, the smallest int and one past every process number. Setting one from a
                // structure is compared in layer 2 (tests/options.rs): its number comes from memory.
                linux::F_SETOWN => pick(&[0, 1, u64::MAX, 0x8000_0000, 4_000_000], c),
                linux::F_GETOWN_EX | linux::F_GET_RW_HINT | linux::F_SET_RW_HINT => match c % 3 {
                    0 => ARENA + VALUES + (c >> 2) % 56 * 8,
                    _ => buffer(c >> 2),
                },
                linux::F_DUPFD_QUERY => fd(c),
                linux::F_GETLK | linux::F_SETLK | linux::F_OFD_GETLK | linux::F_OFD_SETLK => {
                    match c % 8 {
                        0 => buffer(c >> 3),
                        _ => ARENA + FLOCKS + (c >> 3) % FLOCK_COUNT * 32,
                    }
                }
                _ => pick(
                    &[
                        0,
                        1,
                        5,
                        100,
                        0x800,
                        0x400 | 0x800,
                        u64::MAX,
                        8,
                        0x20,
                        0x16,
                        64,
                        65,
                    ],
                    c,
                ),
            };
            [fd(a), command.into(), argument, 0, 0, 0]
        }
        linux::__NR_ioctl => {
            let request = pick(IOCTL_REQUESTS, b);
            let argument = if request == u64::from(ioctl::FICLONE) {
                fd(c)
            } else if request == u64::from(ioctl::FICLONERANGE) {
                // The range names its source descriptor, which must not be one of 0 to 2: the
                // reference's runner reports through them.
                pick(&[ARENA + IOCTL_ARGS + 6 * 64, 0, u64::MAX - 4095], c)
            } else if request == u64::from(ioctl::TIOCGPTPEER) {
                // Never without O_NOCTTY (see `open_flag_bits`) and O_NONBLOCK (see `build`).
                let flags = linux::O_NOCTTY | linux::O_NONBLOCK;
                u64::from(pick(
                    &[
                        flags | linux::O_RDWR,
                        flags | linux::O_RDONLY | linux::O_CLOEXEC,
                        flags | linux::O_WRONLY,
                        flags | linux::O_RDWR | linux::O_PATH,
                        flags | 0x1000_0000,
                    ],
                    c,
                ))
            } else if request == u64::from(ioctl::TIOCSETD) {
                // Never memory other calls write, which could name any discipline.
                match c % 8 {
                    0 => 0,
                    1 => u64::MAX - 4095,
                    _ => ARENA + DISCIPLINES + (c >> 3) % DISCIPLINE_VALUES.len() as u64 * 4,
                }
            } else if BY_VALUE_REQUESTS.contains(&request) {
                pick(&[0, 1, 2, 3, 4, 17, 28, 65, u64::MAX], c)
            } else if SETTINGS_REQUESTS.contains(&request) && c % 4 < 3 {
                ARENA + TERMIOS + (c >> 2) % TERMIOS_COUNT * 48
            } else {
                match c % 4 {
                    0 | 1 => ARENA + IOCTL_ARGS + (c >> 2) % IOCTL_ARG_COUNT * 64,
                    2 => ARENA + OUTPUT + (c >> 2) % 64 * 8,
                    _ => buffer(c >> 2),
                }
            };
            [fd(a), request, argument, 0, 0, 0]
        }
        linux::__NR_mkdirat => [dirfd(a), path(b), mode(c), 0, 0, 0],
        linux::__NR_mknodat => {
            let kind = pick(
                &[
                    0u64, 0o100000, 0o020000, 0o040000, 0o140000, 0o170000, 0o010000,
                ],
                c,
            );
            [
                dirfd(a),
                if kind == 0o010000 { fifo } else { path(b) },
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
        linux::__NR_newfstatat => [dirfd(a), stat_path(b), buffer(c), stat_flags(d), 0, 0],
        linux::__NR_fstat => [fd(a), buffer(b), 0, 0, 0, 0],
        linux::__NR_statx => [
            dirfd(a),
            stat_path(b),
            stat_flags(c),
            pick(
                &[
                    0x7ff,
                    0xfff,
                    0x8000_0000,
                    0,
                    0x4000,
                    0x47ff,
                    0x1000,
                    0x800,
                    0x40,
                    0x5fff,
                ],
                d,
            ),
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
        // Never waiting: vmsplice takes no notice of a descriptor's O_NONBLOCK, and a named pipe
        // opened for reading and writing is its own writer.
        linux::__NR_splice => [
            fd(a),
            pick(&[0, 0, value(b), buffer(b >> 3)], b),
            fd(c),
            pick(&[0, 0, value(d), buffer(d >> 3)], d),
            count(e),
            splice_flags(f),
        ],
        linux::__NR_tee => [fd(a), fd(b), count(c), splice_flags(d), 0, 0],
        linux::__NR_vmsplice => [
            fd(a),
            ARENA + IOVECS + (b % 16) * 16,
            pick(&[0, 1, 2, 3, 16, 1025], c),
            splice_flags(d),
            0,
            0,
        ],
        linux::__NR_sendfile => [
            fd(a),
            fd(b),
            if c % 3 == 0 { value(c) } else { 0 },
            count(d),
            0,
            0,
        ],
        linux::__NR_mmap => {
            // The kind (private, shared, validated, droppable, none, every bit), with a file or not,
            // where it goes, and each other flag one time in eight. Not huge pages of anonymous memory
            // (hugetlbfs is not built); populating and locking only anonymous memory (which pages a
            // file mapping's faults bring in is not known here).
            let kind = pick(&[0x02u64, 0x02, 0x01, 0x01, 0x03, 0x08, 0x00, 0x0f], c);
            let anonymous = u64::from(linux::MAP_ANONYMOUS) * ((c >> 3) % 2);
            let place = pick(&[0x10u64, 0x10, 0x10, 0x10_0000, 0], c >> 4);
            let mut flags = kind | anonymous | place;
            let extras: &[u32] = &[
                linux::MAP_GROWSDOWN,
                linux::MAP_NORESERVE,
                linux::MAP_NONBLOCK,
                linux::MAP_STACK,
                linux::MAP_HUGE_2MB,
                linux::MAP_SYNC,
                linux::MAP_UNINITIALIZED,
                0x20_0000,
                if anonymous != 0 {
                    linux::MAP_LOCKED
                } else {
                    linux::MAP_HUGETLB
                },
                if anonymous != 0 {
                    linux::MAP_POPULATE
                } else {
                    0
                },
            ];
            for (index, extra) in extras.iter().enumerate() {
                if (c >> (8 + 3 * index)).is_multiple_of(8) {
                    flags |= u64::from(*extra);
                }
            }
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
        // Keys from a small pool, sizes the address space holds or that are refused, and the mode in
        // the low bits.
        linux::__NR_shmget => [
            pick(&[0u64, 0x1234, 0x1234, 0x5678, 0xffff_ffff], a),
            pick(
                &[0u64, 1, 100, 4096, 5000, 0x9000, 1 << 20, u64::MAX, 1 << 63],
                b,
            ),
            pick(
                &[0u64, 0o1000, 0o1000, 0o3000, 0o4000, 0o10000, 0o11000],
                c >> 9,
            ) | (c % 0o1000),
            0,
            0,
            0,
        ],
        linux::__NR_shmat => [
            segment(a),
            pick(&[0, 0, 100, play(b)], b >> 2),
            // SHM_RDONLY, SHM_RND, SHM_REMAP and SHM_EXEC in every mix, and an unknown bit.
            ((c % 16) << 12) | if c >> 60 == 0 { 1 << 20 } else { 0 },
            0,
            0,
            0,
        ],
        linux::__NR_shmdt => [pick(&[0, play(a)], a >> 2), 0, 0, 0, 0, 0],
        linux::__NR_shmctl => [
            segment(a),
            pick(
                &[0u64, 1, 2, 3, 11, 12, 13, 14, 15, 2, 2, 99, u64::MAX, 0x102],
                b,
            ),
            buffer(c),
            0,
            0,
            0,
        ],
        // Message queues: keys and modes as shmget's, messages from the arena (their type is its
        // first word), every search and flag, and never a wait (IPC_NOWAIT always).
        linux::__NR_msgget => [
            pick(&[0u64, 0x1234, 0x1234, 0x5678, 0xffff_ffff], a),
            pick(&[0u64, 0o1000, 0o1000, 0o3000, 0o4000], b >> 9) | (b % 0o1000),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_msgsnd => [
            queue(a),
            buffer(b),
            pick(&[0, 1, 5, 100, 8192, 8193, u64::MAX], c),
            u64::from(linux_extra_ipc::IPC_NOWAIT) | pick(&[0, 0, 0o10000, 1 << 20], d),
            0,
            0,
        ],
        linux::__NR_msgrcv => [
            queue(a),
            buffer(b),
            pick(&[0, 1, 4, 100, 8192, u64::MAX], c),
            pick(
                &[0, 0, 1, 2, u64::MAX, -5i64 as u64, i64::MIN as u64, 1 << 40],
                d,
            ),
            u64::from(linux_extra_ipc::IPC_NOWAIT)
                | pick(
                    &[0, 0, 0o10000, 0o20000, 0o40000, 0o60000, 0o50000, 1 << 20],
                    e,
                ),
            0,
        ],
        linux::__NR_msgctl => [
            queue(a),
            pick(&[0u64, 1, 2, 3, 11, 12, 13, 2, 2, 99, u64::MAX], b),
            buffer(c),
            0,
            0,
            0,
        ],
        // Semaphores: keys and modes as shmget's, sets of a few semaphores, operations that never
        // wait unless a timeout ends them, and every semctl command.
        linux::__NR_semget => [
            pick(&[0u64, 0x1234, 0x1234, 0x5678, 0xffff_ffff], a),
            pick(&[0u64, 1, 2, 3, 3, 8, 32000, 32001, u64::MAX], b),
            pick(&[0u64, 0o1000, 0o1000, 0o3000, 0o4000], c >> 9) | (c % 0o1000),
            0,
            0,
            0,
        ],
        linux::__NR_semop => {
            let (address, count) = sembufs(b, SEMBUFS_NOWAIT);
            [semaphores(a), address, count, 0, 0, 0]
        }
        linux::__NR_semtimedop => {
            let timed = d % 4 != 0;
            let (address, count) = sembufs(b, if timed { SEMBUF_COUNT } else { SEMBUFS_NOWAIT });
            let timeout = if timed {
                ARENA + VALUES + 56 * 8 + (d >> 2) % 4 * 16
            } else {
                0
            };
            [semaphores(a), address, count, timeout, 0, 0]
        }
        linux::__NR_semctl => {
            let command = pick(
                &[
                    0u64,
                    1,
                    2,
                    3,
                    11,
                    12,
                    13,
                    14,
                    15,
                    16,
                    16,
                    17,
                    18,
                    19,
                    20,
                    12,
                    99,
                    u64::MAX,
                    0x102,
                ],
                c,
            );
            let argument = if command == 16 {
                pick(&[0u64, 1, 2, 5, 32767, 32768, u64::MAX, (7 << 32) | 3], d)
            } else {
                buffer(d)
            };
            [
                semaphores(a),
                pick(&[0u64, 0, 1, 2, 7, u64::MAX], b),
                command,
                argument,
                0,
                0,
            ]
        }
        // Message queues: names from the path pool, opens never waiting, sends and receives always
        // with a time that has passed or is refused, so nothing waits.
        linux::__NR_mq_open => [
            path(a),
            u64::from(linux::O_NONBLOCK)
                | (b % 4)
                | pick(&[0, 0o100, 0o100, 0o300, 0o40000, 0o1000, 1 << 25], b >> 2),
            c % 0o1000,
            pick(&[0, 0, ARENA + MQ_ATTRS, u64::MAX - 4095], d)
                + (d >> 3) % 4 * 64 * u64::from(d % 4 == 2),
            0,
            0,
        ],
        linux::__NR_mq_unlink => [path(a), 0, 0, 0, 0, 0],
        linux::__NR_mq_timedsend => [
            fd(a),
            buffer(b),
            pick(&[0, 1, 16, 100, 8192, 8193], c),
            pick(&[0, 1, 5, 5, 32767, 32768, 1 << 32], d),
            ARENA + VALUES + 56 * 8 + (e % 4) * 16,
            0,
        ],
        linux::__NR_mq_timedreceive => [
            fd(a),
            buffer(b),
            pick(&[0, 16, 8192, 9000, u64::MAX], c),
            pick(&[0, buffer(d)], d >> 3),
            ARENA + VALUES + 56 * 8 + (e % 4) * 16,
            0,
        ],
        linux::__NR_mq_notify => [
            fd(a),
            pick(&[0, ARENA + SIGEVENTS, 8], b) + (b >> 3) % 4 * 64 * u64::from(b % 3 == 1),
            0,
            0,
            0,
            0,
        ],
        // Futexes: words of `VALUES`, misaligned and bad ones; every wait has a timeout (of the
        // sleeps), so nothing waits for ever. The priority inheriting operations that can take a
        // lock write the caller's thread number, which differs between the runners, so they get
        // only misaligned and bad addresses; the requeue ones never find a waiter and write nothing.
        linux::__NR_futex => {
            let word = |w: u64| match w % 6 {
                0 => ARENA + VALUES + 2,
                1 => 0,
                2 => buffer(w >> 3),
                _ => ARENA + VALUES + (w >> 3) % 52 * 8,
            };
            let command = pick(
                &[
                    linux::FUTEX_WAIT as u64,
                    linux::FUTEX_WAKE as u64,
                    linux::FUTEX_REQUEUE as u64,
                    linux::FUTEX_CMP_REQUEUE as u64,
                    linux::FUTEX_WAKE_OP as u64,
                    linux::FUTEX_WAIT_BITSET as u64,
                    linux::FUTEX_WAKE_BITSET as u64,
                    linux::FUTEX_FD as u64,
                    14,
                    linux::FUTEX_LOCK_PI as u64,
                    linux::FUTEX_UNLOCK_PI as u64,
                    linux::FUTEX_TRYLOCK_PI as u64,
                    linux::FUTEX_LOCK_PI2 as u64,
                    linux::FUTEX_WAIT_REQUEUE_PI as u64,
                    linux::FUTEX_CMP_REQUEUE_PI as u64,
                ],
                b,
            );
            let locks = [
                linux::FUTEX_LOCK_PI as u64,
                linux::FUTEX_UNLOCK_PI as u64,
                linux::FUTEX_TRYLOCK_PI as u64,
                linux::FUTEX_LOCK_PI2 as u64,
            ]
            .contains(&command);
            // An unlock reads the word before it looks at the address, so it gets no misaligned
            // one, whose bytes could name either runner's thread.
            let first = if command == linux::FUTEX_UNLOCK_PI as u64 {
                pick(&[0, u64::MAX - 4095], a)
            } else if locks {
                pick(&[ARENA + VALUES + 2, 0, u64::MAX - 4095], a)
            } else {
                word(a)
            };
            let flags = pick(
                &[
                    0,
                    linux::FUTEX_PRIVATE_FLAG as u64,
                    linux::FUTEX_CLOCK_REALTIME as u64,
                ],
                b >> 8,
            );
            let waits = [
                linux::FUTEX_WAIT as u64,
                linux::FUTEX_WAIT_BITSET as u64,
                linux::FUTEX_LOCK_PI as u64,
                linux::FUTEX_LOCK_PI2 as u64,
                linux::FUTEX_WAIT_REQUEUE_PI as u64,
            ]
            .contains(&command);
            let fourth = if waits {
                ARENA + VALUES + 56 * 8 + (d % 4) * 16
            } else {
                pick(&[0, 1, 2, u64::MAX], d)
            };
            let operation = |w: u64| {
                let op = pick(&[0, 1, 2, 3, 4, 7, 8, 9], w);
                let compare = pick(&[0, 1, 2, 3, 4, 5, 6], w >> 3);
                (op << 28) | (compare << 24) | (((w >> 6) & 0xfff) << 12) | ((w >> 18) & 0xfff)
            };
            [
                first,
                command | flags,
                pick(&[0, 1, u64::from(u32::MAX), (c >> 8) % 3], c),
                fourth,
                word(e),
                if command == linux::FUTEX_WAKE_OP as u64 {
                    operation(f)
                } else {
                    pick(&[0, 1, u64::from(u32::MAX), 0x55], f)
                },
            ]
        }
        linux::__NR_futex_waitv => [
            pick(
                &[ARENA + FUTEX_VECTORS, ARENA + FUTEX_VECTORS + 24, 0, 8],
                a,
            ) + (a >> 2) % 3 * 24 * u64::from(a % 4 == 0),
            pick(&[1, 2, 0, 129], b),
            pick(&[0, 0, 1], c),
            ARENA + VALUES + 56 * 8 + (d % 4) * 16,
            pick(
                &[
                    linux::CLOCK_MONOTONIC as u64,
                    linux::CLOCK_REALTIME as u64,
                    linux::CLOCK_BOOTTIME as u64,
                ],
                e,
            ),
            0,
        ],
        linux::__NR_futex_wake => [
            pick(
                &[ARENA + VALUES, ARENA + VALUES + 2, 0, ARENA + VALUES + 8],
                a,
            ),
            pick(&[u64::from(u32::MAX), 0, 1, 1 << 32], b),
            pick(&[0, 1, 5, u64::MAX], c),
            pick(
                &[
                    u64::from(linux::FUTEX2_SIZE_U32 | linux::FUTEX2_PRIVATE),
                    u64::from(linux::FUTEX2_SIZE_U32),
                    u64::from(linux::FUTEX2_SIZE_U8),
                    0x100,
                ],
                d,
            ),
            0,
            0,
        ],
        linux::__NR_futex_wait => [
            pick(
                &[ARENA + VALUES, ARENA + VALUES + 2, 0, ARENA + VALUES + 8],
                a,
            ),
            pick(&[0, 1, 1 << 32], b),
            pick(&[u64::from(u32::MAX), 0, 1], c),
            pick(
                &[
                    u64::from(linux::FUTEX2_SIZE_U32 | linux::FUTEX2_PRIVATE),
                    u64::from(linux::FUTEX2_SIZE_U32),
                    u64::from(linux::FUTEX2_SIZE_U64),
                ],
                d,
            ),
            ARENA + VALUES + 56 * 8 + (e % 4) * 16,
            pick(
                &[
                    linux::CLOCK_MONOTONIC as u64,
                    linux::CLOCK_REALTIME as u64,
                    7,
                ],
                f,
            ),
        ],
        linux::__NR_futex_requeue => [
            pick(&[ARENA + FUTEX_VECTORS, ARENA + FUTEX_VECTORS + 24, 0], a),
            pick(&[0, 0, 1], b),
            pick(&[0, 1, u64::MAX], c),
            pick(&[0, 1, u64::MAX], d),
            0,
            0,
        ],
        // Timers whose times never come within a sequence (`ITIMERS`), so what they send never
        // races the calls; timer descriptors are always nonblocking.
        linux::__NR_getitimer => [pick(&[0, 1, 2, 3], a), buffer(b), 0, 0, 0, 0],
        linux::__NR_setitimer => [
            pick(&[0, 1, 2, 3], a),
            pick(&[ARENA + ITIMERS, 0, 8], b) + (b >> 2) % 4 * 32 * u64::from(b % 3 == 0),
            pick(&[0, buffer(c)], c >> 3),
            0,
            0,
            0,
        ],
        linux::__NR_timer_create => [
            pick(
                &[
                    linux::CLOCK_MONOTONIC as u64,
                    linux::CLOCK_REALTIME as u64,
                    linux::CLOCK_PROCESS_CPUTIME_ID as u64,
                    linux::CLOCK_THREAD_CPUTIME_ID as u64,
                    linux::CLOCK_BOOTTIME as u64,
                    linux::CLOCK_REALTIME_ALARM as u64,
                    linux::CLOCK_MONOTONIC_RAW as u64,
                    99,
                    (-2i64) as u64,
                    ((!99999i64) << 3 | 2) as u64,
                ],
                a,
            ),
            pick(&[0, ARENA + SIGEVENTS, 8], b) + (b >> 3) % 4 * 64 * u64::from(b % 3 == 1),
            pick(&[buffer(c), 0, 8], c >> 3),
            0,
            0,
            0,
        ],
        linux::__NR_timer_settime => [
            pick(&[0, 1, 2, 99999, u64::MAX], a),
            pick(&[0, linux::TIMER_ABSTIME as u64, 2], b),
            pick(&[ARENA + ITIMERS, 0, 8], c) + (c >> 2) % 4 * 32 * u64::from(c % 3 == 0),
            pick(&[0, buffer(d)], d >> 3),
            0,
            0,
        ],
        linux::__NR_timer_gettime => [pick(&[0, 1, 2, 99999], a), buffer(b), 0, 0, 0, 0],
        linux::__NR_timer_getoverrun | linux::__NR_timer_delete => {
            [pick(&[0, 1, 2, 99999, u64::MAX], a), 0, 0, 0, 0, 0]
        }
        linux::__NR_timerfd_create => [
            pick(
                &[
                    linux::CLOCK_MONOTONIC as u64,
                    linux::CLOCK_REALTIME as u64,
                    linux::CLOCK_BOOTTIME as u64,
                    linux::CLOCK_BOOTTIME_ALARM as u64,
                    linux::CLOCK_PROCESS_CPUTIME_ID as u64,
                ],
                a,
            ),
            u64::from(linux::TFD_NONBLOCK) | pick(&[0, u64::from(linux::TFD_CLOEXEC), 1], b),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_timerfd_settime => [
            fd(a),
            pick(
                &[
                    0,
                    u64::from(linux::TFD_TIMER_ABSTIME),
                    u64::from(linux::TFD_TIMER_ABSTIME | linux::TFD_TIMER_CANCEL_ON_SET),
                    4,
                ],
                b,
            ),
            pick(&[ARENA + ITIMERS, 0, 8], c) + (c >> 2) % 4 * 32 * u64::from(c % 3 == 0),
            pick(&[0, buffer(d)], d >> 3),
            0,
            0,
        ],
        linux::__NR_timerfd_gettime => [fd(a), buffer(b), 0, 0, 0, 0],
        linux::__NR_mq_getsetattr => [
            fd(a),
            pick(&[0, ARENA + MQ_ATTRS, 8], b) + (b >> 3) % 4 * 64 * u64::from(b % 3 == 1),
            pick(&[0, buffer(c)], c >> 3),
            0,
            0,
            0,
        ],
        linux::__NR_mprotect => [play(a), play_length(b), prot(c), 0, 0, 0],
        linux::__NR_pkey_mprotect => [
            play(a),
            play_length(b),
            prot(c),
            pick(&[0, u64::MAX, 1, 15, 16, 1 << 32, (1 << 32) - 1], d),
            0,
            0,
        ],
        linux::__NR_pkey_alloc => [
            pick(&[0, 0, 1, 1 << 32], a),
            pick(&[0, 1, 4, 8, 15, 16, u64::MAX], b),
            0,
            0,
            0,
            0,
        ],
        // Always only a check: a program that ran would replace the reference's runner.
        linux::__NR_execveat => {
            let list = |word: u64| pick(&[ARENA + NULLS, 0, u64::MAX - 4095], word);
            let flags = pick(
                &[
                    0,
                    linux::AT_EMPTY_PATH,
                    linux::AT_SYMLINK_NOFOLLOW,
                    linux::AT_EMPTY_PATH | linux::AT_SYMLINK_NOFOLLOW,
                    2,
                ],
                e,
            );
            [
                dirfd(a),
                path(b),
                list(c),
                list(d),
                u64::from(flags | linux::AT_EXECVE_CHECK),
                0,
            ]
        }
        linux::__NR_pkey_free => [pick(&[0, 0, 1, u64::MAX, 1 << 32], a), 0, 0, 0, 0, 0],
        linux::__NR_map_shadow_stack => [play(a), count(b), value(c), 0, 0, 0],
        // No time slice extension flag (not built).
        linux::__NR_rseq => [
            pick(
                &[
                    ARENA + RSEQS,
                    ARENA + RSEQS,
                    ARENA + RSEQS + RSEQ_AREAS[1],
                    ARENA + RSEQS + RSEQ_AREAS[2],
                    ARENA + RSEQS + 0x10,
                    0,
                    0x1000,
                    u64::MAX - 4095,
                ],
                a,
            ),
            pick(&[32, 32, 33, 64, 28, 31, 4096, 1 << 32 | 32], b),
            pick(&[0, 0, 0, 1, 1, 3, 4, 1 << 32], c),
            pick(&[0, 1, 0x5305_3053], d),
            0,
            0,
        ],
        linux::__NR_membarrier => [
            pick(
                &[
                    0,
                    1,
                    2,
                    4,
                    8,
                    16,
                    32,
                    64,
                    128,
                    256,
                    512,
                    3,
                    1 << 32,
                    u64::MAX,
                    1 << 20,
                ],
                a,
            ),
            pick(&[0, 0, 1, 2], b),
            pick(&[0, 5, u64::MAX], c),
            0,
            0,
            0,
        ],
        // Each refused for a power a container lacks, whatever the arguments.
        linux::__NR_acct
        | linux::__NR_vhangup
        | linux::__NR_swapoff
        | linux::__NR_init_module
        | linux::__NR_finit_module
        | linux::__NR_reboot
        | linux::__NR_syslog
        | linux::__NR_fsopen
        | linux::__NR_fspick
        | linux::__NR_fsmount
        | linux::__NR_move_mount => [path(a), value(b), value(c), 0, 0, 0],
        linux::__NR_madvise => [
            play(a),
            play_length(b),
            pick(
                &[
                    0, 1, 2, 3, 4, 8, 9, 100, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24, 26,
                ],
                c,
            ),
            0,
            0,
            0,
        ],
        linux::__NR_uname => [
            match a % 8 {
                0 => 0,
                1 => u64::MAX - 4095,
                _ => ARENA + UNAME,
            },
            0,
            0,
            0,
            0,
            0,
        ],
        linux::__NR_getuid | linux::__NR_geteuid | linux::__NR_getgid | linux::__NR_getegid => {
            [0; 6]
        }
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
        linux::__NR_clock_gettime | linux::__NR_clock_getres => {
            [pick(CLOCKS, a), buffer(b), 0, 0, 0, 0]
        }
        linux::__NR_getrandom => [
            match a % 8 {
                0 => 0,
                1 => u64::MAX - 4095,
                _ => ARENA + RANDOM,
            },
            pick(&[0, 1, 16, 96], b),
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
        linux::__NR_prctl => {
            let option = pick(
                &[
                    15u64, 16, 3, 4, 7, 8, 24, 38, 39, 9999, 21, 22, 22, 1, 2, 29, 30, 36, 37, 41,
                    42, 66, 27, 35, 57, 58, 47, 47,
                ],
                a,
            );
            // Values for the options that take numbers: signals, slacks, flags and `PR_SET_MM`'s forms.
            let number = |word: u64| pick(&[0, 1, 2, 9, 14, 15, 64, 65, 1000, u64::MAX], word);
            match option {
                1 | 36 => [option, number(b), 0, 0, 0, 0],
                // PR_SET_TIMERSLACK: slacks a sequence's sleeps end within its deadline whatever they
                // are. Where in a long slack Linux's timer ends depends on other timers' interrupts
                // (the machine's load), so long ones are compared in no test.
                29 => [
                    option,
                    pick(&[0, 1, 1000, 50_000, 1_000_000], b),
                    0,
                    0,
                    0,
                    0,
                ],
                30 | 66 | 27 | 58 => [option, 0, 0, 0, 0, 0],
                41 | 42 | 57 => [option, number(b), number(c), 0, 0, 0],
                35 => [option, number(b), buffer(c), number(d), number(e), 0],
                // PR_SET_SECCOMP: a mode and a filter.
                22 => [option, pick(&[0, 1, 2, 2, 3], b), fprog(c), 0, 0, 0],
                // PR_SET_NO_NEW_PRIVS: only 1 is taken.
                38 if b % 2 == 0 => [option, pick(&[1, 0, 2], b >> 1), 0, 0, 0, 0],
                // PR_CAP_AMBIENT: each operation and an unknown one, on powers and numbers past them.
                47 => [
                    option,
                    pick(&[1, 2, 3, 4, 0, 5], b),
                    number(c),
                    pick(&[0, 0, 0, 1], d),
                    0,
                    0,
                ],
                _ => [option, buffer(b), 0, 0, 0, 0],
            }
        }
        linux::__NR_seccomp => {
            use seccomp::*;
            let operation = pick(
                &[
                    u64::from(SECCOMP_SET_MODE_STRICT),
                    u64::from(SECCOMP_SET_MODE_FILTER),
                    u64::from(SECCOMP_SET_MODE_FILTER),
                    u64::from(SECCOMP_SET_MODE_FILTER),
                    u64::from(SECCOMP_GET_ACTION_AVAIL),
                    u64::from(SECCOMP_GET_ACTION_AVAIL),
                    u64::from(SECCOMP_GET_NOTIF_SIZES),
                    4,
                    1 << 32 | u64::from(SECCOMP_SET_MODE_FILTER),
                ],
                a,
            );
            // No listener (not built).
            let flags = pick(
                &[
                    0u64,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    u64::from(SECCOMP_FILTER_FLAG_TSYNC),
                    u64::from(SECCOMP_FILTER_FLAG_LOG),
                    u64::from(SECCOMP_FILTER_FLAG_SPEC_ALLOW),
                    u64::from(SECCOMP_FILTER_FLAG_TSYNC_ESRCH),
                    u64::from(SECCOMP_FILTER_FLAG_TSYNC | SECCOMP_FILTER_FLAG_TSYNC_ESRCH),
                    u64::from(SECCOMP_FILTER_FLAG_WAIT_KILLABLE_RECV),
                    u64::from(SECCOMP_FILTER_FLAG_TSYNC | SECCOMP_FILTER_FLAG_NEW_LISTENER),
                    64,
                    1 << 32,
                ],
                b,
            );
            let argument = match operation as u32 {
                SECCOMP_SET_MODE_FILTER => fprog(c),
                SECCOMP_GET_ACTION_AVAIL if c % 4 != 0 => {
                    ARENA + ACTIONS + (c >> 2) % SECCOMP_ACTIONS.len() as u64 * 4
                }
                _ => pick(&[0, buffer(c >> 2)], c),
            };
            [operation, flags, argument, 0, 0, 0]
        }
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
        linux::__NR_clock_nanosleep => [
            pick(SLEEP_CLOCKS, a),
            if PROCESS_CPU_CLOCKS.contains(&pick(SLEEP_CLOCKS, a)) {
                1
            } else {
                pick(&[0, 1, 2], b)
            },
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
        // Always nonblocking: a read of an empty pipe or a write to a full one would wait for ever.
        // O_NOTIFICATION_PIPE is not generated (watch queues are not built, counted).
        linux::__NR_pipe2 => [
            buffer(a),
            u64::from(linux::O_NONBLOCK)
                | pick(
                    &[
                        0,
                        u64::from(linux::O_CLOEXEC),
                        u64::from(linux::O_DIRECT),
                        u64::from(linux::O_APPEND),
                    ],
                    b,
                ),
            0,
            0,
            0,
            0,
        ],
        // Fork and vfork, whose child ends at once with status 7 (see `CHILD_STATUS`), with exit
        // signals both callers ignore, and with a process descriptor for the parent; refusals of
        // flags that ask for threads without what they need, and of a namespace (Docker's profile).
        // Not the sharing of tables or a parent, which are not built.
        linux::__NR_clone => [
            u64::from(pick(
                &[
                    linux::SIGCHLD,
                    linux::SIGCHLD,
                    0,
                    linux::SIGWINCH,
                    linux::CLONE_VFORK | linux::CLONE_VM | linux::SIGCHLD,
                    linux::SIGCHLD | linux::CLONE_CHILD_SETTID | linux::CLONE_CHILD_CLEARTID,
                    linux::SIGCHLD | linux::CLONE_SETTLS | linux::CLONE_UNTRACED,
                    linux::CLONE_SIGHAND | linux::SIGCHLD,
                    linux::CLONE_THREAD | linux::SIGCHLD,
                    linux::CLONE_NEWNS | linux::SIGCHLD,
                    u32::MAX,
                    linux::CLONE_PIDFD | linux::SIGCHLD,
                    linux::CLONE_PIDFD | linux::SIGCHLD,
                    linux::CLONE_PIDFD | linux::CLONE_VFORK | linux::CLONE_VM | linux::SIGCHLD,
                    linux::CLONE_PIDFD | linux::CLONE_PARENT_SETTID | linux::SIGCHLD,
                    linux::CLONE_PIDFD | linux::CLONE_DETACHED | linux::SIGCHLD,
                ],
                a,
            )),
            0,
            buffer(c),
            0,
            buffer(b),
            0,
        ],
        // Waits never wait when a sequence has children (see `build`); without, there is nothing
        // to wait for.
        linux::__NR_wait4 => [
            pick(&[u64::MAX, 0, SELF, 99999, i32::MIN as u32 as u64], a),
            buffer(b),
            pick(&[0, 1, 2, 8, 1 << 30, 1 << 31, 0x100], c),
            buffer(d),
            0,
            0,
        ],
        linux::__NR_waitid => [
            pick(&[0, 1, 2, 3, 9], a),
            pick(&[0, SELF, 99999, 3, u64::MAX], b),
            buffer(c),
            pick(&[4, 5, 2, 8, 0, 0x0100_0004, 1 << 30], d),
            buffer(e),
            0,
        ],
        linux::__NR_rt_sigqueueinfo => [
            pick(&[SELF, SELF, 0, 99999], a),
            signal(b),
            buffer(c),
            0,
            0,
            0,
        ],
        linux::__NR_rt_tgsigqueueinfo => [
            pick(&[SELF, SELF, 99999], a),
            pick(&[SELF, SELF, 99999], b),
            signal(c),
            buffer(d),
            0,
            0,
        ],
        // Always a timeout, so nothing waits for ever.
        linux::__NR_rt_sigtimedwait => [
            buffer(a),
            if b % 3 == 0 { 0 } else { buffer(b) },
            ARENA + VALUES + 56 * 8 + (c % 4) * 16,
            pick(&[8, 8, 4], d),
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
        // Every socket is nonblocking and every send and receive asks not to wait or signal: a
        // sequence never waits. Kinds and protocols the reference has and this kernel refuses on
        // purpose (raw, ping, SCTP, other netlink protocols) are not made; nor are addresses read
        // back from receives, which hold numbers each runner chooses at random.
        linux::__NR_socket | linux::__NR_socketpair => {
            let family = pick(
                &[
                    net::AF_UNIX,
                    net::AF_UNIX,
                    net::AF_INET,
                    net::AF_INET6,
                    net::AF_NETLINK,
                    99,
                ],
                a,
            );
            let kinds: &[u32] = match family {
                net::AF_UNIX => &[1, 2, 5, 3, 12],
                net::AF_INET | net::AF_INET6 => &[1, 2, 2, 7],
                _ => &[3, 2, 1],
            };
            let kind = pick(kinds, b) | linux::O_NONBLOCK | pick(&[0, linux::O_CLOEXEC], c);
            let protocol = match family {
                net::AF_INET | net::AF_INET6 => pick(&[0u64, 0, 6, 17], d),
                _ => pick(&[0u64, 0, 0, 1], d),
            };
            if number == linux::__NR_socket {
                [u64::from(family), u64::from(kind), protocol, 0, 0, 0]
            } else {
                [
                    u64::from(family),
                    u64::from(kind),
                    protocol,
                    buffer(e),
                    0,
                    0,
                ]
            }
        }
        linux::__NR_bind | linux::__NR_connect => {
            // Binds take the any address with port 0 or the abstract name, never the port.
            let choices: &[usize] = if number == linux::__NR_bind {
                &[0, 4, 4, 3]
            } else {
                &[0, 1, 2, 3, 4]
            };
            let (at, length) = SOCKET_ADDRESS_AT[pick(choices, b)];
            let length = pick(&[length, length, 2, 1, 200], c);
            [fd(a), ARENA + SOCKET_ADDRESSES + at, length, 0, 0, 0]
        }
        linux::__NR_listen => [fd(a), pick(&[0u64, 1, 5, u64::MAX, 100000], b), 0, 0, 0, 0],
        linux::__NR_sendto => {
            let flags = u64::from(net::MSG_DONTWAIT | net::MSG_NOSIGNAL)
                | pick(
                    &[0u64, 0, u64::from(net::MSG_OOB), u64::from(net::MSG_MORE)],
                    d,
                );
            let (address, length) = if e % 4 == 0 {
                let (at, length) = SOCKET_ADDRESS_AT[(e >> 4) as usize % 5];
                (ARENA + SOCKET_ADDRESSES + at, length)
            } else {
                (0, 0)
            };
            let size = pick(&[0u64, 1, 5, 100, 4096, 70000], c);
            [fd(a), buffer(b), size, flags, address, length]
        }
        linux::__NR_recvfrom => {
            let flags = u64::from(net::MSG_DONTWAIT)
                | pick(
                    &[
                        0u64,
                        u64::from(net::MSG_PEEK),
                        u64::from(net::MSG_TRUNC),
                        u64::from(net::MSG_WAITALL),
                        u64::from(net::MSG_OOB),
                    ],
                    d,
                );
            [
                fd(a),
                buffer(b),
                pick(&[0u64, 1, 3, 64, 4096], c),
                flags,
                0,
                0,
            ]
        }
        linux::__NR_shutdown => [fd(a), pick(&[0u64, 1, 2, 3], b), 0, 0, 0, 0],
        // No address back: a connecting socket's automatic name or port is random.
        linux::__NR_accept4 => [
            fd(a),
            0,
            0,
            u64::from(linux::O_NONBLOCK) | pick(&[0u64, u64::from(linux::O_CLOEXEC), 1], b),
            0,
            0,
        ],
        linux::__NR_sendmsg => {
            let flags = u64::from(net::MSG_DONTWAIT | net::MSG_NOSIGNAL)
                | pick(
                    &[
                        0u64,
                        0,
                        u64::from(net::MSG_OOB),
                        u64::from(net::MSG_MORE),
                        u64::from(net::MSG_EOR),
                    ],
                    c,
                );
            [
                fd(a),
                ARENA + MSGHDRS + (b % SEND_HEADERS) * 56,
                flags,
                0,
                0,
                0,
            ]
        }
        linux::__NR_recvmsg => {
            let flags = u64::from(net::MSG_DONTWAIT)
                | pick(
                    &[
                        0u64,
                        0,
                        u64::from(net::MSG_PEEK),
                        u64::from(net::MSG_TRUNC),
                        u64::from(net::MSG_WAITALL),
                        u64::from(net::MSG_OOB),
                        u64::from(net::MSG_CMSG_CLOEXEC),
                        u64::from(net::MSG_ERRQUEUE),
                    ],
                    c,
                );
            let header = SEND_HEADERS + b % (MSGHDR_COUNT - SEND_HEADERS);
            [fd(a), ARENA + MSGHDRS + header * 56, flags, 0, 0, 0]
        }
        linux::__NR_setsockopt | linux::__NR_getsockopt => {
            let (level, name) = pick(
                &[
                    (net::SOL_SOCKET, net::SO_RCVBUF),
                    (net::SOL_SOCKET, net::SO_SNDBUF),
                    (net::SOL_SOCKET, net::SO_REUSEADDR),
                    (net::SOL_SOCKET, net::SO_KEEPALIVE),
                    (net::SOL_SOCKET, net::SO_RCVLOWAT),
                    (net::SOL_SOCKET, net::SO_PASSCRED),
                    (net::SOL_SOCKET, net::SO_TYPE),
                    (net::SOL_SOCKET, net::SO_ERROR),
                    (net::SOL_SOCKET, net::SO_OOBINLINE),
                    (net::SOL_SOCKET, net::SO_PEEK_OFF),
                    (net::SOL_SOCKET, net::SO_PASSPIDFD),
                    (net::SOL_SOCKET, net::SO_PEERPIDFD),
                    (net::SOL_SOCKET, net::SO_PASSRIGHTS),
                    (net::SOL_SOCKET, net::SO_INQ),
                    (net::SOL_SOCKET, net::SO_TIMESTAMP_OLD),
                    (net::SOL_SOCKET, net::SO_TIMESTAMPNS_NEW),
                    (net::SOL_SOCKET, net::SO_LINGER),
                    (net::IPPROTO_TCP as u32, 1),
                    (net::IPPROTO_IP as u32, 2),
                    (net::IPPROTO_IP as u32, net::IP_PKTINFO),
                    (net::IPPROTO_IP as u32, net::IP_RECVTTL),
                    (net::IPPROTO_IP as u32, net::IP_RECVTOS),
                    (net::IPPROTO_IPV6 as u32, 26),
                    (net::IPPROTO_IPV6 as u32, net::IPV6_RECVPKTINFO),
                    (999, 1),
                ],
                b,
            );
            let length = pick(&[4u64, 4, 0, 2, 8, u64::MAX], d);
            if number == linux::__NR_setsockopt {
                [
                    fd(a),
                    u64::from(level),
                    u64::from(name),
                    ARENA + VALUES + (c % 32) * 8,
                    length,
                    0,
                ]
            } else {
                // The length is read from a value the arena holds, and written back there.
                let at = ARENA + VALUES + (c % 32) * 8;
                [fd(a), u64::from(level), u64::from(name), buffer(e), at, 0]
            }
        }
        linux::__NR_eventfd2 => [
            pick(&[0u64, 1, 5, u64::from(u32::MAX)], a),
            u64::from(linux::EFD_NONBLOCK)
                | pick(
                    &[
                        0u64,
                        u64::from(linux::EFD_SEMAPHORE),
                        u64::from(linux::EFD_CLOEXEC),
                        1,
                    ],
                    b,
                ),
            0,
            0,
            0,
            0,
        ],
        linux::__NR_epoll_create1 => [
            pick(&[0u64, u64::from(linux::EPOLL_CLOEXEC), 1], a),
            0,
            0,
            0,
            0,
            0,
        ],
        linux::__NR_epoll_ctl => [
            fd(a),
            pick(&[1u64, 1, 2, 3, 0, 4], b),
            fd(c),
            pick(
                &[
                    ARENA + EPOLL_EVENTS,
                    ARENA + EPOLL_EVENTS + 16,
                    ARENA + EPOLL_EVENTS + 32,
                    ARENA + EPOLL_EVENTS + 48,
                    0,
                ],
                d,
            ),
            0,
            0,
        ],
        linux::__NR_epoll_pwait => [fd(a), buffer(b), pick(&[1u64, 4, 0, u64::MAX], c), 0, 0, 8],
        linux::__NR_inotify_init1 => [
            u64::from(linux::IN_NONBLOCK) | pick(&[0u64, u64::from(linux::IN_CLOEXEC), 1], a),
            0,
            0,
            0,
            0,
            0,
        ],
        linux::__NR_inotify_add_watch => [
            fd(a),
            path(b),
            pick(
                &[
                    u64::from(linux::IN_ALL_EVENTS),
                    u64::from(linux::IN_MODIFY | linux::IN_CLOSE_WRITE),
                    u64::from(linux::IN_CREATE | linux::IN_DELETE),
                    u64::from(linux::IN_ONESHOT | linux::IN_ATTRIB),
                    0,
                    u64::from(linux::IN_MASK_CREATE | linux::IN_OPEN),
                    u64::from(linux::IN_MASK_ADD | linux::IN_ACCESS),
                ],
                c,
            ),
            0,
            0,
            0,
        ],
        linux::__NR_inotify_rm_watch => [fd(a), pick(&[1u64, 1, 2, 3, u64::MAX], b), 0, 0, 0, 0],
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
        let clears = machine_bytes(call);
        for index in 0..CLEARS {
            let clear = clears.get(index).copied().unwrap_or_default();
            for word in [clear.at, clear.length, u64::from(clear.by_value)] {
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
    }
    out
}

/// A range of a call's output that holds bytes no two machines share. Both runners zero it after the
/// call is reported (`clear_machine` in the runner), so no later call reads them: after a failed call
/// only in the arena (a play page the call did not write is not brought in), and with `by_value` only
/// as many bytes as a successful call returned.
#[derive(Clone, Copy, Debug, Default)]
struct Clear {
    at: u64,
    length: u64,
    by_value: bool,
}

/// The most ranges a call clears (the runner's `CLEARS`).
const CLEARS: usize = 5;

/// What a call writes that two machines cannot share: every range `volatile` masks, and the fields
/// next to them a later call could read (a stat's link count, which `/proc` gives as its machine's
/// process count; a whole listing, link or working directory, which can name a process).
fn machine_bytes(call: &Call) -> Vec<Clear> {
    let [a, b, c, d, e, _] = call.arguments;
    let fixed = |at: u64, length: u64| Clear {
        at,
        length,
        by_value: false,
    };
    let by_value = |at: u64, length: u64| Clear {
        at,
        length,
        by_value: true,
    };
    let clears = match call.number {
        linux::__NR_ppoll if c != 0 => vec![fixed(c, 16)],
        // The sender's process number in a signal's information.
        linux::__NR_rt_sigtimedwait if b != 0 => vec![fixed(b + 16, 4)],
        linux::__NR_pselect6 if e != 0 => vec![fixed(e, 16)],
        linux::__NR_fcntl if matches!(b as u32, linux::F_GETLK | linux::F_OFD_GETLK) => {
            vec![fixed(c + 24, 4)]
        }
        linux::__NR_getcpu => vec![fixed(a, 4)],
        linux::__NR_gettimeofday => vec![fixed(a, 16)],
        linux::__NR_mincore => vec![fixed(c, b.div_ceil(4096).min(1 << 20))],
        linux::__NR_name_to_handle_at => vec![fixed(c + 8, 12), fixed(d, 8)],
        linux::__NR_readlinkat => vec![by_value(c, d)],
        linux::__NR_getcwd => vec![by_value(a, b)],
        linux::__NR_getdents64 => vec![by_value(b, c)],
        linux::__NR_newfstatat => vec![fixed(c, 16), fixed(c + 20, 4), fixed(c + 72, 48)],
        linux::__NR_fstat => vec![fixed(b, 16), fixed(b + 20, 4), fixed(b + 72, 48)],
        linux::__NR_statx => vec![
            fixed(e + 16, 4),
            fixed(e + 32, 8),
            fixed(e + 64, 64),
            fixed(e + 136, 24),
        ],
        linux::__NR_statfs | linux::__NR_fstatfs => vec![fixed(b + 40, 24)],
        linux::__NR_ioctl if b == FS_IOC_GETFSUUID => vec![fixed(c + 1, 16)],
        linux::__NR_clock_gettime => vec![fixed(b, 16)],
        // What is left of a timer.
        linux::__NR_timer_gettime | linux::__NR_timerfd_gettime | linux::__NR_getitimer => {
            vec![fixed(b + 16, 16)]
        }
        linux::__NR_timer_settime | linux::__NR_timerfd_settime if d != 0 => {
            vec![fixed(d + 16, 16)]
        }
        linux::__NR_setitimer if c != 0 => vec![fixed(c + 16, 16)],
        linux::__NR_times => vec![fixed(a, 32)],
        linux::__NR_sysinfo => vec![fixed(a, 112)],
        linux::__NR_getrusage => vec![fixed(b, 144)],
        linux::__NR_wait4 => vec![fixed(d, 144)],
        linux::__NR_waitid => vec![fixed(c + 16, 4), fixed(c + 32, 16), fixed(e, 144)],
        linux::__NR_getrandom => vec![by_value(a, b)],
        // A sender's process number and the times (as in `volatile`).
        linux::__NR_recvmsg => vec![fixed(ARENA + RECV_CONTROL, RECV_CONTROL_SIZE)],
        linux::__NR_sched_rr_get_interval => vec![fixed(b, 16)],
        linux::__NR_sched_getattr if c >= 32 => vec![fixed(b + 24, 8)],
        linux::__NR_uname => vec![fixed(a + 130, 130)],
        linux::__NR_sched_getaffinity => vec![by_value(c, b)],
        // The status calls' sequence numbers, times and processes (as in `volatile`).
        linux::__NR_shmctl if matches!(b, 2 | 13 | 15) => vec![fixed(c + 24, 2), fixed(c + 56, 32)],
        linux::__NR_shmctl if b == 14 => vec![fixed(c + 16, 8)],
        linux::__NR_msgctl if matches!(b, 2 | 11 | 13) => {
            vec![fixed(c + 24, 2), fixed(c + 48, 24), fixed(c + 96, 8)]
        }
        linux::__NR_semctl if matches!(c, 2 | 18 | 20) => vec![fixed(d + 24, 2), fixed(d + 48, 16)],
        _ => Vec::new(),
    };
    assert!(clears.len() <= CLEARS, "more clears than the runner takes");
    clears
}

/// The runner in a long-lived reference container, taking one sequence at a time.
pub struct Reference {
    name: String,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Reference {
    /// The runner in the reference; `backend` mounts the first file backend through Linux's FUSE
    /// at [`BACKEND_ROOT`] and runs the runner there without the power to mount, the server
    /// opening files for the page cache if `cached`.
    pub fn start(runner: &Path, backend: bool, cached: bool) -> Self {
        let name = format!(
            "wc-kernel-layer1-{}{}",
            std::process::id(),
            match (backend, cached) {
                (false, _) => "",
                (true, false) => "-backend",
                (true, true) => "-cached",
            }
        );
        let directory = runner
            .parent()
            .expect("runner has a directory")
            .canonicalize()
            .expect("directory exists");
        let program = Path::new("/wc").join(runner.file_name().expect("runner has a name"));
        let mut command = Command::new("docker");
        command
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
            .arg(format!("{}:/wc:ro", directory.display()));
        if backend {
            let server = crate::backend::server(&crate::backend::out());
            let script = format!(
                "{} && mkdir -p {BACKEND_ROOT} && exec {} mount {} {BACKEND_ROOT} {} {BACKEND_ROOT}",
                crate::backend::FUSE_DEVICE_LINK,
                crate::backend::serve_command(cached),
                crate::backend::SERVED,
                program.display()
            );
            command
                .args([
                    "--device",
                    crate::backend::FUSE_DEVICE,
                    "--cap-add",
                    "SYS_ADMIN",
                ])
                .args(["--tmpfs", crate::backend::SERVED])
                .arg("--volume")
                .arg(format!("{}:/wcb:ro", server.display()))
                .args([reference::IMAGE, "sh", "-c", &script]);
        } else {
            command.arg(reference::IMAGE).arg(&program);
        }
        let mut child = command
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
        let (calls, play) = parse_reports(&reported, sequence.calls.len());
        SequenceResult {
            calls,
            status: i32::from_le_bytes(word),
            play,
        }
    }
}

/// The calls a child reported, without a last record cut short by the child's death, and the play
/// area after the last one.
fn parse_reports(mut bytes: &[u8], count: usize) -> (Vec<CallResult>, Option<Vec<u8>>) {
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
        let flags = word(&mut bytes)?;
        let mut changes = Vec::new();
        for _ in 0..word(&mut bytes)? {
            let offset = word(&mut bytes)?;
            let length = word(&mut bytes)?;
            changes.push((offset, take(&mut bytes, length as usize)?.to_vec()));
        }
        Some(CallResult {
            value,
            changes,
            unshared: flags & 1 != 0,
            random: flags & 2 != 0,
        })
    };
    while calls.len() < count
        && let Some(call) = record()
    {
        calls.push(call);
    }
    let play = (calls.len() == count && bytes.len() == PLAY_RECORD).then(|| bytes.to_vec());
    (calls, play)
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

/// Where our side keeps what it needs to zero a random device's bytes, below every address a call is
/// given: an `iovec` pair, then zeros.
const SCRATCH: u64 = 0x6000_0000;
const SCRATCH_ZEROS: u64 = 0x1000;
/// Where the play area is read into at the end.
const SCRATCH_PLAY: u64 = 0x11000;
const SCRATCH_SIZE: u64 = SCRATCH_PLAY + PLAY_PAGES * 4096;
const MAX_RW_COUNT: u64 = 0x7fff_f000;

/// Signals both runners' callers ignore, as guest/layer1.c's does: at their default Linux keeps
/// them from our caller, the container's first process, and not from the reference's. SIGPIPE,
/// SIGIO, and those F_SETSIG may choose.
const IGNORED_SIGNALS: [u32; 8] = [
    linux::SIGPIPE,
    linux::SIGHUP,
    linux::SIGTRAP,
    linux::SIGFPE,
    linux::SIGTTOU,
    linux::SIGIO,
    32,
    64,
];
/// The reference runner's `DEADLINE_SECONDS`.
const DEADLINE_SECONDS: u64 = 60;
/// The status a child made by clone ends with at once (the runner's `CHILD_STATUS`).
const CHILD_STATUS: u64 = 7;

/// Runs a sequence on our kernel: a fresh container with the runner as its program, whose process makes
/// the calls with no instructions run. Where the reference's runner zeroed a random device's bytes
/// after a call (`theirs`), the same is done here, by the same calls.
pub fn run_ours(sequence: &Sequence, runner: &[u8], theirs: &SequenceResult) -> SequenceResult {
    run_ours_with(sequence, runner, theirs, &mut None)
}

/// As [`run_ours`], with the file backend a sequence on it uses: one server for every sequence,
/// started at the first, each kernel connecting afresh, and emptied after each as the reference's
/// runner empties its own.
pub fn run_ours_with(
    sequence: &Sequence,
    runner: &[u8],
    theirs: &SequenceResult,
    backend: &mut Option<crate::backend::Backend>,
) -> SequenceResult {
    let mut host = TestHost::new(0);
    if sequence.backend {
        host.backend = Some(backend.take().unwrap_or_else(|| {
            crate::backend::Backend::start(&crate::backend::out(), None, sequence.cached)
        }));
    }
    let (result, mut host) = run_ours_on(sequence, runner, theirs, host);
    if let Some(mut server) = host.backend.take() {
        // Unmounted, as the reference's runner's mount lets go of everything a sequence looked up.
        server.destroy();
        server.empty();
        *backend = Some(server);
    }
    result
}

/// The run itself, on `host`, which it gives back.
fn run_ours_on(
    sequence: &Sequence,
    runner: &[u8],
    theirs: &SequenceResult,
    mut host: TestHost,
) -> (SequenceResult, TestHost) {
    let mut cpu = BufferCpu::new();
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
        arguments: if sequence.backend {
            &[b"/wc/layer1", BACKEND_ROOT.as_bytes()]
        } else {
            &[b"/wc/layer1"]
        },
        environment: &environment,
        working_directory: b"/",
        hostname: reference::HOSTNAME.as_bytes(),
        groups: &[0, 0, 1, 2, 3, 4, 6, 10, 11, 20, 26, 27],
        backend: sequence.backend.then_some(BACKEND_ROOT.as_bytes()),
    };
    kernel.start(&mut cpu, &mut host, &start);
    assert_eq!(kernel.start_failure(), None, "starting the runner");
    let memory = wcontainers_abi::MemoryId(1);
    let thread = match kernel.next() {
        Next::Run { thread, .. } => thread,
        other => panic!("the runner did not start: {other:?}"),
    };
    // Makes one call, as if the running thread had reached it, and waits for its value; the process's
    // wait status if it ended.
    // The reference's runner kills a sequence that has not ended by its deadline; a sleep that would
    // end later is killed here too.
    let deadline =
        wcontainers_abi::Host::monotonic_nanoseconds(&mut host) + DEADLINE_SECONDS * 1_000_000_000;
    let call = |kernel: &mut Kernel,
                cpu: &mut BufferCpu,
                host: &mut TestHost,
                call: Call|
     -> Result<i64, i32> {
        // The thread's registers hold the call, as a CPU side leaves them.
        let mut registers = wcontainers_abi::RegisterBlock::ZERO;
        cpu.registers(thread, &mut registers);
        registers.x[8] = call.number.into();
        registers.x[..6].copy_from_slice(&call.arguments);
        cpu.set_registers(thread, &registers);
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
                // A child made by clone ends at once, as the reference's does.
                Next::Run { thread: child, .. } if child != thread => {
                    let mut registers = wcontainers_abi::RegisterBlock::ZERO;
                    cpu.registers(child, &mut registers);
                    registers.x[8] = linux::__NR_exit_group.into();
                    registers.x[0] = CHILD_STATUS;
                    cpu.set_registers(child, &registers);
                    kernel.exit(
                        cpu,
                        host,
                        Exit::Syscall(Syscall {
                            number: linux::__NR_exit_group.into(),
                            arguments: [CHILD_STATUS, 0, 0, 0, 0, 0],
                        }),
                    );
                }
                Next::Run {
                    resume: wcontainers_abi::Resume::Return(value),
                    ..
                } => return Ok(value as i64),
                Next::Run { resume, .. } => panic!("a call ended with {resume:?}"),
                Next::Finished { status } => return Err(status),
                // An answer the backend owes comes before any time passes.
                Next::Idle { .. } if host.backend.as_ref().is_some_and(|b| b.waiting()) => {
                    crate::driver::deliver(kernel, cpu, host);
                }
                // Waiting for ever: the runner kills such a sequence at its deadline.
                Next::Idle {
                    until_nanoseconds: None,
                } => return Err(linux::SIGKILL as i32),
                Next::Idle {
                    until_nanoseconds: Some(until),
                } if until > deadline => return Err(linux::SIGKILL as i32),
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
    let scratch = Call {
        number: linux::__NR_mmap,
        arguments: [SCRATCH, SCRATCH_SIZE, 3, 0x32, u64::MAX, 0],
    };
    assert_eq!(
        call(&mut kernel, &mut cpu, &mut host, scratch),
        Ok(SCRATCH as i64)
    );
    // An action of SIG_IGN at the scratch area's start, which the zeroing's vectors take later.
    cpu.write(
        memory,
        SCRATCH,
        &(linux_raw_sys::signal_macros::sig_ign().map_or(0, |handler| handler as usize) as u64)
            .to_le_bytes(),
    )
    .expect("writing an action");
    for signal in IGNORED_SIGNALS {
        let ignore = Call {
            number: linux::__NR_rt_sigaction,
            arguments: [signal.into(), SCRATCH, 0, 8, 0, 0],
        };
        assert_eq!(call(&mut kernel, &mut cpu, &mut host, ignore), Ok(0));
    }
    // The reference's runner has remade `/dev/null` before every sequence while the container's first
    // one stays held, so it is remade here too, while descriptor 0 holds the first one.
    let null = ARENA + PATHS + path_offset("/dev/null");
    for number in [
        linux::__NR_unlinkat,
        linux::__NR_mknodat,
        linux::__NR_fchmodat,
    ] {
        let third = match number {
            linux::__NR_mknodat => u64::from(linux::S_IFCHR | 0o666),
            linux::__NR_fchmodat => 0o666,
            _ => 0,
        };
        let device = if number == linux::__NR_mknodat {
            0x103
        } else {
            0
        };
        let remake = Call {
            number,
            arguments: [linux::AT_FDCWD as u64, null, third, device, 0, 0],
        };
        assert_eq!(call(&mut kernel, &mut cpu, &mut host, remake), Ok(0));
    }
    let mut before = vec![0u8; ARENA_SIZE as usize];
    use wcontainers_abi::Cpu;
    cpu.read(memory, ARENA, &mut before)
        .expect("reading the arena");
    let own = Call {
        number: linux::__NR_getpid,
        arguments: [0; 6],
    };
    let pid = call(&mut kernel, &mut cpu, &mut host, own).expect("getpid") as u64;
    // The runner's `clear_range`: zeros up to the first page the process cannot write.
    let clear = |kernel: &mut Kernel,
                 cpu: &mut BufferCpu,
                 host: &mut TestHost,
                 mut at: u64,
                 mut length: u64| {
        while length > 0 {
            let part = length.min(SCRATCH_SIZE - SCRATCH_ZEROS);
            let pair = [SCRATCH + SCRATCH_ZEROS, part, at, part];
            let bytes: Vec<u8> = pair.iter().flat_map(|word| word.to_le_bytes()).collect();
            cpu.write(memory, SCRATCH, &bytes)
                .expect("writing the scratch");
            let write = Call {
                number: linux::__NR_process_vm_writev,
                arguments: [pid, SCRATCH, 1, SCRATCH + 16, 1, 0],
            };
            let put = call(kernel, cpu, host, write).unwrap_or(-1);
            if put <= 0 {
                return;
            }
            at += put as u64;
            length -= put as u64;
            if (put as u64) < part {
                return;
            }
        }
    };
    let mut results = Vec::new();
    let mut segment = NO_SEGMENT;
    let mut queue = NO_SEGMENT;
    let mut semaphores = NO_SEGMENT;
    for (index, each) in sequence.calls.iter().enumerate() {
        let mut each = *each;
        for argument in &mut each.arguments {
            if *argument == SELF {
                *argument = pid;
            }
            if *argument == SEGMENT {
                *argument = segment;
            }
            if *argument == QUEUE {
                *argument = queue;
            }
            if *argument == SEMAPHORES {
                *argument = semaphores;
            }
        }
        let random = theirs.calls.get(index).is_some_and(|call| call.random);
        let [a, b, c, ..] = each.arguments;
        let out_before = if random && each.number == linux::__NR_sendfile {
            let position = Call {
                number: linux::__NR_lseek,
                arguments: [a, 0, u64::from(linux::SEEK_CUR), 0, 0, 0],
            };
            call(&mut kernel, &mut cpu, &mut host, position).ok()
        } else {
            None
        };
        let outcome = call(&mut kernel, &mut cpu, &mut host, each);
        if let Ok(id) = outcome
            && each.number == linux::__NR_shmget
            && id >= 0
        {
            segment = id as u64;
        }
        if let Ok(id) = outcome
            && each.number == linux::__NR_msgget
            && id >= 0
        {
            queue = id as u64;
        }
        if let Ok(id) = outcome
            && each.number == linux::__NR_semget
            && id >= 0
        {
            semaphores = id as u64;
        }
        // The runner's wait for a child to end, left to be reaped.
        if let Ok(child) = outcome
            && each.number == linux::__NR_clone
            && child > 0
        {
            let wait = Call {
                number: linux::__NR_waitid,
                arguments: [
                    u64::from(linux::P_PID),
                    child as u64,
                    SCRATCH,
                    u64::from(linux::WEXITED | linux::WNOWAIT | linux::__WALL),
                    0,
                    0,
                ],
            };
            let _ = call(&mut kernel, &mut cpu, &mut host, wait);
        }
        match outcome {
            Ok(value) => {
                if random && value >= 0 {
                    // The runner's `clear_random`, by the same calls: the ranges a read was given.
                    let mut ranges = Vec::new();
                    match each.number {
                        linux::__NR_read | linux::__NR_pread64 => {
                            ranges.push((b, c.min(MAX_RW_COUNT)));
                        }
                        linux::__NR_readv | linux::__NR_preadv | linux::__NR_preadv2 => {
                            let mut parts = vec![0u8; (c as usize).min(1024) * 16];
                            if cpu.read(memory, b, &mut parts).is_ok() {
                                let mut total = 0u64;
                                for part in parts.as_chunks::<16>().0 {
                                    if total >= MAX_RW_COUNT {
                                        break;
                                    }
                                    let base = u64::from_le_bytes(part[..8].try_into().expect("8"));
                                    let length =
                                        u64::from_le_bytes(part[8..].try_into().expect("8"))
                                            .min(MAX_RW_COUNT - total);
                                    ranges.push((base, length));
                                    total += length;
                                }
                            }
                        }
                        _ => {}
                    }
                    for (at, length) in ranges {
                        clear(&mut kernel, &mut cpu, &mut host, at, length);
                    }
                    // A sendfile's output is zeroed where the bytes went.
                    if let Some(before) = out_before {
                        let mut done = 0;
                        while done < value {
                            let part = (value - done).min((SCRATCH_SIZE - SCRATCH_ZEROS) as i64);
                            let write = Call {
                                number: linux::__NR_pwrite64,
                                arguments: [
                                    a,
                                    SCRATCH + SCRATCH_ZEROS,
                                    part as u64,
                                    (before + done) as u64,
                                    0,
                                    0,
                                ],
                            };
                            let put = call(&mut kernel, &mut cpu, &mut host, write).unwrap_or(-1);
                            if put <= 0 {
                                break;
                            }
                            done += put;
                        }
                    }
                }
                let mut after = vec![0u8; ARENA_SIZE as usize];
                cpu.read(memory, ARENA, &mut after)
                    .expect("reading the arena");
                results.push(CallResult {
                    value,
                    changes: changes(&before, &after),
                    unshared: false,
                    random: false,
                });
                before = after;
                // The runner's `clear_machine`: what follows sees zeros where the machines differ.
                for range in machine_bytes(&each) {
                    let in_arena = (ARENA..ARENA + ARENA_SIZE).contains(&range.at);
                    if value < 0 && (range.by_value || !in_arena) {
                        continue;
                    }
                    let length = if range.by_value {
                        range.length.min(value as u64)
                    } else {
                        range.length
                    };
                    clear(&mut kernel, &mut cpu, &mut host, range.at, length);
                }
            }
            Err(status) => {
                return (
                    SequenceResult {
                        calls: results,
                        status,
                        play: None,
                    },
                    host,
                );
            }
        }
    }
    // The play area, page by page, by the runner's own calls.
    let mut play = Vec::with_capacity(PLAY_RECORD);
    for page in 0..PLAY_PAGES {
        let into = SCRATCH + SCRATCH_PLAY + page * 4096;
        let pair = [into, 4096, PLAY + page * 4096, 4096];
        let bytes: Vec<u8> = pair.iter().flat_map(|word| word.to_le_bytes()).collect();
        cpu.write(memory, SCRATCH, &bytes)
            .expect("writing the scratch");
        let read = Call {
            number: linux::__NR_process_vm_readv,
            arguments: [pid, SCRATCH, 1, SCRATCH + 16, 1, 0],
        };
        let mut contents = vec![0u8; 4096];
        // A filter of the sequence's may end the process here, as it does the reference's.
        let readable = match call(&mut kernel, &mut cpu, &mut host, read) {
            Ok(value) => value == 4096,
            Err(status) => {
                return (
                    SequenceResult {
                        calls: results,
                        status,
                        play: None,
                    },
                    host,
                );
            }
        };
        if readable {
            cpu.read(memory, into, &mut contents)
                .expect("reading the scratch");
        }
        play.push(u8::from(readable));
        play.extend_from_slice(&contents);
    }
    (
        SequenceResult {
            calls: results,
            status: 0,
            play: Some(play),
        },
        host,
    )
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
/// Where a path of the pool lies in the arena.
fn path_offset(wanted: &str) -> u64 {
    let mut offset = 0;
    for path in PATH_POOL {
        if *path == wanted {
            return offset;
        }
        offset += path.len() as u64 + 1;
    }
    panic!("{wanted} is not in the path pool")
}

/// The ioctl requests layer 1 makes: the common ones of every file (`do_vfs_ioctl`), regular files'
/// (`file_ioctl`), tmpfs's inode flags, terminal and random device requests (which no file here
/// takes), and unknown ones.
const IOCTL_REQUESTS: &[u64] = &[
    ioctl::FIOCLEX as u64,
    ioctl::FIONCLEX as u64,
    ioctl::FIONBIO as u64,
    ioctl::FIOASYNC as u64,
    ioctl::FIONREAD as u64,
    ioctl::FIOQSIZE as u64,
    ioctl::FIGETBSZ as u64,
    ioctl::FIBMAP as u64,
    ioctl::FIFREEZE as u64,
    ioctl::FITHAW as u64,
    ioctl::FS_IOC_FIEMAP as u64,
    ioctl::FICLONE as u64,
    ioctl::FICLONERANGE as u64,
    ioctl::FIDEDUPERANGE as u64,
    ioctl::FS_IOC_GETFLAGS as u64,
    ioctl::FS_IOC_SETFLAGS as u64,
    ioctl::FS_IOC_FSGETXATTR as u64,
    ioctl::FS_IOC_FSSETXATTR as u64,
    FS_IOC_GETFSUUID,
    FS_IOC_GETFSSYSFSPATH,
    // FS_IOC_RESVSP, FS_IOC_UNRESVSP64 and FS_IOC_ZERO_RANGE (`linux/falloc.h`, not a uapi header).
    0x4030_5828,
    0x4030_582b,
    0x4030_5839,
    ioctl::RNDGETENTCNT as u64,
    ioctl::RNDZAPENTCNT as u64,
    0x1234,
    0x8008_6680,
    // Terminal requests (`tty_ioctl`, `tty_mode_ioctl`, `n_tty_ioctl`, `pty_unix98_ioctl`); not
    // TIOCSCTTY, since no sequence may take a controlling terminal.
    ioctl::TCGETS as u64,
    ioctl::TCSETS as u64,
    ioctl::TCSETSW as u64,
    ioctl::TCSETSF as u64,
    ioctl::TCGETS2 as u64,
    ioctl::TCSETS2 as u64,
    ioctl::TCGETA as u64,
    ioctl::TCSETA as u64,
    ioctl::TIOCGWINSZ as u64,
    ioctl::TIOCSWINSZ as u64,
    ioctl::TIOCOUTQ as u64,
    ioctl::TCFLSH as u64,
    ioctl::TCXONC as u64,
    ioctl::TCSBRK as u64,
    ioctl::TIOCPKT as u64,
    ioctl::TIOCGPKT as u64,
    ioctl::TIOCSPTLCK as u64,
    ioctl::TIOCSPTLCK as u64,
    ioctl::TIOCGPTLCK as u64,
    ioctl::TIOCGPTN as u64,
    ioctl::TIOCGPTPEER as u64,
    ioctl::TIOCEXCL as u64,
    ioctl::TIOCNXCL as u64,
    ioctl::TIOCGEXCL as u64,
    ioctl::TIOCGPGRP as u64,
    ioctl::TIOCGSID as u64,
    ioctl::TIOCNOTTY as u64,
    ioctl::TIOCGETD as u64,
    ioctl::TIOCSETD as u64,
    ioctl::TIOCGDEV as u64,
    ioctl::TIOCSTI as u64,
    ioctl::TIOCSIG as u64,
    ioctl::TIOCGSOFTCAR as u64,
    ioctl::TIOCSSOFTCAR as u64,
    ioctl::TIOCVHANGUP as u64,
];

/// Terminal requests whose argument is a value, not an address.
const BY_VALUE_REQUESTS: &[u64] = &[
    ioctl::TCFLSH as u64,
    ioctl::TCXONC as u64,
    ioctl::TCSBRK as u64,
    ioctl::TIOCGPTPEER as u64,
    ioctl::TIOCSIG as u64,
];

/// The terminal requests that read or write a `termios`, `termios2` or `termio`.
const SETTINGS_REQUESTS: &[u64] = &[
    ioctl::TCGETS as u64,
    ioctl::TCSETS as u64,
    ioctl::TCSETSW as u64,
    ioctl::TCSETSF as u64,
    ioctl::TCGETS2 as u64,
    ioctl::TCSETS2 as u64,
    ioctl::TCGETA as u64,
    ioctl::TCSETA as u64,
];

/// `FS_IOC_GETFSUUID` and `FS_IOC_GETFSSYSFSPATH`, which linux-raw-sys's ioctl list lacks: reads of a
/// 17 byte and a 129 byte structure, numbers 0 and 1 of kind 0x15.
const FS_IOC_GETFSUUID: u64 = 0x8011_1500;
const FS_IOC_GETFSSYSFSPATH: u64 = 0x8081_1501;

/// The words of each ioctl argument at `IOCTL_ARGS`.
fn ioctl_arguments() -> [[u64; 8]; IOCTL_ARG_COUNT as usize] {
    let flags = |value: u32| [u64::from(value), 0, 0, 0, 0, 0, 0, 0];
    // `struct fsxattr`: xflags and extent size, then the rest.
    let xattr = |xflags: u32, extent: u32| {
        [
            u64::from(xflags) | u64::from(extent) << 32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    };
    // `struct space_resv`: type and origin, start, length.
    let space = |whence: u16, start: u64, length: u64| {
        [u64::from(whence) << 16, start, length, 0, 0, 0, 0, 0]
    };
    [
        flags(linux::FS_NODUMP_FL | linux::FS_NOATIME_FL),
        flags(linux::FS_IMMUTABLE_FL),
        flags(0x4000_0001),
        flags(0),
        xattr(linux::FS_XFLAG_NODUMP, 0),
        xattr(linux::FS_XFLAG_EXTSIZE, 4096),
        // `struct file_clone_range`: source descriptor, source offset, length, destination offset.
        [3, 0, 4096, 0, 0, 0, 0, 0],
        // `struct file_dedupe_range` with one destination: descriptor 3 at offset 0.
        [0, 4096, 1, 3, 0, 0, 0, 0],
        space(0, 0, 8192),
        space(1, 4096, 4096),
        space(2, 0, 1 << 40),
        space(7, 0, 4096),
    ]
}

/// statfs's inode total depends on the machine's memory (tmpfs's default is half its pages), so the
/// free count is compared as the count in use, and the total is masked.
fn inodes_in_use(arena: &mut [u8], buffer: u64) {
    if !in_arena(buffer, 56) {
        return;
    }
    let at = (buffer - ARENA) as usize;
    let word = |arena: &[u8], at: usize| {
        u64::from_le_bytes(arena[at..at + 8].try_into().expect("eight bytes"))
    };
    let used = word(arena, at + 40).wrapping_sub(word(arena, at + 48));
    arena[at + 48..at + 56].copy_from_slice(&used.to_le_bytes());
}

/// Whether `length` bytes at `address` lie in the arena.
fn in_arena(address: u64, length: u64) -> bool {
    address
        .checked_sub(ARENA)
        .is_some_and(|offset| offset.saturating_add(length) <= ARENA_SIZE)
}

/// A little-endian word of `length` bytes (at most 8) at `address` in the arena.
fn read_word(arena: &[u8], address: u64, length: usize) -> Option<u64> {
    address
        .checked_sub(ARENA)
        .and_then(|offset| arena.get(offset as usize..(offset as usize).checked_add(length)?))
        .map(|bytes| {
            let mut word = [0u8; 8];
            word[..length].copy_from_slice(bytes);
            u64::from_le_bytes(word)
        })
}

fn volatile(call: &Call, value: i64, arena: &[u8]) -> Vec<(u64, u64)> {
    let [a, b, c, d, e, _] = call.arguments;
    let mut out = Vec::new();
    // The CPU a registered rseq area names, which the reference's kernel rewrites whenever its caller
    // moves to another CPU, during any call (one CPU here is a declared difference).
    for offset in RSEQ_AREAS {
        out.push((ARENA + RSEQS + offset, 8));
    }
    // The time left, which ppoll writes back even when it fails.
    if call.number == linux::__NR_ppoll && c != 0 {
        out.push((c, 16));
    }
    if call.number == linux::__NR_pselect6 && e != 0 {
        out.push((e, 16));
    }
    // The process a test of a record lock found holding it: each runner's own number (an open file's
    // lock shows -1, none leaves what was there).
    if call.number == linux::__NR_fcntl
        && matches!(b as u32, linux::F_GETLK | linux::F_OFD_GETLK)
        && value == 0
        && let Some(at) = c.checked_sub(ARENA).map(|at| at as usize + 24)
        && at + 4 <= arena.len()
        && i32::from_le_bytes(arena[at..at + 4].try_into().expect("4")) > 0
    {
        out.push((c + 24, 4));
    }
    // A segment's sequence number, times and processes; the pages all segments hold in memory.
    if call.number == linux::__NR_shmctl && value >= 0 {
        match b {
            2 | 13 | 15 => out.extend([(c + 24, 2), (c + 56, 32)]),
            14 => out.push((c + 16, 8)),
            _ => {}
        }
    }
    // A queue's sequence number, times and processes.
    if call.number == linux::__NR_msgctl && value >= 0 && matches!(b, 2 | 11 | 13) {
        out.extend([(c + 24, 2), (c + 48, 24), (c + 96, 8)]);
    }
    // A semaphore set's sequence number and times.
    if call.number == linux::__NR_semctl && value >= 0 && matches!(c, 2 | 18 | 20) {
        out.extend([(d + 24, 2), (d + 48, 16)]);
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
    // A handle holds inode numbers and a random generation, and the mount number is the machine's;
    // the mount number is written even when the handle does not fit.
    if call.number == linux::__NR_name_to_handle_at {
        out.extend([(c + 8, 12), (d, 8)]);
    }
    // Fixed fields, also when a copy that met a fault left part of them behind.
    match call.number {
        linux::__NR_newfstatat => out.extend([(c, 16), (c + 72, 48)]),
        linux::__NR_fstat => out.extend([(b, 16), (b + 72, 48)]),
        linux::__NR_statx => {
            out.extend([(e + 32, 8), (e + 64, 64), (e + 136, 16)]);
            // The unique mount number counts every mount the machine has made.
            if value == 0 && read_word(arena, e, 4).is_some_and(|mask| mask & 0x4000 != 0) {
                out.push((e + 144, 8));
            }
        }
        // The inode total and the free count (see `inodes_in_use`) and the identity, which tmpfs
        // draws at random; a successful call's free count is compared as the count in use.
        linux::__NR_statfs | linux::__NR_fstatfs if value == 0 => {
            out.extend([(b + 40, 8), (b + 56, 8)]);
            if !in_arena(b, 56) {
                out.push((b + 48, 8));
            }
        }
        linux::__NR_statfs | linux::__NR_fstatfs => out.push((b + 40, 24)),
        linux::__NR_ioctl if b == FS_IOC_GETFSUUID => out.push((c + 1, 16)),
        linux::__NR_clock_gettime => out.push((b, 16)),
        linux::__NR_timer_gettime | linux::__NR_timerfd_gettime | linux::__NR_getitimer => {
            out.push((b + 16, 16))
        }
        linux::__NR_timer_settime | linux::__NR_timerfd_settime if d != 0 => out.push((d + 16, 16)),
        linux::__NR_setitimer if c != 0 => out.push((c + 16, 16)),
        linux::__NR_times => out.push((a, 32)),
        linux::__NR_sysinfo => out.push((a, 112)),
        linux::__NR_getrusage => out.push((b, 144)),
        linux::__NR_wait4 => out.push((d, 144)),
        // A child's process number and times, and its use of the machine.
        linux::__NR_waitid => out.extend([(c + 16, 4), (c + 32, 16), (e, 144)]),
        // The default time slice depends on the machine's CPU count, one here.
        linux::__NR_sched_rr_get_interval => out.push((b, 16)),
        linux::__NR_sched_getattr if c >= 32 => out.push((b + 24, 8)),
        linux::__NR_uname => out.push((a + 130, 130)),
        _ => {}
    }
    if value < 0 {
        return out;
    }
    // The sender's process number and the times in a receive's control messages.
    if call.number == linux::__NR_recvmsg {
        let length = read_word(arena, b + 40, 8)
            .unwrap_or(0)
            .min(RECV_CONTROL_SIZE);
        let mut at = 0;
        while at + 16 <= length {
            let start = ARENA + RECV_CONTROL + at;
            let size = read_word(arena, start, 8).unwrap_or(0);
            if size < 16 {
                break;
            }
            if read_word(arena, start + 8, 4) == Some(u64::from(net::SOL_SOCKET)) {
                match read_word(arena, start + 12, 4).map(|kind| kind as u32) {
                    Some(net::SCM_CREDENTIALS) => out.push((start + 16, 4)),
                    Some(
                        net::SO_TIMESTAMP_OLD
                        | net::SO_TIMESTAMPNS_OLD
                        | net::SO_TIMESTAMP_NEW
                        | net::SO_TIMESTAMPNS_NEW,
                    ) => out.push((start + 16, 16)),
                    _ => {}
                }
            }
            at += size.next_multiple_of(8);
        }
    }
    if call.number == linux::__NR_readlinkat
        && (names_process(c, arena) || names_counted_inode(c, arena))
    {
        out.push((c, d));
    }
    if call.number == linux::__NR_getcwd && under_proc(a, arena) {
        out.push((a, value as u64));
    }
    // The link count of `/proc` itself counts every process on the machine (Linux's
    // `proc_root_getattr`), and the reference shares its machine. `/proc` is inode 1 of a file system
    // with 1024 byte blocks, and so is devpts's root, whose link count goes too.
    let proc_root = |ino: u64, blksize: u64| {
        read_word(arena, ino, 8) == Some(1) && read_word(arena, blksize, 4) == Some(1024)
    };
    match call.number {
        linux::__NR_newfstatat if proc_root(c + 8, c + 56) => out.push((c + 20, 4)),
        linux::__NR_fstat if proc_root(b + 8, b + 56) => out.push((b + 20, 4)),
        linux::__NR_statx if proc_root(e + 32, e + 4) => out.push((e + 16, 4)),
        _ => {}
    }
    match call.number {
        linux::__NR_getrandom => out.push((a, value as u64)),
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

/// Whether a link read to `address` names a pipe or a socket, whose inode numbers each machine
/// hands out from its own count (`get_next_ino`).
fn names_counted_inode(address: u64, arena: &[u8]) -> bool {
    let at = |length: usize| {
        address
            .checked_sub(ARENA)
            .and_then(|offset| arena.get(offset as usize..offset as usize + length))
    };
    at(6).is_some_and(|bytes| bytes == b"pipe:[") || at(8).is_some_and(|bytes| bytes == b"socket:[")
}

/// Whether the path at `address` names a process directory, `/proc/<pid>`, by number.
fn names_process(address: u64, arena: &[u8]) -> bool {
    under_proc(address, arena)
        && address
            .checked_sub(ARENA)
            .and_then(|offset| arena.get(offset as usize + 6))
            .is_some_and(u8::is_ascii_digit)
}

/// Whether a call's value is an address the kernel chose.
/// Process numbers differ too: the reference's caller is a child of its runner.
fn value_is_volatile(call: &Call, arena: &[u8]) -> bool {
    match call.number {
        // A working directory inside `/proc/<pid>` names the caller's process number.
        linux::__NR_getcwd => under_proc(call.arguments[0], arena),
        // So does a link to one, such as `/proc/self/cwd` there.
        linux::__NR_readlinkat => {
            names_process(call.arguments[2], arena) || names_counted_inode(call.arguments[2], arena)
        }
        linux::__NR_mmap => call.arguments[3] & 0x10_0010 == 0,
        linux::__NR_getpid
        | linux::__NR_getppid
        | linux::__NR_gettid
        | linux::__NR_getpgid
        | linux::__NR_getsid
        | linux::__NR_times
        | linux::__NR_setsid
        | linux::__NR_set_tid_address
        | linux::__NR_clone
        | linux::__NR_wait4 => true,
        // Moved by the kernel to a place of its choosing.
        linux::__NR_mremap => call.arguments[3] & 3 == 1,
        // Identifiers and slots (see `SEGMENT`), and a segment placed by the kernel.
        linux::__NR_shmget => true,
        linux::__NR_shmctl => matches!(call.arguments[1], 3 | 13 | 14 | 15),
        linux::__NR_shmat => call.arguments[1] < 4096,
        linux::__NR_msgget => true,
        linux::__NR_msgctl => matches!(call.arguments[1], 3 | 11 | 12 | 13),
        // Identifiers, slots, and the process that last changed a semaphore.
        linux::__NR_semget => true,
        linux::__NR_semctl => matches!(call.arguments[2], 3 | 11 | 18 | 19 | 20),
        _ => false,
    }
}

/// Which bytes of the arena and the play area hold values no two machines share.
struct Mask {
    arena: Vec<bool>,
    play: Vec<bool>,
}

/// The play area's size as far as calls reach into it: its pages and two past them.
const PLAY_REACH: u64 = (PLAY_PAGES + 2) * 4096;

impl Default for Mask {
    fn default() -> Self {
        Mask {
            arena: vec![false; ARENA_SIZE as usize],
            play: vec![false; PLAY_REACH as usize],
        }
    }
}

impl Mask {
    fn slot(&mut self, address: u64) -> Option<&mut bool> {
        if let Some(offset) = address.checked_sub(ARENA).filter(|at| *at < ARENA_SIZE) {
            return self.arena.get_mut(offset as usize);
        }
        let offset = address.checked_sub(PLAY).filter(|at| *at < PLAY_REACH)?;
        self.play.get_mut(offset as usize)
    }

    fn hide(&mut self, start: u64, length: u64) {
        let length = length.min(ARENA_SIZE + PLAY_REACH);
        for address in start..start.saturating_add(length) {
            if let Some(slot) = self.slot(address) {
                *slot = true;
            }
        }
    }

    fn hidden(&mut self, address: u64) -> bool {
        self.slot(address).is_some_and(|slot| *slot)
    }
}

/// `process_vm_readv` and `process_vm_writev` copy within the caller: a byte copied from a hidden
/// one is hidden too. The segment lists are read from the arena as it was before the call.
fn copied_masks(call: &Call, value: i64, before: &[u8], mask: &mut Mask) {
    let write = match call.number {
        linux::__NR_process_vm_readv => false,
        linux::__NR_process_vm_writev => true,
        _ => return,
    };
    if value <= 0 {
        return;
    }
    let [_, local, local_count, remote, remote_count, _] = call.arguments;
    let addresses = |vector: u64, count: u64| -> Vec<u64> {
        let mut out = Vec::new();
        for index in 0..count.min(1024) {
            let at = vector.wrapping_add(index * 16).wrapping_sub(ARENA) as usize;
            let Some(entry) = before.get(at..at + 16) else {
                break;
            };
            let base = u64::from_le_bytes(entry[..8].try_into().expect("8 bytes"));
            let length = u64::from_le_bytes(entry[8..].try_into().expect("8 bytes"));
            for offset in 0..length.min(value as u64 - out.len() as u64) {
                out.push(base.wrapping_add(offset));
            }
            if out.len() as u64 >= value as u64 {
                break;
            }
        }
        out
    };
    let local = addresses(local, local_count);
    let remote = addresses(remote, remote_count);
    let (from, to) = if write {
        (local, remote)
    } else {
        (remote, local)
    };
    for (source, target) in from.into_iter().zip(to) {
        if mask.hidden(source)
            && let Some(slot) = mask.slot(target)
        {
            *slot = true;
        }
    }
}

/// Everything compared for a sequence, call by call, and the final status.
/// `unshared` marks the calls whose results depend on what the runners cannot share; their values
/// and what they wrote are not compared.
pub fn seen(
    sequence: &Sequence,
    result: &SequenceResult,
    unshared: &[bool],
) -> (Vec<Seen>, i32, Option<Vec<u8>>) {
    let mut arena = sequence.arena.clone();
    arena.resize(ARENA_SIZE as usize, 0);
    let mut mask = Mask::default();
    let mut out = Vec::new();
    for (index, (call, each)) in sequence.calls.iter().zip(&result.calls).enumerate() {
        let unshared = unshared.get(index).copied().unwrap_or(false);
        let before = arena.clone();
        for (offset, bytes) in &each.changes {
            arena[*offset as usize..*offset as usize + bytes.len()].copy_from_slice(bytes);
        }
        if each.value == 0 && matches!(call.number, linux::__NR_statfs | linux::__NR_fstatfs) {
            inodes_in_use(&mut arena, call.arguments[1]);
        }
        let mut hidden = volatile(call, each.value, &arena);
        if unshared {
            let [a, b, c, ..] = call.arguments;
            match call.number {
                linux::__NR_getdents64 => hidden.push((b, c.min(ARENA_SIZE))),
                linux::__NR_getcwd => hidden.push((a, b.min(ARENA_SIZE))),
                _ => {}
            }
        }
        for (start, length) in hidden {
            mask.hide(start, length);
        }
        copied_masks(call, each.value, &before, &mut mask);
        let mut shown = arena.clone();
        for (byte, masked) in shown.iter_mut().zip(&mask.arena) {
            if *masked {
                *byte = 0;
            }
        }
        let value = if unshared || value_is_volatile(call, &arena) && each.value >= 0 {
            0
        } else {
            each.value
        };
        out.push(Seen {
            value,
            arena: shown,
        });
    }
    // The play area at the end, with what must differ cleared.
    let play = result.play.as_ref().map(|play| {
        let mut shown = play.clone();
        for (page, record) in shown.chunks_mut(4097).enumerate() {
            for (offset, byte) in record[1..].iter_mut().enumerate() {
                if mask.play[page * 4096 + offset] {
                    *byte = 0;
                }
            }
        }
        shown
    });
    (out, result.status, play)
}

/// A message header as the sequence starts with it, and its first vector, for an account of a
/// sendmsg or recvmsg.
fn header_text(arena: &[u8], address: u64) -> String {
    let word = |at: u64| {
        let at = at.wrapping_sub(ARENA) as usize;
        arena.get(at..at + 8).map_or(0, |bytes| {
            u64::from_le_bytes(bytes.try_into().expect("8 bytes"))
        })
    };
    let vector = word(address + 16);
    format!(
        " [name {:x} length {} vector {:x} count {} first ({:x}, {:x}) control {:x} room {:x}]",
        word(address),
        word(address + 8) as u32,
        vector,
        word(address + 24),
        word(vector),
        word(vector + 8),
        word(address + 32),
        word(address + 40)
    )
}

/// A readable account of where two runs of a sequence part.
pub fn difference(
    sequence: &Sequence,
    ours: &SequenceResult,
    theirs: &SequenceResult,
) -> Option<String> {
    let unshared: Vec<bool> = theirs.calls.iter().map(|call| call.unshared).collect();
    let (our_seen, our_status, our_play) = seen(sequence, ours, &unshared);
    let (their_seen, their_status, their_play) = seen(sequence, theirs, &unshared);
    let mut text = String::new();
    for (index, call) in sequence.calls.iter().enumerate() {
        let name = wcontainers_kernel::system_call_name(call.number.into()).unwrap_or("?");
        let mut line = format!("{index}: {name}({:x?})", call.arguments);
        if matches!(call.number, linux::__NR_sendmsg | linux::__NR_recvmsg) {
            line.push_str(&header_text(&sequence.arena, call.arguments[1]));
        }
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
    if our_status != their_status {
        return Some(format!(
            "{text}status: ours {our_status:#x} theirs {their_status:#x}"
        ));
    }
    if our_play != their_play {
        text.push_str("play area after the last call:\n");
        let (ours, theirs) = (our_play.unwrap_or_default(), their_play.unwrap_or_default());
        if ours.len() != theirs.len() {
            text.push_str(&format!(
                "  read by one side only (ours {} bytes, theirs {})\n",
                ours.len(),
                theirs.len()
            ));
        }
        for (page, (mine, other)) in ours.chunks(4097).zip(theirs.chunks(4097)).enumerate() {
            if mine[0] != other[0] {
                text.push_str(&format!(
                    "  page {page}: readable ours {} theirs {}\n",
                    mine[0], other[0]
                ));
            }
            for (offset, (a, b)) in mine[1..].iter().zip(&other[1..]).enumerate() {
                if a != b {
                    text.push_str(&format!(
                        "  play+{:#x}: ours {a:#04x} theirs {b:#04x}\n",
                        page * 4096 + offset
                    ));
                }
            }
        }
        return Some(text);
    }
    None
}
