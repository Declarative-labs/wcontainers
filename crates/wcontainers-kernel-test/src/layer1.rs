//! Layer 1: random sequences of system calls, made in the reference by a small native runner and in our
//! kernel with a plain buffer as guest memory. Every return value and every byte written into the
//! sequence's memory is compared; the few values that must differ (inode numbers, times, process
//! numbers) are masked on both sides.

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
const VALUES: u64 = 0x3800;
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
];

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
    let open_flags = |word: u64| {
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
        ];
        let mut flags = 0;
        for (index, bit) in bits.iter().enumerate() {
            if (word >> (index * 3)) & 7 == 0 {
                flags |= bit;
            }
        }
        u64::from(flags)
    };
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
                    1000,
                ],
                b,
            );
            [
                fd(a),
                command.into(),
                pick(&[0, 1, 5, 100, 0x800, 0x400 | 0x800, u64::MAX], c),
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
            pick(&[0, 1, 2, 3, 4, 8, 9, 100], c),
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
            pick(&[0, 99999], a),
            b % 17,
            if c % 3 == 0 { value(c) } else { 0 },
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
            pick(&[1, 2, 9, 10, 15, 17, 19, 64, 65, 0], a),
            if b % 3 == 0 { 0 } else { value(b) },
            if c % 2 == 0 { buffer(c) } else { 0 },
            pick(&[8, 8, 4], d),
            0,
            0,
        ],
        linux::__NR_sigaltstack => [
            if a % 3 == 0 { 0 } else { value(a) },
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
        linux::__NR_sched_getaffinity => [0, pick(&[0, 4, 8, 128], a), buffer(b), 0, 0, 0],
        linux::__NR_nanosleep => [ARENA + VALUES + 56 * 8 + (a % 4) * 16, 0, 0, 0, 0, 0],
        linux::__NR_capget => [value(a), if b % 2 == 0 { buffer(b) } else { 0 }, 0, 0, 0, 0],
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
        let mut calls = Vec::new();
        loop {
            let mut word = [0u8; 4];
            self.output.read_exact(&mut word).expect("reading a result");
            if u32::from_le_bytes(word) == u32::MAX {
                self.output
                    .read_exact(&mut word)
                    .expect("reading the status");
                return SequenceResult {
                    calls,
                    status: i32::from_le_bytes(word),
                };
            }
            let mut value = [0u8; 8];
            self.output
                .read_exact(&mut value)
                .expect("reading a result");
            let value = i64::from_le_bytes(value);
            self.output.read_exact(&mut word).expect("reading a result");
            let mut changes = Vec::new();
            for _ in 0..u32::from_le_bytes(word) {
                let mut header = [0u8; 8];
                self.output
                    .read_exact(&mut header)
                    .expect("reading a change");
                let offset = u32::from_le_bytes(header[..4].try_into().expect("4 bytes"));
                let length = u32::from_le_bytes(header[4..].try_into().expect("4 bytes"));
                let mut bytes = vec![0; length as usize];
                self.output
                    .read_exact(&mut bytes)
                    .expect("reading a change");
                changes.push((offset, bytes));
            }
            calls.push(CallResult { value, changes });
        }
    }
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
                Next::Idle { until_nanoseconds } => {
                    let until = until_nanoseconds.expect("idle with nothing to wake it");
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
    let mut results = Vec::new();
    for each in &sequence.calls {
        match call(&mut kernel, &mut cpu, &mut host, *each) {
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
    if value < 0 {
        return out;
    }
    match call.number {
        linux::__NR_newfstatat => out.extend([(c, 16), (c + 72, 48)]),
        linux::__NR_fstat => out.extend([(b, 16), (b + 72, 48)]),
        linux::__NR_statx => out.extend([(e + 32, 8), (e + 64, 64), (e + 136, 16)]),
        linux::__NR_fstatfs => out.push((b + 16, 48)),
        linux::__NR_clock_gettime => out.push((b, 16)),
        linux::__NR_getrandom => out.push((a, value as u64)),
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

/// Whether a call's value is an address the kernel chose.
fn value_is_volatile(call: &Call) -> bool {
    call.number == linux::__NR_mmap && call.arguments[3] & 0x10_0010 == 0
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
        let value = if value_is_volatile(call) && each.value >= 0 {
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
                    "{line}: ours {:?} theirs {:?} (one side ended)\n",
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
