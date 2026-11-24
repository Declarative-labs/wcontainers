//! The zygote: a process forked once from the embedder, holding none of its memory, that forks every stub.

use core::ffi::c_void;
use core::mem::MaybeUninit;
use std::io::IoSlice;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::sync::Mutex;

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags};
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags,
    SocketType,
};
use rustix::process::{Pid, WaitOptions};

use crate::layout::{OUR_AREA, ZYGOTE, ZYGOTE_CODE_SIZE, ZYGOTE_SIZE};
use crate::pages::ThreadPages;
use crate::probe::{HostProbe, Unavailable};
use crate::stub::Stub;

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

    use linux_raw_sys::general as linux;

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
    pub const MESSAGE_CONTROL: usize = MESSAGE_HEADER + offset_of!(libc::msghdr, msg_control);
    pub const MESSAGE_CONTROL_LENGTH: usize =
        MESSAGE_HEADER + offset_of!(libc::msghdr, msg_controllen);
    /// A `wait4` status.
    pub const STATUS: usize = MESSAGE_HEADER + size_of::<libc::msghdr>();
    /// The descriptors a fork request carries: one control message.
    pub const CONTROL: usize = STATUS + 8;
    pub const CONTROL_LEVEL: usize = CONTROL + offset_of!(libc::cmsghdr, cmsg_level);
    pub const CONTROL_TYPE: usize = CONTROL + offset_of!(libc::cmsghdr, cmsg_type);
    /// The descriptors themselves, in the order of [`super::StubDescriptor`].
    pub const CONTROL_DESCRIPTORS: usize = CONTROL + size_of::<libc::cmsghdr>();
    /// The control message's length with every descriptor.
    pub const CONTROL_FULL: usize =
        size_of::<libc::cmsghdr>() + super::STUB_DESCRIPTORS * size_of::<i32>();
    pub const CONTROL_SIZE: usize = CONTROL_FULL.next_multiple_of(size_of::<usize>());
    /// The stub's `sock_fprog`, naming the filter in its control page.
    pub const FILTER_PROGRAM: usize = CONTROL + CONTROL_SIZE;
    pub const FILTER_PROGRAM_INSTRUCTIONS: usize =
        FILTER_PROGRAM + offset_of!(libc::sock_fprog, filter);
    /// The area registered to prove no restartable-sequences registration is left.
    pub const OWN_AREA: usize =
        (FILTER_PROGRAM + size_of::<libc::sock_fprog>()).next_multiple_of(super::RSEQ_AREA);
    /// A stub's stop handler, as `rt_sigaction` reads it.
    pub const ACTION: usize = OWN_AREA + super::RSEQ_AREA;
    pub const ACTION_HANDLER: usize =
        ACTION + offset_of!(linux::kernel_sigaction, sa_handler_kernel);
    pub const ACTION_FLAGS: usize = ACTION + offset_of!(linux::kernel_sigaction, sa_flags);
    pub const ACTION_RESTORER: usize = ACTION + offset_of!(linux::kernel_sigaction, sa_restorer);
    pub const ACTION_MASK: usize = ACTION + offset_of!(linux::kernel_sigaction, sa_mask);
    /// The whole frame, from the stack's top.
    pub const FRAME: usize =
        (ACTION + size_of::<linux::kernel_sigaction>()).next_multiple_of(super::RSEQ_AREA);

    // The assembly zeroes the header as seven words, and the stack's top is page-aligned.
    const _: () = assert!(size_of::<libc::msghdr>() == 7 * 8);
    const _: () = assert!(size_of::<libc::iovec>() == 2 * 8);
    const _: () = assert!(offset_of!(libc::sock_fprog, len) == 0);
}

/// The bytes a shared block's receive spans on the register page, from its `RECEIVE`.
pub(crate) const RECEIVE_SIZE: usize = block_frame::SIZE;
/// Where in it the one byte received lands.
pub(crate) const RECEIVE_BYTE: usize = block_frame::BYTE;

/// What a stub's thread writes on its register page to receive a shared block's memfd: offsets from `RECEIVE`.
mod block_frame {
    use core::mem::{offset_of, size_of};

    /// The `msghdr`.
    pub const HEADER: usize = 0;
    pub const HEADER_IOVEC: usize = HEADER + offset_of!(libc::msghdr, msg_iov);
    pub const HEADER_IOVEC_COUNT: usize = HEADER + offset_of!(libc::msghdr, msg_iovlen);
    pub const HEADER_CONTROL: usize = HEADER + offset_of!(libc::msghdr, msg_control);
    pub const HEADER_CONTROL_LENGTH: usize = HEADER + offset_of!(libc::msghdr, msg_controllen);
    /// The one byte the control page's `iovec` names.
    pub const BYTE: usize = HEADER + size_of::<libc::msghdr>();
    /// One control message of one descriptor, whose number the stub never reads: it is `BLOCK_FD`.
    pub const CONTROL: usize = BYTE + size_of::<u64>();
    pub const CONTROL_LEVEL: usize = CONTROL + offset_of!(libc::cmsghdr, cmsg_level);
    pub const CONTROL_TYPE: usize = CONTROL + offset_of!(libc::cmsghdr, cmsg_type);
    /// The control message's length with its descriptor, and the room it takes.
    pub const CONTROL_FULL: usize = size_of::<libc::cmsghdr>() + size_of::<i32>();
    pub const CONTROL_SIZE: usize = CONTROL_FULL.next_multiple_of(size_of::<usize>());
    /// The whole receive.
    pub const SIZE: usize = (CONTROL + CONTROL_SIZE).next_multiple_of(16);

    // The assembly zeroes the header as seven words and reads the level and type as one pair.
    const _: () = assert!(size_of::<libc::msghdr>() == 7 * 8);
    const _: () = assert!(CONTROL_TYPE == CONTROL_LEVEL + 4);
}

/// The descriptors a fork request carries, in order.
#[repr(usize)]
#[derive(Clone, Copy)]
enum StubDescriptor {
    /// The stub's end of its own socket.
    Socket = 0,
    /// Its thread-pages memfd, closed once mapped.
    ThreadPages = 1,
    /// Its memory's memfd.
    Memory = 2,
}

/// How many descriptors a fork request carries.
const STUB_DESCRIPTORS: usize = 3;

/// What the embedder asks of the zygote; each answer echoes the kind and carries a result.
#[repr(u64)]
#[derive(Clone, Copy)]
enum Request {
    /// Answers 0.
    Ping = 1,
    /// Forks a stub with the descriptors the message carries, closes them, and answers its process id.
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
    /// A stub could not put its descriptors at their fixed numbers.
    DescriptorsNotPlaced = 9,
    /// A stub's filter was refused.
    FilterRefused = 10,
    /// A stub's stop handler or one of its threads' signal stacks was refused.
    SignalsRefused = 11,
    /// A stub's own code faulted ("stub fault").
    StubFault = 12,
}

impl ZygoteEnd {
    const ALL: [Self; 13] = [
        Self::EmbedderGone,
        Self::PagesRefused,
        Self::RestartableSequencesNotUnregistered,
        Self::RestartableSequencesLeftRegistered,
        Self::DescriptorsNotClosed,
        Self::UnmapRefused,
        Self::BadRequest,
        Self::AnswerRefused,
        Self::Orphaned,
        Self::DescriptorsNotPlaced,
        Self::FilterRefused,
        Self::SignalsRefused,
        Self::StubFault,
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
    /// A number no other zygote of this process has, which its stubs carry.
    number: u64,
    reaped: bool,
    /// The host's signal frames hold no FP/SIMD record, so its stubs' filters let a handler return through its frame.
    frames_without_vectors: bool,
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

/// The size of the restartable-sequences area the C library registered for the calling thread, which the zygote's
/// fork drops in its child; zero where it registered none.
pub fn registered_area_size() -> u32 {
    registration().size
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
    let length = (end as usize)
        .checked_sub(start as usize)
        .expect("the zygote's end label before its start");
    #[expect(
        unsafe_code,
        reason = "the zygote's code, between two labels of its own assembly"
    )]
    // SAFETY: both labels are in one text section, start before end (checked above, so a reordered assembly panics
    // rather than making a slice past the section), and text is never written.
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
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        #[cfg_attr(
            not(target_arch = "aarch64"),
            expect(unused_mut, reason = "set by ARM64's frame probe")
        )]
        let mut zygote = Self {
            socket: Mutex::new(ours),
            process: child,
            number: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            reaped: false,
            frames_without_vectors: false,
        };
        match zygote.ask(Request::Ping, 0, &[]) {
            Ok(_) => {
                #[cfg(target_arch = "aarch64")]
                {
                    zygote.frames_without_vectors = zygote.probe_frames()?;
                }
                Ok(zygote)
            }
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

    /// Whether this zygote forked `stub`.
    pub fn forked(&self, stub: &Stub) -> bool {
        stub.zygote() == self.number
    }

    /// Checks that the zygote still answers.
    pub fn ping(&self) -> Result<(), Errno> {
        self.ask(Request::Ping, 0, &[]).map(drop)
    }

    /// Whether the host's signal frames hold no FP/SIMD record, as found at the zygote's start, so its stubs' handlers
    /// may return through their frames.
    pub fn frames_without_vectors(&self) -> bool {
        self.frames_without_vectors
    }

    /// Whether the host's signal frames hold no FP/SIMD record: a stub whose filter refuses a return through its frame
    /// is stopped in a spinning turn, which its handler ends where the frame holds them, and is killed for where not.
    #[cfg(target_arch = "aarch64")]
    fn probe_frames(&self) -> Result<bool, Unavailable> {
        use crate::pages::Resume;
        use crate::stub::TurnEnd;
        let mut stub = self.fork_stub()?;
        let spinning = stub.new_thread().ok_or(Unavailable::Host(Errno::NOMEM))?;
        let made = stub.turn(0, &[spinning]).map_err(Unavailable::Host)?;
        if !matches!(made, TurnEnd::Done { .. }) {
            return Err(Unavailable::Host(Errno::PROTO));
        }
        let turn = stub
            .begin(1, &[], Resume::Spin)
            .map_err(Unavailable::Host)?;
        let end = loop {
            // A stop sent before the thread is in its turn is ignored, so they are sent until one ends it.
            match stub.stop(1) {
                Ok(()) | Err(Errno::SRCH) => {}
                Err(errno) => return Err(Unavailable::Host(errno)),
            }
            let within = std::time::Duration::from_millis(1);
            if let Some(end) = stub
                .wait_within(1, turn, within)
                .map_err(Unavailable::Host)?
            {
                break end;
            }
            if stub.hung_up().map_err(Unavailable::Host)? {
                break TurnEnd::StubEnded;
            }
        };
        let process = stub.process();
        if end != TurnEnd::StubEnded {
            self.kill(process).map_err(Unavailable::Host)?;
        }
        let ended = self.reap(process).map_err(Unavailable::Host)?;
        match (end, ended) {
            (TurnEnd::Stopped { .. }, _) => Ok(false),
            (TurnEnd::StubEnded, Ended::Killed(signal))
                if signal == linux_raw_sys::general::SIGSYS =>
            {
                Ok(true)
            }
            _ => Err(Unavailable::Host(Errno::PROTO)),
        }
    }

    /// Forks a stub with its own socket, thread pages and memory, and waits until it has installed its filter.
    pub fn fork_stub(&self) -> Result<Stub, Unavailable> {
        let pages = ThreadPages::new().map_err(Unavailable::Host)?;
        Stub::write_start(&pages, self.frames_without_vectors);
        let memory = rustix::fs::memfd_create("wcontainers guest memory", MemfdFlags::CLOEXEC)
            .map_err(Unavailable::Host)?;
        rustix::fs::ftruncate(&memory, OUR_AREA as u64).map_err(Unavailable::Host)?;
        let (ours, theirs) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .map_err(Unavailable::Host)?;
        let mut descriptors = [theirs.as_fd(); STUB_DESCRIPTORS];
        descriptors[StubDescriptor::Socket as usize] = theirs.as_fd();
        descriptors[StubDescriptor::ThreadPages as usize] = pages.file();
        descriptors[StubDescriptor::Memory as usize] = memory.as_fd();
        let process = self
            .ask(Request::Fork, 0, &descriptors)
            .map_err(Unavailable::Host)?;
        let process = i32::try_from(process)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(Unavailable::Host(Errno::PROTO))?;
        drop(theirs);
        let stub = Stub::new(process, self.number, ours, pages, memory);
        match stub.started() {
            Ok(crate::stub::TurnEnd::Done { .. }) => Ok(stub),
            Ok(crate::stub::TurnEnd::StubEnded) => match self.reap(process) {
                Ok(Ended::Exited(status)) => Err(ZygoteEnd::from_status(status)
                    .map_or(Unavailable::Host(Errno::CHILD), Unavailable::Zygote)),
                Ok(Ended::Killed(_)) => Err(Unavailable::Host(Errno::CHILD)),
                Err(errno) => Err(Unavailable::Host(errno)),
            },
            Ok(_) => {
                let _ = self.kill(process);
                let _ = self.reap(process);
                Err(Unavailable::Host(Errno::PROTO))
            }
            Err(errno) => Err(Unavailable::Host(errno)),
        }
    }

    /// Sends SIGKILL to a stub the zygote has not reaped.
    pub fn kill(&self, stub: Pid) -> Result<(), Errno> {
        self.ask(Request::Kill, stub.as_raw_nonzero().get() as u64, &[])
            .map(drop)
    }

    /// Waits for a stub's end and reaps it.
    pub fn reap(&self, stub: Pid) -> Result<Ended, Errno> {
        let status = self.ask(Request::Reap, stub.as_raw_nonzero().get() as u64, &[])?;
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

    fn ask(
        &self,
        request: Request,
        value: u64,
        descriptors: &[BorrowedFd<'_>],
    ) -> Result<i64, Errno> {
        let socket = self
            .socket
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut message = [0; MESSAGE];
        message[..8].copy_from_slice(&(request as u64).to_ne_bytes());
        message[8..].copy_from_slice(&value.to_ne_bytes());
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(STUB_DESCRIPTORS))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !descriptors.is_empty() && !control.push(SendAncillaryMessage::ScmRights(descriptors)) {
            return Err(Errno::NOBUFS);
        }
        let sent = rustix::net::sendmsg(
            &*socket,
            &[IoSlice::new(&message)],
            &mut control,
            SendFlags::NOSIGNAL,
        )?;
        if sent != MESSAGE {
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
    use nix::sys::signal::{SigSet, SigmaskHow, pthread_sigmask};
    let mut previous = SigSet::empty();
    pthread_sigmask(
        SigmaskHow::SIG_SETMASK,
        Some(&SigSet::all()),
        Some(&mut previous),
    )
    .map_err(|error| Errno::from_raw_os_error(error as i32))?;
    #[expect(
        unsafe_code,
        reason = "forks the embedder; the child runs only the zygote's own code"
    )]
    // SAFETY: the child makes no call but the jump, which leaves Rust for good.
    let forked = unsafe { libc::fork() };
    if forked == 0 {
        enter(&entry);
    }
    let restored = pthread_sigmask(SigmaskHow::SIG_SETMASK, Some(&previous), None);
    debug_assert!(
        restored.is_ok(),
        "a mask the host gave back is always valid"
    );
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
