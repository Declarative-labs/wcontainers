//! The zygote: a process forked once from the embedder, holding none of its memory, that forks every stub.

use core::ffi::c_void;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Mutex;

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags};
use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketFlags, SocketType};
use rustix::process::{Pid, WaitOptions};

use crate::layout::{ZYGOTE, ZYGOTE_CODE_SIZE, ZYGOTE_SIZE};
use crate::probe::{HostProbe, Unavailable};

#[cfg(target_arch = "aarch64")]
#[expect(
    unsafe_code,
    reason = "the zygote runs with none of the embedder's memory, so it is assembly that refers to nothing outside itself"
)]
mod aarch64;
#[cfg(target_arch = "x86_64")]
#[expect(
    unsafe_code,
    reason = "the zygote runs with none of the embedder's memory, so it is assembly that refers to nothing outside itself"
)]
mod x86_64;

/// The zygote's stack frame, which every stub inherits: offsets from its stack pointer.
mod frame {
    use core::mem::{offset_of, size_of};

    /// A request, then the answer written over it: its kind.
    pub const BUFFER: usize = 0;
    /// Its value.
    pub const VALUE: usize = BUFFER + 8;
    /// The one `iovec`, over the buffer.
    pub const IOVEC: usize = BUFFER + super::MESSAGE;
    pub const IOVEC_LENGTH: usize = IOVEC + offset_of!(libc::iovec, iov_len);
    /// The `msghdr` of every `recvmsg` and `sendmsg`.
    pub const MESSAGE_HEADER: usize = IOVEC + size_of::<libc::iovec>();
    pub const MESSAGE_IOVEC: usize = MESSAGE_HEADER + offset_of!(libc::msghdr, msg_iov);
    pub const MESSAGE_IOVEC_COUNT: usize = MESSAGE_HEADER + offset_of!(libc::msghdr, msg_iovlen);
    /// A `wait4` status, and a stub's futex word.
    pub const STATUS: usize = MESSAGE_HEADER + size_of::<libc::msghdr>();
    /// The area registered to prove no restartable-sequences registration is left.
    pub const OWN_AREA: usize = STATUS.next_multiple_of(super::RSEQ_AREA);
    /// The whole frame, from the stack's top.
    pub const FRAME: usize = (OWN_AREA + super::RSEQ_AREA).next_multiple_of(super::RSEQ_AREA);

    // The assembly zeroes the header as seven words, and the stack's top is page-aligned.
    const _: () = assert!(size_of::<libc::msghdr>() == 7 * 8);
    const _: () = assert!(size_of::<libc::iovec>() == 2 * 8);
}

/// What the embedder asks of the zygote; each answer echoes the kind and carries a result.
#[repr(u64)]
#[derive(Clone, Copy)]
enum Request {
    /// Answers 0.
    Ping = 1,
    /// Forks a stub and answers its process id.
    Fork = 2,
    /// Sends SIGKILL to an unreaped child.
    Kill = 3,
    /// Waits for a child's end and answers its wait status.
    Reap = 4,
}

/// The size of a request and of an answer: a kind and a value.
const MESSAGE: usize = 16;

/// Why the zygote, or a stub before its body, ended; its exit status.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZygoteEnd {
    /// The embedder's end of the socket closed.
    EmbedderGone = 0,
    /// Its code or stack could not be mapped.
    PagesRefused = 1,
    /// The forking thread's restartable-sequences area could not be unregistered.
    RestartableSequencesNotUnregistered = 2,
    /// A restartable-sequences registration was left after unregistering.
    RestartableSequencesLeftRegistered = 3,
    /// The embedder's file descriptors could not be closed.
    DescriptorsNotClosed = 4,
    /// The embedder's memory could not be unmapped.
    UnmapRefused = 5,
    /// A request was malformed or could not be read.
    BadRequest = 6,
    /// An answer could not be sent.
    AnswerRefused = 7,
    /// A stub found that its parent was no longer the zygote.
    Orphaned = 8,
}

impl ZygoteEnd {
    const ALL: [Self; 9] = [
        Self::EmbedderGone,
        Self::PagesRefused,
        Self::RestartableSequencesNotUnregistered,
        Self::RestartableSequencesLeftRegistered,
        Self::DescriptorsNotClosed,
        Self::UnmapRefused,
        Self::BadRequest,
        Self::AnswerRefused,
        Self::Orphaned,
    ];

    fn from_status(status: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|end| u32::from(*end as u8) == status)
    }
}

/// How a zygote or one of its children ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// It exited with this status.
    Exited(u32),
    /// A signal killed it.
    Killed(u32),
}

impl Ended {
    fn from_wait(status: i32) -> Self {
        if libc::WIFSIGNALED(status) {
            Self::Killed(libc::WTERMSIG(status).unsigned_abs())
        } else {
            Self::Exited(libc::WEXITSTATUS(status).unsigned_abs())
        }
    }
}

/// A running zygote, owned by the host-process host.
pub struct Zygote {
    socket: Mutex<OwnedFd>,
    process: Pid,
    reaped: bool,
}

/// The forking thread's restartable-sequences registration, as the C library describes it.
#[derive(Clone, Copy)]
struct Registration {
    offset_from_thread_pointer: isize,
    size: u32,
}

#[cfg(target_env = "gnu")]
#[expect(
    unsafe_code,
    reason = "glibc's description of each thread's restartable-sequences area, which no crate exposes"
)]
// SAFETY: glibc 2.35 and later define both, constant after start-up.
unsafe extern "C" {
    safe static __rseq_offset: isize;
    safe static __rseq_size: u32;
}

#[cfg(target_env = "gnu")]
fn registration() -> Registration {
    Registration {
        offset_from_thread_pointer: __rseq_offset,
        size: __rseq_size,
    }
}

#[cfg(not(target_env = "gnu"))]
fn registration() -> Registration {
    Registration {
        offset_from_thread_pointer: 0,
        size: 0,
    }
}

/// The signature glibc registers its areas with (sysdeps/unix/sysv/linux/*/bits/rseq.h), which an unregister repeats.
#[cfg(target_arch = "aarch64")]
const RSEQ_SIG: u32 = 0xd428_bc00;
#[cfg(target_arch = "x86_64")]
const RSEQ_SIG: u32 = 0x5305_3053;

/// Linux's `RSEQ_FLAG_UNREGISTER` (include/uapi/linux/rseq.h), which neither libc nor linux-raw-sys defines.
const RSEQ_FLAG_UNREGISTER: u32 = 1;

/// The original `struct rseq`'s size and alignment, the smallest registration Linux accepts.
const RSEQ_AREA: usize = 32;

#[expect(unsafe_code, reason = "the two labels around the zygote's assembly")]
// SAFETY: the zygote's assembly defines both, in one section.
unsafe extern "C" {
    safe static wcontainers_host_process_zygote_start: u8;
    safe static wcontainers_host_process_zygote_end: u8;
}

/// The zygote's machine code, which runs copied to `ZYGOTE`.
fn code() -> &'static [u8] {
    let start = &raw const wcontainers_host_process_zygote_start;
    let end = &raw const wcontainers_host_process_zygote_end;
    let length = end as usize - start as usize;
    #[expect(
        unsafe_code,
        reason = "the zygote's code, between two labels of its own assembly"
    )]
    // SAFETY: both labels are in one text section, start before end, and text is never written.
    unsafe {
        core::slice::from_raw_parts(start, length)
    }
}

impl Zygote {
    /// Forks the zygote from the calling thread and waits until it answers.
    pub fn start(probe: &HostProbe) -> Result<Self, Unavailable> {
        let code = code();
        assert!(
            code.len() <= ZYGOTE_CODE_SIZE,
            "the zygote's code outgrew its pages"
        );
        let memfd = rustix::fs::memfd_create("wcontainers zygote", MemfdFlags::CLOEXEC)
            .map_err(Unavailable::Host)?;
        rustix::io::write(&memfd, code).map_err(Unavailable::Host)?;
        rustix::fs::ftruncate(&memfd, ZYGOTE_CODE_SIZE as u64).map_err(Unavailable::Host)?;
        let (ours, theirs) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .map_err(Unavailable::Host)?;
        let reservation = Reservation::new()?;
        let child = fork_into_zygote(Entry {
            memfd: memfd.as_raw_fd(),
            socket: theirs.as_raw_fd(),
            registration: registration(),
            user_space_top: probe.user_space_top(),
        })
        .map_err(Unavailable::Host)?;
        drop((memfd, theirs, reservation));
        let zygote = Self {
            socket: Mutex::new(ours),
            process: child,
            reaped: false,
        };
        match zygote.ask(Request::Ping, 0) {
            Ok(_) => Ok(zygote),
            Err(_) => match zygote.end() {
                Ok(Ended::Exited(status)) => Err(ZygoteEnd::from_status(status)
                    .map_or(Unavailable::Host(Errno::CHILD), Unavailable::Zygote)),
                Ok(Ended::Killed(_)) => Err(Unavailable::Host(Errno::CHILD)),
                Err(errno) => Err(Unavailable::Host(errno)),
            },
        }
    }

    /// The zygote's process id.
    pub fn process(&self) -> Pid {
        self.process
    }

    /// Checks that the zygote still answers.
    pub fn ping(&self) -> Result<(), Errno> {
        self.ask(Request::Ping, 0).map(drop)
    }

    /// Forks a stub; it has no body yet and waits until it is killed.
    pub fn fork_stub(&self) -> Result<Pid, Errno> {
        let pid = self.ask(Request::Fork, 0)?;
        i32::try_from(pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(Errno::PROTO)
    }

    /// Sends SIGKILL to a stub the zygote has not reaped.
    pub fn kill(&self, stub: Pid) -> Result<(), Errno> {
        self.ask(Request::Kill, stub.as_raw_nonzero().get() as u64)
            .map(drop)
    }

    /// Waits for a stub's end and reaps it.
    pub fn reap(&self, stub: Pid) -> Result<Ended, Errno> {
        let status = self.ask(Request::Reap, stub.as_raw_nonzero().get() as u64)?;
        let status = i32::try_from(status).map_err(|_| Errno::PROTO)?;
        Ok(Ended::from_wait(status))
    }

    /// Closes the zygote's socket, which ends it, and reaps it.
    pub fn end(mut self) -> Result<Ended, Errno> {
        self.close();
        self.reaped = true;
        let (_, status) = rustix::process::waitpid(Some(self.process), WaitOptions::empty())?
            .ok_or(Errno::CHILD)?;
        Ok(Ended::from_wait(status.as_raw()))
    }

    fn close(&mut self) {
        let socket = self
            .socket
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = rustix::net::shutdown(&*socket, rustix::net::Shutdown::Both);
    }

    fn ask(&self, request: Request, value: u64) -> Result<i64, Errno> {
        let socket = self
            .socket
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut message = [0; MESSAGE];
        message[..8].copy_from_slice(&(request as u64).to_ne_bytes());
        message[8..].copy_from_slice(&value.to_ne_bytes());
        if rustix::net::send(&*socket, &message, SendFlags::NOSIGNAL)? != MESSAGE {
            return Err(Errno::PROTO);
        }
        let mut answer = [0; MESSAGE];
        let (_, received) = rustix::net::recv(&*socket, &mut answer, RecvFlags::empty())?;
        if received == 0 {
            return Err(Errno::PIPE);
        }
        if received != MESSAGE || answer[..8] != message[..8] {
            return Err(Errno::PROTO);
        }
        let result = i64::from_ne_bytes(answer[8..].try_into().map_err(|_| Errno::PROTO)?);
        match i32::try_from(result) {
            Ok(errno) if errno < 0 => Err(Errno::from_raw_os_error(-errno)),
            _ => Ok(result),
        }
    }
}

impl Drop for Zygote {
    fn drop(&mut self) {
        if !self.reaped {
            self.close();
            let _ = rustix::process::waitpid(Some(self.process), WaitOptions::empty());
        }
    }
}

/// Our hold on the zygote's pages in the embedder while the child maps over them.
struct Reservation;

impl Reservation {
    fn new() -> Result<Self, Unavailable> {
        let flags = MapFlags::PRIVATE | MapFlags::FIXED_NOREPLACE | MapFlags::NORESERVE;
        #[expect(
            unsafe_code,
            reason = "maps a fixed address, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: FIXED_NOREPLACE never replaces a mapping; the range is inaccessible and dropped with this value.
        let mapped = unsafe {
            rustix::mm::mmap_anonymous(
                ZYGOTE as *mut c_void,
                ZYGOTE_SIZE,
                ProtFlags::empty(),
                flags,
            )
        };
        match mapped {
            Ok(address) if address as usize == ZYGOTE => Ok(Self),
            Ok(_) | Err(Errno::EXIST) => Err(Unavailable::ReservationRefused),
            Err(errno) => Err(Unavailable::Host(errno)),
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "unmaps the reservation made by `new`")]
        // SAFETY: the range is this value's alone and nothing refers to it.
        let _ = unsafe { rustix::mm::munmap(ZYGOTE as *mut c_void, ZYGOTE_SIZE) };
    }
}

/// What the zygote's code is given at its first instruction.
struct Entry {
    memfd: i32,
    socket: i32,
    registration: Registration,
    user_space_top: usize,
}

/// Forks with every signal blocked; the child jumps into the zygote's code and never returns.
fn fork_into_zygote(entry: Entry) -> Result<Pid, Errno> {
    let mut every = core::mem::MaybeUninit::<libc::sigset_t>::uninit();
    let mut previous = core::mem::MaybeUninit::<libc::sigset_t>::uninit();
    #[expect(
        unsafe_code,
        reason = "fills a signal set and blocks it, through libc: rustix has no signal mask call"
    )]
    // SAFETY: both sets are written before they are read.
    let blocked = unsafe {
        libc::sigfillset(every.as_mut_ptr());
        libc::pthread_sigmask(libc::SIG_SETMASK, every.as_ptr(), previous.as_mut_ptr())
    };
    if blocked != 0 {
        return Err(Errno::from_raw_os_error(blocked));
    }
    #[expect(
        unsafe_code,
        reason = "forks the embedder; the child runs only the zygote's own code"
    )]
    // SAFETY: the child makes no call but the jump, which leaves Rust for good.
    let forked = unsafe { libc::fork() };
    if forked == 0 {
        enter(&entry);
    }
    #[expect(unsafe_code, reason = "puts back the signal mask saved above")]
    // SAFETY: `previous` was written by the call that blocked every signal.
    unsafe {
        libc::pthread_sigmask(libc::SIG_SETMASK, previous.as_ptr(), core::ptr::null_mut());
    }
    Pid::from_raw(forked).ok_or_else(|| {
        Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::AGAIN)
    })
}

#[cfg(target_arch = "aarch64")]
fn enter(entry: &Entry) -> ! {
    #[expect(unsafe_code, reason = "jumps into the zygote's code in the fork child")]
    // SAFETY: the zygote's code maps its own pages and stack and never returns.
    unsafe {
        core::arch::asm!(
            "b wcontainers_host_process_zygote_start",
            in("x0") i64::from(entry.memfd),
            in("x1") i64::from(entry.socket),
            in("x2") entry.registration.offset_from_thread_pointer,
            in("x3") u64::from(entry.registration.size),
            in("x4") entry.user_space_top,
            options(noreturn),
        )
    }
}

#[cfg(target_arch = "x86_64")]
fn enter(entry: &Entry) -> ! {
    #[expect(unsafe_code, reason = "jumps into the zygote's code in the fork child")]
    // SAFETY: the zygote's code maps its own pages and stack and never returns.
    unsafe {
        core::arch::asm!(
            "jmp wcontainers_host_process_zygote_start",
            in("rdi") i64::from(entry.memfd),
            in("rsi") i64::from(entry.socket),
            in("rdx") entry.registration.offset_from_thread_pointer,
            in("rcx") u64::from(entry.registration.size),
            in("r8") entry.user_space_top,
            options(noreturn),
        )
    }
}
