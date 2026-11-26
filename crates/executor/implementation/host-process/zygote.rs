//! The zygote: a process started from its own executable image, holding none of the embedder's memory, that forks every
//! stub.

use core::mem::MaybeUninit;
use std::io::IoSlice;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use rustix::net::{
    AddressFamily, RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags,
    SocketType,
};
use rustix::process::{Pid, WaitOptions};

use crate::layout::{OUR_AREA, ZYGOTE, ZYGOTE_CODE_SIZE};
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
    /// Its stack could not be mapped.
    PagesRefused = 1,
    /// Its signals could not all be blocked.
    SignalsNotBlocked = 2,
    /// A restartable-sequences registration was left after its start.
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
        Self::SignalsNotBlocked,
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

/// The signature the zygote registers its own area with, glibc's (sysdeps/unix/sysv/linux/*/bits/rseq.h).
#[cfg(target_arch = "aarch64")]
const RSEQ_SIG: u32 = 0xd428_bc00;
#[cfg(target_arch = "x86_64")]
const RSEQ_SIG: u32 = 0x5305_3053;

/// Linux's `RSEQ_FLAG_UNREGISTER` (include/uapi/linux/rseq.h), which neither libc nor linux-raw-sys defines.
const RSEQ_FLAG_UNREGISTER: u32 = 1;

/// The original `struct rseq`'s size and alignment, the smallest registration Linux accepts.
const RSEQ_AREA: usize = 32;

/// The section of the embedder's executable that holds the zygote's assembly and nothing else, kept by the linker
/// though nothing refers to it (`SHF_GNU_RETAIN`).
const CODE_SECTION: &str = "wcontainers_host_process_zygote";

/// The zygote's machine code, which runs at `ZYGOTE`, read once from the embedder's own executable.
fn code() -> Result<&'static [u8], Unavailable> {
    static CODE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    if let Some(code) = CODE.get() {
        return Ok(code);
    }
    let code = read_code()?;
    Ok(CODE.get_or_init(|| code))
}

/// The bytes of `CODE_SECTION` in the file this process was started from.
fn read_code() -> Result<Vec<u8>, Unavailable> {
    use object::read::elf::{FileHeader, SectionHeader};
    // proc(5)'s name for the file this process was exec'd from, which stays that file if its path is replaced.
    let file = std::fs::File::open("/proc/self/exe")
        .map_err(|error| Unavailable::Host(Errno::from_io_error(&error).unwrap_or(Errno::IO)))?;
    let file = object::read::ReadCache::new(file);
    let not_found = |_| Unavailable::ZygoteCodeNotFound;
    let header =
        object::elf::FileHeader64::<object::Endianness>::parse(&file).map_err(not_found)?;
    let endian = header.endian().map_err(not_found)?;
    let sections = header.sections(endian, &file).map_err(not_found)?;
    let (_, section) = sections
        .section_by_name(endian, CODE_SECTION.as_bytes())
        .ok_or(Unavailable::ZygoteCodeNotFound)?;
    if section.sh_type(endian) != object::elf::SHT_PROGBITS
        || section.sh_flags(endian).0 & object::elf::SHF_EXECINSTR.0 == 0
    {
        return Err(Unavailable::ZygoteCodeNotFound);
    }
    let code = section.data(endian, &file).map_err(not_found)?;
    if code.is_empty() {
        return Err(Unavailable::ZygoteCodeNotFound);
    }
    Ok(code.to_vec())
}

impl Zygote {
    /// Starts the zygote from its own executable image and waits until it answers.
    pub fn start(probe: &HostProbe) -> Result<Self, Unavailable> {
        let image =
            memfd_image(&image(code()?, probe.user_space_top())).map_err(Unavailable::Host)?;
        let (ours, theirs) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .map_err(Unavailable::Host)?;
        // proc(5)'s name for one of our own descriptors, which execve opens before it closes the descriptor.
        let child = Command::new(format!("/proc/self/fd/{}", image.as_raw_fd()))
            .env_clear()
            .stdin(Stdio::from(theirs))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                Unavailable::Host(Errno::from_io_error(&error).unwrap_or(Errno::IO))
            })?;
        drop(image);
        let child = i32::try_from(child.id())
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(Unavailable::Host(Errno::CHILD))?;
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
            // A stop sent at once is most often too early, so the next follows soon.
            let within = std::time::Duration::from_micros(50);
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
        // The zygote reaps only when asked, so the number still names its child.
        let pidfd = match rustix::process::pidfd_open(process, rustix::process::PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(errno) => {
                let _ = self.kill(process);
                let _ = self.reap(process);
                return Err(Unavailable::Host(errno));
            }
        };
        let stub = Stub::new(process, pidfd, self.number, ours, pages, memory);
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
        let (_, status) = wait_for(self.process)?.ok_or(Errno::CHILD)?;
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
        // A signal to the calling thread before the message went is not an error, nor one before the answer came.
        let sent = loop {
            match rustix::net::sendmsg(
                &*socket,
                &[IoSlice::new(&message)],
                &mut control,
                SendFlags::NOSIGNAL,
            ) {
                Err(Errno::INTR) => {}
                sent => break sent?,
            }
        };
        if sent != MESSAGE {
            return Err(Errno::PROTO);
        }
        let mut answer = [0; MESSAGE];
        let (_, received) = loop {
            match rustix::net::recv(&*socket, &mut answer, RecvFlags::empty()) {
                Err(Errno::INTR) => {}
                received => break received?,
            }
        };
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
            let _ = wait_for(self.process);
        }
    }
}

/// Waits for a child's end and reaps it; a signal to the calling thread is not an error.
fn wait_for(
    child: Pid,
) -> Result<Option<(Pid, rustix::process::WaitStatus)>, Errno> {
    loop {
        match rustix::process::waitpid(Some(child), WaitOptions::empty()) {
            Err(Errno::INTR) => {}
            waited => return waited,
        }
    }
}

/// Where the zygote's image holds the user-space top: the last word of its code pages.
const TOP_AT: usize = ZYGOTE + ZYGOTE_CODE_SIZE - size_of::<u64>();

/// The descriptor the zygote's socket has when it starts: its standard input.
const SOCKET_AT_START: i32 = libc::STDIN_FILENO;

/// The zygote's executable image: an ELF header with its program headers in the first `ZYGOTE_CODE_SIZE` bytes, then
/// the code pages loaded at `ZYGOTE`, which start with its code and end with the user-space top.
fn image(code: &[u8], user_space_top: usize) -> Vec<u8> {
    use object::elf::{self, FileFlags, FileHeader64, ProgramFlags, ProgramHeader64, ProgramType};
    use object::endian::{Endianness, U16, U32, U64};
    let top_offset = TOP_AT - ZYGOTE;
    assert!(
        code.len() <= top_offset,
        "the zygote's code outgrew its pages"
    );
    let endian = Endianness::default();
    let word = |value: usize| U64::new(endian, value as u64);
    let header_size = size_of::<FileHeader64<Endianness>>();
    let program_size = size_of::<ProgramHeader64<Endianness>>();
    let segment =
        |p_type: ProgramType, flags: [ProgramFlags; 2], offset, address, size| ProgramHeader64 {
            p_type: U32::new(endian, p_type),
            p_flags: U32::new(endian, ProgramFlags(flags[0].0 | flags[1].0)),
            p_offset: word(offset),
            p_vaddr: word(address),
            p_paddr: word(address),
            p_filesz: word(size),
            p_memsz: word(size),
            p_align: word(ZYGOTE_CODE_SIZE),
        };
    let programs = [
        segment(
            elf::PT_LOAD,
            [elf::PF_R, elf::PF_X],
            ZYGOTE_CODE_SIZE,
            ZYGOTE,
            ZYGOTE_CODE_SIZE,
        ),
        // A stack that is not executable, and no READ_IMPLIES_EXEC.
        segment(elf::PT_GNU_STACK, [elf::PF_R, elf::PF_W], 0, 0, 0),
    ];
    let header = FileHeader64 {
        e_ident: elf::Ident {
            magic: elf::ELFMAG,
            class: elf::ELFCLASS64,
            data: match endian {
                Endianness::Little => elf::ELFDATA2LSB,
                Endianness::Big => elf::ELFDATA2MSB,
            },
            version: elf::EV_CURRENT,
            os_abi: elf::ELFOSABI_NONE,
            abi_version: 0,
            padding: [0; 7],
        },
        e_type: U16::new(endian, elf::ET_EXEC),
        #[cfg(target_arch = "aarch64")]
        e_machine: U16::new(endian, elf::EM_AARCH64),
        #[cfg(target_arch = "x86_64")]
        e_machine: U16::new(endian, elf::EM_X86_64),
        e_version: U32::new(endian, u32::from(elf::EV_CURRENT.0)),
        e_entry: word(ZYGOTE),
        e_phoff: word(header_size),
        e_shoff: word(0),
        e_flags: U32::new(endian, FileFlags::default()),
        e_ehsize: U16::new(endian, header_size as u16),
        e_phentsize: U16::new(endian, program_size as u16),
        e_phnum: U16::new(endian, programs.len() as u16),
        e_shentsize: U16::new(endian, 0),
        e_shnum: U16::new(endian, 0),
        e_shstrndx: U16::new(endian, elf::SHN_UNDEF),
    };
    let mut image = vec![0; 2 * ZYGOTE_CODE_SIZE];
    image[..header_size].copy_from_slice(object::pod::bytes_of(&header));
    for (index, program) in programs.iter().enumerate() {
        let at = header_size + index * program_size;
        image[at..at + program_size].copy_from_slice(object::pod::bytes_of(program));
    }
    let code_pages = &mut image[ZYGOTE_CODE_SIZE..];
    code_pages[..code.len()].copy_from_slice(code);
    code_pages[top_offset..].copy_from_slice(&(user_space_top as u64).to_ne_bytes());
    image
}

/// A memfd that holds `bytes` and that the calling process may execute.
fn memfd_image(bytes: &[u8]) -> Result<OwnedFd, Errno> {
    let memfd = rustix::fs::memfd_create("wcontainers zygote", MemfdFlags::CLOEXEC)?;
    let mut written = 0;
    while written < bytes.len() {
        match rustix::io::write(&memfd, &bytes[written..])? {
            0 => return Err(Errno::NOSPC),
            count => written += count,
        }
    }
    Ok(memfd)
}
