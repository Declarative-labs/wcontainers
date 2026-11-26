//! The zygote's code and what it shares with the embedder: its stack frame, its requests and answers, why it ends,
//! and the descriptors a stub is forked with.

use crate::layout::{ZYGOTE, ZYGOTE_CODE_SIZE};

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
pub mod frame {
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
pub const RECEIVE_SIZE: usize = block_frame::SIZE;
/// Where in it the one byte received lands.
pub const RECEIVE_BYTE: usize = block_frame::BYTE;

/// What a stub's thread writes on its register page to receive a shared block's memfd: offsets from `RECEIVE`.
pub mod block_frame {
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
pub enum StubDescriptor {
    /// The stub's end of its own socket.
    Socket = 0,
    /// Its thread-pages memfd, closed once mapped.
    ThreadPages = 1,
    /// Its memory's memfd.
    Memory = 2,
}

/// How many descriptors a fork request carries.
pub const STUB_DESCRIPTORS: usize = 3;

/// What the embedder asks of the zygote; each answer echoes the kind and carries a result.
#[repr(u64)]
#[derive(Clone, Copy)]
pub enum Request {
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
pub const MESSAGE: usize = 16;

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
    pub const ALL: [Self; 13] = [
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

    pub fn from_status(status: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|end| u32::from(*end as u8) == status)
    }
}

/// The signature the zygote registers its own area with, glibc's (sysdeps/unix/sysv/linux/*/bits/rseq.h).
#[cfg(target_arch = "aarch64")]
pub const RSEQ_SIG: u32 = 0xd428_bc00;
#[cfg(target_arch = "x86_64")]
pub const RSEQ_SIG: u32 = 0x5305_3053;

/// Linux's `RSEQ_FLAG_UNREGISTER` (include/uapi/linux/rseq.h), which neither libc nor linux-raw-sys defines.
pub const RSEQ_FLAG_UNREGISTER: u32 = 1;

/// The original `struct rseq`'s size and alignment, the smallest registration Linux accepts.
pub const RSEQ_AREA: usize = 32;

/// The section of the embedder's executable that holds the zygote's assembly and nothing else, kept by the linker
/// though nothing refers to it (`SHF_GNU_RETAIN`).
pub const CODE_SECTION: &str = "wcontainers_host_process_zygote";

/// Where the zygote's image holds the user-space top: the last word of its code pages.
pub const TOP_AT: usize = ZYGOTE + ZYGOTE_CODE_SIZE - size_of::<u64>();

/// The descriptor the zygote's socket has when it starts: its standard input.
pub const SOCKET_AT_START: i32 = libc::STDIN_FILENO;
